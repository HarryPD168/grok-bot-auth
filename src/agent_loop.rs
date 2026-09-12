//! Host-exec Agent loop for injected `gb-*` models.
//! LLM tool-use → Cursor tool_call_started / exec_server_message → wait BidiAppend
//! exec_client_message → tool_call_completed → next model round.
//! Cursor host executes Read/Grep/Glob/Ls/Shell/Edit/Write. GBA does not exec on disk.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::agent_proto::{
    edit_result, exec_client_message, exec_server_message, glob_tool_result, grep_result,
    grep_union_result, ls_result, read_result, read_success, read_tool_result, read_tool_success,
    shell_result, shell_stream, tool_call, write_result, EditArgs, EditError, EditFileNotFound,
    EditResult, EditSuccess, EditToolCall, ExecClientMessage, GlobToolArgs, GlobToolCall,
    GlobToolError, GlobToolResult, GlobToolSuccess, GrepArgs, GrepError, GrepResult, GrepToolCall,
    LsArgs, LsError, LsResult, LsToolCall, ReadArgs, ReadToolArgs, ReadToolCall, ReadToolError,
    ReadToolResult, ReadToolSuccess, ShellArgs, ShellCommandParsingResult, ShellExecutableCommand,
    ShellFailure, ShellResult, ShellSuccess, ShellToolCall, ToolCall, WriteArgs,
};
use crate::agent_wire::{
    self, agent_server_message, interaction_update, AgentServerMessage, HeartbeatUpdate,
    InteractionUpdate, LocalRun, TextDeltaUpdate, ThinkingCompletedUpdate, ThinkingDeltaUpdate,
    TurnEndedUpdate,
};

const MAX_ROUNDS: usize = 64;
const SHELL_COLLECT_MAX: usize = 1_000_000;
const EXEC_TIMEOUT: Duration = Duration::from_secs(90);
const TASK_TIMEOUT: Duration = Duration::from_secs(1_800);
const LLM_TIMEOUT: Duration = Duration::from_secs(150);
const HEARTBEAT_MS: u64 = 4_000;
const IDLE_HINT_MS: u64 = 3_000;
const SHELL_OUTPUT_FULL_MAX: usize = 10_000;
const SHELL_OUTPUT_SIDE: usize = 5_000;
const SHELL_TIMEOUT_MS: i32 = 30_000;
const SHELL_HARD_TIMEOUT_MS: i32 = 86_400_000;
const SHELL_FILE_OUTPUT_THRESHOLD: u64 = 40_000;
const SHELL_TIMEOUT_BEHAVIOR_BACKGROUND: i32 = 2;
const ATTACHED_INLINE_CHARS: usize = 2_000;
const USER_TEXT_CHARS: usize = 80_000;
const TOOL_RESULT_CHARS: usize = 16_000;
/// CCursor AUTOCOMPACT_TRIGGER_RESERVE_RATIO: keep 15% of the window free.
const CONTEXT_RESERVE_PCT: u64 = 15;

const TOOLKIT: &str = r#"You are Cursor's coding agent on the user's machine. The host executes tools — you do not.
When you need files or a shell, emit one or more blocks and nothing else in those blocks:
<tool_call>
{"name":"Read","arguments":{"path":"ABS_PATH"}}
</tool_call>
Tools:
- Read: path (absolute), optional offset, limit
- Grep: pattern (required), optional path, glob, output_mode (content|files_with_matches|count), head_limit
- Glob: glob_pattern (required), optional target_directory
- Ls: path (directory)
- Shell: command (required), optional working_directory, description
- Edit: path, old_string, new_string, optional replace_all
- Write: path, contents
- WebSearch: search_term (required), optional explanation
- WebFetch: url (required)
- TodoWrite: todos[] with id, content, status; optional merge
- Delete: path (absolute)
- AskQuestion: title, questions[] with id, prompt, options[]
- SwitchMode: target_mode_id (plan|agent), optional explanation
- ApplyPatch: patch (*** Begin Patch ... *** End Patch)
- Await / AwaitShell: optional task_id, block_until_ms
- updateCurrentStep: current_step, optional final_summary, completed_subtitle
- ReadLints: optional paths[]
- Task: description, prompt, optional subagent_type (explore|shell|generalPurpose)
- CreatePlan: name, overview, plan, optional todos[]
- CallDynamicTool / CallMcpTool: namespace, toolName, arguments
- GetDynamicTools: optional namespace, toolName, pattern
- EditNotebook: target_notebook, cell_idx, is_new_cell, cell_language, old_string, new_string
- GenerateImage: description (required), optional filename, reference_image_paths[]
- ListMcpResources: optional server
- FetchMcpResource: server, uri, optional downloadPath
Do not claim you lack a workspace when Workspace: is provided. Prefer tools over guessing file contents.
After tool results arrive, continue until you can answer the user in plain text."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTool {
    Read,
    Grep,
    Glob,
    Ls,
    Shell,
    Edit,
    Write,
    WebSearch,
    WebFetch,
    TodoWrite,
    Delete,
    AskQuestion,
    SwitchMode,
    ApplyPatch,
    Await,
    UpdateCurrentStep,
    ReadLints,
    Task,
    CreatePlan,
    Mcp,
    GetMcpTools,
    EditNotebook,
    GenerateImage,
    ListMcpResources,
    FetchMcpResource,
}

#[derive(Debug, Clone)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
}

#[derive(Debug, Clone)]
pub enum LlmChunk {
    Thinking(String),
    Text(String),
    Tokens(i32),
    ToolStart { id: String, name: String },
}

#[derive(Debug, Clone, Default)]
pub struct ModelTurn {
    pub thinking: String,
    pub text: String,
    pub tools: Vec<ToolUse>,
    pub usage: TokenUsage,
}

pub fn estimate_tokens(text: &str) -> i32 {
    i32::try_from(text.len().div_ceil(4)).unwrap_or(i32::MAX)
}

pub fn is_token_limit_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("token limit")
        || lower.contains("context length")
        || lower.contains("context_length")
        || lower.contains("maximum context")
}

pub fn map_tool_name(name: &str) -> Option<CursorTool> {
    match name.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "read" | "readfile" | "read_file" => Some(CursorTool::Read),
        "grep" | "rg" => Some(CursorTool::Grep),
        "glob" => Some(CursorTool::Glob),
        "ls" | "list_dir" | "listdir" | "list_directory" => Some(CursorTool::Ls),
        "shell" | "bash" | "run_terminal_cmd" | "run_terminal_command" => Some(CursorTool::Shell),
        "edit" | "strreplace" | "str_replace" => Some(CursorTool::Edit),
        "write" | "write_file" => Some(CursorTool::Write),
        "websearch" | "web_search" => Some(CursorTool::WebSearch),
        "webfetch" | "web_fetch" => Some(CursorTool::WebFetch),
        "todowrite" | "todo_write" | "updatetodos" | "update_todos" => Some(CursorTool::TodoWrite),
        "delete" | "delete_file" => Some(CursorTool::Delete),
        "askquestion" | "ask_question" => Some(CursorTool::AskQuestion),
        "switchmode" | "switch_mode" => Some(CursorTool::SwitchMode),
        "applypatch" | "apply_patch" => Some(CursorTool::ApplyPatch),
        "await" | "awaitshell" | "await_shell" => Some(CursorTool::Await),
        "updatecurrentstep" | "update_current_step" => Some(CursorTool::UpdateCurrentStep),
        "readlints" | "read_lints" => Some(CursorTool::ReadLints),
        "task" | "subagent" => Some(CursorTool::Task),
        "createplan" | "create_plan" => Some(CursorTool::CreatePlan),
        "calldynamictool" | "call_dynamic_tool" | "callmcptool" | "call_mcp_tool" => {
            Some(CursorTool::Mcp)
        }
        "getdynamictools" | "get_dynamic_tools" | "getmcptools" => Some(CursorTool::GetMcpTools),
        "editnotebook" | "edit_notebook" => Some(CursorTool::EditNotebook),
        "generateimage" | "generate_image" => Some(CursorTool::GenerateImage),
        "listmcpresources" | "list_mcp_resources" => Some(CursorTool::ListMcpResources),
        "fetchmcpresource" | "fetch_mcp_resource" | "readmcpresource" | "read_mcp_resource" => {
            Some(CursorTool::FetchMcpResource)
        }
        _ => None,
    }
}

fn resolve_task_model(run: &LocalRun, args: &Value) -> Option<String> {
    let llm = json_str(args, &["model", "model_id", "modelId"]);
    if !llm.is_empty() {
        return Some(llm);
    }
    let ty = json_str(args, &["subagent_type", "subagentType"]);
    let ty = if ty.is_empty() { "explore" } else { ty.as_str() };
    for (name, sel) in &run.subagent_overrides {
        if name == &ty {
            return match sel {
                crate::agent_wire::SubagentOverride::Model(model) => Some(model.clone()),
                crate::agent_wire::SubagentOverride::Inherit => Some(run.model_id.clone()),
                crate::agent_wire::SubagentOverride::Disabled => Some(String::new()),
            };
        }
    }
    (!run.model_id.is_empty()).then(|| run.model_id.clone())
}

/// Char budget for outbound prompts: 85% of the model window, 4 chars/token
/// (inverse of `estimate_tokens`). Matches CCursor's 15% autocomact reserve.
pub fn prompt_char_budget(context_tokens: u32) -> usize {
    let tokens = if context_tokens == 0 {
        crate::agent_wire::DEFAULT_CONTEXT_TOKENS
    } else {
        context_tokens
    };
    let usable = (u64::from(tokens) * (100 - CONTEXT_RESERVE_PCT)) / 100;
    usize::try_from(usable.saturating_mul(4)).unwrap_or(usize::MAX)
}

pub fn toolkit_prompt(run: &LocalRun) -> String {
    let budget = prompt_char_budget(run.max_tokens);
    let user_cap = USER_TEXT_CHARS.min(budget / 2).max(2_048);
    let history_cap = 24_000.min(budget / 4).max(1_024);
    let mut out = TOOLKIT.to_string();
    out.push_str(crate::agent_wire::PREAMBLE_MARK);
    let mut preamble = String::new();
    let info = user_info_block(run);
    if !info.is_empty() {
        preamble.push_str("<user_info>\n");
        preamble.push_str(&info);
        preamble.push_str("\n</user_info>\n");
    }
    preamble.push_str(&attached_files_block(&run.attached_files, budget / 4));
    if !run.extra_prompt.is_empty() {
        preamble.push('\n');
        preamble.push_str(&crate::agent_wire::clip_utf8(&run.extra_prompt, budget / 4));
        preamble.push('\n');
    }
    if !run.system_prompt.is_empty() {
        preamble.push('\n');
        preamble.push_str(&crate::agent_wire::clip_utf8(&run.system_prompt, 8_000));
        preamble.push('\n');
    }
    out.push_str(&crate::agent_wire::assemble_preamble(&preamble));
    if let Some(root) = run.workspace.as_deref().filter(|path| !path.is_empty()) {
        out.push_str("\nWorkspace: ");
        out.push_str(root);
        out.push('\n');
        out.push_str(
            "Use Read/Grep/Glob/Ls/Shell on this root. Do not ask for a dump of the repo.\n",
        );
    }
    if !run.history_blob_ids.is_empty() {
        let prior = format_blob_prior(&run.history_blob_ids, history_cap);
        if !prior.is_empty() {
            out.push_str("\n\nPrior:\n");
            out.push_str(&prior);
        }
    } else if !run.history.is_empty() {
        out.push_str("\n\nPrior:\n");
        out.push_str(&crate::agent_wire::clip_utf8_tail(
            &run.history,
            history_cap,
        ));
    }
    let mut user_body = run.user_text.clone();
    if !run.images.is_empty() {
        for (mime, _) in &run.images {
            if !user_body.contains("[image ") {
                if !user_body.is_empty() {
                    user_body.push('\n');
                }
                user_body.push_str(&format!("[image {mime}]"));
            }
        }
    }
    if user_body.is_empty() && !run.history_blob_ids.is_empty() {
        if let Some(last) = crate::blob::hydrate_turns(&run.history_blob_ids)
            .into_iter()
            .rev()
            .find(|turn| turn.role != "system")
        {
            user_body = last.text;
        }
    }
    out.push_str(crate::agent_wire::USER_MARK);
    if run.mode == 2 {
        out.push_str("Mode: Ask. Do not write, edit, delete, or run mutating shell commands.\n");
    } else if run.mode == 3 {
        out.push_str("Mode: Plan. Prefer CreatePlan. Do not mutate the workspace until the user confirms.\n");
    } else if run.mode == 4 {
        out.push_str("Mode: Debug. Do not call SwitchMode or CreatePlan.\n");
    }
    if run.dynamic_tool_transition {
        out.push_str(
            "<system_reminder>\nDynamic tools have been enabled for this conversation. Some tools that appeared as direct tool calls in earlier turns must now be called through CallDynamicTool. Task and Subagent stay direct tools: call them by name, and emit multiple calls in one message to run them in parallel. Discover other tool schemas with GetDynamicTools.\n</system_reminder>\n",
        );
    }
    out.push_str("<user_query>\n");
    out.push_str(&crate::agent_wire::clip_utf8_tail(&user_body, user_cap));
    out.push_str("\n</user_query>");
    crate::agent_wire::budget_keep_user(&out, budget)
}

fn format_blob_prior(ids: &[Vec<u8>], cap: usize) -> String {
    let mut out = String::new();
    for turn in crate::blob::hydrate_turns(ids) {
        if turn.role == "system" {
            continue;
        }
        let label = if turn.role == "assistant" {
            "Assistant"
        } else {
            "User"
        };
        out.push_str(label);
        out.push_str(":\n");
        out.push_str(&turn.text);
        out.push_str("\n\n");
    }
    crate::agent_wire::clip_utf8_tail(&out, cap)
}

/// Split the agent blob into system / preamble / user / tool-result turns (CCursor messages[]).
pub fn prompt_to_turns(prompt: &str) -> Vec<crate::compact::ChatTurn> {
    let preamble_at = prompt.find(crate::agent_wire::PREAMBLE_MARK);
    let user_at = prompt.find(crate::agent_wire::USER_MARK);
    let mut turns = Vec::new();
    let (system_head, mid, rest) = match (preamble_at, user_at) {
        (Some(preamble), Some(user)) if preamble < user => {
            let system = prompt[..preamble].trim();
            let mid = &prompt[preamble + crate::agent_wire::PREAMBLE_MARK.len()..user];
            let rest = &prompt[user + crate::agent_wire::USER_MARK.len()..];
            (system, mid, rest)
        }
        (_, Some(user)) => {
            let head = prompt[..user].trim();
            let rest = &prompt[user + crate::agent_wire::USER_MARK.len()..];
            (head, "", rest)
        }
        _ => {
            return vec![crate::compact::ChatTurn::new("user", prompt)];
        }
    };
    if preamble_at.is_some() {
        if !system_head.is_empty() {
            turns.push(crate::compact::ChatTurn::new("system", system_head));
        }
        let (preamble, prior) = split_prior(mid.trim());
        if !preamble.is_empty() {
            turns.push(crate::compact::ChatTurn::new("user", preamble));
        }
        turns.extend(prior_turns(&prior));
    } else {
        let (system_text, prior) = split_prior(system_head);
        if !system_text.is_empty() {
            turns.push(crate::compact::ChatTurn::new("system", system_text));
        }
        turns.extend(prior_turns(&prior));
    }
    if let Some(tool_at) = rest.find("\n\n<tool_result") {
        let user = rest[..tool_at].trim();
        if !user.is_empty() {
            turns.push(crate::compact::ChatTurn::new("user", user));
        }
        let tools = rest[tool_at..].trim();
        if !tools.is_empty() {
            turns.push(crate::compact::ChatTurn::new("user", tools));
        }
    } else if !rest.trim().is_empty() {
        turns.push(crate::compact::ChatTurn::new("user", rest.trim()));
    }
    if turns.is_empty() {
        turns.push(crate::compact::ChatTurn::new("user", prompt));
    }
    turns
}

fn split_prior(head: &str) -> (String, String) {
    const MARK: &str = "\n\nPrior:\n";
    match head.find(MARK) {
        Some(at) => (
            head[..at].trim().to_owned(),
            head[at + MARK.len()..].to_owned(),
        ),
        None => (head.to_owned(), String::new()),
    }
}

fn prior_turns(prior: &str) -> Vec<crate::compact::ChatTurn> {
    let prior = prior.trim();
    if prior.is_empty() {
        return Vec::new();
    }
    let mut turns = Vec::new();
    let mut rest = prior;
    while !rest.is_empty() {
        let (role, skip) = if rest.starts_with("User:\n") {
            ("user", 6usize)
        } else if rest.starts_with("Assistant:\n") {
            ("assistant", 11usize)
        } else {
            turns.push(crate::compact::ChatTurn::new("user", rest));
            break;
        };
        rest = &rest[skip..];
        let next = rest
            .find("\n\nUser:\n")
            .into_iter()
            .chain(rest.find("\n\nAssistant:\n"))
            .min()
            .unwrap_or(rest.len());
        let text = rest[..next].trim();
        if !text.is_empty() {
            turns.push(crate::compact::ChatTurn::new(role, text));
        }
        rest = rest[next..].trim_start();
        if rest.starts_with("\n\n") {
            rest = rest.trim_start();
        }
    }
    turns
}

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn user_info_block(run: &LocalRun) -> String {
    let mut lines = Vec::new();
    if !run.os_version.is_empty() {
        lines.push(format!("OS Version: {}", run.os_version));
    }
    if !run.shell.is_empty() {
        lines.push(format!("Shell: {}", run.shell));
    }
    if let Some(root) = run.workspace.as_deref().filter(|path| !path.is_empty()) {
        lines.push(format!("Workspace Path: {root}"));
    }
    if !run.time_zone.is_empty() {
        lines.push(format!("Time Zone: {}", run.time_zone));
    }
    lines.join("\n\n")
}

fn attached_files_block(files: &[(String, String)], cap: usize) -> String {
    let files: Vec<_> = files
        .iter()
        .filter(|(path, body)| !path.is_empty() && !body.is_empty())
        .collect();
    if files.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n\n<attached_files description=\"Files the user referenced via @File. Read them with the Read tool to see their contents.\">\n",
    );
    let mut used = 0usize;
    for (path, body) in files {
        if used >= cap {
            break;
        }
        if body.len() <= ATTACHED_INLINE_CHARS {
            out.push_str(&format!(
                "<attached_file path=\"{}\">{}</attached_file>\n",
                xml_attr(path),
                xml_attr(&crate::agent_wire::clip_utf8(body, ATTACHED_INLINE_CHARS))
            ));
            used += body.len();
        } else {
            let lines = body.lines().count();
            out.push_str(&format!(
                "<attached_file path=\"{}\" size=\"{}\" lines=\"{lines}\">Use Read to view this file.</attached_file>\n",
                xml_attr(path),
                body.len()
            ));
        }
    }
    out.push_str("</attached_files>");
    out
}

pub fn outbound_turns(prompt: &str, context_tokens: u32) -> Vec<crate::compact::ChatTurn> {
    crate::compact::compact_turns(&prompt_to_turns(prompt), prompt_char_budget(context_tokens))
}

pub fn outbound_turns_for(run: &LocalRun, prompt: &str) -> Vec<crate::compact::ChatTurn> {
    let mut turns = outbound_turns(prompt, run.max_tokens);
    bind_history_images(&mut turns, &run.history_images);
    turns
}

fn bind_history_images(turns: &mut [crate::compact::ChatTurn], images: &[(String, String)]) {
    if images.is_empty() {
        return;
    }
    let mut next = 0usize;
    for turn in turns {
        if next >= images.len() {
            break;
        }
        if turn.role != "user" || turn.text.contains("<user_query>") {
            continue;
        }
        let n = turn.text.matches("[image ").count();
        for _ in 0..n {
            if next >= images.len() {
                break;
            }
            turn.images.push(images[next].clone());
            next += 1;
        }
    }
}

pub fn parse_tool_uses(text: &str) -> Vec<ToolUse> {
    let mut tools = Vec::new();
    for (start_tag, end_tag) in [("<tool_call>", "</tool_call>"), ("<tool>", "</tool>")] {
        let mut rest = text;
        while let Some(start) = rest.find(start_tag) {
            let after = &rest[start + start_tag.len()..];
            let Some(end) = after.find(end_tag) else {
                break;
            };
            if let Some(tool) = tool_from_json(after[..end].trim()) {
                tools.push(tool);
            }
            rest = &after[end + end_tag.len()..];
        }
    }
    if tools.is_empty() {
        for block in fenced_blocks(text, "tool_call") {
            if let Some(tool) = tool_from_json(block) {
                tools.push(tool);
            }
        }
    }
    if tools.is_empty() {
        if let Some(tool) = tool_from_json(text.trim()) {
            tools.push(tool);
        }
    }
    for (index, tool) in tools.iter_mut().enumerate() {
        if tool.id.is_empty() {
            tool.id = format!("call_{}", index + 1);
        }
    }
    tools
}

fn fenced_blocks<'a>(text: &'a str, lang: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let needle = format!("```{lang}");
    let mut rest = text;
    while let Some(start) = rest.find(&needle) {
        let after = &rest[start + needle.len()..];
        let after = after.strip_prefix('\r').unwrap_or(after);
        let after = after.strip_prefix('\n').unwrap_or(after);
        let Some(end) = after.find("```") else {
            break;
        };
        out.push(after[..end].trim());
        rest = &after[end + 3..];
    }
    out
}

fn tool_from_json(raw: &str) -> Option<ToolUse> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    let name = obj
        .get("name")
        .or_else(|| obj.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if name.is_empty() {
        return None;
    }
    let id = obj
        .get("id")
        .or_else(|| obj.get("call_id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let arguments = if let Some(args) = obj.get("arguments").cloned() {
        match args {
            Value::String(text) => serde_json::from_str(&text).unwrap_or(json!({"raw": text})),
            other => other,
        }
    } else if let Some(input) = obj.get("input").cloned() {
        input
    } else {
        let mut args = json!({});
        for (key, val) in obj {
            if matches!(key.as_str(), "name" | "tool" | "id" | "call_id") {
                continue;
            }
            args[key] = val.clone();
        }
        args
    };
    Some(ToolUse {
        id,
        name,
        arguments,
    })
}

fn native_tool_usable(call: &crate::connect::ChatToolCall) -> bool {
    if call.name.is_empty() || map_tool_name(&call.name).is_none() {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(&call.arguments) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    match map_tool_name(&call.name) {
        Some(CursorTool::Read | CursorTool::Edit | CursorTool::Write | CursorTool::Ls) => obj
            .get("path")
            .or_else(|| obj.get("file_path"))
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty()),
        Some(CursorTool::Grep) => obj
            .get("pattern")
            .and_then(Value::as_str)
            .is_some_and(|pattern| !pattern.is_empty()),
        Some(CursorTool::Glob) => obj
            .get("glob_pattern")
            .or_else(|| obj.get("globPattern"))
            .or_else(|| obj.get("pattern"))
            .and_then(Value::as_str)
            .is_some_and(|pattern| !pattern.is_empty()),
        Some(CursorTool::Shell) => obj
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| !command.is_empty()),
        Some(CursorTool::WebSearch) => obj
            .get("search_term")
            .or_else(|| obj.get("searchTerm"))
            .and_then(Value::as_str)
            .is_some_and(|term| !term.is_empty()),
        Some(CursorTool::Task) => obj
            .get("prompt")
            .and_then(Value::as_str)
            .is_some_and(|prompt| !prompt.is_empty()),
        Some(CursorTool::CreatePlan) => obj
            .get("plan")
            .and_then(Value::as_str)
            .is_some_and(|plan| !plan.is_empty()),
        Some(CursorTool::Mcp) => obj
            .get("toolName")
            .or_else(|| obj.get("tool_name"))
            .or_else(|| obj.get("name"))
            .and_then(Value::as_str)
            .is_some_and(|name| !name.is_empty()),
        Some(CursorTool::GetMcpTools) => true,
        Some(CursorTool::WebFetch) => obj
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|url| url.starts_with("http")),
        Some(CursorTool::TodoWrite) => obj.get("todos").and_then(Value::as_array).is_some(),
        Some(CursorTool::Delete) => obj
            .get("path")
            .or_else(|| obj.get("file_path"))
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty()),
        Some(CursorTool::AskQuestion) => true,
        Some(CursorTool::SwitchMode) => obj
            .get("target_mode_id")
            .or_else(|| obj.get("targetModeId"))
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty()),
        Some(CursorTool::ApplyPatch) => obj
            .get("patch")
            .and_then(Value::as_str)
            .is_some_and(|p| p.contains("Begin Patch")),
        Some(CursorTool::Await | CursorTool::UpdateCurrentStep | CursorTool::ReadLints) => true,
        Some(CursorTool::EditNotebook) => obj
            .get("target_notebook")
            .or_else(|| obj.get("path"))
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty()),
        Some(CursorTool::GenerateImage) => obj
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.is_empty()),
        Some(CursorTool::ListMcpResources) => true,
        Some(CursorTool::FetchMcpResource) => {
            obj.get("server")
                .and_then(Value::as_str)
                .is_some_and(|server| !server.is_empty())
                && obj
                    .get("uri")
                    .and_then(Value::as_str)
                    .is_some_and(|uri| !uri.is_empty())
        }
        None => false,
    }
}

pub fn parse_model_turn(
    thinking: String,
    text: String,
    sse_tools: Vec<crate::connect::ChatToolCall>,
) -> ModelTurn {
    let native_attempted = !sse_tools.is_empty();
    let mut tools: Vec<ToolUse> = sse_tools
        .into_iter()
        .enumerate()
        .filter_map(|(index, call)| {
            if !native_tool_usable(&call) {
                return None;
            }
            let arguments = serde_json::from_str(&call.arguments)
                .unwrap_or_else(|_| json!({ "raw": call.arguments }));
            Some(ToolUse {
                id: if call.id.is_empty() {
                    format!("call_{}", index + 1)
                } else {
                    call.id
                },
                name: call.name,
                arguments,
            })
        })
        .collect();
    if tools.is_empty() && !native_attempted {
        tools = parse_tool_uses(&text);
    }
    let visible = if tools.is_empty() {
        text
    } else {
        strip_tool_markup(&text)
    };
    ModelTurn {
        thinking,
        text: visible,
        tools,
        usage: TokenUsage::default(),
    }
}

fn strip_tool_markup(text: &str) -> String {
    let mut out = text.to_owned();
    for (start_tag, end_tag) in [("<tool_call>", "</tool_call>"), ("<tool>", "</tool>")] {
        while let Some(start) = out.find(start_tag) {
            if let Some(rel) = out[start..].find(end_tag) {
                let end = start + rel + end_tag.len();
                out.replace_range(start..end, "");
            } else {
                break;
            }
        }
    }
    out.trim().to_owned()
}

fn json_str(args: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = args.get(*key) {
            if let Some(text) = value.as_str() {
                if !text.is_empty() {
                    return text.to_owned();
                }
            }
        }
    }
    String::new()
}

fn json_i32(args: &Value, keys: &[&str]) -> Option<i32> {
    for key in keys {
        if let Some(value) = args.get(*key) {
            if let Some(n) = value.as_i64() {
                return Some(n as i32);
            }
            if let Some(text) = value.as_str() {
                if let Ok(n) = text.parse::<i32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn json_bool(args: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| args.get(*key).and_then(Value::as_bool) == Some(true))
}

fn abs_path(workspace: Option<&str>, path: &str) -> String {
    if path.is_empty() {
        return workspace.unwrap_or(".").to_owned();
    }
    let raw = PathBuf::from(path);
    let joined = if raw.is_absolute() {
        raw
    } else {
        match workspace {
            Some(root) if !root.is_empty() => Path::new(root).join(&raw),
            _ => raw,
        }
    };
    let Some(root) = workspace.filter(|root| !root.is_empty()) else {
        return joined.to_string_lossy().into_owned();
    };
    if path_inside_workspace(Path::new(root), &joined) {
        return joined.to_string_lossy().into_owned();
    }
    Path::new(root).join(".gba-denied").to_string_lossy().into_owned()
}

fn path_inside_workspace(root: &Path, candidate: &Path) -> bool {
    let root_n = normalize_path_components(root);
    let cand_n = normalize_path_components(candidate);
    let root_s = root_n.to_string_lossy().to_ascii_lowercase();
    let cand_s = cand_n.to_string_lossy().to_ascii_lowercase();
    cand_s == root_s
        || cand_s.starts_with(&(root_s.clone() + "\\"))
        || cand_s.starts_with(&(root_s + "/"))
}

fn normalize_path_components(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                let _ = out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn iu(message: interaction_update::Message) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::InteractionUpdate(
            InteractionUpdate {
                message: Some(message),
            },
        )),
    }
}

pub fn heartbeat_frame() -> AgentServerMessage {
    iu(interaction_update::Message::Heartbeat(HeartbeatUpdate {}))
}

fn thinking_delta_frame(text: &str) -> AgentServerMessage {
    iu(interaction_update::Message::ThinkingDelta(
        ThinkingDeltaUpdate {
            text: text.to_owned(),
            thinking_style: Some(1),
        },
    ))
}

fn thinking_completed_frame(duration_ms: i32) -> AgentServerMessage {
    iu(interaction_update::Message::ThinkingCompleted(
        ThinkingCompletedUpdate {
            thinking_duration_ms: duration_ms,
        },
    ))
}

fn thinking_frames(thinking: &str) -> Vec<AgentServerMessage> {
    if thinking.is_empty() {
        return Vec::new();
    }
    vec![thinking_delta_frame(thinking), thinking_completed_frame(0)]
}

fn token_delta_frame(tokens: i32) -> AgentServerMessage {
    iu(interaction_update::Message::TokenDelta(
        agent_wire::TokenDeltaUpdate { tokens },
    ))
}

fn maybe_autocompact_prompt<E>(
    prompt: String,
    max_tokens: u32,
    budget: usize,
    emit: &mut E,
    archives: &mut Vec<Vec<u8>>,
) -> String
where
    E: FnMut(AgentServerMessage),
{
    let cap = if max_tokens == 0 {
        crate::agent_wire::DEFAULT_CONTEXT_TOKENS
    } else {
        max_tokens
    };
    if (estimate_tokens(&prompt) as u32).saturating_mul(100) < cap.saturating_mul(85) {
        return prompt;
    }
    emit(iu(interaction_update::Message::SummaryStarted(
        agent_wire::SummaryStartedUpdate {},
    )));
    let turns = prompt_to_turns(&prompt);
    let compacted = crate::compact::compact_turns(&turns, budget.max(4_096));
    let summary = compacted
        .iter()
        .find(|turn| turn.role == "assistant")
        .map(|turn| turn.text.clone())
        .unwrap_or_else(|| "Conversation compacted.".into());
    emit(iu(interaction_update::Message::Summary(
        agent_wire::SummaryUpdate {
            summary: summary.clone(),
        },
    )));
    emit(iu(interaction_update::Message::SummaryCompleted(
        agent_wire::SummaryCompletedUpdate {
            hook_message: Some("Chat context summarized.".into()),
        },
    )));
    let (id, data) = crate::blob::encode_role("summary", &summary);
    let _ = data;
    archives.push(crate::blob::id_bytes(&id));
    turns_to_prompt(&compacted)
}

fn turns_to_prompt(turns: &[crate::compact::ChatTurn]) -> String {
    let mut out = String::new();
    let mut wrote_preamble = false;
    for turn in turns {
        match turn.role.as_str() {
            "user" if !wrote_preamble && looks_like_preamble(&turn.text) => {
                out.push_str(crate::agent_wire::PREAMBLE_MARK);
                out.push_str(&turn.text);
                wrote_preamble = true;
            }
            "user" => {
                out.push_str(crate::agent_wire::USER_MARK);
                out.push_str(&turn.text);
            }
            "assistant" => {
                out.push_str("\n\nAssistant:\n");
                out.push_str(&turn.text);
            }
            _ => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&turn.text);
            }
        }
    }
    out
}

fn looks_like_preamble(text: &str) -> bool {
    text.contains("<user_info>")
        || text.contains("<agent_skills>")
        || text.contains("<rules>")
        || text.contains("<attached_files>")
        || text.contains("<git_status>")
        || text.contains("<manually_attached_skills>")
}

#[derive(Default)]
struct CheckpointTrack {
    read_paths: Vec<String>,
    file_states: std::collections::HashMap<String, Vec<u8>>,
    plans: std::collections::HashMap<String, crate::agent_wire::PlanRegistryEntry>,
    subagent_states: std::collections::HashMap<String, crate::agent_wire::SubagentPersistedState>,
    plan: Option<Vec<u8>>,
}

fn emit_run_checkpoint<E>(
    emit: &mut E,
    run: &LocalRun,
    used: u32,
    max: u32,
    blobs: &[Vec<u8>],
    prompt: &str,
    todos: &[crate::agent_proto::TodoItem],
    pending: &[String],
    archives: &[Vec<u8>],
    track: &CheckpointTrack,
) where
    E: FnMut(AgentServerMessage),
{
    use prost::Message;
    let mut frame = agent_wire::checkpoint_with_prompt(
        used,
        max,
        run.workspace.as_deref(),
        blobs,
        prompt,
    );
    agent_wire::apply_checkpoint_extras(
        &mut frame,
        &agent_wire::CheckpointExtras {
            mode: run.mode,
            git_repos: run.git_repos.clone(),
            todos: todos.iter().map(|todo| todo.encode_to_vec()).collect(),
            pending_tool_calls: pending.to_vec(),
            summary_archives: archives.to_vec(),
            read_paths: track.read_paths.clone(),
            file_states: track.file_states.clone(),
            plans: track.plans.clone(),
            subagent_states: track.subagent_states.clone(),
            plan: track.plan.clone(),
        },
    );
    emit(frame);
}

fn note_read_path(track: &mut CheckpointTrack, path: String) {
    if path.is_empty() {
        return;
    }
    if !track.read_paths.contains(&path) {
        track.read_paths.push(path.clone());
    }
    track.file_states.entry(path).or_default();
}

fn note_tool_checkpoint(
    track: &mut CheckpointTrack,
    run: &LocalRun,
    kind: CursorTool,
    tool: &ToolUse,
    result_text: &str,
) {
    match kind {
        CursorTool::Read
        | CursorTool::Write
        | CursorTool::Edit
        | CursorTool::ApplyPatch
        | CursorTool::EditNotebook => {
            let path = abs_path(
                run.workspace.as_deref(),
                &json_str(
                    &tool.arguments,
                    &["path", "file_path", "target_notebook"],
                ),
            );
            note_read_path(track, path);
        }
        CursorTool::CreatePlan => {
            let mut name = json_str(&tool.arguments, &["name"]);
            if name.is_empty() {
                name = "plan".into();
            }
            let path = result_text
                .rsplit(" at ")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches('.')
                .to_owned();
            track.plans.insert(
                name.clone(),
                crate::agent_wire::PlanRegistryEntry {
                    id: name,
                    path: path.clone(),
                },
            );
            if !path.is_empty() {
                track.plan = Some(path.into_bytes());
            }
        }
        CursorTool::Task => {
            track.subagent_states.insert(
                tool.id.clone(),
                crate::agent_wire::SubagentPersistedState {
                    model_id: resolve_task_model(run, &tool.arguments),
                },
            );
        }
        _ => {}
    }
}

fn pending_assistant_json(thinking: &str, text: &str) -> Vec<String> {
    if thinking.is_empty() && text.is_empty() {
        return Vec::new();
    }
    let mut content = Vec::new();
    if !thinking.is_empty() {
        content.push(serde_json::json!({"type":"reasoning","text": thinking}));
    }
    if !text.is_empty() {
        content.push(serde_json::json!({"type":"text","text": text}));
    }
    vec![serde_json::json!({
        "id": "1",
        "role": "assistant",
        "content": content,
    })
    .to_string()]
}

fn step_started_frame(step_id: u64) -> AgentServerMessage {
    iu(interaction_update::Message::StepStarted(
        agent_wire::StepStartedUpdate { step_id },
    ))
}

fn step_completed_frame(step_id: u64, duration_ms: i64) -> AgentServerMessage {
    iu(interaction_update::Message::StepCompleted(
        agent_wire::StepCompletedUpdate {
            step_id,
            step_duration_ms: duration_ms,
        },
    ))
}

fn text_frame(text: &str) -> AgentServerMessage {
    iu(interaction_update::Message::TextDelta(TextDeltaUpdate {
        text: text.to_owned(),
        is_server_notice: false,
    }))
}

fn turn_ended_usage(usage: &TokenUsage, fallback_text: &str) -> AgentServerMessage {
    let input = if usage.input > 0 {
        usage.input
    } else {
        i64::from(estimate_tokens(fallback_text)).max(1)
    };
    iu(interaction_update::Message::TurnEnded(TurnEndedUpdate {
        input_tokens: Some(input),
        output_tokens: Some(usage.output.max(0)),
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: (usage.reasoning > 0).then_some(usage.reasoning),
    }))
}

fn shell_args(command: &str, cwd: &str, call_id: &str, description: &str) -> ShellArgs {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let name = parts.first().copied().unwrap_or("");
    ShellArgs {
        command: command.to_owned(),
        working_directory: cwd.to_owned(),
        timeout: SHELL_TIMEOUT_MS,
        tool_call_id: call_id.to_owned(),
        simple_commands: if name.is_empty() {
            Vec::new()
        } else {
            vec![name.to_owned()]
        },
        parsing_result: Some(ShellCommandParsingResult {
            parsing_failed: false,
            executable_commands: if name.is_empty() {
                Vec::new()
            } else {
                vec![ShellExecutableCommand {
                    name: name.to_owned(),
                    full_text: command.to_owned(),
                }]
            },
        }),
        file_output_threshold_bytes: Some(SHELL_FILE_OUTPUT_THRESHOLD),
        is_background: false,
        skip_approval: false,
        timeout_behavior: SHELL_TIMEOUT_BEHAVIOR_BACKGROUND,
        hard_timeout: Some(SHELL_HARD_TIMEOUT_MS),
        description: (!description.is_empty()).then(|| description.to_owned()),
        close_stdin: false,
    }
}

/// Shipped encoder: Read (+ Grep) tool-use → started + exec_server frames.
pub fn encode_tool_start_frames(
    workspace: Option<&str>,
    tool: &ToolUse,
    exec_id: u32,
    model_call_id: &str,
) -> Option<(Vec<AgentServerMessage>, CursorTool)> {
    encode_tool_start_frames_ex(workspace, tool, exec_id, model_call_id, None, false, None)
}

pub fn encode_tool_start_frames_ex(
    workspace: Option<&str>,
    tool: &ToolUse,
    exec_id: u32,
    model_call_id: &str,
    conversation_id: Option<&str>,
    force_task_background: bool,
    task_model: Option<&str>,
) -> Option<(Vec<AgentServerMessage>, CursorTool)> {
    let kind = map_tool_name(&tool.name)?;
    let call_id = if tool.id.is_empty() {
        format!("call_{exec_id}")
    } else {
        tool.id.clone()
    };
    let exec_name = format!("{call_id}-exec");
    let args = &tool.arguments;
    let started_tool = match kind {
        CursorTool::Read => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            ToolCall {
                tool: Some(tool_call::Tool::ReadToolCall(ReadToolCall {
                    args: Some(ReadToolArgs {
                        path,
                        offset: json_i32(args, &["offset"]),
                        limit: json_i32(args, &["limit"]),
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::Grep => ToolCall {
            tool: Some(tool_call::Tool::GrepToolCall(GrepToolCall {
                args: Some(grep_args(workspace, args, &call_id, false)),
                result: None,
            })),
        },
        CursorTool::Glob => {
            let pattern = json_str(args, &["glob_pattern", "globPattern", "pattern"]);
            let dir = json_str(args, &["target_directory", "targetDirectory", "path"]);
            ToolCall {
                tool: Some(tool_call::Tool::GlobToolCall(GlobToolCall {
                    args: Some(GlobToolArgs {
                        target_directory: (!dir.is_empty()).then(|| abs_path(workspace, &dir)),
                        glob_pattern: pattern,
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::Ls => {
            let path = abs_path(workspace, &json_str(args, &["path"]));
            ToolCall {
                tool: Some(tool_call::Tool::LsToolCall(LsToolCall {
                    args: Some(LsArgs {
                        path,
                        ignore: Vec::new(),
                        tool_call_id: call_id.clone(),
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::Shell => {
            let command = json_str(args, &["command"]);
            let cwd = abs_path(
                workspace,
                &json_str(args, &["working_directory", "workingDirectory", "cwd"]),
            );
            let description = json_str(args, &["description"]);
            let shell = shell_args(&command, &cwd, &call_id, &description);
            ToolCall {
                tool: Some(tool_call::Tool::ShellToolCall(ShellToolCall {
                    args: Some(shell.clone()),
                    result: None,
                    description: shell.description.clone(),
                })),
            }
        }
        CursorTool::Edit | CursorTool::Write => {
            let path = abs_path(workspace, &json_str(args, &["path"]));
            ToolCall {
                tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                    args: Some(EditArgs {
                        path,
                        stream_content: None,
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::WebSearch => ToolCall {
            tool: Some(tool_call::Tool::WebSearchToolCall(
                crate::agent_proto::WebSearchToolCall {
                    args: Some(web_search_args(args, &call_id)),
                    result: None,
                },
            )),
        },
        CursorTool::Task => ToolCall {
            tool: Some(tool_call::Tool::TaskToolCall(crate::agent_proto::TaskToolCall {
                args: Some(crate::agent_proto::TaskArgs {
                    description: json_str(args, &["description"]),
                    prompt: json_str(args, &["prompt"]),
                }),
                result: None,
            })),
        },
        CursorTool::CreatePlan => ToolCall {
            tool: Some(tool_call::Tool::CreatePlanToolCall(
                crate::agent_proto::CreatePlanToolCall {
                    args: Some(create_plan_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::Mcp => ToolCall {
            tool: Some(tool_call::Tool::McpToolCall(crate::agent_proto::McpToolCall {
                args: Some(mcp_args(args, &call_id)),
                result: None,
            })),
        },
        CursorTool::GetMcpTools => ToolCall {
            tool: Some(tool_call::Tool::GetMcpToolsToolCall(
                crate::agent_proto::GetMcpToolsToolCall {
                    args: Some(crate::agent_proto::GetMcpToolsArgs {
                        server: none_if_empty(json_str(args, &["namespace", "server"])),
                        tool_name: none_if_empty(json_str(args, &["toolName", "tool_name"])),
                        pattern: none_if_empty(json_str(args, &["pattern"])),
                    }),
                    result: None,
                },
            )),
        },
        CursorTool::WebFetch => ToolCall {
            tool: Some(tool_call::Tool::WebFetchToolCall(
                crate::agent_proto::WebFetchToolCall {
                    args: Some(web_fetch_args(args, &call_id)),
                    result: None,
                },
            )),
        },
        CursorTool::TodoWrite => ToolCall {
            tool: Some(tool_call::Tool::UpdateTodosToolCall(
                crate::agent_proto::UpdateTodosToolCall {
                    args: Some(update_todos_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::Delete => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            ToolCall {
                tool: Some(tool_call::Tool::DeleteToolCall(
                    crate::agent_proto::DeleteToolCall {
                        args: Some(crate::agent_proto::DeleteArgs {
                            path,
                            tool_call_id: call_id.clone(),
                        }),
                        result: None,
                    },
                )),
            }
        }
        CursorTool::AskQuestion => ToolCall {
            tool: Some(tool_call::Tool::AskQuestionToolCall(
                crate::agent_proto::AskQuestionToolCall {
                    args: Some(ask_question_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::SwitchMode => ToolCall {
            tool: Some(tool_call::Tool::SwitchModeToolCall(
                crate::agent_proto::SwitchModeToolCall {
                    args: Some(switch_mode_args(args, &call_id)),
                    result: None,
                },
            )),
        },
        CursorTool::ApplyPatch => {
            let path = apply_patch_path(workspace, args);
            ToolCall {
                tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                    args: Some(EditArgs {
                        path,
                        stream_content: None,
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::Await => ToolCall {
            tool: Some(tool_call::Tool::AwaitToolCall(
                crate::agent_proto::AwaitToolCall {
                    args: Some(await_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::UpdateCurrentStep => ToolCall {
            tool: Some(tool_call::Tool::CommunicateUpdateToolCall(
                crate::agent_proto::CommunicateUpdateToolCall {
                    args: Some(communicate_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::ReadLints => ToolCall {
            tool: Some(tool_call::Tool::ReadLintsToolCall(
                crate::agent_proto::ReadLintsToolCall {
                    args: Some(read_lints_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::EditNotebook => {
            let path = abs_path(workspace, &json_str(args, &["target_notebook", "path"]));
            ToolCall {
                tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                    args: Some(EditArgs {
                        path,
                        stream_content: none_if_empty(json_str(args, &["new_string", "newString"])),
                    }),
                    result: None,
                })),
            }
        }
        CursorTool::GenerateImage => ToolCall {
            tool: Some(tool_call::Tool::GenerateImageToolCall(
                crate::agent_proto::GenerateImageToolCall {
                    args: Some(generate_image_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::ListMcpResources => ToolCall {
            tool: Some(tool_call::Tool::ListMcpResourcesToolCall(
                crate::agent_proto::ListMcpResourcesToolCall {
                    args: Some(list_mcp_resources_args(args)),
                    result: None,
                },
            )),
        },
        CursorTool::FetchMcpResource => ToolCall {
            tool: Some(tool_call::Tool::ReadMcpResourceToolCall(
                crate::agent_proto::ReadMcpResourceToolCall {
                    args: Some(read_mcp_resource_args(args, &call_id)),
                    result: None,
                },
            )),
        },
    };
    let exec_msg = match kind {
        CursorTool::Read => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            exec_server_message::Message::ReadArgs(ReadArgs {
                path,
                tool_call_id: call_id.clone(),
                offset: json_i32(args, &["offset"]),
                limit: json_i32(args, &["limit"]).map(|n| n as u32),
            })
        }
        CursorTool::Grep => {
            exec_server_message::Message::GrepArgs(grep_args(workspace, args, &call_id, false))
        }
        CursorTool::Glob => {
            exec_server_message::Message::GrepArgs(grep_args(workspace, args, &call_id, true))
        }
        CursorTool::Ls => {
            let path = abs_path(workspace, &json_str(args, &["path"]));
            exec_server_message::Message::LsArgs(LsArgs {
                path,
                ignore: Vec::new(),
                tool_call_id: call_id.clone(),
            })
        }
        CursorTool::Shell => {
            let command = json_str(args, &["command"]);
            let cwd = abs_path(
                workspace,
                &json_str(args, &["working_directory", "workingDirectory", "cwd"]),
            );
            exec_server_message::Message::ShellStreamArgs(shell_args(
                &command,
                &cwd,
                &call_id,
                &json_str(args, &["description"]),
            ))
        }
        CursorTool::Write | CursorTool::Edit | CursorTool::ApplyPatch | CursorTool::EditNotebook => {
            let path = if kind == CursorTool::ApplyPatch {
                apply_patch_path(workspace, args)
            } else if kind == CursorTool::EditNotebook {
                abs_path(workspace, &json_str(args, &["target_notebook", "path"]))
            } else {
                abs_path(workspace, &json_str(args, &["path"]))
            };
            exec_server_message::Message::ReadArgs(ReadArgs {
                path,
                tool_call_id: call_id.clone(),
                offset: None,
                limit: None,
            })
        }
        CursorTool::ListMcpResources => {
            exec_server_message::Message::ListMcpResourcesExecArgs(list_mcp_resources_args(args))
        }
        CursorTool::FetchMcpResource => {
            exec_server_message::Message::ReadMcpResourceExecArgs(read_mcp_resource_args(
                args, &call_id,
            ))
        }
        CursorTool::ReadLints => {
            let path = lint_path(workspace, args);
            exec_server_message::Message::DiagnosticsArgs(crate::agent_proto::DiagnosticsArgs {
                path,
                tool_call_id: call_id.clone(),
            })
        }
        CursorTool::Mcp => exec_server_message::Message::McpArgs(mcp_args(args, &call_id)),
        CursorTool::Task => {
            let subagent_type = json_str(args, &["subagent_type", "subagentType"]);
            exec_server_message::Message::SubagentArgs(crate::agent_proto::SubagentArgs {
                tool_call_id: call_id.clone(),
                subagent_type: if subagent_type.is_empty() {
                    "explore".into()
                } else {
                    subagent_type
                },
                model_id: {
                    let llm = json_str(args, &["model", "model_id", "modelId"]);
                    if !llm.is_empty() {
                        llm
                    } else {
                        task_model.unwrap_or("").to_owned()
                    }
                },
                prompt: json_str(args, &["prompt"]),
                readonly: json_bool(args, &["readonly"]),
                resume_agent_id: none_if_empty(json_str(args, &["resume", "resume_agent_id"])),
                run_in_background: (force_task_background
                    || json_bool(args, &["run_in_background", "runInBackground"]))
                .then_some(true),
                parent_conversation_id: none_if_empty(json_str(
                    args,
                    &["parent_conversation_id", "parentConversationId"],
                ))
                .or_else(|| conversation_id.map(str::to_owned)),
                fork_agent_id: none_if_empty(json_str(args, &["fork_agent_id", "forkAgentId"]))
                    .or_else(|| {
                        let resume = json_str(args, &["resume"]);
                        (resume == "self").then(|| {
                            conversation_id
                                .map(str::to_owned)
                                .unwrap_or_else(|| call_id.clone())
                        })
                    }),
            })
        }
        CursorTool::Delete => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            exec_server_message::Message::DeleteArgs(crate::agent_proto::DeleteArgs {
                path,
                tool_call_id: call_id.clone(),
            })
        }
        CursorTool::GetMcpTools => {
            let namespace = json_str(args, &["namespace", "server"]);
            exec_server_message::Message::McpStateExecArgs(crate::agent_proto::McpStateExecArgs {
                server_identifiers: if namespace.is_empty() {
                    Vec::new()
                } else {
                    vec![namespace]
                },
                kick_only: false,
            })
        }
        CursorTool::WebSearch
        | CursorTool::CreatePlan
        | CursorTool::WebFetch
        | CursorTool::TodoWrite
        | CursorTool::AskQuestion
        | CursorTool::SwitchMode
        | CursorTool::Await
        | CursorTool::UpdateCurrentStep
        | CursorTool::GenerateImage => {
            return Some((
                vec![agent_wire::server_tool_started(
                    &call_id,
                    started_tool,
                    model_call_id,
                )],
                kind,
            ));
        }
    };
    let mut frames = Vec::new();
    frames.push(agent_wire::server_tool_started(
        &call_id,
        started_tool,
        model_call_id,
    ));
    frames.push(agent_wire::server_exec(exec_id, &exec_name, exec_msg));
    Some((frames, kind))
}

fn none_if_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn web_fetch_args(args: &Value, call_id: &str) -> crate::agent_proto::WebFetchArgs {
    crate::agent_proto::WebFetchArgs {
        url: json_str(args, &["url"]),
        tool_call_id: call_id.to_owned(),
    }
}

fn update_todos_args(args: &Value) -> crate::agent_proto::UpdateTodosArgs {
    let todos = args
        .get("todos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(crate::agent_proto::TodoItem {
                id: item.get("id")?.as_str()?.to_owned(),
                content: item.get("content")?.as_str()?.to_owned(),
                status: match item.get("status").and_then(Value::as_str).unwrap_or("") {
                    "in_progress" | "in-progress" => 2,
                    "completed" | "complete" => 3,
                    "cancelled" => 4,
                    _ => 1,
                },
            })
        })
        .collect();
    crate::agent_proto::UpdateTodosArgs {
        todos,
        merge: json_bool(args, &["merge"]),
    }
}

fn ask_question_args(args: &Value) -> crate::agent_proto::AskQuestionArgs {
    let questions = args
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, item)| crate::agent_proto::AskQuestionQuestion {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("q{index}")),
            prompt: item
                .get("prompt")
                .or_else(|| item.get("question"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            options: item
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(oi, opt)| crate::agent_proto::AskQuestionOption {
                    id: opt
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("o{oi}")),
                    label: opt
                        .as_str()
                        .or_else(|| opt.get("label").and_then(Value::as_str))
                        .unwrap_or("")
                        .to_owned(),
                })
                .collect(),
            allow_multiple: item
                .get("allow_multiple")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect();
    crate::agent_proto::AskQuestionArgs {
        title: json_str(args, &["title"]),
        questions,
    }
}

fn switch_mode_args(args: &Value, call_id: &str) -> crate::agent_proto::SwitchModeArgs {
    crate::agent_proto::SwitchModeArgs {
        target_mode_id: json_str(args, &["target_mode_id", "targetModeId"]),
        explanation: none_if_empty(json_str(args, &["explanation"])),
        tool_call_id: call_id.to_owned(),
    }
}

fn await_args(args: &Value) -> crate::agent_proto::AwaitArgs {
    crate::agent_proto::AwaitArgs {
        task_id: json_str(args, &["task_id", "taskId"]),
        block_until_ms: json_i32(args, &["block_until_ms", "blockUntilMs"]).map(|n| n.max(0) as u32),
        regex: none_if_empty(json_str(args, &["pattern", "regex"])),
    }
}

fn communicate_args(args: &Value) -> crate::agent_proto::CommunicateUpdateArgs {
    crate::agent_proto::CommunicateUpdateArgs {
        current_step: none_if_empty(json_str(args, &["current_step", "currentStep"])),
        final_summary: none_if_empty(json_str(args, &["final_summary", "finalSummary"])),
        completed_subtitle: none_if_empty(json_str(
            args,
            &["completed_subtitle", "completedSubtitle"],
        )),
    }
}

fn generate_image_args(args: &Value) -> crate::agent_proto::GenerateImageArgs {
    crate::agent_proto::GenerateImageArgs {
        description: json_str(args, &["description"]),
        file_path: none_if_empty(json_str(args, &["filename", "file_path", "filePath", "path"])),
        reference_image_paths: json_str_list(args, &["reference_image_paths", "referenceImagePaths"]),
        aspect_ratio: none_if_empty(json_str(args, &["aspect_ratio", "aspectRatio"])),
    }
}

fn list_mcp_resources_args(args: &Value) -> crate::agent_proto::ListMcpResourcesExecArgs {
    crate::agent_proto::ListMcpResourcesExecArgs {
        server: none_if_empty(json_str(args, &["server", "namespace"])),
    }
}

fn read_mcp_resource_args(args: &Value, call_id: &str) -> crate::agent_proto::ReadMcpResourceExecArgs {
    crate::agent_proto::ReadMcpResourceExecArgs {
        server: json_str(args, &["server"]),
        uri: json_str(args, &["uri"]),
        download_path: none_if_empty(json_str(args, &["downloadPath", "download_path"])),
        tool_call_id: call_id.to_owned(),
    }
}

fn json_str_list(args: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(items) = args.get(*key).and_then(Value::as_array) {
            return items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect();
        }
    }
    Vec::new()
}

fn read_lints_args(args: &Value) -> crate::agent_proto::ReadLintsToolArgs {
    let paths = args
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    crate::agent_proto::ReadLintsToolArgs { paths }
}

fn lint_path(workspace: Option<&str>, args: &Value) -> String {
    let first = args
        .get("paths")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .unwrap_or("");
    if first.is_empty() {
        workspace.unwrap_or(".").to_owned()
    } else {
        abs_path(workspace, first)
    }
}

fn apply_patch_path(workspace: Option<&str>, args: &Value) -> String {
    let patch = json_str(args, &["patch"]);
    let path = parse_patch_path(&patch).unwrap_or_default();
    abs_path(workspace, &path)
}

fn parse_patch_path(patch: &str) -> Option<String> {
    for line in patch.lines() {
        let line = line.trim();
        for prefix in ["*** Add File: ", "*** Update File: ", "*** Delete File: "] {
            if let Some(path) = line.strip_prefix(prefix) {
                return Some(path.trim().to_owned());
            }
        }
    }
    None
}

fn apply_patch_to_content(patch: &str, before: &str) -> Result<String, String> {
    let patch = patch.replace("\r\n", "\n").replace('\r', "\n");
    if patch.contains("*** Delete File:") {
        return Err("ApplyPatch Delete File is not supported; use the Delete tool".into());
    }
    if let Some(path_line) = patch.lines().find(|l| l.trim().starts_with("*** Add File:")) {
        let mut body = String::new();
        let mut take = false;
        for line in patch.lines() {
            if line.trim() == path_line.trim() {
                take = true;
                continue;
            }
            if line.trim().starts_with("*** End Patch") {
                break;
            }
            if take {
                if let Some(rest) = line.strip_prefix('+') {
                    body.push_str(rest);
                    body.push('\n');
                }
            }
        }
        return Ok(normalize_cursor_nl(&body));
    }
    let mut content = normalize_cursor_nl(before);
    let mut old = String::new();
    let mut new = String::new();
    let mut in_hunk = false;
    for line in patch.lines() {
        if line.starts_with("@@") {
            if in_hunk && !old.is_empty() {
                content = apply_edit(&content, old.trim_end_matches('\n'), new.trim_end_matches('\n'), false)?;
            }
            old.clear();
            new.clear();
            in_hunk = true;
            continue;
        }
        if !in_hunk || line.starts_with("***") {
            continue;
        }
        if let Some(rest) = line.strip_prefix('-') {
            old.push_str(rest);
            old.push('\n');
        } else if let Some(rest) = line.strip_prefix('+') {
            new.push_str(rest);
            new.push('\n');
        } else {
            let rest = line.strip_prefix(' ').unwrap_or(line);
            old.push_str(rest);
            old.push('\n');
            new.push_str(rest);
            new.push('\n');
        }
    }
    if in_hunk && !old.is_empty() {
        content = apply_edit(&content, old.trim_end_matches('\n'), new.trim_end_matches('\n'), false)?;
    }
    Ok(content)
}

fn web_search_args(args: &Value, call_id: &str) -> crate::agent_proto::WebSearchArgs {
    crate::agent_proto::WebSearchArgs {
        search_term: json_str(args, &["search_term", "searchTerm"]),
        tool_call_id: call_id.to_owned(),
    }
}

fn create_plan_args(args: &Value) -> crate::agent_proto::CreatePlanArgs {
    let todos = args
        .get("todos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(crate::agent_proto::PlanTodoItem {
                id: item.get("id")?.as_str()?.to_owned(),
                content: item.get("content")?.as_str()?.to_owned(),
            })
        })
        .collect();
    crate::agent_proto::CreatePlanArgs {
        plan: json_str(args, &["plan"]),
        todos,
        overview: json_str(args, &["overview"]),
        name: json_str(args, &["name"]),
    }
}

fn json_to_proto_value(value: &Value) -> prost_types::Value {
    use prost_types::value::Kind;
    prost_types::Value {
        kind: Some(match value {
            Value::Null => Kind::NullValue(0),
            Value::Bool(flag) => Kind::BoolValue(*flag),
            Value::Number(num) => Kind::NumberValue(num.as_f64().unwrap_or(0.0)),
            Value::String(text) => Kind::StringValue(text.clone()),
            Value::Array(items) => Kind::ListValue(prost_types::ListValue {
                values: items.iter().map(json_to_proto_value).collect(),
            }),
            Value::Object(map) => Kind::StructValue(prost_types::Struct {
                fields: map
                    .iter()
                    .map(|(key, val)| (key.clone(), json_to_proto_value(val)))
                    .collect(),
            }),
        }),
    }
}

fn mcp_args(args: &Value, call_id: &str) -> crate::agent_proto::McpArgs {
    let nested = args
        .get("arguments")
        .or_else(|| args.get("args"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let map = match nested {
        Value::Object(fields) => fields
            .iter()
            .map(|(key, val)| (key.clone(), json_to_proto_value(val)))
            .collect(),
        other => {
            let mut one = std::collections::HashMap::new();
            one.insert("value".into(), json_to_proto_value(&other));
            one
        }
    };
    crate::agent_proto::McpArgs {
        name: json_str(args, &["name", "toolName", "tool_name"]),
        args: map,
        tool_call_id: call_id.to_owned(),
        provider_identifier: json_str(args, &["providerIdentifier", "provider", "namespace"]),
        tool_name: json_str(args, &["toolName", "tool_name", "name"]),
        server_identifier: json_str(args, &["serverIdentifier", "server", "namespace"]),
    }
}

fn empty_tool_call(kind: CursorTool) -> ToolCall {
    match kind {
        CursorTool::Read => ToolCall {
            tool: Some(tool_call::Tool::ReadToolCall(ReadToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Grep => ToolCall {
            tool: Some(tool_call::Tool::GrepToolCall(GrepToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Glob => ToolCall {
            tool: Some(tool_call::Tool::GlobToolCall(GlobToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Ls => ToolCall {
            tool: Some(tool_call::Tool::LsToolCall(LsToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Shell => ToolCall {
            tool: Some(tool_call::Tool::ShellToolCall(ShellToolCall {
                args: None,
                result: None,
                description: None,
            })),
        },
        CursorTool::Edit | CursorTool::Write => ToolCall {
            tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::WebSearch => ToolCall {
            tool: Some(tool_call::Tool::WebSearchToolCall(crate::agent_proto::WebSearchToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Task => ToolCall {
            tool: Some(tool_call::Tool::TaskToolCall(crate::agent_proto::TaskToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::CreatePlan => ToolCall {
            tool: Some(tool_call::Tool::CreatePlanToolCall(
                crate::agent_proto::CreatePlanToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::Mcp => ToolCall {
            tool: Some(tool_call::Tool::McpToolCall(crate::agent_proto::McpToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::GetMcpTools => ToolCall {
            tool: Some(tool_call::Tool::GetMcpToolsToolCall(
                crate::agent_proto::GetMcpToolsToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::WebFetch => ToolCall {
            tool: Some(tool_call::Tool::WebFetchToolCall(
                crate::agent_proto::WebFetchToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::TodoWrite => ToolCall {
            tool: Some(tool_call::Tool::UpdateTodosToolCall(
                crate::agent_proto::UpdateTodosToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::Delete => ToolCall {
            tool: Some(tool_call::Tool::DeleteToolCall(
                crate::agent_proto::DeleteToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::AskQuestion => ToolCall {
            tool: Some(tool_call::Tool::AskQuestionToolCall(
                crate::agent_proto::AskQuestionToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::SwitchMode => ToolCall {
            tool: Some(tool_call::Tool::SwitchModeToolCall(
                crate::agent_proto::SwitchModeToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::ApplyPatch => ToolCall {
            tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::Await => ToolCall {
            tool: Some(tool_call::Tool::AwaitToolCall(
                crate::agent_proto::AwaitToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::UpdateCurrentStep => ToolCall {
            tool: Some(tool_call::Tool::CommunicateUpdateToolCall(
                crate::agent_proto::CommunicateUpdateToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::ReadLints => ToolCall {
            tool: Some(tool_call::Tool::ReadLintsToolCall(
                crate::agent_proto::ReadLintsToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::EditNotebook => ToolCall {
            tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                args: None,
                result: None,
            })),
        },
        CursorTool::GenerateImage => ToolCall {
            tool: Some(tool_call::Tool::GenerateImageToolCall(
                crate::agent_proto::GenerateImageToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::ListMcpResources => ToolCall {
            tool: Some(tool_call::Tool::ListMcpResourcesToolCall(
                crate::agent_proto::ListMcpResourcesToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
        CursorTool::FetchMcpResource => ToolCall {
            tool: Some(tool_call::Tool::ReadMcpResourceToolCall(
                crate::agent_proto::ReadMcpResourceToolCall {
                    args: None,
                    result: None,
                },
            )),
        },
    }
}

pub fn encode_tool_preview_frames(
    tool: &ToolUse,
    model_call_id: &str,
) -> Option<Vec<AgentServerMessage>> {
    let kind = map_tool_name(&tool.name)?;
    let call_id = if tool.id.is_empty() {
        "call".to_owned()
    } else {
        tool.id.clone()
    };
    let mut frames = vec![agent_wire::server_partial_tool(
        &call_id,
        empty_tool_call(kind),
        model_call_id,
    )];
    let delta = match kind {
        CursorTool::Write => json_str(&tool.arguments, &["contents", "content", "file_text"]),
        CursorTool::Edit | CursorTool::EditNotebook => {
            json_str(&tool.arguments, &["new_string", "newString"])
        }
        _ => String::new(),
    };
    if !delta.is_empty() {
        frames.push(agent_wire::server_edit_stream_delta(
            &call_id,
            &delta,
            model_call_id,
        ));
    }
    Some(frames)
}

fn grep_args(workspace: Option<&str>, args: &Value, call_id: &str, glob_mode: bool) -> GrepArgs {
    if glob_mode {
        let dir = json_str(args, &["target_directory", "targetDirectory", "path"]);
        let pattern = json_str(args, &["glob_pattern", "globPattern", "pattern"]);
        return GrepArgs {
            pattern: String::new(),
            path: (!dir.is_empty()).then(|| abs_path(workspace, &dir)),
            glob: Some(pattern),
            output_mode: Some("files_with_matches".into()),
            context_before: None,
            context_after: None,
            context: None,
            case_insensitive: None,
            r#type: None,
            head_limit: json_i32(args, &["head_limit", "headLimit"]),
            multiline: None,
            tool_call_id: call_id.to_owned(),
            offset: None,
        };
    }
    let path = json_str(args, &["path"]);
    GrepArgs {
        pattern: json_str(args, &["pattern"]),
        path: (!path.is_empty()).then(|| abs_path(workspace, &path)),
        glob: {
            let glob = json_str(args, &["glob"]);
            (!glob.is_empty()).then_some(glob)
        },
        output_mode: {
            let mode = json_str(args, &["output_mode", "outputMode"]);
            (!mode.is_empty()).then_some(mode)
        },
        context_before: json_i32(args, &["-B", "context_before"]),
        context_after: json_i32(args, &["-A", "context_after"]),
        context: json_i32(args, &["-C", "context"]),
        case_insensitive: json_bool(args, &["-i", "case_insensitive"]).then_some(true),
        r#type: {
            let ty = json_str(args, &["type"]);
            (!ty.is_empty()).then_some(ty)
        },
        head_limit: json_i32(args, &["head_limit", "headLimit"]),
        multiline: json_bool(args, &["multiline"]).then_some(true),
        tool_call_id: call_id.to_owned(),
        offset: json_i32(args, &["offset"]),
    }
}

pub fn exec_result_text(exec: &ExecClientMessage) -> String {
    match &exec.message {
        Some(exec_client_message::Message::ReadResult(result)) => read_result_text(result),
        Some(exec_client_message::Message::GrepResult(result)) => grep_result_text(result),
        Some(exec_client_message::Message::LsResult(result)) => ls_result_text(result),
        Some(exec_client_message::Message::WriteResult(result)) => write_result_text(result),
        Some(exec_client_message::Message::ShellResult(result)) => shell_result_text(result),
        Some(exec_client_message::Message::ShellStream(stream)) => match &stream.event {
            Some(shell_stream::Event::Stdout(out)) => out.data.clone(),
            Some(shell_stream::Event::Stderr(err)) => err.data.clone(),
            Some(shell_stream::Event::Exit(exit)) => format!("exit {}", exit.code),
            Some(shell_stream::Event::Rejected(rej)) => format!("rejected: {}", rej.reason),
            Some(shell_stream::Event::PermissionDenied(denied)) => {
                format!("permission denied: {}", denied.error)
            }
            Some(shell_stream::Event::Backgrounded(bg)) => {
                format!("backgrounded shell_id={}", bg.shell_id)
            }
            _ => String::new(),
        },
        Some(exec_client_message::Message::McpResult(result)) => mcp_result_text(result),
        Some(exec_client_message::Message::SubagentResult(result)) => subagent_result_text(result),
        Some(exec_client_message::Message::DiagnosticsResult(result)) => match &result.result {
            Some(crate::agent_proto::diagnostics_result::Result::Success(ok)) => {
                format!("{} diagnostics in {}", ok.total_diagnostics, ok.path)
            }
            Some(crate::agent_proto::diagnostics_result::Result::Error(err)) => {
                format!("lints error {}: {}", err.path, err.error)
            }
            None => "(empty diagnostics)".into(),
        },
        Some(exec_client_message::Message::DeleteResult(result)) => match &result.result {
            Some(crate::agent_proto::delete_result::Result::Success(ok)) => {
                format!("deleted {}", ok.path)
            }
            Some(crate::agent_proto::delete_result::Result::FileNotFound(miss)) => {
                format!("file not found: {}", miss.path)
            }
            Some(crate::agent_proto::delete_result::Result::Error(err)) => {
                format!("delete error {}: {}", err.path, err.error)
            }
            None => "(empty delete result)".into(),
        },
        Some(exec_client_message::Message::ListMcpResourcesExecResult(result)) => {
            list_mcp_resources_text(result)
        }
        Some(exec_client_message::Message::ReadMcpResourceExecResult(result)) => {
            read_mcp_resource_text(result)
        }
        Some(exec_client_message::Message::McpStateExecResult(result)) => {
            mcp_state_catalog_text(result, false)
        }
        None => "(empty exec result)".into(),
    }
}

fn mcp_state_catalog_text_from_exec(exec: &ExecClientMessage, supports_mcp_auth: bool) -> String {
    match &exec.message {
        Some(exec_client_message::Message::McpStateExecResult(result)) => {
            mcp_state_catalog_text(result, supports_mcp_auth)
        }
        _ => crate::mcp::catalog_json_auth(None, None, None, supports_mcp_auth),
    }
}

fn mcp_state_catalog_text(
    result: &crate::agent_proto::McpStateExecResult,
    supports_mcp_auth: bool,
) -> String {
    match &result.result {
        Some(crate::agent_proto::mcp_state_exec_result::Result::Success(ok)) => {
            if ok.servers.is_empty() {
                return crate::mcp::catalog_json_auth(None, None, None, supports_mcp_auth);
            }
            let mut lines = Vec::new();
            for server in &ok.servers {
                lines.push(format!(
                    "# {} ({}) {}",
                    server.server_name,
                    server.server_identifier,
                    server.status.clone().unwrap_or_default()
                ));
                for tool in &server.tools {
                    let name = if tool.tool_name.is_empty() {
                        tool.name.clone()
                    } else {
                        tool.tool_name.clone()
                    };
                    lines.push(format!("- {name}: {}", tool.description));
                    if let Some(schema) = tool.input_schema_json.as_ref().filter(|s| !s.is_empty()) {
                        lines.push(schema.clone());
                    }
                }
                if supports_mcp_auth
                    && !server.tools.iter().any(|tool| {
                        let name = if tool.tool_name.is_empty() {
                            tool.name.as_str()
                        } else {
                            tool.tool_name.as_str()
                        };
                        name == crate::mcp::MCP_AUTH_TOOL
                    })
                {
                    lines.push(format!(
                        "- {}: {}",
                        crate::mcp::MCP_AUTH_TOOL,
                        crate::mcp::MCP_AUTH_DESCRIPTION
                    ));
                }
            }
            lines.join("\n")
        }
        Some(crate::agent_proto::mcp_state_exec_result::Result::Error(err)) => {
            format!(
                "MCP state error: {}\n{}",
                err.error,
                crate::mcp::catalog_json_auth(None, None, None, supports_mcp_auth)
            )
        }
        Some(crate::agent_proto::mcp_state_exec_result::Result::Rejected(rej)) => {
            format!("MCP state rejected: {}", rej.reason)
        }
        None => crate::mcp::catalog_json_auth(None, None, None, supports_mcp_auth),
    }
}

fn mcp_result_text(result: &crate::agent_proto::McpResult) -> String {
    match &result.result {
        Some(crate::agent_proto::mcp_result::Result::Success(ok)) => {
            let mut lines = Vec::new();
            for item in &ok.content {
                match &item.content {
                    Some(crate::agent_proto::mcp_tool_result_content_item::Content::Text(text)) => {
                        if text.text.is_empty() {
                            if let Some(loc) = &text.output_location {
                                lines.push(format_output_location(loc));
                            }
                        } else {
                            lines.push(text.text.clone());
                        }
                    }
                    Some(crate::agent_proto::mcp_tool_result_content_item::Content::Image(image)) => {
                        let mime = if image.mime_type.is_empty() {
                            "unknown"
                        } else {
                            image.mime_type.as_str()
                        };
                        lines.push(format!("[image {mime}]"));
                    }
                    None => {}
                }
            }
            if lines.is_empty() {
                "MCP tool completed successfully.".into()
            } else {
                lines.join("\n\n")
            }
        }
        Some(crate::agent_proto::mcp_result::Result::Error(err)) => format!(
            "MCP error: {}\n<system_reminder>\nThe MCP server rejected these arguments as invalid. Before retrying, inspect this MCP tool's input schema/tool definition and rebuild the arguments from that schema.\n</system_reminder>",
            err.error
        ),
        Some(crate::agent_proto::mcp_result::Result::Rejected(rej)) => {
            format!("MCP rejected: {}", rej.reason)
        }
        Some(crate::agent_proto::mcp_result::Result::PermissionDenied(denied)) => {
            format!("MCP permission denied: {}", denied.error)
        }
        Some(crate::agent_proto::mcp_result::Result::ToolNotFound(miss)) => {
            format!(
                "MCP tool not found: {}. available: {}",
                miss.name,
                miss.available_tools.join(", ")
            )
        }
        Some(crate::agent_proto::mcp_result::Result::ServerNotFound(miss)) => {
            format!(
                "MCP server not found: {}. available: {}",
                miss.name,
                miss.available_servers.join(", ")
            )
        }
        Some(crate::agent_proto::mcp_result::Result::Approved(_)) => {
            "MCP tool approved; continue.".into()
        }
        None => "(empty mcp result)".into(),
    }
}

fn list_mcp_resources_text(result: &crate::agent_proto::ListMcpResourcesExecResult) -> String {
    match &result.result {
        Some(crate::agent_proto::list_mcp_resources_exec_result::Result::Success(ok)) => {
            if ok.resources.is_empty() {
                "No MCP resources available.".into()
            } else {
                ok.resources
                    .iter()
                    .map(|resource| {
                        let name = resource.name.clone().unwrap_or_default();
                        if name.is_empty() {
                            format!("{} {}", resource.server, resource.uri)
                        } else {
                            format!("{} {} — {name}", resource.server, resource.uri)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Some(crate::agent_proto::list_mcp_resources_exec_result::Result::Error(err)) => {
            format!("List MCP resources error: {}", err.error)
        }
        Some(crate::agent_proto::list_mcp_resources_exec_result::Result::Rejected(rej)) => {
            format!("List MCP resources rejected: {}", rej.reason)
        }
        None => "(empty list MCP resources result)".into(),
    }
}

fn read_mcp_resource_text(result: &crate::agent_proto::ReadMcpResourceExecResult) -> String {
    match &result.result {
        Some(crate::agent_proto::read_mcp_resource_exec_result::Result::Success(ok)) => {
            match &ok.content {
                Some(crate::agent_proto::read_mcp_resource_success::Content::Text(text)) => {
                    text.clone()
                }
                Some(crate::agent_proto::read_mcp_resource_success::Content::Blob(_)) => {
                    format!("Read MCP resource {} (binary blob).", ok.uri)
                }
                None => {
                    if let Some(path) = &ok.download_path {
                        format!("MCP resource {} saved to {path}", ok.uri)
                    } else {
                        format!("Read MCP resource {}", ok.uri)
                    }
                }
            }
        }
        Some(crate::agent_proto::read_mcp_resource_exec_result::Result::Error(err)) => {
            format!("Read MCP resource {}: {}", err.uri, err.error)
        }
        Some(crate::agent_proto::read_mcp_resource_exec_result::Result::Rejected(rej)) => {
            format!("Read MCP resource {} rejected: {}", rej.uri, rej.reason)
        }
        Some(crate::agent_proto::read_mcp_resource_exec_result::Result::NotFound(miss)) => {
            format!("MCP resource not found: {}", miss.uri)
        }
        None => "(empty read MCP resource result)".into(),
    }
}

fn format_output_location(loc: &crate::agent_proto::OutputLocation) -> String {
    let size = if loc.size_bytes >= 1024 {
        format!("{:.1} KB", loc.size_bytes as f64 / 1024.0)
    } else {
        format!("{} bytes", loc.size_bytes)
    };
    format!(
        "Content written to file: {}\nSize: {}, {} lines",
        loc.file_path, size, loc.line_count
    )
}

fn subagent_result_text(result: &crate::agent_proto::SubagentResult) -> String {
    match &result.result {
        Some(crate::agent_proto::subagent_result::Result::Success(ok)) => ok
            .final_message
            .clone()
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| format!("subagent {} completed", ok.agent_id)),
        Some(crate::agent_proto::subagent_result::Result::Error(err)) => {
            format!("task error: {}", err.error)
        }
        None => "(empty task result)".into(),
    }
}

fn read_result_text(result: &crate::agent_proto::ReadResult) -> String {
    match &result.result {
        Some(read_result::Result::Success(success)) => match &success.output {
            Some(read_success::Output::Content(text)) => text.clone(),
            Some(read_success::Output::Data(bytes)) => binary_file_marker(&success.path, bytes),
            None => String::new(),
        },
        Some(read_result::Result::Error(err)) => format!("read error {}: {}", err.path, err.error),
        Some(read_result::Result::FileNotFound(miss)) => format!("file not found: {}", miss.path),
        Some(read_result::Result::Rejected(rej)) => format!("read rejected: {}", rej.reason),
        Some(read_result::Result::PermissionDenied(denied)) => {
            format!("permission denied: {}", denied.path)
        }
        Some(read_result::Result::InvalidFile(invalid)) => {
            format!("invalid file {}: {}", invalid.path, invalid.reason)
        }
        None => "(empty read result)".into(),
    }
}

fn grep_result_text(result: &GrepResult) -> String {
    match &result.result {
        Some(grep_result::Result::Error(err)) => format!("grep error: {}", err.error),
        Some(grep_result::Result::Success(success)) => {
            let mut lines = Vec::new();
            for union in success.workspace_results.values() {
                match &union.result {
                    Some(grep_union_result::Result::Files(files)) => {
                        lines.extend(files.files.iter().cloned());
                    }
                    Some(grep_union_result::Result::Content(content)) => {
                        for file in &content.matches {
                            for hit in &file.matches {
                                lines.push(format!(
                                    "{}:{}:{}",
                                    file.file, hit.line_number, hit.content
                                ));
                            }
                        }
                    }
                    Some(grep_union_result::Result::Count(count)) => {
                        for item in &count.counts {
                            lines.push(format!("{}:{}", item.file, item.count));
                        }
                    }
                    None => {}
                }
            }
            if lines.is_empty() {
                format!("no matches for {}", success.pattern)
            } else {
                lines.join("\n")
            }
        }
        None => "(empty grep result)".into(),
    }
}

fn ls_result_text(result: &LsResult) -> String {
    match &result.result {
        Some(ls_result::Result::Success(success)) => {
            let mut lines = Vec::new();
            if let Some(root) = &success.directory_tree_root {
                walk_ls(root, 0, &mut lines);
            }
            if lines.is_empty() {
                "(empty directory)".into()
            } else {
                lines.join("\n")
            }
        }
        Some(ls_result::Result::Error(err)) => format!("ls error {}: {}", err.path, err.error),
        Some(ls_result::Result::Rejected(rej)) => format!("ls rejected: {}", rej.reason),
        Some(ls_result::Result::Timeout(_)) => "ls timed out".into(),
        None => "(empty ls result)".into(),
    }
}

fn walk_ls(node: &crate::agent_proto::LsDirectoryTreeNode, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    lines.push(format!("{indent}{}/", node.abs_path));
    for file in &node.children_files {
        lines.push(format!("{indent}  {}", file.name));
    }
    for child in &node.children_dirs {
        walk_ls(child, depth + 1, lines);
    }
}

fn write_result_text(result: &crate::agent_proto::WriteResult) -> String {
    match &result.result {
        Some(write_result::Result::Success(ok)) => ok
            .file_content_after_write
            .clone()
            .filter(|body| !body.is_empty())
            .unwrap_or_else(|| format!("wrote {}", ok.path)),
        Some(write_result::Result::Error(err)) => {
            format!("write error {}: {}", err.path, err.error)
        }
        Some(write_result::Result::PermissionDenied(denied)) => {
            format!("write permission denied: {}", denied.error)
        }
        Some(write_result::Result::Rejected(rej)) => format!("write rejected: {}", rej.reason),
        Some(write_result::Result::NoSpace(space)) => format!("no space: {}", space.path),
        None => "(empty write result)".into(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ShellTranscript {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: String,
    pub rejected: Option<String>,
    pub permission_denied: Option<String>,
    pub backgrounded: Option<String>,
    pub timeout_ms: Option<i32>,
    pub spawn_error: Option<String>,
    pub finished: bool,
}

fn apply_shell_exec(out: &mut ShellTranscript, msg: &ExecClientMessage) {
    match &msg.message {
        Some(exec_client_message::Message::ShellResult(result)) => match &result.result {
            Some(shell_result::Result::Success(ok)) => {
                push_capped(&mut out.stdout, &ok.stdout);
                push_capped(&mut out.stderr, &ok.stderr);
                out.exit_code = ok.exit_code;
                if !ok.working_directory.is_empty() {
                    out.cwd = ok.working_directory.clone();
                }
                out.finished = true;
            }
            Some(shell_result::Result::Failure(fail)) => {
                push_capped(&mut out.stdout, &fail.stdout);
                push_capped(&mut out.stderr, &fail.stderr);
                out.exit_code = fail.exit_code;
                if !fail.working_directory.is_empty() {
                    out.cwd = fail.working_directory.clone();
                }
                out.finished = true;
            }
            Some(shell_result::Result::Rejected(rej)) => {
                out.rejected = Some(rej.reason.clone());
                out.finished = true;
            }
            Some(shell_result::Result::PermissionDenied(denied)) => {
                out.permission_denied = Some(denied.error.clone());
                out.finished = true;
            }
            Some(shell_result::Result::Timeout(timeout)) => {
                out.timeout_ms = Some(timeout.timeout_ms);
                if !timeout.working_directory.is_empty() {
                    out.cwd = timeout.working_directory.clone();
                }
                out.finished = true;
            }
            Some(shell_result::Result::SpawnError(err)) => {
                out.spawn_error = Some(err.error.clone());
                if !err.working_directory.is_empty() {
                    out.cwd = err.working_directory.clone();
                }
                out.finished = true;
            }
            None => {}
        },
        Some(exec_client_message::Message::ShellStream(stream)) => match &stream.event {
            Some(shell_stream::Event::Stdout(chunk)) => push_capped(&mut out.stdout, &chunk.data),
            Some(shell_stream::Event::Stderr(chunk)) => push_capped(&mut out.stderr, &chunk.data),
            Some(shell_stream::Event::Exit(exit)) => {
                out.exit_code = exit.code as i32;
                if !exit.cwd.is_empty() {
                    out.cwd = exit.cwd.clone();
                }
                out.finished = true;
            }
            Some(shell_stream::Event::Rejected(rej)) => {
                out.rejected = Some(rej.reason.clone());
                out.finished = true;
            }
            Some(shell_stream::Event::PermissionDenied(denied)) => {
                out.permission_denied = Some(denied.error.clone());
                out.finished = true;
            }
            Some(shell_stream::Event::Backgrounded(bg)) => {
                out.backgrounded = Some(format!(
                    "Command moved to background shell_id={} {}",
                    bg.shell_id, bg.command
                ));
                out.finished = true;
            }
            _ => {}
        },
        _ => {}
    }
}

pub fn complete_shell_tool(
    workspace: Option<&str>,
    tool: &ToolUse,
    transcript: &ShellTranscript,
) -> (ToolCall, String) {
    let command = json_str(&tool.arguments, &["command"]);
    let cwd = if transcript.cwd.is_empty() {
        abs_path(
            workspace,
            &json_str(
                &tool.arguments,
                &["working_directory", "workingDirectory", "cwd"],
            ),
        )
    } else {
        transcript.cwd.clone()
    };
    let args = Some(shell_args(
        &command,
        &cwd,
        &tool.id,
        &json_str(&tool.arguments, &["description"]),
    ));
    let (result, text) = if let Some(reason) = &transcript.rejected {
        (
            shell_result::Result::Rejected(crate::agent_proto::ShellRejected {
                command: command.clone(),
                reason: reason.clone(),
            }),
            format!("rejected: {reason}"),
        )
    } else if let Some(error) = &transcript.permission_denied {
        (
            shell_result::Result::PermissionDenied(crate::agent_proto::ShellPermissionDenied {
                command: command.clone(),
                error: error.clone(),
            }),
            format!("permission denied: {error}"),
        )
    } else if let Some(timeout_ms) = transcript.timeout_ms {
        (
            shell_result::Result::Timeout(crate::agent_proto::ShellTimeout {
                command: command.clone(),
                working_directory: cwd.clone(),
                timeout_ms,
            }),
            format!("timeout after {timeout_ms}ms"),
        )
    } else if let Some(error) = &transcript.spawn_error {
        (
            shell_result::Result::SpawnError(crate::agent_proto::ShellSpawnError {
                command: command.clone(),
                working_directory: cwd.clone(),
                error: error.clone(),
            }),
            format!("spawn error: {error}"),
        )
    } else if let Some(bg) = &transcript.backgrounded {
        (
            shell_result::Result::Success(shell_success_with_clip(
                command.clone(),
                cwd.clone(),
                0,
                bg,
                "",
            )),
            bg.clone(),
        )
    } else if transcript.exit_code == 0 {
        let (text, success) = shell_success_pair(
            command.clone(),
            cwd.clone(),
            0,
            &transcript.stdout,
            &transcript.stderr,
        );
        (shell_result::Result::Success(success), text)
    } else {
        (
            shell_result::Result::Failure(ShellFailure {
                command: command.clone(),
                working_directory: cwd.clone(),
                exit_code: transcript.exit_code,
                stdout: clip_shell_body(&transcript.stdout),
                stderr: clip_shell_body(&transcript.stderr),
            }),
            format!(
                "exit {}\n{}\n{}",
                transcript.exit_code, transcript.stdout, transcript.stderr
            ),
        )
    };
    (
        ToolCall {
            tool: Some(tool_call::Tool::ShellToolCall(ShellToolCall {
                args,
                result: Some(ShellResult {
                    result: Some(result),
                }),
                description: {
                    let d = json_str(&tool.arguments, &["description"]);
                    (!d.is_empty()).then_some(d)
                },
            })),
        },
        text,
    )
}

fn push_capped(dst: &mut String, more: &str) {
    if dst.len() >= SHELL_COLLECT_MAX {
        return;
    }
    let room = SHELL_COLLECT_MAX - dst.len();
    if more.len() <= room {
        dst.push_str(more);
        return;
    }
    let mut end = room;
    while end > 0 && !more.is_char_boundary(end) {
        end -= 1;
    }
    dst.push_str(&more[..end]);
}

fn clip_shell_body(body: &str) -> String {
    crate::agent_wire::clip_utf8(body, SHELL_COLLECT_MAX)
}

fn shell_success_with_clip(
    command: String,
    cwd: String,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> ShellSuccess {
    shell_success_pair(command, cwd, exit_code, stdout, stderr).1
}

fn shell_success_pair(
    command: String,
    cwd: String,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> (String, ShellSuccess) {
    let combined = format!("{stdout}\n{stderr}");
    let chars: Vec<char> = combined.chars().collect();
    if chars.len() <= SHELL_OUTPUT_FULL_MAX {
        let text = combined.trim().to_owned();
        return (
            text.clone(),
            ShellSuccess {
                command,
                working_directory: cwd,
                exit_code,
                stdout: stdout.to_owned(),
                stderr: stderr.to_owned(),
                output_head: None,
                output_tail: None,
                elided_chars: None,
            },
        );
    }
    let head: String = chars.iter().take(SHELL_OUTPUT_SIDE).collect();
    let tail: String = chars.iter().skip(chars.len() - SHELL_OUTPUT_SIDE).collect();
    let elided = (chars.len() - SHELL_OUTPUT_FULL_MAX) as u32;
    let text = format!("{head}\n...\n{tail}");
    (
        text,
        ShellSuccess {
            command,
            working_directory: cwd,
            exit_code,
            stdout: head.clone(),
            stderr: String::new(),
            output_head: Some(head),
            output_tail: Some(tail),
            elided_chars: Some(elided),
        },
    )
}

fn shell_result_text(result: &ShellResult) -> String {
    match &result.result {
        Some(shell_result::Result::Success(ok)) => {
            format!("{}\n{}", ok.stdout, ok.stderr).trim().to_owned()
        }
        Some(shell_result::Result::Failure(fail)) => {
            format!("exit {}\n{}\n{}", fail.exit_code, fail.stdout, fail.stderr)
        }
        Some(shell_result::Result::Rejected(rej)) => format!("rejected: {}", rej.reason),
        Some(shell_result::Result::PermissionDenied(denied)) => {
            format!("permission denied: {}", denied.error)
        }
        Some(shell_result::Result::Timeout(timeout)) => {
            format!("timeout after {}ms", timeout.timeout_ms)
        }
        Some(shell_result::Result::SpawnError(err)) => format!("spawn error: {}", err.error),
        None => String::new(),
    }
}

fn path_or(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

fn read_for_edit(exec: &ExecClientMessage, allow_missing: bool) -> Result<String, String> {
    match &exec.message {
        Some(exec_client_message::Message::ReadResult(result)) => match &result.result {
            Some(read_result::Result::Success(ok)) if ok.truncated => Err(format!(
                "read truncated ({} lines); refusing to apply Edit",
                ok.total_lines
            )),
            Some(read_result::Result::Success(_)) => {
                Ok(normalize_cursor_nl(&exec_result_text(exec)))
            }
            Some(read_result::Result::FileNotFound(_)) if allow_missing => Ok(String::new()),
            _ => Err(exec_result_text(exec)),
        },
        _ => Err(exec_result_text(exec)),
    }
}

fn write_exec_success(exec: &ExecClientMessage) -> bool {
    matches!(
        &exec.message,
        Some(exec_client_message::Message::WriteResult(result))
            if matches!(result.result, Some(write_result::Result::Success(_)))
    )
}

fn complete_edit_error(workspace: Option<&str>, tool: &ToolUse, error: &str) -> (ToolCall, String) {
    let path = abs_path(
        workspace,
        &json_str(&tool.arguments, &["path", "target_notebook"]),
    );
    (
        ToolCall {
            tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                args: Some(EditArgs {
                    path: path.clone(),
                    stream_content: None,
                }),
                result: Some(EditResult {
                    result: Some(edit_result::Result::Error(EditError {
                        path,
                        error: error.to_owned(),
                    })),
                }),
            })),
        },
        error.to_owned(),
    )
}

fn read_tool_from_exec(
    exec: &ExecClientMessage,
    path: &str,
    content: &str,
) -> read_tool_result::Result {
    match &exec.message {
        Some(exec_client_message::Message::ReadResult(result)) => match &result.result {
            Some(read_result::Result::Success(ok)) => {
                let body = match &ok.output {
                    Some(read_success::Output::Content(text)) => text.clone(),
                    Some(read_success::Output::Data(bytes)) => binary_file_marker(&ok.path, bytes),
                    None => content.to_owned(),
                };
                read_tool_result::Result::Success(ReadToolSuccess {
                    is_empty: body.is_empty(),
                    exceeded_limit: ok.truncated,
                    total_lines: if ok.total_lines > 0 {
                        ok.total_lines as u32
                    } else {
                        body.lines().count() as u32
                    },
                    file_size: if ok.file_size > 0 {
                        ok.file_size.min(i64::from(u32::MAX)) as u32
                    } else {
                        body.len() as u32
                    },
                    path: path_or(&ok.path, path),
                    output: Some(read_tool_success::Output::Content(body)),
                })
            }
            Some(read_result::Result::Error(err)) => {
                read_tool_result::Result::Error(ReadToolError {
                    error_message: format!("read error {}: {}", err.path, err.error),
                })
            }
            Some(read_result::Result::FileNotFound(miss)) => {
                read_tool_result::Result::Error(ReadToolError {
                    error_message: format!("file not found: {}", miss.path),
                })
            }
            Some(read_result::Result::Rejected(rej)) => {
                read_tool_result::Result::Error(ReadToolError {
                    error_message: format!("read rejected: {}", rej.reason),
                })
            }
            Some(read_result::Result::PermissionDenied(denied)) => {
                read_tool_result::Result::Error(ReadToolError {
                    error_message: format!("permission denied: {}", denied.path),
                })
            }
            Some(read_result::Result::InvalidFile(invalid)) => {
                read_tool_result::Result::Error(ReadToolError {
                    error_message: format!("invalid file {}: {}", invalid.path, invalid.reason),
                })
            }
            None => read_tool_result::Result::Error(ReadToolError {
                error_message: "no result".into(),
            }),
        },
        _ => read_tool_result::Result::Error(ReadToolError {
            error_message: if content.is_empty() {
                "no result".into()
            } else {
                content.to_owned()
            },
        }),
    }
}

fn edit_line_diff(before: Option<&str>, after: &str) -> (i32, i32, String) {
    let before = before.unwrap_or("");
    let before_n = before.lines().count() as i32;
    let after_n = after.lines().count() as i32;
    (
        (after_n - before_n).max(0),
        (before_n - after_n).max(0),
        format!("@@ {before_n} lines -> {after_n} lines"),
    )
}

fn edit_result_from_exec(
    exec: &ExecClientMessage,
    path: &str,
    after: &str,
    before: Option<&str>,
) -> edit_result::Result {
    match &exec.message {
        Some(exec_client_message::Message::WriteResult(result)) => match &result.result {
            Some(write_result::Result::Success(ok)) => {
                let body = if !after.is_empty() {
                    after.to_owned()
                } else {
                    ok.file_content_after_write
                        .clone()
                        .filter(|body| !body.is_empty())
                        .unwrap_or_default()
                };
                let path = path_or(&ok.path, path);
                let (lines_added, lines_removed, diff_string) = edit_line_diff(before, &body);
                edit_result::Result::Success(EditSuccess {
                    message: Some(if body.is_empty() {
                        format!("Wrote contents to {path}")
                    } else {
                        format!("The file {path} has been updated.")
                    }),
                    path,
                    lines_added: Some(lines_added),
                    lines_removed: Some(lines_removed),
                    diff_string: Some(diff_string),
                    before_full_file_content: before
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned),
                    after_full_file_content: body,
                })
            }
            Some(write_result::Result::Error(err)) => edit_result::Result::Error(EditError {
                path: path_or(&err.path, path),
                error: err.error.clone(),
            }),
            Some(write_result::Result::PermissionDenied(denied)) => {
                edit_result::Result::Error(EditError {
                    path: path_or(&denied.path, path),
                    error: if denied.error.is_empty() {
                        "write permission denied".into()
                    } else {
                        denied.error.clone()
                    },
                })
            }
            Some(write_result::Result::Rejected(rej)) => edit_result::Result::Error(EditError {
                path: path.to_owned(),
                error: format!("rejected: {}", rej.reason),
            }),
            Some(write_result::Result::NoSpace(space)) => edit_result::Result::Error(EditError {
                path: path_or(&space.path, path),
                error: "no space".into(),
            }),
            None => edit_result::Result::Error(EditError {
                path: path.to_owned(),
                error: "no result".into(),
            }),
        },
        Some(exec_client_message::Message::ReadResult(result)) => match &result.result {
            Some(read_result::Result::FileNotFound(miss)) => {
                edit_result::Result::FileNotFound(EditFileNotFound {
                    path: path_or(&miss.path, path),
                })
            }
            _ => edit_result::Result::Error(EditError {
                path: path.to_owned(),
                error: exec_result_text(exec),
            }),
        },
        _ => edit_result::Result::Error(EditError {
            path: path.to_owned(),
            error: exec_result_text(exec),
        }),
    }
}

pub fn complete_tool_call(
    workspace: Option<&str>,
    tool: &ToolUse,
    exec: &ExecClientMessage,
    prior_read: Option<&str>,
    before: Option<&str>,
) -> (ToolCall, String) {
    complete_tool_call_ex(workspace, tool, exec, prior_read, before, false)
}

fn complete_tool_call_ex(
    workspace: Option<&str>,
    tool: &ToolUse,
    exec: &ExecClientMessage,
    prior_read: Option<&str>,
    before: Option<&str>,
    supports_mcp_auth: bool,
) -> (ToolCall, String) {
    let kind = map_tool_name(&tool.name).unwrap_or(CursorTool::Read);
    let args = &tool.arguments;
    let text = if kind == CursorTool::Edit || kind == CursorTool::EditNotebook {
        prior_read
            .map(str::to_owned)
            .unwrap_or_else(|| exec_result_text(exec))
    } else if kind == CursorTool::Write {
        match &exec.message {
            Some(exec_client_message::Message::WriteResult(result)) => {
                if let Some(write_result::Result::Success(ok)) = &result.result {
                    ok.file_content_after_write
                        .clone()
                        .filter(|body| !body.is_empty())
                        .or_else(|| {
                            let from_args = json_str(args, &["contents", "content", "file_text"]);
                            (!from_args.is_empty()).then_some(from_args)
                        })
                        .unwrap_or_default()
                } else {
                    exec_result_text(exec)
                }
            }
            _ => {
                let from_args = json_str(args, &["contents", "content", "file_text"]);
                if from_args.is_empty() {
                    exec_result_text(exec)
                } else {
                    from_args
                }
            }
        }
    } else if let Some(prior) = prior_read {
        prior.to_owned()
    } else {
        exec_result_text(exec)
    };
    let call_id = tool.id.clone();
    let tool_call = match kind {
        CursorTool::Read => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            let content = exec_result_text(exec);
            ToolCall {
                tool: Some(tool_call::Tool::ReadToolCall(ReadToolCall {
                    args: Some(ReadToolArgs {
                        path: path.clone(),
                        offset: json_i32(args, &["offset"]),
                        limit: json_i32(args, &["limit"]),
                    }),
                    result: Some(ReadToolResult {
                        result: Some(read_tool_from_exec(exec, &path, &content)),
                    }),
                })),
            }
        }
        CursorTool::Grep => ToolCall {
            tool: Some(tool_call::Tool::GrepToolCall(GrepToolCall {
                args: Some(grep_args(workspace, args, &call_id, false)),
                result: Some(grep_tool_from_exec(exec)),
            })),
        },
        CursorTool::Glob => {
            let pattern = json_str(args, &["glob_pattern", "globPattern", "pattern"]);
            let dir = json_str(args, &["target_directory", "targetDirectory", "path"]);
            let path = abs_path(workspace, &dir);
            ToolCall {
                tool: Some(tool_call::Tool::GlobToolCall(GlobToolCall {
                    args: Some(GlobToolArgs {
                        target_directory: (!dir.is_empty()).then(|| path.clone()),
                        glob_pattern: pattern.clone(),
                    }),
                    result: Some(GlobToolResult {
                        result: Some(glob_tool_from_exec(exec, &pattern, &path)),
                    }),
                })),
            }
        }
        CursorTool::Ls => {
            let path = abs_path(workspace, &json_str(args, &["path"]));
            ToolCall {
                tool: Some(tool_call::Tool::LsToolCall(LsToolCall {
                    args: Some(LsArgs {
                        path: path.clone(),
                        ignore: Vec::new(),
                        tool_call_id: call_id,
                    }),
                    result: Some(ls_tool_from_exec(exec, &path)),
                })),
            }
        }
        CursorTool::Shell => {
            let mut transcript = ShellTranscript::default();
            apply_shell_exec(&mut transcript, exec);
            return complete_shell_tool(workspace, tool, &transcript);
        }
        CursorTool::Edit | CursorTool::Write | CursorTool::EditNotebook => {
            let path = if kind == CursorTool::EditNotebook {
                abs_path(workspace, &json_str(args, &["target_notebook", "path"]))
            } else {
                abs_path(workspace, &json_str(args, &["path"]))
            };
            ToolCall {
                tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                    args: Some(EditArgs {
                        path: path.clone(),
                        stream_content: None,
                    }),
                    result: Some(EditResult {
                        result: Some(edit_result_from_exec(exec, &path, &text, before)),
                    }),
                })),
            }
        }
        CursorTool::Mcp => ToolCall {
            tool: Some(tool_call::Tool::McpToolCall(crate::agent_proto::McpToolCall {
                args: Some(mcp_args(args, &call_id)),
                result: Some(mcp_tool_from_exec(exec)),
            })),
        },
        CursorTool::Task => ToolCall {
            tool: Some(tool_call::Tool::TaskToolCall(crate::agent_proto::TaskToolCall {
                args: Some(crate::agent_proto::TaskArgs {
                    description: json_str(args, &["description"]),
                    prompt: json_str(args, &["prompt"]),
                }),
                result: Some(task_from_exec(exec)),
            })),
        },
        CursorTool::Delete => {
            let path = abs_path(workspace, &json_str(args, &["path", "file_path"]));
            ToolCall {
                tool: Some(tool_call::Tool::DeleteToolCall(
                    crate::agent_proto::DeleteToolCall {
                        args: Some(crate::agent_proto::DeleteArgs {
                            path: path.clone(),
                            tool_call_id: call_id,
                        }),
                        result: Some(delete_from_exec(exec, &path)),
                    },
                )),
            }
        }
        CursorTool::ReadLints => {
            let paths = read_lints_args(args).paths;
            let (files, total) = lints_from_exec(exec);
            ToolCall {
                tool: Some(tool_call::Tool::ReadLintsToolCall(
                    crate::agent_proto::ReadLintsToolCall {
                        args: Some(crate::agent_proto::ReadLintsToolArgs { paths }),
                        result: Some(crate::agent_proto::ReadLintsToolResult {
                            result: Some(crate::agent_proto::read_lints_tool_result::Result::Success(
                                crate::agent_proto::ReadLintsToolSuccess {
                                    total_files: files.len() as i32,
                                    total_diagnostics: total,
                                    file_diagnostics: files,
                                },
                            )),
                        }),
                    },
                )),
            }
        }
        CursorTool::ApplyPatch => {
            let path = apply_patch_path(workspace, args);
            ToolCall {
                tool: Some(tool_call::Tool::EditToolCall(EditToolCall {
                    args: Some(EditArgs {
                        path: path.clone(),
                        stream_content: None,
                    }),
                    result: Some(EditResult {
                        result: Some(edit_result_from_exec(exec, &path, &text, before)),
                    }),
                })),
            }
        }
        CursorTool::ListMcpResources => ToolCall {
            tool: Some(tool_call::Tool::ListMcpResourcesToolCall(
                crate::agent_proto::ListMcpResourcesToolCall {
                    args: Some(list_mcp_resources_args(args)),
                    result: Some(list_mcp_resources_from_exec(exec)),
                },
            )),
        },
        CursorTool::FetchMcpResource => ToolCall {
            tool: Some(tool_call::Tool::ReadMcpResourceToolCall(
                crate::agent_proto::ReadMcpResourceToolCall {
                    args: Some(read_mcp_resource_args(args, &call_id)),
                    result: Some(read_mcp_resource_from_exec(exec, args)),
                },
            )),
        },
        CursorTool::GetMcpTools => {
            let json = mcp_state_catalog_text_from_exec(exec, supports_mcp_auth);
            return (
                ToolCall {
                    tool: Some(tool_call::Tool::GetMcpToolsToolCall(
                        crate::agent_proto::GetMcpToolsToolCall {
                            args: Some(crate::agent_proto::GetMcpToolsArgs {
                                server: none_if_empty(json_str(args, &["namespace", "server"])),
                                tool_name: none_if_empty(json_str(
                                    args,
                                    &["toolName", "tool_name"],
                                )),
                                pattern: none_if_empty(json_str(args, &["pattern"])),
                            }),
                            result: Some(crate::agent_proto::GetMcpToolsResult {
                                result: Some(
                                    crate::agent_proto::get_mcp_tools_result::Result::Success(
                                        crate::agent_proto::GetMcpToolsSuccess {
                                            catalog_json: json.clone(),
                                        },
                                    ),
                                ),
                            }),
                        },
                    )),
                },
                json,
            );
        }
        CursorTool::WebSearch
        | CursorTool::CreatePlan
        | CursorTool::WebFetch
        | CursorTool::TodoWrite
        | CursorTool::AskQuestion
        | CursorTool::SwitchMode
        | CursorTool::Await
        | CursorTool::UpdateCurrentStep
        | CursorTool::GenerateImage => {
            return (
                empty_tool_call(kind),
                "use complete_local_tool".into(),
            );
        }
    };
    (tool_call, text)
}

fn mcp_tool_from_exec(exec: &ExecClientMessage) -> crate::agent_proto::McpToolResult {
    match &exec.message {
        Some(exec_client_message::Message::McpResult(result)) => match &result.result {
            Some(crate::agent_proto::mcp_result::Result::Success(ok)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Success(
                        ok.clone(),
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::Error(err)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Error(
                        crate::agent_proto::McpToolError {
                            error: err.error.clone(),
                            read_tool_def_reminder: String::new(),
                        },
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::Rejected(rej)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Rejected(
                        rej.clone(),
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::PermissionDenied(denied)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::PermissionDenied(
                        denied.clone(),
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::ToolNotFound(miss)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Error(
                        crate::agent_proto::McpToolError {
                            error: format!("tool not found: {}", miss.name),
                            read_tool_def_reminder: String::new(),
                        },
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::ServerNotFound(miss)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Error(
                        crate::agent_proto::McpToolError {
                            error: format!("server not found: {}", miss.name),
                            read_tool_def_reminder: String::new(),
                        },
                    )),
                }
            }
            Some(crate::agent_proto::mcp_result::Result::Approved(_)) => {
                crate::agent_proto::McpToolResult {
                    result: Some(crate::agent_proto::mcp_tool_result::Result::Success(
                        crate::agent_proto::McpSuccess {
                            content: Vec::new(),
                            is_error: false,
                        },
                    )),
                }
            }
            None => crate::agent_proto::McpToolResult {
                result: Some(crate::agent_proto::mcp_tool_result::Result::Error(
                    crate::agent_proto::McpToolError {
                        error: "no result".into(),
                        read_tool_def_reminder: String::new(),
                    },
                )),
            },
        },
        _ => crate::agent_proto::McpToolResult {
            result: Some(crate::agent_proto::mcp_tool_result::Result::Error(
                crate::agent_proto::McpToolError {
                    error: exec_result_text(exec),
                    read_tool_def_reminder: String::new(),
                },
            )),
        },
    }
}

fn list_mcp_resources_from_exec(
    exec: &ExecClientMessage,
) -> crate::agent_proto::ListMcpResourcesExecResult {
    match &exec.message {
        Some(exec_client_message::Message::ListMcpResourcesExecResult(result)) => result.clone(),
        _ => crate::agent_proto::ListMcpResourcesExecResult {
            result: Some(
                crate::agent_proto::list_mcp_resources_exec_result::Result::Error(
                    crate::agent_proto::ListMcpResourcesError {
                        error: exec_result_text(exec),
                    },
                ),
            ),
        },
    }
}

fn read_mcp_resource_from_exec(
    exec: &ExecClientMessage,
    args: &Value,
) -> crate::agent_proto::ReadMcpResourceExecResult {
    match &exec.message {
        Some(exec_client_message::Message::ReadMcpResourceExecResult(result)) => result.clone(),
        _ => crate::agent_proto::ReadMcpResourceExecResult {
            result: Some(
                crate::agent_proto::read_mcp_resource_exec_result::Result::Error(
                    crate::agent_proto::ReadMcpResourceError {
                        uri: json_str(args, &["uri"]),
                        error: exec_result_text(exec),
                    },
                ),
            ),
        },
    }
}

fn task_from_exec(exec: &ExecClientMessage) -> crate::agent_proto::TaskResult {
    match &exec.message {
        Some(exec_client_message::Message::SubagentResult(result)) => match &result.result {
            Some(crate::agent_proto::subagent_result::Result::Success(ok)) => {
                crate::agent_proto::TaskResult {
                    result: Some(crate::agent_proto::task_result::Result::Success(
                        crate::agent_proto::TaskSuccess {
                            agent_id: Some(ok.agent_id.clone()),
                            is_background: false,
                            result_suffix: ok.final_message.clone(),
                        },
                    )),
                }
            }
            Some(crate::agent_proto::subagent_result::Result::Error(err)) => {
                crate::agent_proto::TaskResult {
                    result: Some(crate::agent_proto::task_result::Result::Error(
                        crate::agent_proto::TaskError {
                            error: err.error.clone(),
                        },
                    )),
                }
            }
            None => crate::agent_proto::TaskResult {
                result: Some(crate::agent_proto::task_result::Result::Error(
                    crate::agent_proto::TaskError {
                        error: "no result".into(),
                    },
                )),
            },
        },
        _ => crate::agent_proto::TaskResult {
            result: Some(crate::agent_proto::task_result::Result::Error(
                crate::agent_proto::TaskError {
                    error: exec_result_text(exec),
                },
            )),
        },
    }
}

fn ix_rejected(resp: &crate::agent_wire::InteractionResponse) -> Option<String> {
    match &resp.result {
        Some(crate::agent_wire::interaction_response::Result::WebSearchRequestResponse(r)) => {
            match &r.result {
                Some(crate::agent_proto::web_search_request_response::Result::Rejected(rej)) => {
                    Some(rej.reason.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::WebFetchRequestResponse(r)) => {
            match &r.result {
                Some(crate::agent_proto::web_fetch_request_response::Result::Rejected(rej)) => {
                    Some(rej.reason.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::CreatePlanRequestResponse(r)) => {
            match r.result.as_ref().and_then(|res| res.result.as_ref()) {
                Some(crate::agent_proto::create_plan_result::Result::Error(err)) => {
                    Some(err.error.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::AskQuestionInteractionResponse(r)) => {
            match r.result.as_ref().and_then(|res| res.result.as_ref()) {
                Some(crate::agent_proto::ask_question_result::Result::Error(err)) => {
                    Some(err.error_message.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::SwitchModeRequestResponse(r)) => {
            match &r.result {
                Some(crate::agent_proto::switch_mode_request_response::Result::Rejected(rej)) => {
                    Some(rej.reason.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::GenerateImageRequestResponse(r)) => {
            match &r.result {
                Some(crate::agent_proto::generate_image_request_response::Result::Rejected(rej)) => {
                    Some(rej.reason.clone())
                }
                _ => None,
            }
        }
        Some(crate::agent_wire::interaction_response::Result::SetupVmEnvironmentResult(r)) => {
            (!r.error.is_empty()).then(|| r.error.clone())
        }
        Some(crate::agent_wire::interaction_response::Result::PrManagementResult(r)) => {
            (!r.error.is_empty()).then(|| r.error.clone())
        }
        Some(crate::agent_wire::interaction_response::Result::McpAuthRequestResponse(r)) => {
            r.rejected_reason.clone()
        }
        Some(crate::agent_wire::interaction_response::Result::ReplaceEnvResult(r)) => {
            (!r.error.is_empty()).then(|| r.error.clone())
        }
        Some(crate::agent_wire::interaction_response::Result::ConnectScmRequestResponse(r)) => {
            (!r.error.is_empty()).then(|| r.error.clone())
        }
        None => None,
    }
}

fn create_plan_uri_from_ix(
    resp: &crate::agent_wire::InteractionResponse,
) -> Option<String> {
    match &resp.result {
        Some(crate::agent_wire::interaction_response::Result::CreatePlanRequestResponse(inner)) => {
            inner
                .result
                .as_ref()
                .map(|r| r.plan_uri.clone())
                .filter(|uri| !uri.is_empty())
        }
        _ => None,
    }
}

async fn complete_local_tool<E, I, If, W, Wf>(
    run: &LocalRun,
    tool: &ToolUse,
    kind: CursorTool,
    emit: &mut E,
    ix_id: &mut u32,
    wait_ix: &mut I,
    exec_id: &mut u32,
    wait_exec: &mut W,
) -> (ToolCall, String)
where
    E: FnMut(AgentServerMessage),
    I: FnMut(u32) -> If,
    If: Future<Output = Option<crate::agent_wire::InteractionResponse>>,
    W: FnMut(u32) -> Wf,
    Wf: Future<Output = Option<ExecClientMessage>>,
{
    let call_id = tool.id.clone();
    match kind {
        CursorTool::WebSearch => {
            let term = json_str(
                &tool.arguments,
                &["search_term", "searchTerm"],
            );
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::WebSearchRequestQuery(
                    crate::agent_proto::WebSearchRequestQuery {
                        args: Some(web_search_args(&tool.arguments, &call_id)),
                    },
                ),
            ));
            *ix_id += 1;
            if let Some(resp) = wait_ix(id).await {
                if let Some(reason) = ix_rejected(&resp) {
                    return (
                        ToolCall {
                            tool: Some(tool_call::Tool::WebSearchToolCall(
                                crate::agent_proto::WebSearchToolCall {
                                    args: Some(web_search_args(&tool.arguments, &call_id)),
                                    result: Some(crate::agent_proto::WebSearchResult {
                                        result: Some(
                                            crate::agent_proto::web_search_result::Result::Rejected(
                                                crate::agent_proto::WebSearchRejected { reason: reason.clone() },
                                            ),
                                        ),
                                    }),
                                },
                            )),
                        },
                        format!("web search rejected: {reason}"),
                    );
                }
            }
            let refs = crate::web::search_web(&term).await.unwrap_or_default();
            let text = if refs.is_empty() {
                format!("no web results for {term}")
            } else {
                refs.iter()
                    .map(|item| format!("- {} ({})\n  {}", item.title, item.url, item.chunk))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let proto_refs = refs
                .into_iter()
                .map(|item| crate::agent_proto::WebSearchReference {
                    title: item.title,
                    url: item.url,
                    chunk: item.chunk,
                })
                .collect();
            (
                ToolCall {
                    tool: Some(tool_call::Tool::WebSearchToolCall(
                        crate::agent_proto::WebSearchToolCall {
                            args: Some(web_search_args(&tool.arguments, &call_id)),
                            result: Some(crate::agent_proto::WebSearchResult {
                                result: Some(crate::agent_proto::web_search_result::Result::Success(
                                    crate::agent_proto::WebSearchSuccess {
                                        references: proto_refs,
                                    },
                                )),
                            }),
                        },
                    )),
                },
                text,
            )
        }
        CursorTool::CreatePlan => {
            let args = create_plan_args(&tool.arguments);
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::CreatePlanRequestQuery(
                    crate::agent_proto::CreatePlanRequestQuery {
                        args: Some(args.clone()),
                        tool_call_id: call_id.clone(),
                    },
                ),
            ));
            *ix_id += 1;
            let resp = wait_ix(id).await;
            if let Some(resp) = &resp {
                if let Some(reason) = ix_rejected(resp) {
                    return (
                        ToolCall {
                            tool: Some(tool_call::Tool::CreatePlanToolCall(
                                crate::agent_proto::CreatePlanToolCall {
                                    args: Some(args),
                                    result: Some(crate::agent_proto::CreatePlanResult {
                                        plan_uri: String::new(),
                                        result: Some(
                                            crate::agent_proto::create_plan_result::Result::Error(
                                                crate::agent_proto::CreatePlanError { error: reason.clone() },
                                            ),
                                        ),
                                    }),
                                },
                            )),
                        },
                        format!("CreatePlan failed: {reason}"),
                    );
                }
            }
            let name = if args.name.is_empty() {
                "plan".into()
            } else {
                args.name.clone()
            };
            let uri = resp
                .as_ref()
                .and_then(create_plan_uri_from_ix)
                .unwrap_or_else(|| {
                    run.workspace
                        .as_deref()
                        .map(|root| {
                            format!(
                                "file:///{}/.cursor/plans/{name}.plan.md",
                                root.replace('\\', "/")
                            )
                        })
                        .unwrap_or_else(|| format!("file:///.cursor/plans/{name}.plan.md"))
                });
            (
                ToolCall {
                    tool: Some(tool_call::Tool::CreatePlanToolCall(
                        crate::agent_proto::CreatePlanToolCall {
                            args: Some(args),
                            result: Some(crate::agent_proto::CreatePlanResult {
                                plan_uri: uri.clone(),
                                result: Some(crate::agent_proto::create_plan_result::Result::Success(
                                    crate::agent_proto::CreatePlanSuccess {},
                                )),
                            }),
                        },
                    )),
                },
                format!("Created plan at {uri}"),
            )
        }
        CursorTool::WebFetch => {
            let url = json_str(&tool.arguments, &["url"]);
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::WebFetchRequestQuery(
                    crate::agent_proto::WebFetchRequestQuery {
                        args: Some(web_fetch_args(&tool.arguments, &call_id)),
                    },
                ),
            ));
            *ix_id += 1;
            if let Some(resp) = wait_ix(id).await {
                if let Some(reason) = ix_rejected(&resp) {
                    return (
                        ToolCall {
                            tool: Some(tool_call::Tool::WebFetchToolCall(
                                crate::agent_proto::WebFetchToolCall {
                                    args: Some(web_fetch_args(&tool.arguments, &call_id)),
                                    result: Some(crate::agent_proto::WebFetchResult {
                                        result: Some(
                                            crate::agent_proto::web_fetch_result::Result::Rejected(
                                                crate::agent_proto::WebFetchRejected {
                                                    reason: reason.clone(),
                                                },
                                            ),
                                        ),
                                    }),
                                },
                            )),
                        },
                        format!("web fetch rejected: {reason}"),
                    );
                }
            }
            match crate::web::fetch_url(&url).await {
                Ok((final_url, markdown)) => (
                    ToolCall {
                        tool: Some(tool_call::Tool::WebFetchToolCall(
                            crate::agent_proto::WebFetchToolCall {
                                args: Some(web_fetch_args(&tool.arguments, &call_id)),
                                result: Some(crate::agent_proto::WebFetchResult {
                                    result: Some(
                                        crate::agent_proto::web_fetch_result::Result::Success(
                                            crate::agent_proto::WebFetchSuccess {
                                                url: final_url.clone(),
                                                markdown: markdown.clone(),
                                            },
                                        ),
                                    ),
                                }),
                            },
                        )),
                    },
                    markdown,
                ),
                Err(error) => (
                    ToolCall {
                        tool: Some(tool_call::Tool::WebFetchToolCall(
                            crate::agent_proto::WebFetchToolCall {
                                args: Some(web_fetch_args(&tool.arguments, &call_id)),
                                result: Some(crate::agent_proto::WebFetchResult {
                                    result: Some(crate::agent_proto::web_fetch_result::Result::Error(
                                        crate::agent_proto::WebFetchError {
                                            url: url.clone(),
                                            error: error.clone(),
                                        },
                                    )),
                                }),
                            },
                        )),
                    },
                    format!("web fetch error {url}: {error}"),
                ),
            }
        }
        CursorTool::TodoWrite => {
            let args = update_todos_args(&tool.arguments);
            let text = args
                .todos
                .iter()
                .map(|todo| format!("- [{}] {} ({})", todo.status, todo.content, todo.id))
                .collect::<Vec<_>>()
                .join("\n");
            let total = args.todos.len() as i32;
            let merge = args.merge;
            (
                ToolCall {
                    tool: Some(tool_call::Tool::UpdateTodosToolCall(
                        crate::agent_proto::UpdateTodosToolCall {
                            args: Some(args.clone()),
                            result: Some(crate::agent_proto::UpdateTodosResult {
                                result: Some(crate::agent_proto::update_todos_result::Result::Success(
                                    crate::agent_proto::UpdateTodosSuccess {
                                        todos: args.todos,
                                        total_count: total,
                                        was_merge: merge,
                                    },
                                )),
                            }),
                        },
                    )),
                },
                if text.is_empty() {
                    "Updated todos".into()
                } else {
                    text
                },
            )
        }
        CursorTool::AskQuestion => {
            let args = ask_question_args(&tool.arguments);
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::AskQuestionInteractionQuery(
                    crate::agent_proto::AskQuestionInteractionQuery {
                        args: Some(args.clone()),
                        tool_call_id: call_id.clone(),
                    },
                ),
            ));
            *ix_id += 1;
            match wait_ix(id).await {
                Some(resp) => {
                    if let Some(reason) = ix_rejected(&resp) {
                        (
                            ToolCall {
                                tool: Some(tool_call::Tool::AskQuestionToolCall(
                                    crate::agent_proto::AskQuestionToolCall {
                                        args: Some(args),
                                        result: Some(crate::agent_proto::AskQuestionResult {
                                            result: Some(
                                                crate::agent_proto::ask_question_result::Result::Error(
                                                    crate::agent_proto::AskQuestionError {
                                                        error_message: reason.clone(),
                                                    },
                                                ),
                                            ),
                                        }),
                                    },
                                )),
                            },
                            format!("AskQuestion error: {reason}"),
                        )
                    } else if let Some(
                        crate::agent_wire::interaction_response::Result::AskQuestionInteractionResponse(
                            inner,
                        ),
                    ) = resp.result
                    {
                        let answers = match inner.result.and_then(|r| r.result) {
                            Some(crate::agent_proto::ask_question_result::Result::Success(ok)) => {
                                ok.answers
                            }
                            _ => Vec::new(),
                        };
                        let text = if answers.is_empty() {
                            "AskQuestion completed.".into()
                        } else {
                            answers
                                .iter()
                                .map(|a| {
                                    format!(
                                        "{}: {} {}",
                                        a.question_id,
                                        a.selected_option_ids.join(","),
                                        a.freeform_text
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        };
                        (
                            ToolCall {
                                tool: Some(tool_call::Tool::AskQuestionToolCall(
                                    crate::agent_proto::AskQuestionToolCall {
                                        args: Some(args),
                                        result: Some(crate::agent_proto::AskQuestionResult {
                                            result: Some(
                                                crate::agent_proto::ask_question_result::Result::Success(
                                                    crate::agent_proto::AskQuestionSuccess { answers },
                                                ),
                                            ),
                                        }),
                                    },
                                )),
                            },
                            text,
                        )
                    } else {
                        (
                            ToolCall {
                                tool: Some(tool_call::Tool::AskQuestionToolCall(
                                    crate::agent_proto::AskQuestionToolCall {
                                        args: Some(args),
                                        result: Some(crate::agent_proto::AskQuestionResult {
                                            result: Some(
                                                crate::agent_proto::ask_question_result::Result::Error(
                                                    crate::agent_proto::AskQuestionError {
                                                        error_message: "ask question response missing"
                                                            .into(),
                                                    },
                                                ),
                                            ),
                                        }),
                                    },
                                )),
                            },
                            "ask question response missing".into(),
                        )
                    }
                }
                None => (
                    ToolCall {
                        tool: Some(tool_call::Tool::AskQuestionToolCall(
                            crate::agent_proto::AskQuestionToolCall {
                                args: Some(args),
                                result: Some(crate::agent_proto::AskQuestionResult {
                                    result: Some(
                                        crate::agent_proto::ask_question_result::Result::Error(
                                            crate::agent_proto::AskQuestionError {
                                                error_message: "ask question response missing".into(),
                                            },
                                        ),
                                    ),
                                }),
                            },
                        )),
                    },
                    "ask question response missing".into(),
                ),
            }
        }
        CursorTool::SwitchMode => {
            let args = switch_mode_args(&tool.arguments, &call_id);
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::SwitchModeRequestQuery(
                    crate::agent_proto::SwitchModeRequestQuery {
                        args: Some(args.clone()),
                    },
                ),
            ));
            *ix_id += 1;
            if let Some(resp) = wait_ix(id).await {
                if let Some(reason) = ix_rejected(&resp) {
                    return (
                        ToolCall {
                            tool: Some(tool_call::Tool::SwitchModeToolCall(
                                crate::agent_proto::SwitchModeToolCall {
                                    args: Some(args),
                                    result: Some(crate::agent_proto::SwitchModeResult {
                                        result: Some(
                                            crate::agent_proto::switch_mode_result::Result::Rejected(
                                                crate::agent_proto::SwitchModeRejected {
                                                    reason: reason.clone(),
                                                },
                                            ),
                                        ),
                                    }),
                                },
                            )),
                        },
                        format!("mode switch rejected: {reason}"),
                    );
                }
            }
            let to = args.target_mode_id.clone();
            (
                ToolCall {
                    tool: Some(tool_call::Tool::SwitchModeToolCall(
                        crate::agent_proto::SwitchModeToolCall {
                            args: Some(args),
                            result: Some(crate::agent_proto::SwitchModeResult {
                                result: Some(crate::agent_proto::switch_mode_result::Result::Success(
                                    crate::agent_proto::SwitchModeSuccess {
                                        from_mode_id: "agent".into(),
                                        to_mode_id: to.clone(),
                                    },
                                )),
                            }),
                        },
                    )),
                },
                format!("{{\"toModeId\":\"{to}\"}}"),
            )
        }
        CursorTool::Await => {
            let args = await_args(&tool.arguments);
            let wait_ms = args.block_until_ms.unwrap_or(30_000).min(60_000);
            if wait_ms > 0 && !cfg!(test) {
                tokio::time::sleep(Duration::from_millis(u64::from(wait_ms))).await;
            }
            (
                ToolCall {
                    tool: Some(tool_call::Tool::AwaitToolCall(
                        crate::agent_proto::AwaitToolCall {
                            args: Some(args.clone()),
                            result: Some(crate::agent_proto::AwaitResult {
                                result: Some(crate::agent_proto::await_result::Result::StillRunning(
                                    crate::agent_proto::AwaitTaskStillRunning {
                                        task_id: args.task_id.clone(),
                                        runtime_ms: u64::from(wait_ms),
                                    },
                                )),
                            }),
                        },
                    )),
                },
                if args.task_id.is_empty() {
                    format!("slept {wait_ms}ms")
                } else {
                    format!("still running {}", args.task_id)
                },
            )
        }
        CursorTool::UpdateCurrentStep => {
            let args = communicate_args(&tool.arguments);
            let step = args.current_step.clone().unwrap_or_default();
            (
                ToolCall {
                    tool: Some(tool_call::Tool::CommunicateUpdateToolCall(
                        crate::agent_proto::CommunicateUpdateToolCall {
                            args: Some(args),
                            result: Some(crate::agent_proto::CommunicateUpdateResult {
                                result: Some(
                                    crate::agent_proto::communicate_update_result::Result::Success(
                                        crate::agent_proto::CommunicateUpdateSuccess {
                                            current_step: step.clone(),
                                            message_index: 0,
                                        },
                                    ),
                                ),
                            }),
                        },
                    )),
                },
                if step.is_empty() {
                    "updated current step".into()
                } else {
                    step
                },
            )
        }
        CursorTool::GenerateImage => {
            let args = generate_image_args(&tool.arguments);
            let id = *ix_id;
            emit(agent_wire::interaction_query(
                id,
                crate::agent_wire::interaction_query::Query::GenerateImageRequestQuery(
                    crate::agent_proto::GenerateImageRequestQuery {
                        args: Some(args.clone()),
                        tool_call_id: call_id.clone(),
                    },
                ),
            ));
            *ix_id += 1;
            let image_error = |args: crate::agent_proto::GenerateImageArgs, error: String| {
                (
                    ToolCall {
                        tool: Some(tool_call::Tool::GenerateImageToolCall(
                            crate::agent_proto::GenerateImageToolCall {
                                args: Some(args),
                                result: Some(crate::agent_proto::GenerateImageResult {
                                    result: Some(
                                        crate::agent_proto::generate_image_result::Result::Error(
                                            crate::agent_proto::GenerateImageError {
                                                error: error.clone(),
                                            },
                                        ),
                                    ),
                                }),
                            },
                        )),
                    },
                    format!("image generation error: {error}"),
                )
            };
            match wait_ix(id).await {
                Some(resp) => {
                    if let Some(reason) = ix_rejected(&resp) {
                        return image_error(args, reason);
                    }
                }
                None => {
                    return image_error(args, "image generation response missing".into());
                }
            }
            let path = abs_path(
                run.workspace.as_deref(),
                args.file_path.as_deref().unwrap_or("generated.png"),
            );
            let write = exec_server_message::Message::WriteArgs(WriteArgs {
                path: path.clone(),
                file_text: String::new(),
                tool_call_id: call_id.clone(),
                return_file_content_after_write: false,
                file_bytes: Vec::new(),
            });
            emit(agent_wire::server_exec(
                *exec_id,
                &format!("{call_id}-write"),
                write,
            ));
            let write_exec =
                wait_exec_heartbeat(wait_exec, *exec_id, emit, EXEC_TIMEOUT).await;
            *exec_id += 1;
            match write_exec {
                Some(msg) if write_exec_success(&msg) => {}
                Some(msg) => {
                    return image_error(args, exec_result_text(&msg));
                }
                None => {
                    return image_error(args, "image write timed out waiting for Cursor host".into());
                }
            }
            (
                ToolCall {
                    tool: Some(tool_call::Tool::GenerateImageToolCall(
                        crate::agent_proto::GenerateImageToolCall {
                            args: Some(args),
                            result: Some(crate::agent_proto::GenerateImageResult {
                                result: Some(
                                    crate::agent_proto::generate_image_result::Result::Success(
                                        crate::agent_proto::GenerateImageSuccess {
                                            file_path: path.clone(),
                                            image_data: String::new(),
                                        },
                                    ),
                                ),
                            }),
                        },
                    )),
                },
                format!("Image request approved. Host save path: {path}."),
            )
        }
        CursorTool::GetMcpTools => {
            let json = crate::mcp::catalog_json_auth(
                none_if_empty(json_str(
                    &tool.arguments,
                    &["namespace", "server"],
                ))
                .as_deref(),
                none_if_empty(json_str(
                    &tool.arguments,
                    &["toolName", "tool_name"],
                ))
                .as_deref(),
                none_if_empty(json_str(&tool.arguments, &["pattern"])).as_deref(),
                run.supports_mcp_auth,
            );
            (
                ToolCall {
                    tool: Some(tool_call::Tool::GetMcpToolsToolCall(
                        crate::agent_proto::GetMcpToolsToolCall {
                            args: Some(crate::agent_proto::GetMcpToolsArgs {
                                server: none_if_empty(json_str(
                                    &tool.arguments,
                                    &["namespace", "server"],
                                )),
                                tool_name: none_if_empty(json_str(
                                    &tool.arguments,
                                    &["toolName", "tool_name"],
                                )),
                                pattern: none_if_empty(json_str(&tool.arguments, &["pattern"])),
                            }),
                            result: Some(crate::agent_proto::GetMcpToolsResult {
                                result: Some(
                                    crate::agent_proto::get_mcp_tools_result::Result::Success(
                                        crate::agent_proto::GetMcpToolsSuccess {
                                            catalog_json: json.clone(),
                                        },
                                    ),
                                ),
                            }),
                        },
                    )),
                },
                json,
            )
        }
        _ => (empty_tool_call(kind), "unsupported local tool".into()),
    }
}

fn lints_from_exec(
    exec: &ExecClientMessage,
) -> (Vec<crate::agent_proto::FileDiagnostics>, i32) {
    match &exec.message {
        Some(exec_client_message::Message::DiagnosticsResult(result)) => match &result.result {
            Some(crate::agent_proto::diagnostics_result::Result::Success(ok)) => (
                vec![crate::agent_proto::FileDiagnostics {
                    path: ok.path.clone(),
                    diagnostics: ok.diagnostics.clone(),
                }],
                ok.total_diagnostics,
            ),
            Some(crate::agent_proto::diagnostics_result::Result::Error(err)) => (
                vec![crate::agent_proto::FileDiagnostics {
                    path: err.path.clone(),
                    diagnostics: vec![crate::agent_proto::LintDiagnostic {
                        message: err.error.clone(),
                        source: "linter".into(),
                    }],
                }],
                1,
            ),
            None => (Vec::new(), 0),
        },
        _ => (Vec::new(), 0),
    }
}

fn delete_from_exec(exec: &ExecClientMessage, path: &str) -> crate::agent_proto::DeleteResult {
    match &exec.message {
        Some(exec_client_message::Message::DeleteResult(result)) if result.result.is_some() => {
            result.clone()
        }
        _ => crate::agent_proto::DeleteResult {
            result: Some(crate::agent_proto::delete_result::Result::Error(
                crate::agent_proto::DeleteError {
                    path: path.to_owned(),
                    error: exec_result_text(exec),
                },
            )),
        },
    }
}

fn glob_files_from_exec(exec: &ExecClientMessage) -> (Vec<String>, bool, bool) {
    let Some(exec_client_message::Message::GrepResult(result)) = &exec.message else {
        return (Vec::new(), false, false);
    };
    let Some(grep_result::Result::Success(success)) = &result.result else {
        return (Vec::new(), false, false);
    };
    let mut files = Vec::new();
    let mut client_truncated = false;
    let mut ripgrep_truncated = false;
    for union in success.workspace_results.values() {
        if let Some(grep_union_result::Result::Files(list)) = &union.result {
            files.extend(list.files.iter().cloned());
            client_truncated |= list.client_truncated;
            ripgrep_truncated |= list.ripgrep_truncated;
            if list.total_files > 0 && (list.files.len() as i32) < list.total_files {
                client_truncated = true;
            }
        }
    }
    (files, client_truncated, ripgrep_truncated)
}

fn glob_tool_from_exec(
    exec: &ExecClientMessage,
    pattern: &str,
    path: &str,
) -> glob_tool_result::Result {
    match &exec.message {
        Some(exec_client_message::Message::GrepResult(result)) => match &result.result {
            Some(grep_result::Result::Success(_)) => {
                let (files, client_truncated, ripgrep_truncated) = glob_files_from_exec(exec);
                glob_tool_result::Result::Success(GlobToolSuccess {
                    pattern: pattern.to_owned(),
                    path: path.to_owned(),
                    total_files: files.len() as i32,
                    files,
                    client_truncated,
                    ripgrep_truncated,
                })
            }
            Some(grep_result::Result::Error(err)) => {
                glob_tool_result::Result::Error(GlobToolError {
                    error: err.error.clone(),
                })
            }
            None => glob_tool_result::Result::Error(GlobToolError {
                error: "no result".into(),
            }),
        },
        _ => glob_tool_result::Result::Error(GlobToolError {
            error: "no result".into(),
        }),
    }
}

fn grep_tool_from_exec(exec: &ExecClientMessage) -> GrepResult {
    match &exec.message {
        Some(exec_client_message::Message::GrepResult(result)) if result.result.is_some() => {
            result.clone()
        }
        _ => GrepResult {
            result: Some(grep_result::Result::Error(GrepError {
                error: "no result".into(),
            })),
        },
    }
}

fn ls_tool_from_exec(exec: &ExecClientMessage, path: &str) -> LsResult {
    match &exec.message {
        Some(exec_client_message::Message::LsResult(result)) if result.result.is_some() => {
            result.clone()
        }
        _ => LsResult {
            result: Some(ls_result::Result::Error(LsError {
                path: path.to_owned(),
                error: "no result".into(),
            })),
        },
    }
}

fn normalize_cursor_nl(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn binary_file_marker(path: &str, bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        if !text.contains('\0') {
            return text.to_owned();
        }
    }
    format!("[Binary file: {path} ({} bytes)]", bytes.len())
}

fn apply_edit(before: &str, old: &str, new: &str, replace_all: bool) -> Result<String, String> {
    let bom = if before.starts_with('\u{FEFF}') {
        "\u{FEFF}"
    } else {
        ""
    };
    let before = before.strip_prefix('\u{FEFF}').unwrap_or(before);
    let old = normalize_cursor_nl(old);
    let new = normalize_cursor_nl(new);
    if old == new {
        return Err("old_string and new_string are the same".into());
    }
    if old.is_empty() {
        return Err("Edit does not create files. Use Write.".into());
    }
    let count = before.matches(&old).count();
    if count == 0 {
        return Err(format!("String to replace not found:\n{old}"));
    }
    if count > 1 && !replace_all {
        return Err(format!(
            "Found {count} matches; set replace_all or give more context"
        ));
    }
    let edited = if replace_all {
        before.replace(&old, &new)
    } else {
        before.replacen(&old, &new, 1)
    };
    Ok(format!("{bom}{edited}"))
}

fn apply_notebook_edit(
    before: &str,
    cell_idx: i32,
    is_new_cell: bool,
    cell_language: &str,
    old_string: &str,
    new_string: &str,
) -> Result<String, String> {
    let raw = if before.trim().is_empty() {
        r#"{"cells":[],"metadata":{},"nbformat":4,"nbformat_minor":5}"#
    } else {
        before
    };
    let mut notebook: Value = serde_json::from_str(raw)
        .map_err(|error| format!("invalid notebook JSON: {error}"))?;
    let cells = notebook
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "notebook has no cells array".to_string())?;
    let index = usize::try_from(cell_idx).map_err(|_| format!("cell_idx {cell_idx} is invalid"))?;
    let new = normalize_cursor_nl(new_string);
    if is_new_cell {
        if index > cells.len() {
            return Err(format!("cell_idx {index} is past the end of the notebook"));
        }
        let cell_type = if cell_language == "markdown" || cell_language == "raw" {
            cell_language
        } else {
            "code"
        };
        let mut cell = json!({
            "cell_type": cell_type,
            "metadata": {},
            "source": notebook_source_lines(&new),
        });
        if cell_type == "code" {
            cell["execution_count"] = Value::Null;
            cell["outputs"] = json!([]);
        }
        cells.insert(index, cell);
    } else {
        let cell = cells
            .get_mut(index)
            .ok_or_else(|| format!("cell_idx {index} does not exist"))?;
        let source = notebook_cell_source(cell.get("source").unwrap_or(&Value::Null))?;
        let old = normalize_cursor_nl(old_string);
        if old.is_empty() {
            return Err("old_string must not be empty".into());
        }
        let occurrences = source.match_indices(&old).count();
        let edited = match occurrences {
            0 => return Err("old_string was not found in the notebook cell".into()),
            1 => source.replacen(&old, &new, 1),
            count => {
                return Err(format!(
                    "old_string is not unique in the notebook cell; found {count} occurrences"
                ));
            }
        };
        cell["source"] = Value::Array(notebook_source_lines(&edited));
    }
    serde_json::to_string_pretty(&notebook)
        .map(|value| {
            if value.ends_with('\n') {
                value
            } else {
                format!("{value}\n")
            }
        })
        .map_err(|error| error.to_string())
}

fn notebook_cell_source(value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(normalize_cursor_nl(text)),
        Value::Array(lines) => lines
            .iter()
            .map(|line| {
                line.as_str()
                    .ok_or_else(|| "notebook cell source contains a non-string".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| normalize_cursor_nl(&lines.concat())),
        Value::Null => Ok(String::new()),
        _ => Err("notebook cell source is not text".into()),
    }
}

fn notebook_source_lines(value: &str) -> Vec<Value> {
    if value.is_empty() {
        Vec::new()
    } else {
        value
            .split_inclusive('\n')
            .map(|line| Value::String(line.to_owned()))
            .collect()
    }
}

fn looks_like_tool_markup(text: &str) -> bool {
    text.contains("<tool") || text.contains("```tool_call")
}

fn emit_remaining_text<E>(visible: &str, streamed: &str, emit: &mut E)
where
    E: FnMut(AgentServerMessage),
{
    if visible.is_empty() || looks_like_tool_markup(visible) {
        return;
    }
    if streamed.is_empty() {
        emit(text_frame(visible));
        return;
    }
    if visible.starts_with(streamed) {
        let rest = &visible[streamed.len()..];
        if !rest.is_empty() && !looks_like_tool_markup(rest) {
            emit(text_frame(rest));
        }
    }
}

fn apply_live_chunk<E>(
    chunk: LlmChunk,
    emit: &mut E,
    streamed_thinking: &mut bool,
    streamed_text: &mut String,
    pending_tokens: &mut i32,
) where
    E: FnMut(AgentServerMessage),
{
    match chunk {
        LlmChunk::Thinking(text) if !text.is_empty() => {
            *streamed_thinking = true;
            emit(thinking_delta_frame(&text));
            *pending_tokens += estimate_tokens(&text);
            if *pending_tokens >= 3 {
                emit(token_delta_frame(*pending_tokens));
                *pending_tokens = 0;
            }
        }
        LlmChunk::Text(text)
            if !text.is_empty()
                && !looks_like_tool_markup(&text)
                && !looks_like_tool_markup(streamed_text) =>
        {
            streamed_text.push_str(&text);
            emit(text_frame(&text));
            *pending_tokens += estimate_tokens(&text);
            if *pending_tokens >= 5 {
                emit(token_delta_frame(*pending_tokens));
                *pending_tokens = 0;
            }
        }
        LlmChunk::Tokens(n) if n > 0 => emit(token_delta_frame(n)),
        LlmChunk::ToolStart { id, name } => {
            let preview = ToolUse {
                id,
                name,
                arguments: json!({}),
            };
            if let Some(frames) = encode_tool_preview_frames(&preview, "live") {
                for frame in frames {
                    emit(frame);
                }
            }
            emit(token_delta_frame(1));
        }
        _ => {}
    }
}

fn run_cancelled(run: &LocalRun) -> bool {
    run.cancel
        .load(std::sync::atomic::Ordering::Relaxed)
}

async fn await_llm_turn<L, Lf, E>(
    prompt: String,
    llm: &mut L,
    emit: &mut E,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(ModelTurn, bool, String), String>
where
    L: FnMut(String, mpsc::UnboundedSender<LlmChunk>) -> Lf,
    Lf: Future<Output = Result<ModelTurn, String>>,
    E: FnMut(AgentServerMessage),
{
    let (live_tx, mut live_rx) = mpsc::unbounded_channel();
    let fut = llm(prompt, live_tx);
    tokio::pin!(fut);
    let mut streamed_thinking = false;
    let mut streamed_text = String::new();
    let mut pending_tokens = 0i32;
    let mut live_open = true;
    let thinking_started = Instant::now();
    let mut last_content = Instant::now();
    let mut idle_hint = false;
    let deadline = Instant::now() + LLM_TIMEOUT;
    let turn = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("LLM timed out waiting for Stream".into());
        }
        tokio::select! {
            result = &mut fut => {
                while let Ok(chunk) = live_rx.try_recv() {
                    apply_live_chunk(
                        chunk,
                        emit,
                        &mut streamed_thinking,
                        &mut streamed_text,
                        &mut pending_tokens,
                    );
                }
                break result;
            }
            chunk = live_rx.recv(), if live_open => {
                match chunk {
                    Some(chunk) => {
                        let before = streamed_text.len();
                        apply_live_chunk(
                            chunk,
                            emit,
                            &mut streamed_thinking,
                            &mut streamed_text,
                            &mut pending_tokens,
                        );
                        if streamed_text.len() > before {
                            last_content = Instant::now();
                            idle_hint = false;
                        }
                    }
                    None => live_open = false,
                }
            }
            _ = tokio::time::sleep(left.min(Duration::from_millis(HEARTBEAT_MS))) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err("cancelled".into());
                }
                if !idle_hint
                    && !streamed_text.is_empty()
                    && last_content.elapsed() >= Duration::from_millis(IDLE_HINT_MS)
                {
                    idle_hint = true;
                    emit(thinking_completed_frame(0));
                }
                emit(heartbeat_frame());
            }
        }
    };
    if pending_tokens > 0 {
        emit(token_delta_frame(pending_tokens));
    }
    if streamed_thinking {
        let ms = thinking_started.elapsed().as_millis().min(i32::MAX as u128) as i32;
        emit(thinking_completed_frame(ms.max(1)));
    }
    let turn = turn?;
    Ok((turn, streamed_thinking, streamed_text))
}

fn emit_role_blob<E>(
    emit: &mut E,
    kv_id: &mut u32,
    blob_ids: &mut Vec<Vec<u8>>,
    role: &str,
    content: &str,
) where
    E: FnMut(AgentServerMessage),
{
    let (id, data) = crate::blob::encode_role(role, content);
    emit(agent_wire::kv_set_blob(*kv_id, &id, &data));
    blob_ids.push(crate::blob::id_bytes(&id));
    *kv_id = kv_id.saturating_add(1);
}

fn blob_tool_result<E>(
    emit: &mut E,
    kv_id: &mut u32,
    blob_ids: &mut Vec<Vec<u8>>,
    tool_name: &str,
    result_text: &str,
) where
    E: FnMut(AgentServerMessage),
{
    emit_role_blob(
        emit,
        kv_id,
        blob_ids,
        "user",
        &format!("<tool_result name=\"{tool_name}\">\n{result_text}\n</tool_result>"),
    );
}

/// Drive the shipped host-exec loop. `wait_exec` must not skip the wait.
pub async fn run_injected_agent<L, Lf, W, Wf, I, If, E>(
    run: &LocalRun,
    mut llm: L,
    mut wait_exec: W,
    mut wait_ix: I,
    mut emit: E,
) -> Result<(), String>
where
    L: FnMut(String, mpsc::UnboundedSender<LlmChunk>) -> Lf,
    Lf: Future<Output = Result<ModelTurn, String>>,
    W: FnMut(u32) -> Wf,
    Wf: Future<Output = Option<ExecClientMessage>>,
    I: FnMut(u32) -> If,
    If: Future<Output = Option<crate::agent_wire::InteractionResponse>>,
    E: FnMut(AgentServerMessage),
{
    let mut prompt = toolkit_prompt(run);
    let mut exec_id: u32 = 1;
    let mut interaction_id: u32 = 1;
    let mut kv_id: u32 = 0;
    let mut blob_ids = run.history_blob_ids.clone();
    let mut session_todos: Vec<crate::agent_proto::TodoItem> = Vec::new();
    let mut summary_archives: Vec<Vec<u8>> = Vec::new();
    let mut track = CheckpointTrack::default();
    if run.is_background_completion && !run.user_text.is_empty() {
        emit(agent_wire::user_message_appended(&run.user_text, run.mode));
    }
    let max_tokens = if run.max_tokens == 0 {
        crate::agent_wire::DEFAULT_CONTEXT_TOKENS
    } else {
        run.max_tokens
    };
    let prompt_budget = prompt_char_budget(max_tokens);
    let system_text = prompt
        .split(crate::agent_wire::PREAMBLE_MARK)
        .next()
        .unwrap_or(prompt.as_str())
        .trim()
        .to_owned();
    let preamble_text = prompt
        .split_once(crate::agent_wire::PREAMBLE_MARK)
        .and_then(|(_, rest)| rest.split_once(crate::agent_wire::USER_MARK))
        .map(|(preamble, _)| preamble.trim().to_owned())
        .unwrap_or_default();
    if run.history_blob_ids.is_empty() {
        emit_role_blob(&mut emit, &mut kv_id, &mut blob_ids, "system", &system_text);
        if !preamble_text.is_empty() {
            emit_role_blob(
                &mut emit,
                &mut kv_id,
                &mut blob_ids,
                "user",
                &preamble_text,
            );
        }
        emit_role_blob(
            &mut emit,
            &mut kv_id,
            &mut blob_ids,
            "user",
            &run.user_text,
        );
    } else if !run.user_text.is_empty() {
        emit_role_blob(
            &mut emit,
            &mut kv_id,
            &mut blob_ids,
            "user",
            &run.user_text,
        );
    }
    emit_run_checkpoint(
        &mut emit,
        run,
        run.used_tokens.max(estimate_tokens(&prompt) as u32),
        max_tokens,
        &blob_ids,
        &prompt,
        &session_todos,
        &[],
        &summary_archives,
            &track,
    );
    for round in 0..MAX_ROUNDS {
        prompt = maybe_autocompact_prompt(
            prompt,
            max_tokens,
            prompt_budget,
            &mut emit,
            &mut summary_archives,
        );
        emit(step_started_frame(round as u64 + 1));
        let round_started = Instant::now();
        let mut attempt = 0;
        let (turn, streamed_thinking, streamed_text) = loop {
            if run_cancelled(run) {
                emit(agent_wire::exec_server_abort(exec_id));
                emit(text_frame("Cancelled."));
                emit(turn_ended_usage(&TokenUsage::default(), &prompt));
                return Err("cancelled".into());
            }
            match await_llm_turn(prompt.clone(), &mut llm, &mut emit, &run.cancel).await {
                Ok(result) => break result,
                Err(error) if attempt == 0 && is_token_limit_error(&error) => {
                    prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget / 2);
                    attempt += 1;
                    emit(heartbeat_frame());
                }
                Err(error) => {
                    emit(text_frame(&error));
                    emit(turn_ended_usage(&TokenUsage::default(), &prompt));
                    return Err(error);
                }
            }
        };
        if !streamed_thinking {
            for frame in thinking_frames(&turn.thinking) {
                emit(frame);
            }
        }
        let visible = &turn.text;
        emit_remaining_text(visible, &streamed_text, &mut emit);
        for tool in &turn.tools {
            let model_call_id = format!("{}-{round}-{}", run.request_id, tool.id);
            if let Some(preview) = encode_tool_preview_frames(tool, &model_call_id) {
                for frame in preview {
                    emit(frame);
                }
            }
        }
        emit(step_completed_frame(
            round as u64 + 1,
            round_started.elapsed().as_millis() as i64,
        ));
        let assistant_blob = if turn.thinking.is_empty() {
            turn.text.clone()
        } else if turn.text.is_empty() {
            turn.thinking.clone()
        } else {
            format!("{}\n{}", turn.thinking, turn.text)
        };
        if !assistant_blob.is_empty() {
            emit_role_blob(
                &mut emit,
                &mut kv_id,
                &mut blob_ids,
                "assistant",
                &assistant_blob,
            );
        }
        if turn.tools.is_empty() {
            let used = if turn.usage.input > 0 {
                turn.usage.input as u32
            } else {
                estimate_tokens(&prompt) as u32
            };
            let pending = pending_assistant_json(&turn.thinking, &turn.text);
            emit_run_checkpoint(
                &mut emit,
                run,
                used,
                max_tokens,
                &blob_ids,
                &prompt,
                &session_todos,
                &pending,
                &summary_archives,
            &track,
            );
            emit(turn_ended_usage(&turn.usage, &prompt));
            return Ok(());
        }
        let force_task_background = turn
            .tools
            .iter()
            .any(|tool| map_tool_name(&tool.name) == Some(CursorTool::Task));
        let mut pending_tasks: Vec<(u32, ToolUse, String)> = Vec::new();
        let mut ordered: Vec<&ToolUse> = turn
            .tools
            .iter()
            .filter(|tool| map_tool_name(&tool.name) == Some(CursorTool::Task))
            .collect();
        ordered.extend(
            turn.tools
                .iter()
                .filter(|tool| map_tool_name(&tool.name) != Some(CursorTool::Task)),
        );
        for tool in ordered {
            let model_call_id = format!("{}-{round}-{}", run.request_id, tool.id);
            let task_model = resolve_task_model(run, &tool.arguments);
            let Some((frames, kind)) = encode_tool_start_frames_ex(
                run.workspace.as_deref(),
                tool,
                exec_id,
                &model_call_id,
                run.conversation_id.as_deref(),
                force_task_background,
                task_model.as_deref(),
            )
            else {
                prompt.push_str(&format!(
                    "\n\n<tool_result name=\"{}\">unknown tool</tool_result>\n",
                    tool.name
                ));
                prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                continue;
            };
            for frame in frames {
                emit(frame);
            }
            if kind == CursorTool::Task {
                pending_tasks.push((exec_id, tool.clone(), model_call_id));
                exec_id += 1;
                continue;
            }
            if kind == CursorTool::WebSearch
                || kind == CursorTool::CreatePlan
                || kind == CursorTool::WebFetch
                || kind == CursorTool::TodoWrite
                || kind == CursorTool::AskQuestion
                || kind == CursorTool::SwitchMode
                || kind == CursorTool::Await
                || kind == CursorTool::UpdateCurrentStep
                || kind == CursorTool::GenerateImage
            {
                let (completed, result_text) = complete_local_tool(
                    run,
                    tool,
                    kind,
                    &mut emit,
                    &mut interaction_id,
                    &mut wait_ix,
                    &mut exec_id,
                    &mut wait_exec,
                )
                .await;
                if kind == CursorTool::TodoWrite {
                    session_todos = update_todos_args(&tool.arguments).todos;
                }
                note_tool_checkpoint(&mut track, run, kind, tool, &result_text);
                emit(agent_wire::server_tool_completed(
                    &tool.id,
                    completed,
                    &model_call_id,
                ));
                blob_tool_result(
                    &mut emit,
                    &mut kv_id,
                    &mut blob_ids,
                    &tool.name,
                    &result_text,
                );
                emit_run_checkpoint(
                    &mut emit,
                    run,
                    estimate_tokens(&prompt) as u32,
                    max_tokens,
                    &blob_ids,
                    &prompt,
                    &session_todos,
                    &pending_assistant_json(&turn.thinking, &turn.text),
                    &summary_archives,
            &track,
                );
                prompt.push_str("\n\n<tool_result name=\"");
                prompt.push_str(&tool.name);
                prompt.push_str("\">\n");
                prompt.push_str(&crate::agent_wire::clip_utf8(
                    &result_text,
                    TOOL_RESULT_CHARS,
                ));
                prompt.push_str("\n</tool_result>\nContinue.\n");
                prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                continue;
            }
            let exec_timeout = if kind == CursorTool::Task {
                TASK_TIMEOUT
            } else {
                EXEC_TIMEOUT
            };
            let exec =
                wait_exec_heartbeat(&mut wait_exec, exec_id, &mut emit, exec_timeout).await;
            let exec = match exec {
                Some(exec) => exec,
                None => {
                    if run_cancelled(run) {
                        emit(agent_wire::exec_server_abort(exec_id));
                        emit(text_frame("Cancelled."));
                        emit(turn_ended_usage(&turn.usage, &prompt));
                        return Err("cancelled".into());
                    }
                    emit(text_frame(&format!(
                        "Tool {} timed out waiting for Cursor host.",
                        tool.name
                    )));
                    emit(turn_ended_usage(&turn.usage, &prompt));
                    return Err(format!("exec timeout for {}", tool.name));
                }
            };
            exec_id += 1;
            let result_text = if kind == CursorTool::Edit
                || kind == CursorTool::ApplyPatch
                || kind == CursorTool::EditNotebook
            {
                let allow_missing = kind == CursorTool::EditNotebook
                    && json_bool(&tool.arguments, &["is_new_cell", "isNewCell"]);
                let before = match read_for_edit(&exec, allow_missing) {
                    Ok(before) => before,
                    Err(error) => {
                        let (completed, _) =
                            complete_edit_error(run.workspace.as_deref(), tool, &error);
                        emit(agent_wire::server_tool_completed(
                            &tool.id,
                            completed,
                            &model_call_id,
                        ));
                        blob_tool_result(
                            &mut emit,
                            &mut kv_id,
                            &mut blob_ids,
                            &tool.name,
                            &error,
                        );
                        emit_run_checkpoint(
                            &mut emit,
                            run,
                            estimate_tokens(&prompt) as u32,
                            max_tokens,
                            &blob_ids,
                            &prompt,
                            &session_todos,
                            &pending_assistant_json(&turn.thinking, &turn.text),
                            &summary_archives,
            &track,
                        );
                        prompt.push_str(&format!(
                            "\n\n<tool_result name=\"{}\">{}</tool_result>\n",
                            tool.name,
                            crate::agent_wire::clip_utf8(&error, TOOL_RESULT_CHARS)
                        ));
                        prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                        continue;
                    }
                };
                let applied = if kind == CursorTool::ApplyPatch {
                    apply_patch_to_content(&json_str(&tool.arguments, &["patch"]), &before)
                } else if kind == CursorTool::EditNotebook {
                    apply_notebook_edit(
                        &before,
                        json_i32(&tool.arguments, &["cell_idx", "cellIdx"]).unwrap_or(0),
                        json_bool(&tool.arguments, &["is_new_cell", "isNewCell"]),
                        &json_str(&tool.arguments, &["cell_language", "cellLanguage"]),
                        &json_str(&tool.arguments, &["old_string", "oldString"]),
                        &json_str(&tool.arguments, &["new_string", "newString"]),
                    )
                } else {
                    apply_edit(
                        &before,
                        &json_str(&tool.arguments, &["old_string", "oldString"]),
                        &json_str(&tool.arguments, &["new_string", "newString"]),
                        json_bool(&tool.arguments, &["replace_all", "replaceAll"]),
                    )
                };
                match applied {
                    Ok(after) => {
                        let write_path = if kind == CursorTool::ApplyPatch {
                            apply_patch_path(run.workspace.as_deref(), &tool.arguments)
                        } else if kind == CursorTool::EditNotebook {
                            abs_path(
                                run.workspace.as_deref(),
                                &json_str(&tool.arguments, &["target_notebook", "path"]),
                            )
                        } else {
                            abs_path(
                                run.workspace.as_deref(),
                                &json_str(&tool.arguments, &["path"]),
                            )
                        };
                        let write = exec_server_message::Message::WriteArgs(WriteArgs {
                            path: write_path,
                            file_text: normalize_cursor_nl(&after),
                            tool_call_id: tool.id.clone(),
                            return_file_content_after_write: kind == CursorTool::EditNotebook,
                            file_bytes: Vec::new(),
                        });
                        emit(agent_wire::server_exec(
                            exec_id,
                            &format!("{}-write", tool.id),
                            write,
                        ));
                        let write_exec =
                            wait_exec_heartbeat(&mut wait_exec, exec_id, &mut emit, EXEC_TIMEOUT)
                                .await;
                        exec_id += 1;
                        match write_exec {
                            Some(msg) if write_exec_success(&msg) => {
                                let (completed, text) = complete_tool_call(
                                    run.workspace.as_deref(),
                                    tool,
                                    &msg,
                                    Some(&after),
                                    Some(&before),
                                );
                                emit(agent_wire::server_tool_completed(
                                    &tool.id,
                                    completed,
                                    &model_call_id,
                                ));
                                blob_tool_result(
                                    &mut emit,
                                    &mut kv_id,
                                    &mut blob_ids,
                                    &tool.name,
                                    &text,
                                );
                                emit_run_checkpoint(
                                    &mut emit,
                                    run,
                                    estimate_tokens(&prompt) as u32,
                                    max_tokens,
                                    &blob_ids,
                                    &prompt,
                                    &session_todos,
                                    &pending_assistant_json(&turn.thinking, &turn.text),
                                    &summary_archives,
            &track,
                                );
                                prompt.push_str("\n\n<tool_result name=\"");
                                prompt.push_str(&tool.name);
                                prompt.push_str("\">\n");
                                prompt.push_str(&crate::agent_wire::clip_utf8(
                                    &text,
                                    TOOL_RESULT_CHARS,
                                ));
                                prompt.push_str("\n</tool_result>\nContinue.\n");
                                prompt =
                                    crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                                continue;
                            }
                            Some(msg) => {
                                let error = exec_result_text(&msg);
                                let (completed, _) =
                                    complete_edit_error(run.workspace.as_deref(), tool, &error);
                                emit(agent_wire::server_tool_completed(
                                    &tool.id,
                                    completed,
                                    &model_call_id,
                                ));
                                blob_tool_result(
                                    &mut emit,
                                    &mut kv_id,
                                    &mut blob_ids,
                                    &tool.name,
                                    &error,
                                );
                                emit_run_checkpoint(
                                    &mut emit,
                                    run,
                                    estimate_tokens(&prompt) as u32,
                                    max_tokens,
                                    &blob_ids,
                                    &prompt,
                                    &session_todos,
                                    &pending_assistant_json(&turn.thinking, &turn.text),
                                    &summary_archives,
            &track,
                                );
                                prompt.push_str(&format!(
                                    "\n\n<tool_result name=\"{}\">{}</tool_result>\n",
                                    tool.name,
                                    crate::agent_wire::clip_utf8(&error, TOOL_RESULT_CHARS)
                                ));
                                prompt =
                                    crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                                continue;
                            }
                            None => {
                                if run_cancelled(run) {
                                    emit(agent_wire::exec_server_abort(exec_id));
                                    emit(text_frame("Cancelled."));
                                    emit(turn_ended_usage(&turn.usage, &prompt));
                                    return Err("cancelled".into());
                                }
                                emit(text_frame("Edit write timed out waiting for Cursor host."));
                                emit(turn_ended_usage(&turn.usage, &prompt));
                                return Err("edit write timeout".into());
                            }
                        }
                    }
                    Err(error) => {
                        let (completed, _) =
                            complete_edit_error(run.workspace.as_deref(), tool, &error);
                        emit(agent_wire::server_tool_completed(
                            &tool.id,
                            completed,
                            &model_call_id,
                        ));
                        blob_tool_result(
                            &mut emit,
                            &mut kv_id,
                            &mut blob_ids,
                            &tool.name,
                            &error,
                        );
                        emit_run_checkpoint(
                            &mut emit,
                            run,
                            estimate_tokens(&prompt) as u32,
                            max_tokens,
                            &blob_ids,
                            &prompt,
                            &session_todos,
                            &pending_assistant_json(&turn.thinking, &turn.text),
                            &summary_archives,
            &track,
                        );
                        prompt.push_str(&format!(
                            "\n\n<tool_result name=\"{}\">{}</tool_result>\n",
                            tool.name,
                            crate::agent_wire::clip_utf8(&error, TOOL_RESULT_CHARS)
                        ));
                        prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                        continue;
                    }
                }
            } else if kind == CursorTool::Write {
                let before = match read_for_edit(&exec, true) {
                    Ok(before) => before,
                    Err(error) => {
                        let (completed, _) =
                            complete_edit_error(run.workspace.as_deref(), tool, &error);
                        emit(agent_wire::server_tool_completed(
                            &tool.id,
                            completed,
                            &model_call_id,
                        ));
                        blob_tool_result(
                            &mut emit,
                            &mut kv_id,
                            &mut blob_ids,
                            &tool.name,
                            &error,
                        );
                        emit_run_checkpoint(
                            &mut emit,
                            run,
                            estimate_tokens(&prompt) as u32,
                            max_tokens,
                            &blob_ids,
                            &prompt,
                            &session_todos,
                            &pending_assistant_json(&turn.thinking, &turn.text),
                            &summary_archives,
            &track,
                        );
                        prompt.push_str(&format!(
                            "\n\n<tool_result name=\"{}\">{}</tool_result>\n",
                            tool.name,
                            crate::agent_wire::clip_utf8(&error, TOOL_RESULT_CHARS)
                        ));
                        prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                        continue;
                    }
                };
                let after = normalize_cursor_nl(&json_str(
                    &tool.arguments,
                    &["contents", "content", "file_text"],
                ));
                let write = exec_server_message::Message::WriteArgs(WriteArgs {
                    path: abs_path(
                        run.workspace.as_deref(),
                        &json_str(&tool.arguments, &["path"]),
                    ),
                    file_text: after.clone(),
                    tool_call_id: tool.id.clone(),
                    return_file_content_after_write: false,
                    file_bytes: Vec::new(),
                });
                emit(agent_wire::server_exec(
                    exec_id,
                    &format!("{}-write", tool.id),
                    write,
                ));
                let write_exec =
                    wait_exec_heartbeat(&mut wait_exec, exec_id, &mut emit, EXEC_TIMEOUT).await;
                exec_id += 1;
                match write_exec {
                    Some(msg) if write_exec_success(&msg) => {
                        let (completed, text) = complete_tool_call(
                            run.workspace.as_deref(),
                            tool,
                            &msg,
                            Some(&after),
                            Some(&before),
                        );
                        emit(agent_wire::server_tool_completed(
                            &tool.id,
                            completed,
                            &model_call_id,
                        ));
                        blob_tool_result(
                            &mut emit,
                            &mut kv_id,
                            &mut blob_ids,
                            &tool.name,
                            &text,
                        );
                        emit_run_checkpoint(
                            &mut emit,
                            run,
                            estimate_tokens(&prompt) as u32,
                            max_tokens,
                            &blob_ids,
                            &prompt,
                            &session_todos,
                            &pending_assistant_json(&turn.thinking, &turn.text),
                            &summary_archives,
            &track,
                        );
                        prompt.push_str("\n\n<tool_result name=\"");
                        prompt.push_str(&tool.name);
                        prompt.push_str("\">\n");
                        prompt.push_str(&crate::agent_wire::clip_utf8(&text, TOOL_RESULT_CHARS));
                        prompt.push_str("\n</tool_result>\nContinue.\n");
                        prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                        continue;
                    }
                    Some(msg) => {
                        let error = exec_result_text(&msg);
                        let (completed, _) =
                            complete_edit_error(run.workspace.as_deref(), tool, &error);
                        emit(agent_wire::server_tool_completed(
                            &tool.id,
                            completed,
                            &model_call_id,
                        ));
                        blob_tool_result(
                            &mut emit,
                            &mut kv_id,
                            &mut blob_ids,
                            &tool.name,
                            &error,
                        );
                        emit_run_checkpoint(
                            &mut emit,
                            run,
                            estimate_tokens(&prompt) as u32,
                            max_tokens,
                            &blob_ids,
                            &prompt,
                            &session_todos,
                            &pending_assistant_json(&turn.thinking, &turn.text),
                            &summary_archives,
            &track,
                        );
                        prompt.push_str(&format!(
                            "\n\n<tool_result name=\"{}\">{}</tool_result>\n",
                            tool.name,
                            crate::agent_wire::clip_utf8(&error, TOOL_RESULT_CHARS)
                        ));
                        prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                        continue;
                    }
                    None => {
                        if run_cancelled(run) {
                            emit(agent_wire::exec_server_abort(exec_id));
                            emit(text_frame("Cancelled."));
                            emit(turn_ended_usage(&turn.usage, &prompt));
                            return Err("cancelled".into());
                        }
                        emit(text_frame("Write timed out waiting for Cursor host."));
                        emit(turn_ended_usage(&turn.usage, &prompt));
                        return Err("write timeout".into());
                    }
                }
            } else if kind == CursorTool::Shell {
                let transcript = collect_shell_output(
                    &exec,
                    &mut wait_exec,
                    exec.id,
                    &tool.id,
                    &model_call_id,
                    &mut emit,
                )
                .await;
                let (completed, result_text) =
                    complete_shell_tool(run.workspace.as_deref(), tool, &transcript);
                let shell_for_model = if result_text.chars().count() > SHELL_OUTPUT_FULL_MAX {
                    crate::agent_wire::clip_utf8_head_tail(
                        &result_text,
                        SHELL_OUTPUT_SIDE,
                        SHELL_OUTPUT_SIDE,
                    )
                } else {
                    result_text
                };
                emit(agent_wire::server_tool_completed(
                    &tool.id,
                    completed,
                    &model_call_id,
                ));
                blob_tool_result(
                    &mut emit,
                    &mut kv_id,
                    &mut blob_ids,
                    &tool.name,
                    &shell_for_model,
                );
                emit_run_checkpoint(
                    &mut emit,
                    run,
                    estimate_tokens(&prompt) as u32,
                    max_tokens,
                    &blob_ids,
                    &prompt,
                    &session_todos,
                    &pending_assistant_json(&turn.thinking, &turn.text),
                    &summary_archives,
            &track,
                );
                prompt.push_str("\n\n<tool_result name=\"");
                prompt.push_str(&tool.name);
                prompt.push_str("\">\n");
                prompt.push_str(&shell_for_model);
                prompt.push_str("\n</tool_result>\nContinue.\n");
                prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
                continue;
            } else {
                exec_result_text(&exec)
            };
            let (completed, _) = complete_tool_call_ex(
                run.workspace.as_deref(),
                tool,
                &exec,
                None,
                None,
                run.supports_mcp_auth,
            );
            note_tool_checkpoint(&mut track, run, kind, tool, &result_text);
            emit(agent_wire::server_tool_completed(
                &tool.id,
                completed,
                &model_call_id,
            ));
            blob_tool_result(
                &mut emit,
                &mut kv_id,
                &mut blob_ids,
                &tool.name,
                &result_text,
            );
            emit_run_checkpoint(
                &mut emit,
                run,
                estimate_tokens(&prompt) as u32,
                max_tokens,
                &blob_ids,
                &prompt,
                &session_todos,
                &pending_assistant_json(&turn.thinking, &turn.text),
                &summary_archives,
            &track,
            );
            prompt.push_str("\n\n<tool_result name=\"");
            prompt.push_str(&tool.name);
            prompt.push_str("\">\n");
            prompt.push_str(&crate::agent_wire::clip_utf8(
                &result_text,
                TOOL_RESULT_CHARS,
            ));
            prompt.push_str("\n</tool_result>\nContinue.\n");
            prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
        }
        for (task_exec_id, tool, model_call_id) in pending_tasks {
            let exec = wait_exec_heartbeat(&mut wait_exec, task_exec_id, &mut emit, TASK_TIMEOUT)
                .await;
            let exec = match exec {
                Some(exec) => exec,
                None => {
                    if run_cancelled(run) {
                        emit(agent_wire::exec_server_abort(task_exec_id));
                        emit(text_frame("Cancelled."));
                        emit(turn_ended_usage(&turn.usage, &prompt));
                        return Err("cancelled".into());
                    }
                    emit(text_frame(&format!(
                        "Tool {} timed out waiting for Cursor host.",
                        tool.name
                    )));
                    emit(turn_ended_usage(&turn.usage, &prompt));
                    return Err(format!("exec timeout for {}", tool.name));
                }
            };
            let result_text = exec_result_text(&exec);
            let (completed, _) = complete_tool_call_ex(
                run.workspace.as_deref(),
                &tool,
                &exec,
                None,
                None,
                run.supports_mcp_auth,
            );
            note_tool_checkpoint(&mut track, run, CursorTool::Task, &tool, &result_text);
            emit(agent_wire::server_tool_completed(
                &tool.id,
                completed,
                &model_call_id,
            ));
            blob_tool_result(
                &mut emit,
                &mut kv_id,
                &mut blob_ids,
                &tool.name,
                &result_text,
            );
            emit_run_checkpoint(
                &mut emit,
                run,
                estimate_tokens(&prompt) as u32,
                max_tokens,
                &blob_ids,
                &prompt,
                &session_todos,
                &pending_assistant_json(&turn.thinking, &turn.text),
                &summary_archives,
            &track,
            );
            prompt.push_str("\n\n<tool_result name=\"");
            prompt.push_str(&tool.name);
            prompt.push_str("\">\n");
            prompt.push_str(&crate::agent_wire::clip_utf8(
                &result_text,
                TOOL_RESULT_CHARS,
            ));
            prompt.push_str("\n</tool_result>\nContinue.\n");
            prompt = crate::agent_wire::budget_keep_user(&prompt, prompt_budget);
        }
    }
    emit(text_frame("Stopped after too many tool rounds."));
    emit(turn_ended_usage(&TokenUsage::default(), &prompt));
    Ok(())
}

async fn wait_exec_heartbeat<W, Wf, E>(
    wait_exec: &mut W,
    id: u32,
    emit: &mut E,
    timeout: Duration,
) -> Option<ExecClientMessage>
where
    W: FnMut(u32) -> Wf,
    Wf: Future<Output = Option<ExecClientMessage>>,
    E: FnMut(AgentServerMessage),
{
    let fut = wait_exec(id);
    tokio::pin!(fut);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        let slice = left.min(Duration::from_millis(HEARTBEAT_MS));
        tokio::select! {
            result = &mut fut => return result,
            _ = tokio::time::sleep(slice) => {
                emit(heartbeat_frame());
            }
        }
    }
}

fn emit_shell_stream_delta<E>(
    msg: &ExecClientMessage,
    call_id: &str,
    model_call_id: &str,
    emit: &mut E,
) where
    E: FnMut(AgentServerMessage),
{
    if let Some(exec_client_message::Message::ShellStream(stream)) = &msg.message {
        match &stream.event {
            Some(shell_stream::Event::Stdout(out)) if !out.data.is_empty() => {
                emit(agent_wire::server_shell_stdout_delta(
                    call_id,
                    &out.data,
                    model_call_id,
                ));
            }
            Some(shell_stream::Event::Stderr(err)) if !err.data.is_empty() => {
                emit(agent_wire::server_shell_stderr_delta(
                    call_id,
                    &err.data,
                    model_call_id,
                ));
            }
            _ => {}
        }
    }
}

async fn collect_shell_output<W, Wf, E>(
    first: &ExecClientMessage,
    wait_exec: &mut W,
    id: u32,
    call_id: &str,
    model_call_id: &str,
    emit: &mut E,
) -> ShellTranscript
where
    W: FnMut(u32) -> Wf,
    Wf: Future<Output = Option<ExecClientMessage>>,
    E: FnMut(AgentServerMessage),
{
    let mut transcript = ShellTranscript::default();
    let mut push = |msg: &ExecClientMessage, emit: &mut E| {
        emit_shell_stream_delta(msg, call_id, model_call_id, emit);
        apply_shell_exec(&mut transcript, msg);
        transcript.finished
    };
    if push(first, emit) {
        return transcript;
    }
    loop {
        match wait_exec_heartbeat(wait_exec, id, emit, EXEC_TIMEOUT).await {
            Some(msg) => {
                if push(&msg, emit) {
                    return transcript;
                }
            }
            None => return transcript,
        }
    }
}

/// Concatenate Connect frames for a finished injected run (tests + unary callers).
pub fn encode_agent_frames(messages: &[AgentServerMessage], error: Option<&str>) -> Vec<u8> {
    let mut out = Vec::new();
    for message in messages {
        out.extend_from_slice(&agent_wire::encode_server(message));
    }
    if let Some(error) = error {
        out.extend_from_slice(&agent_wire::encode_end_stream_error("unavailable", error));
    } else {
        out.extend_from_slice(&agent_wire::encode_end_stream_ok());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_proto::{
        exec_client_message, read_result, read_success, ExecClientMessage, GrepContentMatch,
        GrepContentResult, GrepFileMatch, GrepSuccess, GrepUnionResult, ReadResult, ReadSuccess,
    };
    use crate::agent_wire::{self, decode_connect_server_messages};
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    fn read_exec(id: u32, content: &str) -> ExecClientMessage {
        ExecClientMessage {
            id,
            exec_id: "call_1-exec".into(),
            message: Some(exec_client_message::Message::ReadResult(ReadResult {
                result: Some(read_result::Result::Success(ReadSuccess {
                    path: "README.md".into(),
                    total_lines: content.lines().count() as i32,
                    file_size: content.len() as i64,
                    truncated: false,
                    range_applied: false,
                    output: Some(read_success::Output::Content(content.into())),
                })),
            })),
        }
    }

    fn grep_exec(id: u32, needle: &str) -> ExecClientMessage {
        let mut workspace_results = std::collections::HashMap::new();
        workspace_results.insert(
            "ws".into(),
            GrepUnionResult {
                result: Some(grep_union_result::Result::Content(GrepContentResult {
                    matches: vec![GrepFileMatch {
                        file: "lib.rs".into(),
                        matches: vec![GrepContentMatch {
                            line_number: 3,
                            content: needle.into(),
                        }],
                    }],
                    total_lines: 1,
                    total_matched_lines: 1,
                })),
            },
        );
        ExecClientMessage {
            id,
            exec_id: "call_g-exec".into(),
            message: Some(exec_client_message::Message::GrepResult(GrepResult {
                result: Some(grep_result::Result::Success(GrepSuccess {
                    pattern: "needle".into(),
                    path: ".".into(),
                    output_mode: "content".into(),
                    workspace_results,
                    active_editor_result: None,
                })),
            })),
        }
    }

    #[test]
    fn write_complete_keeps_host_file_content() {
        let tool = ToolUse {
            id: "call_w".into(),
            name: "Write".into(),
            arguments: json!({"path": "a.rs", "contents": "ignored"}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "w-exec".into(),
            message: Some(exec_client_message::Message::WriteResult(
                crate::agent_proto::WriteResult {
                    result: Some(crate::agent_proto::write_result::Result::Success(
                        crate::agent_proto::WriteSuccess {
                            path: "a.rs".into(),
                            lines_created: 1,
                            file_size: 11,
                            file_content_after_write: Some("AFTER_BODY_ZX".into()),
                        },
                    )),
                },
            )),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(text.contains("AFTER_BODY_ZX"));
        match completed.tool {
            Some(tool_call::Tool::EditToolCall(edit)) => match edit.result.and_then(|r| r.result) {
                Some(crate::agent_proto::edit_result::Result::Success(ok)) => {
                    assert_eq!(ok.after_full_file_content, "AFTER_BODY_ZX");
                    assert!(ok.lines_added.is_some());
                    assert!(ok.diff_string.is_some());
                }
                other => panic!("expected edit success, got {other:?}"),
            },
            other => panic!("expected edit tool call, got {other:?}"),
        }
    }

    #[test]
    fn read_complete_maps_host_truncated_flag() {
        let tool = ToolUse {
            id: "call_r".into(),
            name: "Read".into(),
            arguments: json!({"path": "big.rs"}),
        };
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "r-exec".into(),
            message: Some(exec_client_message::Message::ReadResult(ReadResult {
                result: Some(read_result::Result::Success(ReadSuccess {
                    path: "big.rs".into(),
                    total_lines: 400,
                    file_size: 9000,
                    truncated: true,
                    range_applied: true,
                    output: Some(read_success::Output::Content("HEAD_ONLY".into())),
                })),
            })),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(text.contains("HEAD_ONLY"));
        match completed.tool {
            Some(tool_call::Tool::ReadToolCall(read)) => match read.result.and_then(|r| r.result) {
                Some(read_tool_result::Result::Success(ok)) => {
                    assert!(ok.exceeded_limit, "host truncated must set exceeded_limit");
                    assert_eq!(ok.total_lines, 400);
                    assert_eq!(ok.file_size, 9000);
                    assert_eq!(
                        ok.output,
                        Some(read_tool_success::Output::Content("HEAD_ONLY".into()))
                    );
                }
                other => panic!("expected read success, got {other:?}"),
            },
            other => panic!("expected read tool, got {other:?}"),
        }
    }

    #[test]
    fn read_host_error_is_not_success() {
        let tool = ToolUse {
            id: "call_r".into(),
            name: "Read".into(),
            arguments: json!({"path": "missing.rs"}),
        };
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "r-exec".into(),
            message: Some(exec_client_message::Message::ReadResult(ReadResult {
                result: Some(read_result::Result::FileNotFound(
                    crate::agent_proto::ReadFileNotFound {
                        path: "missing.rs".into(),
                    },
                )),
            })),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(text.contains("missing.rs"));
        match completed.tool {
            Some(tool_call::Tool::ReadToolCall(read)) => match read.result.and_then(|r| r.result) {
                Some(read_tool_result::Result::Error(err)) => {
                    assert!(
                        err.error_message.contains("not found"),
                        "{}",
                        err.error_message
                    );
                }
                other => panic!("expected read error, got {other:?}"),
            },
            other => panic!("expected read tool, got {other:?}"),
        }
    }

    #[test]
    fn write_host_error_is_edit_error() {
        let tool = ToolUse {
            id: "call_w".into(),
            name: "Write".into(),
            arguments: json!({"path": "a.rs", "contents": "x"}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "w-exec".into(),
            message: Some(exec_client_message::Message::WriteResult(
                crate::agent_proto::WriteResult {
                    result: Some(write_result::Result::Error(
                        crate::agent_proto::WriteError {
                            path: "a.rs".into(),
                            error: "disk full ZX".into(),
                        },
                    )),
                },
            )),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(text.contains("disk full ZX"));
        match completed.tool {
            Some(tool_call::Tool::EditToolCall(edit)) => match edit.result.and_then(|r| r.result) {
                Some(edit_result::Result::Error(err)) => {
                    assert!(err.error.contains("disk full ZX"), "{}", err.error);
                }
                other => panic!("expected edit error, got {other:?}"),
            },
            other => panic!("expected edit tool, got {other:?}"),
        }
    }

    #[test]
    fn write_success_without_host_body_uses_args_contents() {
        let tool = ToolUse {
            id: "call_w".into(),
            name: "Write".into(),
            arguments: json!({"path": "a.rs", "contents": "ARGS_BODY_ZX"}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "w-exec".into(),
            message: Some(exec_client_message::Message::WriteResult(
                crate::agent_proto::WriteResult {
                    result: Some(write_result::Result::Success(
                        crate::agent_proto::WriteSuccess {
                            path: "a.rs".into(),
                            lines_created: 1,
                            file_size: 12,
                            file_content_after_write: None,
                        },
                    )),
                },
            )),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert_eq!(text, "ARGS_BODY_ZX");
        assert!(!text.contains("wrote "));
        match completed.tool {
            Some(tool_call::Tool::EditToolCall(edit)) => match edit.result.and_then(|r| r.result) {
                Some(edit_result::Result::Success(ok)) => {
                    assert_eq!(ok.after_full_file_content, "ARGS_BODY_ZX");
                }
                other => panic!("expected edit success, got {other:?}"),
            },
            other => panic!("expected edit tool, got {other:?}"),
        }
    }

    #[test]
    fn read_wrong_exec_kind_is_error() {
        let tool = ToolUse {
            id: "call_r".into(),
            name: "Read".into(),
            arguments: json!({"path": "a.rs"}),
        };
        let exec = grep_exec(1, "not-a-read");
        let (completed, _) = complete_tool_call(Some("."), &tool, &exec, None, None);
        match completed.tool {
            Some(tool_call::Tool::ReadToolCall(read)) => match read.result.and_then(|r| r.result) {
                Some(read_tool_result::Result::Error(_)) => {}
                other => panic!("expected read error, got {other:?}"),
            },
            other => panic!("expected read tool, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn loop_edit_refuses_truncated_host_read() {
        let run = LocalRun {
            request_id: "req-edit-trunc".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "edit a.rs".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_e".into(),
                    name: "Edit".into(),
                    arguments: json!({
                        "path": "a.rs",
                        "old_string": "fn main() {}",
                        "new_string": "fn main() { 1 }"
                    }),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "could not apply truncated".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let wait = move |_id: u32| {
            let msg = ExecClientMessage {
                id: 1,
                exec_id: "r-exec".into(),
                message: Some(exec_client_message::Message::ReadResult(ReadResult {
                    result: Some(read_result::Result::Success(ReadSuccess {
                        path: "a.rs".into(),
                        total_lines: 400,
                        file_size: 9000,
                        truncated: true,
                        range_applied: false,
                        output: Some(read_success::Output::Content("fn main() {}\n".into())),
                    })),
                })),
            };
            async move { Some(msg) }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("truncated edit loop");
        match completed_edit_case(&frames) {
            Some(edit_result::Result::Error(err)) => {
                assert!(err.error.contains("truncated"), "{}", err.error);
            }
            other => panic!("expected EditError, got {other:?}"),
        }
        assert!(prompts.borrow()[1].contains("truncated"));
    }

    #[test]
    fn grep_missing_host_result_is_error() {
        let tool = ToolUse {
            id: "call_g".into(),
            name: "Grep".into(),
            arguments: json!({"pattern": "x"}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "g-exec".into(),
            message: None,
        };
        let (completed, _) = complete_tool_call(Some("."), &tool, &exec, None, None);
        match completed.tool {
            Some(tool_call::Tool::GrepToolCall(grep)) => match grep.result.and_then(|r| r.result) {
                Some(grep_result::Result::Error(err)) => {
                    assert!(err.error.contains("no result"), "{}", err.error);
                }
                other => panic!("expected grep error, got {other:?}"),
            },
            other => panic!("expected grep tool, got {other:?}"),
        }
    }

    #[test]
    fn ls_missing_host_result_is_error() {
        let tool = ToolUse {
            id: "call_l".into(),
            name: "Ls".into(),
            arguments: json!({"path": "."}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "l-exec".into(),
            message: None,
        };
        let (completed, _) = complete_tool_call(Some("."), &tool, &exec, None, None);
        match completed.tool {
            Some(tool_call::Tool::LsToolCall(ls)) => match ls.result.and_then(|r| r.result) {
                Some(ls_result::Result::Error(err)) => {
                    assert!(err.error.contains("no result"), "{}", err.error);
                }
                other => panic!("expected ls error, got {other:?}"),
            },
            other => panic!("expected ls tool, got {other:?}"),
        }
    }

    #[test]
    fn glob_host_error_is_not_empty_success() {
        let tool = ToolUse {
            id: "call_g".into(),
            name: "Glob".into(),
            arguments: json!({"glob_pattern": "*.rs", "target_directory": "."}),
        };
        let exec = ExecClientMessage {
            id: 2,
            exec_id: "g-exec".into(),
            message: Some(exec_client_message::Message::GrepResult(GrepResult {
                result: Some(grep_result::Result::Error(crate::agent_proto::GrepError {
                    error: "GLOB_FAIL_ZX".into(),
                })),
            })),
        };
        let (completed, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(text.contains("GLOB_FAIL_ZX"));
        match completed.tool {
            Some(tool_call::Tool::GlobToolCall(glob)) => match glob.result.and_then(|r| r.result) {
                Some(glob_tool_result::Result::Error(err)) => {
                    assert!(err.error.contains("GLOB_FAIL_ZX"), "{}", err.error);
                }
                other => panic!("expected glob error, got {other:?}"),
            },
            other => panic!("expected glob tool, got {other:?}"),
        }
    }

    #[test]
    fn edit_apply_normalizes_crlf_from_host() {
        let before = normalize_cursor_nl("hello\r\nworld\r\n");
        let after = apply_edit(&before, "hello\nworld", "hello\nthere", false)
            .expect("crlf-normalized host text must match LF old_string");
        assert_eq!(after, "hello\nthere\n");
        let after_needles = apply_edit(&before, "hello\r\nworld", "hello\r\nthere", false)
            .expect("CRLF needles must match LF host text");
        assert_eq!(after_needles, "hello\nthere\n");
    }

    #[test]
    fn toolkit_prompt_inlines_small_attached_files() {
        let run = LocalRun {
            user_text: "look at foo".into(),
            workspace: Some(r"C:\ws".into()),
            os_version: "win32".into(),
            shell: "powershell".into(),
            attached_files: vec![("foo.rs".into(), "fn main() {}".into())],
            history: "User:\nPREV_TURN_ZX9\n\nAssistant:\nok\n".into(),
            ..Default::default()
        };
        let prompt = toolkit_prompt(&run);
        assert!(prompt.contains("<user_info>"));
        assert!(prompt.contains("OS Version: win32"));
        assert!(prompt.contains("<attached_files"));
        assert!(prompt.contains("fn main() {}"));
        let turns = prompt_to_turns(&prompt);
        assert!(
            turns
                .iter()
                .any(|turn| turn.role == "user" && turn.text.contains("PREV_TURN_ZX9")),
            "history user must be its own turn: {turns:?}"
        );
        assert!(turns.iter().any(|turn| turn.role == "assistant"));
    }

    #[test]
    fn shell_started_args_use_background_timeout_behavior() {
        let args = shell_args("git status", ".", "call_sh", "status");
        assert_eq!(args.timeout_behavior, SHELL_TIMEOUT_BEHAVIOR_BACKGROUND);
        assert_eq!(args.hard_timeout, Some(SHELL_HARD_TIMEOUT_MS));
        assert_eq!(
            args.file_output_threshold_bytes,
            Some(SHELL_FILE_OUTPUT_THRESHOLD)
        );
        assert!(!args.close_stdin);
    }

    #[test]
    fn read_binary_data_is_marker_not_lossy_utf8() {
        let tool = ToolUse {
            id: "call_r".into(),
            name: "Read".into(),
            arguments: json!({"path": "a.png"}),
        };
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "r-exec".into(),
            message: Some(exec_client_message::Message::ReadResult(ReadResult {
                result: Some(read_result::Result::Success(ReadSuccess {
                    path: "a.png".into(),
                    total_lines: 0,
                    file_size: 4,
                    truncated: false,
                    range_applied: false,
                    output: Some(read_success::Output::Data(vec![0xff, 0xd8, 0x00, 0x00])),
                })),
            })),
        };
        let (_, text) = complete_tool_call(Some("."), &tool, &exec, None, None);
        assert!(
            text.contains("[Binary file: a.png"),
            "binary read must not be lossy UTF-8: {text}"
        );
    }

    #[test]
    fn abs_path_clamps_outside_workspace() {
        let inside = abs_path(Some(r"C:\ws"), r"C:\ws\src\lib.rs");
        assert!(
            inside.to_ascii_lowercase().contains("src"),
            "{inside}"
        );
        let outside = abs_path(Some(r"C:\ws"), r"C:\Windows\notepad.exe");
        assert!(
            outside.contains(".gba-denied"),
            "absolute escape must not be forwarded: {outside}"
        );
        let dots = abs_path(Some(r"C:\ws"), r"..\..\Windows\notepad.exe");
        assert!(
            dots.contains(".gba-denied"),
            "relative escape must not be forwarded: {dots}"
        );
    }

    #[test]
    fn outbound_turns_keeps_user_after_compact() {
        let run = LocalRun {
            user_text: "KEEP_USER_ZX9 please".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut prompt = toolkit_prompt(&run);
        prompt.push_str("\n\n<tool_result name=\"Read\">");
        prompt.push_str(&"PAD ".repeat(80_000));
        prompt.push_str("</tool_result>\nContinue.\n");
        let small = outbound_turns(&prompt, 8_000);
        let large = outbound_turns(&prompt, 256_000);
        assert!(
            small
                .iter()
                .any(|turn| turn.role == "user" && turn.text.contains("KEEP_USER_ZX9")),
            "user turn survived compact: {small:?}"
        );
        let small_n = crate::compact::total_chars(&small);
        let large_n = crate::compact::total_chars(&large);
        assert!(
            small_n < large_n,
            "8k window {small_n} must compact harder than 256k {large_n}"
        );
    }

    #[test]
    fn prompt_char_budget_scales_with_official_windows() {
        let tiny = prompt_char_budget(8_000);
        let mid = prompt_char_budget(32_000);
        let grok = prompt_char_budget(256_000);
        assert!(tiny < mid && mid < grok, "{tiny} < {mid} < {grok}");
        assert_eq!(prompt_char_budget(0), prompt_char_budget(256_000));
        assert!(grok > 180_000, "256k Stream must not use the old 180k cap");
    }

    fn completed_edit_case(frames: &[AgentServerMessage]) -> Option<edit_result::Result> {
        frames
            .iter()
            .rev()
            .find_map(|message| match &message.message {
                Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                    message: Some(interaction_update::Message::ToolCallCompleted(update)),
                })) => match &update.tool_call.as_ref()?.tool {
                    Some(tool_call::Tool::EditToolCall(edit)) => {
                        edit.result.as_ref()?.result.clone()
                    }
                    _ => None,
                },
                _ => None,
            })
    }

    #[tokio::test]
    async fn loop_edit_apply_fail_emits_error_not_success() {
        let run = LocalRun {
            request_id: "req-edit".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "edit a.rs".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_e".into(),
                    name: "Edit".into(),
                    arguments: json!({
                        "path": "a.rs",
                        "old_string": "DOES_NOT_EXIST_ZX",
                        "new_string": "x"
                    }),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "could not apply".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let wait = move |_id: u32| {
            let msg = read_exec(1, "fn main() {}\n");
            async move { Some(msg) }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("edit fail loop");
        match completed_edit_case(&frames) {
            Some(edit_result::Result::Error(err)) => {
                assert!(
                    err.error.contains("DOES_NOT_EXIST_ZX")
                        || err.error.contains("not found")
                        || err.error.contains("String to replace"),
                    "{}",
                    err.error
                );
            }
            other => panic!("expected EditError, got {other:?}"),
        }
        assert!(prompts.borrow()[1].contains("DOES_NOT_EXIST_ZX"));
        assert!(!prompts.borrow()[1].contains("fn main() {}\nContinue"));
    }

    #[tokio::test]
    async fn loop_edit_write_fail_emits_error_not_success() {
        let run = LocalRun {
            request_id: "req-edit-w".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "edit a.rs".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_e".into(),
                    name: "Edit".into(),
                    arguments: json!({
                        "path": "a.rs",
                        "old_string": "fn main() {}",
                        "new_string": "fn main() { 1 }"
                    }),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "write failed".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let mut execs = VecDeque::from([
            read_exec(1, "fn main() {}\n"),
            ExecClientMessage {
                id: 2,
                exec_id: "e-write".into(),
                message: Some(exec_client_message::Message::WriteResult(
                    crate::agent_proto::WriteResult {
                        result: Some(write_result::Result::Error(
                            crate::agent_proto::WriteError {
                                path: "a.rs".into(),
                                error: "WRITE_DENIED_ZX".into(),
                            },
                        )),
                    },
                )),
            },
        ]);
        let wait = move |_id: u32| {
            let next = execs.pop_front();
            async move { next }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("edit write fail loop");
        match completed_edit_case(&frames) {
            Some(edit_result::Result::Error(err)) => {
                assert!(err.error.contains("WRITE_DENIED_ZX"), "{}", err.error);
            }
            other => panic!("expected EditError, got {other:?}"),
        }
        assert!(prompts.borrow()[1].contains("WRITE_DENIED_ZX"));
    }

    #[test]
    fn shipped_encoder_emits_started_exec_completed_tags() {
        let read = ToolUse {
            id: "call_1".into(),
            name: "Read".into(),
            arguments: json!({"path": "README.md"}),
        };
        let grep = ToolUse {
            id: "call_2".into(),
            name: "Grep".into(),
            arguments: json!({"pattern": "needle", "path": "."}),
        };
        let read_preview = encode_tool_preview_frames(&read, "m-1").expect("read preview");
        let (read_frames, _) =
            encode_tool_start_frames(Some("."), &read, 1, "m-1").expect("read frames");
        let read_kinds: Vec<_> = read_preview
            .iter()
            .chain(read_frames.iter())
            .filter_map(agent_wire::frame_kind)
            .collect();
        let (grep_frames, _) =
            encode_tool_start_frames(Some("."), &grep, 2, "m-2").expect("grep frames");
        let exec = read_exec(1, "GROUND_TOKEN_ZX9");
        let (completed, text) = complete_tool_call(Some("."), &read, &exec, None, None);
        let mut messages = Vec::new();
        messages.extend(read_preview);
        messages.extend(read_frames);
        messages.extend(grep_frames);
        messages.push(agent_wire::server_tool_completed(
            "call_1", completed, "m-1",
        ));
        messages.push(text_frame(&format!("saw {text}")));
        let bytes = encode_agent_frames(&messages, None);
        let decoded = decode_connect_server_messages(&bytes);
        assert!(
            agent_wire::has_tool_call_started(&decoded),
            "tool_call_started field 2 missing"
        );
        assert!(
            agent_wire::has_partial_tool_call(&decoded),
            "CCursor emits partialToolCall before toolCallStarted"
        );
        assert!(
            agent_wire::has_exec_server(&decoded),
            "exec_server_message field 2 missing"
        );
        assert_eq!(
            read_kinds,
            ["partial", "started", "exec"],
            "official Read order is partial → started → exec"
        );
        let edit = ToolUse {
            id: "call_e".into(),
            name: "Edit".into(),
            arguments: json!({"path": "a.rs", "old_string": "a", "new_string": "b"}),
        };
        let edit_preview = encode_tool_preview_frames(&edit, "m-e").expect("edit preview");
        let (edit_frames, _) =
            encode_tool_start_frames(Some("."), &edit, 3, "m-e").expect("edit frames");
        let edit_kinds: Vec<_> = edit_preview
            .iter()
            .chain(edit_frames.iter())
            .filter_map(agent_wire::frame_kind)
            .collect();
        assert_eq!(
            edit_kinds,
            ["partial", "delta", "started", "exec"],
            "official Edit is empty partial + editToolCallDelta then started(path) then readArgs"
        );
        assert!(
            agent_wire::has_tool_call_completed(&decoded),
            "tool_call_completed field 3 missing"
        );
        assert!(agent_wire::text_deltas(&decoded).contains("GROUND_TOKEN_ZX9"));
        let only_chat = agent_wire::encode_local_completion("think", "hi");
        let chat_decoded = decode_connect_server_messages(&only_chat);
        assert!(!agent_wire::has_tool_call_started(&chat_decoded));
        assert!(!agent_wire::has_exec_server(&chat_decoded));
    }

    #[tokio::test]
    async fn loop_waits_for_exec_before_second_model_call() {
        let run = LocalRun {
            request_id: "req-loop".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "Read README and quote GROUND_TOKEN_ZX9".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let mut replies = VecDeque::from([
            ModelTurn {
                thinking: "need file".into(),
                text: String::new(),
                tools: vec![ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    arguments: json!({"path": "README.md"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                thinking: String::new(),
                text: "The file contains GROUND_TOKEN_ZX9".into(),
                tools: Vec::new(),
                ..Default::default()
            },
        ]);
        let calls_llm = calls.clone();
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            calls_llm.set(calls_llm.get() + 1);
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let exec_called = Rc::new(Cell::new(false));
        let exec_flag = exec_called.clone();
        let wait = move |id: u32| {
            exec_flag.set(true);
            let msg = read_exec(id, "GROUND_TOKEN_ZX9 from host");
            async move { Some(msg) }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert_eq!(calls.get(), 2, "second model call must happen after exec");
        assert!(exec_called.get(), "must wait for exec_client_message");
        assert!(
            prompts.borrow()[1].contains("GROUND_TOKEN_ZX9 from host"),
            "exec result must be in the second prompt"
        );
        assert!(agent_wire::has_tool_call_started(&frames));
        assert!(agent_wire::has_exec_server(&frames));
        assert!(agent_wire::has_tool_call_completed(&frames));
        assert!(agent_wire::text_deltas(&frames).contains("GROUND_TOKEN_ZX9"));
        let bytes = encode_agent_frames(&frames, None);
        let decoded = decode_connect_server_messages(&bytes);
        assert!(agent_wire::has_tool_call_started(&decoded));
        assert!(agent_wire::has_exec_server(&decoded));
        assert!(agent_wire::has_tool_call_completed(&decoded));
        let kinds: Vec<_> = frames.iter().filter_map(agent_wire::frame_kind).collect();
        let first_ended = kinds.iter().position(|kind| *kind == "turn_ended");
        let last_completed = kinds.iter().rposition(|kind| *kind == "completed");
        assert!(
            first_ended
                .zip(last_completed)
                .is_some_and(|(ended, done)| ended > done),
            "turn_ended must wait until tools are done: {kinds:?}"
        );
        assert!(kinds.contains(&"checkpoint"));
        assert!(kinds.contains(&"partial"));
        let step_at = kinds.iter().position(|kind| *kind == "step_completed");
        let started_at = kinds.iter().position(|kind| *kind == "started");
        let partial_at = kinds.iter().position(|kind| *kind == "partial");
        assert!(
            partial_at
                .zip(step_at)
                .zip(started_at)
                .is_some_and(|((partial, step), started)| partial < step && step < started),
            "CCursor: empty partial during/after LLM, stepCompleted, then started: {kinds:?}"
        );
    }

    #[tokio::test]
    async fn loop_from_history_payload_keeps_prior_and_exec() {
        use crate::agent_wire::{
            agent_client_message, conversation_action, conversation_history_message,
            conversation_history_user_content, local_run_from_agent_payload, AgentClientMessage,
            AgentRunRequest, ConversationAction, ConversationHistory, ConversationHistoryMessage,
            ConversationHistoryText, ConversationHistoryUserContent,
            ConversationHistoryUserMessage, RequestContext, RequestContextEnv, RequestedModel,
            UserMessage, UserMessageAction,
        };
        use prost::Message;
        let history = ConversationHistory {
            messages: vec![ConversationHistoryMessage {
                message: Some(conversation_history_message::Message::User(
                    ConversationHistoryUserMessage {
                        content: vec![ConversationHistoryUserContent {
                            content: Some(conversation_history_user_content::Content::Text(
                                ConversationHistoryText {
                                    text: "PREV_TURN_ZX9".into(),
                                },
                            )),
                        }],
                    },
                )),
            }],
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: "Read README and quote GROUND_TOKEN_ZX9".into(),
                                ..Default::default()
                            }),
                            request_context: Some(RequestContext {
                                env: Some(RequestContextEnv {
                                    os_version: "win32".into(),
                                    workspace_paths: vec![".".into()],
                                    shell: "powershell".into(),
                                    project_folder: ".".into(),
                                    ..Default::default()
                                }),
                                file_contents: std::collections::HashMap::new(),
                                ..Default::default()
                            }),
                            conversation_history: Some(history),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                }),
                requested_model: Some(RequestedModel {
                    model_id: "gb-grok-4.6".into(),
                    max_mode: false,
                    parameters: Vec::new(),
                }),
                ..Default::default()
            })),
        }
        .encode_to_vec();
        let run = local_run_from_agent_payload("req-hist".into(), &payload).expect("decode");
        assert!(run.history.contains("PREV_TURN_ZX9"));
        assert_eq!(run.workspace.as_deref(), Some("."));
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    arguments: json!({"path": "README.md"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "saw GROUND_TOKEN_ZX9".into(),
                ..Default::default()
            },
        ]);
        let calls_llm = calls.clone();
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            calls_llm.set(calls_llm.get() + 1);
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let wait = move |id: u32| {
            let msg = read_exec(id, "GROUND_TOKEN_ZX9 from host");
            async move { Some(msg) }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("history loop");
        assert_eq!(calls.get(), 2);
        assert!(prompts.borrow()[0].contains("PREV_TURN_ZX9"));
        assert!(prompts.borrow()[1].contains("PREV_TURN_ZX9"));
        assert!(prompts.borrow()[1].contains("GROUND_TOKEN_ZX9 from host"));
        let turns = prompt_to_turns(&prompts.borrow()[0]);
        assert!(turns.iter().any(|turn| turn.role == "system"));
        assert!(turns.iter().any(|turn| turn.role == "user"));
        assert!(agent_wire::has_checkpoint(&frames));
        assert!(agent_wire::has_turn_ended(&frames));
    }

    #[test]
    fn parse_model_turn_uses_sse_tools_not_text_xml() {
        let turn = parse_model_turn(
            "think".into(),
            "I'll look at Cargo.toml".into(),
            vec![crate::connect::ChatToolCall {
                id: "call_sse".into(),
                name: "Read".into(),
                arguments: r#"{"path":"Cargo.toml"}"#.into(),
            }],
        );
        assert_eq!(turn.tools.len(), 1);
        assert_eq!(turn.tools[0].name, "Read");
        assert_eq!(turn.tools[0].id, "call_sse");
        assert_eq!(turn.text, "I'll look at Cargo.toml");
        assert_eq!(
            turn.tools[0].arguments.get("path").and_then(|v| v.as_str()),
            Some("Cargo.toml")
        );
        let fallback = parse_model_turn(
            String::new(),
            r#"<tool_call>{"name":"Read","arguments":{"path":"README.md"}}</tool_call>"#.into(),
            vec![crate::connect::ChatToolCall {
                id: "broken".into(),
                name: "Read".into(),
                arguments: "{".into(),
            }],
        );
        assert!(
            fallback.tools.is_empty(),
            "incomplete native JSON must not execute XML: {:?}",
            fallback.tools
        );
    }

    #[test]
    fn prompt_to_turns_splits_system_user_and_tool_results() {
        let run = LocalRun {
            user_text: "check Cargo.toml".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut prompt = toolkit_prompt(&run);
        prompt.push_str("\n\n<tool_result name=\"Read\">ok</tool_result>\nContinue.\n");
        let turns = prompt_to_turns(&prompt);
        assert!(turns.iter().any(|turn| turn.role == "system"));
        assert!(turns
            .iter()
            .any(|turn| turn.role == "user" && turn.text.contains("check Cargo.toml")));
        assert!(turns.iter().any(|turn| turn.text.contains("<tool_result")));
        assert!(
            turns.iter().any(|turn| turn.role == "user" && turn.text.contains("<user_query>")),
            "current user must wrap <user_query>: {turns:?}"
        );
        let preamble = turns.iter().find(|turn| {
            turn.role == "user" && turn.text.contains("<user_info>")
        });
        assert!(preamble.is_some(), "preamble user missing: {turns:?}");
        assert!(
            !turns[0].text.contains("<user_query>"),
            "system must not hold the query"
        );
    }

    #[test]
    fn bind_history_images_skips_current_user_query() {
        let mut turns = vec![
            crate::compact::ChatTurn::new("system", "sys"),
            crate::compact::ChatTurn::new("user", "see [image image/png]"),
            crate::compact::ChatTurn::new("user", "<user_query>\nnow\n</user_query>"),
        ];
        bind_history_images(
            &mut turns,
            &[("image/png".into(), "abc".into())],
        );
        assert_eq!(turns[1].images, vec![("image/png".into(), "abc".into())]);
        assert!(turns[2].images.is_empty());
    }

    #[test]
    fn task_override_fills_model_when_llm_omits_it() {
        let task = ToolUse {
            id: "call_ov".into(),
            name: "Task".into(),
            arguments: json!({
                "description": "explore",
                "prompt": "look",
                "subagent_type": "explore"
            }),
        };
        let (frames, kind) = encode_tool_start_frames_ex(
            None,
            &task,
            1,
            "m-ov",
            Some("conv"),
            false,
            Some("gb-grok-4"),
        )
        .expect("task");
        assert_eq!(kind, CursorTool::Task);
        let exec = frames.iter().find_map(|f| match &f.message {
            Some(agent_wire::agent_server_message::Message::ExecServerMessage(msg)) => {
                msg.message.clone()
            }
            _ => None,
        });
        match exec {
            Some(exec_server_message::Message::SubagentArgs(args)) => {
                assert_eq!(args.model_id, "gb-grok-4");
            }
            other => panic!("expected subagentArgs, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn shell_collect_emits_stdout_tool_call_delta() {
        let first = ExecClientMessage {
            id: 1,
            exec_id: "sh-exec".into(),
            message: Some(exec_client_message::Message::ShellStream(
                crate::agent_proto::ShellStream {
                    event: Some(shell_stream::Event::Stdout(
                        crate::agent_proto::ShellStreamStdout {
                            data: "hello-out".into(),
                        },
                    )),
                },
            )),
        };
        let exit = ExecClientMessage {
            id: 1,
            exec_id: "sh-exec".into(),
            message: Some(exec_client_message::Message::ShellStream(
                crate::agent_proto::ShellStream {
                    event: Some(shell_stream::Event::Exit(
                        crate::agent_proto::ShellStreamExit {
                            code: 0,
                            ..Default::default()
                        },
                    )),
                },
            )),
        };
        let wait = move |_id: u32| {
            let exit = exit.clone();
            async move { Some(exit) }
        };
        let mut wait = wait;
        let mut frames = Vec::new();
        let transcript =
            collect_shell_output(&first, &mut wait, 1, "call_sh", "m-sh", &mut |msg| {
                frames.push(msg)
            })
            .await;
        assert!(transcript.stdout.contains("hello-out"));
        assert_eq!(transcript.exit_code, 0);
        assert!(agent_wire::has_tool_call_delta(&frames));
    }

    fn shell_stdout(id: u32, data: &str) -> ExecClientMessage {
        ExecClientMessage {
            id,
            exec_id: "sh-exec".into(),
            message: Some(exec_client_message::Message::ShellStream(
                crate::agent_proto::ShellStream {
                    event: Some(shell_stream::Event::Stdout(
                        crate::agent_proto::ShellStreamStdout { data: data.into() },
                    )),
                },
            )),
        }
    }

    fn shell_exit(id: u32, code: u32) -> ExecClientMessage {
        ExecClientMessage {
            id,
            exec_id: "sh-exec".into(),
            message: Some(exec_client_message::Message::ShellStream(
                crate::agent_proto::ShellStream {
                    event: Some(shell_stream::Event::Exit(
                        crate::agent_proto::ShellStreamExit {
                            code,
                            cwd: "/ws".into(),
                            ..Default::default()
                        },
                    )),
                },
            )),
        }
    }

    #[tokio::test]
    async fn loop_shell_completed_uses_full_transcript_and_exit() {
        let run = LocalRun {
            request_id: "req-sh".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "run fail-cmd".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_sh".into(),
                    name: "Shell".into(),
                    arguments: json!({"command": "fail-cmd"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "saw the failure".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let mut execs = VecDeque::from([
            shell_stdout(3, "chunk-a"),
            shell_stdout(3, "chunk-b"),
            shell_exit(3, 7),
        ]);
        let wait = move |_id: u32| {
            let next = execs.pop_front();
            async move { next }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("shell loop");
        let kinds: Vec<_> = frames.iter().filter_map(agent_wire::frame_kind).collect();
        let delta_at = kinds.iter().position(|kind| *kind == "delta");
        let completed_at = kinds.iter().position(|kind| *kind == "completed");
        assert!(
            delta_at
                .zip(completed_at)
                .is_some_and(|(delta, done)| delta < done),
            "IU15 stdout must precede completed: {kinds:?}"
        );
        match agent_wire::completed_shell_result(&frames) {
            Some(shell_result::Result::Failure(fail)) => {
                assert_eq!(fail.exit_code, 7);
                assert!(fail.stdout.contains("chunk-a"), "{}", fail.stdout);
                assert!(fail.stdout.contains("chunk-b"), "{}", fail.stdout);
            }
            other => panic!("expected ShellFailure, got {other:?}"),
        }
        assert!(prompts.borrow()[1].contains("chunk-a"));
        assert!(prompts.borrow()[1].contains("chunk-b"));
        assert!(prompts.borrow()[1].contains("exit 7"));
    }

    #[test]
    fn parse_read_and_legacy_xml_are_host_exec_not_disk() {
        let modern = parse_tool_uses(
            r#"<tool_call>{"name":"Read","arguments":{"path":"README.md"}}</tool_call>"#,
        );
        assert_eq!(modern[0].name, "Read");
        let legacy = parse_tool_uses(r#"<tool>{"name":"read_file","path":"README.md"}</tool>"#);
        assert_eq!(map_tool_name(&legacy[0].name), Some(CursorTool::Read));
        let grep = parse_tool_uses(
            r#"<tool_call>{"name":"Grep","arguments":{"pattern":"foo"}}</tool_call>"#,
        );
        assert_eq!(map_tool_name(&grep[0].name), Some(CursorTool::Grep));
    }

    #[test]
    fn grep_exec_text_keeps_host_payload() {
        let exec = grep_exec(2, "GROUND_GREP_QQ");
        assert!(exec_result_text(&exec).contains("GROUND_GREP_QQ"));
    }

    #[tokio::test]
    async fn exec_hub_delivers_bidi_exec_into_shipped_loop() {
        use crate::agent_session::{spawn_wait_bridge, wait_via_bridge, ClientEvt, ExecHub};

        let hub = ExecHub::new();
        let run = LocalRun {
            request_id: "req-hub".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "Read README and quote GROUND_TOKEN_ZX9".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        hub.put_run(run.clone()).await;
        let jobs = spawn_wait_bridge(hub.clone(), run.request_id.clone(), run.cancel.clone());
        let pusher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            pusher
                .push(
                    "req-hub",
                    ClientEvt::Exec(read_exec(1, "GROUND_TOKEN_ZX9 from host")),
                )
                .await;
        });
        let calls = Rc::new(Cell::new(0));
        let mut replies = VecDeque::from([
            ModelTurn {
                thinking: "need file".into(),
                text: String::new(),
                tools: vec![ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    arguments: json!({"path": "README.md"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                thinking: String::new(),
                text: "The file contains GROUND_TOKEN_ZX9".into(),
                tools: Vec::new(),
                ..Default::default()
            },
        ]);
        let calls_llm = calls.clone();
        let llm = move |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            calls_llm.set(calls_llm.get() + 1);
            let turn = replies.pop_front().expect("llm turn");
            async move { Ok(turn) }
        };
        let wait = move |id: u32| {
            let jobs = jobs.clone();
            async move { wait_via_bridge(&jobs, id).await }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("hub loop");
        assert_eq!(calls.get(), 2);
        assert!(agent_wire::has_tool_call_started(&frames));
        assert!(agent_wire::has_exec_server(&frames));
        assert!(agent_wire::has_tool_call_completed(&frames));
        assert!(agent_wire::text_deltas(&frames).contains("GROUND_TOKEN_ZX9"));
    }

    /// Live 47821 probe: BidiAppend + RunSSE + host-exec reply. Run with --ignored while GBA is up.
    #[tokio::test]
    #[ignore]
    async fn live_gba_http_tool_roundtrip() {
        let health = reqwest::Client::new()
            .get("http://127.0.0.1:47821/health")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .expect("GBA health");
        let body = health.text().await.unwrap_or_default();
        assert!(
            body.contains(env!("CARGO_PKG_VERSION")),
            "GBA {body}"
        );
        let client = reqwest::Client::new();
        let request_id = format!("probe-{}", uuid::Uuid::new_v4());
        let workspace = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_owned()));
        let bidi = crate::agent_wire::encode_bidi_run(
            &request_id,
            "gb-p/xai-xAI/grok-4.6",
            "Use the Read tool on Cargo.toml. Do not answer from memory. Quote grok-bot-auth after the tool result.",
            workspace.as_deref(),
        );
        let bidi_resp = client
            .post("http://127.0.0.1:47821/aiserver.v1.BidiService/BidiAppend")
            .header("content-type", "application/connect+proto")
            .header("connect-protocol-version", "1")
            .body(bidi)
            .send()
            .await
            .expect("bidi");
        assert!(
            bidi_resp.status().is_success(),
            "bidi {}",
            bidi_resp.status()
        );
        let sse = crate::agent_wire::encode_run_sse_request(&request_id);
        let mut sse_resp = client
            .post("http://127.0.0.1:47821/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .header("connect-protocol-version", "1")
            .body(sse)
            .send()
            .await
            .expect("runsse");
        assert!(
            sse_resp.status().is_success(),
            "runsse {}",
            sse_resp.status()
        );
        let mut collected = Vec::new();
        let mut saw_started = false;
        let mut saw_exec = false;
        let mut saw_completed = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(5), sse_resp.chunk()).await {
                Ok(Ok(Some(chunk))) => collected.extend_from_slice(&chunk),
                Ok(Ok(None)) => break,
                _ => {}
            }
            let messages = crate::agent_wire::decode_connect_server_messages(&collected);
            if crate::agent_wire::has_tool_call_started(&messages) {
                saw_started = true;
            }
            if crate::agent_wire::has_exec_server(&messages) && !saw_exec {
                saw_exec = true;
                let reply = read_exec(1, "[package]\nname = \"grok-bot-auth\"\n");
                let exec_body = crate::agent_wire::encode_exec_client_bidi(&request_id, &reply);
                let _ = client
                    .post("http://127.0.0.1:47821/aiserver.v1.BidiService/BidiAppend")
                    .header("content-type", "application/connect+proto")
                    .header("connect-protocol-version", "1")
                    .body(exec_body)
                    .send()
                    .await;
            }
            if crate::agent_wire::has_tool_call_completed(&messages) {
                saw_completed = true;
            }
            let text = crate::agent_wire::text_deltas(&messages);
            if saw_completed && text.contains("grok-bot-auth") {
                break;
            }
            if !saw_started
                && !text.is_empty()
                && crate::agent_wire::decode_connect_server_messages(&collected)
                    .iter()
                    .any(|m| {
                        matches!(
                            m.message,
                            Some(crate::agent_wire::agent_server_message::Message::InteractionUpdate(
                                crate::agent_wire::InteractionUpdate {
                                    message: Some(crate::agent_wire::interaction_update::Message::TurnEnded(_)),
                                }
                            ))
                        )
                    })
            {
                break;
            }
        }
        eprintln!(
            "live probe started={saw_started} exec={saw_exec} completed={saw_completed} text={}",
            crate::agent_wire::text_deltas(&crate::agent_wire::decode_connect_server_messages(
                &collected
            ))
            .chars()
            .take(400)
            .collect::<String>()
        );
        assert!(
            saw_started && saw_exec && saw_completed,
            "expected Cursor host-exec frames; started={saw_started} exec={saw_exec} completed={saw_completed}"
        );
    }

    #[test]
    fn toolkit_prompt_mentions_workspace_and_tools() {
        let run = LocalRun {
            user_text: "检查进度".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let prompt = toolkit_prompt(&run);
        assert!(prompt.contains("Workspace") || prompt.contains("Read"));
        assert!(prompt.contains("检查进度"));
        assert!(prompt.contains("<tool_call>"));
        assert!(!prompt.contains("read_file"));
        assert!(
            !prompt.contains("<file path="),
            "must not dump workspace files into Stream"
        );
        assert!(!prompt.contains("Project files:"));
        let mut huge = "RULES ".repeat(20_000);
        huge.push_str("UNIQUE_USER_QUESTION_ZX9");
        let run = LocalRun {
            user_text: huge,
            workspace: Some("C:\\\\ws".into()),
            attached_files: vec![("big.rs".into(), "x".repeat(20_000))],
            ..Default::default()
        };
        let prompt = toolkit_prompt(&run);
        assert!(prompt.contains("UNIQUE_USER_QUESTION_ZX9"));
        assert!(!prompt.contains("<file path="));
        assert!(
            prompt.len() <= 100_000,
            "prompt too large for Grok Stream: {}",
            prompt.len()
        );
    }

    #[tokio::test]
    async fn loop_streams_thinking_and_turn_ended_tokens() {
        let run = LocalRun {
            request_id: "req-live".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "hi".into(),
            ..Default::default()
        };
        let llm = move |_prompt: String, live: mpsc::UnboundedSender<LlmChunk>| async move {
            let _ = live.send(LlmChunk::Thinking("plan step".into()));
            let _ = live.send(LlmChunk::Tokens(3));
            Ok(ModelTurn {
                thinking: String::new(),
                text: "done".into(),
                tools: Vec::new(),
                usage: TokenUsage {
                    input: 40500,
                    output: 120,
                    reasoning: 80,
                },
            })
        };
        let wait = |_id: u32| async { None };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(
            crate::agent_wire::has_thinking_delta(&frames),
            "thinking must stream before the model turn finishes"
        );
        assert!(
            crate::agent_wire::has_token_delta(&frames),
            "token_delta field 8 must be present for the context meter"
        );
        assert_eq!(crate::agent_wire::turn_ended_input(&frames), Some(40500));
        assert!(crate::agent_wire::text_deltas(&frames).contains("done"));
        assert!(
            crate::agent_wire::has_checkpoint(&frames),
            "conversation_checkpoint_update field 3 drives the context meter"
        );
        let token_bumps: Vec<i32> =
            frames
                .iter()
                .filter_map(|message| match &message.message {
                    Some(crate::agent_wire::agent_server_message::Message::InteractionUpdate(
                        crate::agent_wire::InteractionUpdate {
                            message:
                                Some(crate::agent_wire::interaction_update::Message::TokenDelta(d)),
                        },
                    )) => Some(d.tokens),
                    _ => None,
                })
                .collect();
        assert!(
            token_bumps.iter().all(|n| *n <= 32),
            "token_delta must be incremental output, not the whole prompt: {token_bumps:?}"
        );
    }

    #[tokio::test]
    async fn loop_does_not_redump_streamed_text() {
        let run = LocalRun {
            request_id: "req-text".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "hi".into(),
            ..Default::default()
        };
        let llm = move |_prompt: String, live: mpsc::UnboundedSender<LlmChunk>| async move {
            let _ = live.send(LlmChunk::Text("do".into()));
            Ok(ModelTurn {
                text: "done".into(),
                ..Default::default()
            })
        };
        let wait = |_id: u32| async { None };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert_eq!(crate::agent_wire::text_deltas(&frames), "done");
    }

    #[tokio::test]
    async fn loop_resume_does_not_reblob_system() {
        let (id, data) = crate::blob::encode_role("user", "OLD_TURN_ZX9");
        crate::blob::put(&id, &data);
        let run = LocalRun {
            request_id: "req-resume-blob".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "now".into(),
            history_blob_ids: vec![crate::blob::id_bytes(&id)],
            ..Default::default()
        };
        let llm = move |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| async move {
            Ok(ModelTurn {
                text: "ok".into(),
                ..Default::default()
            })
        };
        let wait = |_id: u32| async { panic!("no exec"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        let kv = frames
            .iter()
            .filter(|msg| agent_wire::frame_kind(msg) == Some("kv"))
            .count();
        assert_eq!(
            kv, 2,
            "resume emits current user + assistant, not a second system scaffold, got {kv}"
        );
        let prompt = toolkit_prompt(&run);
        assert!(prompt.contains("OLD_TURN_ZX9"));
    }

    #[tokio::test]
    async fn loop_stops_when_cancel_flag_is_set() {
        let run = LocalRun {
            request_id: "req-cancel".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "hi".into(),
            ..Default::default()
        };
        run.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let llm = move |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| async move {
            panic!("LLM must not run after cancel");
        };
        let wait = |_id: u32| async { panic!("exec must not run after cancel"); };
        let mut frames = Vec::new();
        let err = run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect_err("cancelled");
        assert_eq!(err, "cancelled");
    }

    #[test]
    fn token_limit_error_is_detected() {
        assert!(is_token_limit_error(
            "InferenceService/Stream: Input token limit exceeded"
        ));
        assert!(!is_token_limit_error("unauthenticated"));
    }

    #[test]
    fn toolkit_prompt_hydrates_history_blobs() {
        let (id, data) = crate::blob::encode_role("user", "BLOB_PRIOR_ZX9");
        crate::blob::put(&id, &data);
        let run = LocalRun {
            request_id: "req-blob-h".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "now".into(),
            history_blob_ids: vec![crate::blob::id_bytes(&id)],
            ..Default::default()
        };
        let prompt = toolkit_prompt(&run);
        assert!(prompt.contains("BLOB_PRIOR_ZX9"), "{prompt}");
        assert!(prompt.contains("now"));
    }

    #[test]
    fn apply_patch_update_replaces_hunk() {
        let patch = "*** Begin Patch\n*** Update File: a.rs\n@@\n fn main() {}\n-old\n+new\n*** End Patch\n";
        let after = apply_patch_to_content(patch, "fn main() {}\nold\n").expect("apply");
        assert!(after.contains("new"));
        assert!(!after.contains("old\n") || after.matches("old").count() == 0);
    }

    #[test]
    fn encode_delete_and_webfetch_and_todos() {
        let del = ToolUse {
            id: "call_d".into(),
            name: "Delete".into(),
            arguments: json!({"path": "gone.rs"}),
        };
        let (frames, kind) = encode_tool_start_frames(Some("."), &del, 9, "m-d").expect("delete");
        assert_eq!(kind, CursorTool::Delete);
        assert!(frames
            .iter()
            .any(|f| agent_wire::frame_kind(f) == Some("exec")));
        let fetch = ToolUse {
            id: "call_f".into(),
            name: "WebFetch".into(),
            arguments: json!({"url": "https://example.com"}),
        };
        let (frames, kind) =
            encode_tool_start_frames(None, &fetch, 10, "m-f").expect("fetch");
        assert_eq!(kind, CursorTool::WebFetch);
        assert_eq!(
            frames
                .iter()
                .filter_map(agent_wire::frame_kind)
                .collect::<Vec<_>>(),
            ["started"]
        );
        let todos = ToolUse {
            id: "call_td".into(),
            name: "TodoWrite".into(),
            arguments: json!({"todos":[{"id":"1","content":"a","status":"pending"}]}),
        };
        assert_eq!(map_tool_name(&todos.name), Some(CursorTool::TodoWrite));
    }

    #[test]
    fn encode_websearch_started_without_exec() {
        let tool = ToolUse {
            id: "call_w".into(),
            name: "WebSearch".into(),
            arguments: json!({"search_term": "rust async"}),
        };
        let (frames, kind) =
            encode_tool_start_frames(None, &tool, 1, "m-w").expect("websearch frames");
        assert_eq!(kind, CursorTool::WebSearch);
        let kinds: Vec<_> = frames.iter().filter_map(agent_wire::frame_kind).collect();
        assert_eq!(kinds, ["started"]);
    }

    #[test]
    fn encode_mcp_and_task_host_exec_args() {
        let mcp = ToolUse {
            id: "call_m".into(),
            name: "CallMcpTool".into(),
            arguments: json!({
                "namespace": "memory",
                "toolName": "create_entities",
                "arguments": {"name": "n"}
            }),
        };
        let (frames, kind) = encode_tool_start_frames(None, &mcp, 3, "m-m").expect("mcp");
        assert_eq!(kind, CursorTool::Mcp);
        assert!(frames.iter().any(|f| agent_wire::frame_kind(f) == Some("exec")));
        let exec = frames.iter().find_map(|f| match &f.message {
            Some(agent_wire::agent_server_message::Message::ExecServerMessage(msg)) => {
                msg.message.clone()
            }
            _ => None,
        });
        match exec {
            Some(exec_server_message::Message::McpArgs(args)) => {
                assert_eq!(args.tool_name, "create_entities");
                assert_eq!(args.server_identifier, "memory");
            }
            other => panic!("expected mcpArgs, got {other:?}"),
        }
        let task = ToolUse {
            id: "call_t".into(),
            name: "Task".into(),
            arguments: json!({
                "description": "explore",
                "prompt": "look around",
                "subagent_type": "explore"
            }),
        };
        let (frames, kind) = encode_tool_start_frames(None, &task, 4, "m-t").expect("task");
        assert_eq!(kind, CursorTool::Task);
        let exec = frames.iter().find_map(|f| match &f.message {
            Some(agent_wire::agent_server_message::Message::ExecServerMessage(msg)) => {
                msg.message.clone()
            }
            _ => None,
        });
        match exec {
            Some(exec_server_message::Message::SubagentArgs(args)) => {
                assert_eq!(args.subagent_type, "explore");
                assert_eq!(args.prompt, "look around");
            }
            other => panic!("expected subagentArgs, got {other:?}"),
        }
        let task_default = ToolUse {
            id: "call_t2".into(),
            name: "Task".into(),
            arguments: json!({
                "description": "scan",
                "prompt": "find it"
            }),
        };
        let (frames, _) =
            encode_tool_start_frames(None, &task_default, 5, "m-t2").expect("task default");
        let exec = frames.iter().find_map(|f| match &f.message {
            Some(agent_wire::agent_server_message::Message::ExecServerMessage(msg)) => {
                msg.message.clone()
            }
            _ => None,
        });
        match exec {
            Some(exec_server_message::Message::SubagentArgs(args)) => {
                assert_eq!(args.subagent_type, "explore");
                assert_eq!(args.prompt, "find it");
            }
            other => panic!("expected default explore, got {other:?}"),
        }
    }

    #[test]
    fn mcp_result_text_uses_output_location() {
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "mcp-exec".into(),
            message: Some(exec_client_message::Message::McpResult(
                crate::agent_proto::McpResult {
                    result: Some(crate::agent_proto::mcp_result::Result::Success(
                        crate::agent_proto::McpSuccess {
                            content: vec![crate::agent_proto::McpToolResultContentItem {
                                content: Some(
                                    crate::agent_proto::mcp_tool_result_content_item::Content::Text(
                                        crate::agent_proto::McpTextContent {
                                            text: String::new(),
                                            output_location: Some(
                                                crate::agent_proto::OutputLocation {
                                                    file_path: r"C:\ws\agent-tools\a.txt".into(),
                                                    size_bytes: 2048,
                                                    line_count: 40,
                                                },
                                            ),
                                        },
                                    ),
                                ),
                            }],
                            is_error: false,
                        },
                    )),
                },
            )),
        };
        let text = exec_result_text(&exec);
        assert!(text.contains("Content written to file:"), "{text}");
        assert!(text.contains("2.0 KB"), "{text}");
        assert!(text.contains("40 lines"), "{text}");
    }

    #[tokio::test]
    async fn loop_emits_kv_set_blob_and_checkpoint_ids() {
        let run = LocalRun {
            request_id: "req-kv".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "hello kv".into(),
            ..Default::default()
        };
        let llm = move |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| async move {
            Ok(ModelTurn {
                text: "done".into(),
                ..Default::default()
            })
        };
        let wait = |_id: u32| async { panic!("no host exec on text-only turn"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        let kinds: Vec<_> = frames.iter().filter_map(agent_wire::frame_kind).collect();
        let kv_count = kinds.iter().filter(|k| **k == "kv").count();
        assert!(
            kv_count >= 3,
            "system+user+assistant setBlob, got {kv_count} {kinds:?}"
        );
        let blob_len = frames.iter().rev().find_map(|msg| match &msg.message {
            Some(agent_wire::agent_server_message::Message::ConversationCheckpointUpdate(
                state,
            )) => Some(state.root_prompt_messages_json.len()),
            _ => None,
        });
        assert!(
            blob_len.unwrap_or(0) >= 3,
            "checkpoint must carry blob ids, got {blob_len:?}"
        );
    }

    #[tokio::test]
    async fn loop_websearch_honors_interaction_reject() {
        let run = LocalRun {
            request_id: "req-ws-rej".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "search".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_s".into(),
                    name: "WebSearch".into(),
                    arguments: json!({"search_term": "rust"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "stopped".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("no exec"); };
        let wait_ix = |_id: u32| async {
            Some(crate::agent_wire::InteractionResponse {
                id: 1,
                result: Some(
                    crate::agent_wire::interaction_response::Result::WebSearchRequestResponse(
                        crate::agent_proto::WebSearchRequestResponse {
                            result: Some(
                                crate::agent_proto::web_search_request_response::Result::Rejected(
                                    crate::agent_proto::WebSearchRejected {
                                        reason: "user denied".into(),
                                    },
                                ),
                            ),
                        },
                    ),
                ),
            })
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, wait_ix, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(prompts.borrow()[1].contains("user denied"));
    }

    #[tokio::test]
    async fn loop_switchmode_await_and_step_are_local() {
        let run = LocalRun {
            request_id: "req-p2b".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "switch".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![
                    ToolUse {
                        id: "call_sm".into(),
                        name: "SwitchMode".into(),
                        arguments: json!({"target_mode_id":"plan","explanation":"design"}),
                    },
                    ToolUse {
                        id: "call_aw".into(),
                        name: "Await".into(),
                        arguments: json!({"block_until_ms": 5}),
                    },
                    ToolUse {
                        id: "call_st".into(),
                        name: "updateCurrentStep".into(),
                        arguments: json!({"current_step": "Planning auth"}),
                    },
                ],
                ..Default::default()
            },
            ModelTurn {
                text: "ok".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("no exec"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        let joined = prompts.borrow()[1].clone();
        assert!(joined.contains("toModeId") || joined.contains("plan"), "{joined}");
        assert!(joined.contains("Planning auth"), "{joined}");
        assert!(frames
            .iter()
            .any(|msg| agent_wire::frame_kind(msg) == Some("query")));
    }

    #[tokio::test]
    async fn loop_todowrite_and_webfetch_complete_locally() {
        let run = LocalRun {
            request_id: "req-p2".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "do".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![
                    ToolUse {
                        id: "call_td".into(),
                        name: "TodoWrite".into(),
                        arguments: json!({"todos":[{"id":"1","content":"step","status":"in_progress"}]}),
                    },
                    ToolUse {
                        id: "call_f".into(),
                        name: "WebFetch".into(),
                        arguments: json!({"url": "https://example.com/a"}),
                    },
                ],
                ..Default::default()
            },
            ModelTurn {
                text: "done p2".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("no exec"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        let joined = prompts.borrow()[1].clone();
        assert!(joined.contains("step"), "{joined}");
        assert!(joined.contains("stub fetch body"), "{joined}");
        assert!(frames
            .iter()
            .any(|msg| agent_wire::frame_kind(msg) == Some("query")));
    }

    #[tokio::test]
    async fn loop_create_plan_emits_query_and_success_uri() {
        let run = LocalRun {
            request_id: "req-plan".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "plan it".into(),
            workspace: Some(r"C:\ws".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_p".into(),
                    name: "CreatePlan".into(),
                    arguments: json!({
                        "name": "auth-fix",
                        "overview": "fix",
                        "plan": "do the thing",
                        "todos": [{"id":"1","content":"step"}]
                    }),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "plan ready".into(),
                ..Default::default()
            },
        ]);
        let llm = move |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("CreatePlan must not host-exec"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(
            frames
                .iter()
                .any(|msg| agent_wire::frame_kind(msg) == Some("query")),
            "CreatePlan interactionQuery missing"
        );
        let completed = frames.iter().find_map(|msg| match &msg.message {
            Some(agent_wire::agent_server_message::Message::InteractionUpdate(
                crate::agent_wire::InteractionUpdate {
                    message: Some(crate::agent_wire::interaction_update::Message::ToolCallCompleted(
                        done,
                    )),
                },
            )) => done.tool_call.clone(),
            _ => None,
        })
        .expect("completed");
        match completed.tool {
            Some(tool_call::Tool::CreatePlanToolCall(plan)) => {
                let uri = plan.result.as_ref().map(|r| r.plan_uri.as_str()).unwrap_or("");
                assert!(
                    uri.contains(".cursor/plans/auth-fix.plan.md"),
                    "{uri}"
                );
            }
            other => panic!("expected CreatePlan, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn loop_websearch_completes_without_host_exec() {
        let run = LocalRun {
            request_id: "req-web".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "search".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_s".into(),
                    name: "WebSearch".into(),
                    arguments: json!({"search_term": "rust async"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "found it".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("WebSearch must not host-exec"); };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(frames
            .iter()
            .any(|msg| agent_wire::frame_kind(msg) == Some("query")));
        assert!(prompts.borrow()[1].contains("Stub rust async"));
        let completed = frames.iter().find_map(|msg| match &msg.message {
            Some(agent_wire::agent_server_message::Message::InteractionUpdate(
                crate::agent_wire::InteractionUpdate {
                    message: Some(crate::agent_wire::interaction_update::Message::ToolCallCompleted(
                        done,
                    )),
                },
            )) => done.tool_call.clone(),
            _ => None,
        })
        .expect("completed");
        match completed.tool {
            Some(tool_call::Tool::WebSearchToolCall(search)) => {
                let refs = match search.result.as_ref().and_then(|r| r.result.as_ref()) {
                    Some(crate::agent_proto::web_search_result::Result::Success(ok)) => {
                        &ok.references
                    }
                    other => panic!("expected success, {other:?}"),
                };
                assert!(!refs.is_empty());
                assert!(refs[0].title.contains("Stub"));
            }
            other => panic!("expected WebSearch, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn loop_get_mcp_tools_returns_catalog() {
        let run = LocalRun {
            request_id: "req-dyn".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "tools".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_g".into(),
                    name: "GetDynamicTools".into(),
                    arguments: json!({}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "listed".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async {
            Some(ExecClientMessage {
                id: 1,
                exec_id: "1".into(),
                message: Some(exec_client_message::Message::McpStateExecResult(
                    crate::agent_proto::McpStateExecResult {
                        result: Some(crate::agent_proto::mcp_state_exec_result::Result::Success(
                            crate::agent_proto::McpStateSuccess {
                                servers: vec![crate::agent_proto::McpStateServer {
                                    server_name: "memory".into(),
                                    server_identifier: "memory".into(),
                                    tools: vec![crate::agent_proto::McpToolDefinition {
                                        name: "create_entities".into(),
                                        description: "create".into(),
                                        tool_name: "create_entities".into(),
                                        ..Default::default()
                                    }],
                                    status: Some("ready".into()),
                                }],
                            },
                        )),
                    },
                )),
            })
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(prompts.borrow()[1].contains("memory"));
        let completed = frames.iter().find_map(|msg| match &msg.message {
            Some(agent_wire::agent_server_message::Message::InteractionUpdate(
                crate::agent_wire::InteractionUpdate {
                    message: Some(crate::agent_wire::interaction_update::Message::ToolCallCompleted(
                        done,
                    )),
                },
            )) => done.tool_call.clone(),
            _ => None,
        })
        .expect("completed");
        match completed.tool {
            Some(tool_call::Tool::GetMcpToolsToolCall(_)) => {}
            other => panic!("expected GetMcpTools, got {other:?}"),
        }
    }

    #[test]
    fn edit_notebook_replaces_unique_cell_source() {
        let before = json!({
            "cells": [{"cell_type": "code", "source": ["print('hi')\n"]}],
        })
        .to_string();
        let after = apply_notebook_edit(&before, 0, false, "python", "hi", "replacement")
            .expect("notebook edit");
        assert!(after.contains("print('replacement')"), "{after}");
    }

    #[test]
    fn edit_notebook_rejects_empty_old_string() {
        let before = json!({
            "cells": [{"cell_type": "code", "source": ["print('hi')\n"]}],
        })
        .to_string();
        let error = apply_notebook_edit(&before, 0, false, "python", "", "x").unwrap_err();
        assert_eq!(error, "old_string must not be empty");
    }

    #[test]
    fn edit_notebook_inserts_new_cell() {
        let after = apply_notebook_edit("", 0, true, "markdown", "", "# title").expect("new cell");
        assert!(after.contains("markdown"), "{after}");
        assert!(after.contains("# title"), "{after}");
    }

    #[test]
    fn encode_list_and_fetch_mcp_resource_host_exec() {
        let list = ToolUse {
            id: "call_lr".into(),
            name: "ListMcpResources".into(),
            arguments: json!({"server": "memory"}),
        };
        let (frames, kind) =
            encode_tool_start_frames(None, &list, 4, "m-lr").expect("list mcp");
        assert_eq!(kind, CursorTool::ListMcpResources);
        assert!(frames
            .iter()
            .any(|frame| agent_wire::frame_kind(frame) == Some("exec")));
        let fetch = ToolUse {
            id: "call_fr".into(),
            name: "FetchMcpResource".into(),
            arguments: json!({"server": "memory", "uri": "memory://notes"}),
        };
        let (frames, kind) =
            encode_tool_start_frames(None, &fetch, 5, "m-fr").expect("fetch mcp");
        assert_eq!(kind, CursorTool::FetchMcpResource);
        assert!(frames
            .iter()
            .any(|frame| agent_wire::frame_kind(frame) == Some("exec")));
    }

    #[test]
    fn encode_edit_notebook_reads_then_generate_image_is_local() {
        let notebook = ToolUse {
            id: "call_nb".into(),
            name: "EditNotebook".into(),
            arguments: json!({
                "target_notebook": "notes.ipynb",
                "cell_idx": 0,
                "is_new_cell": false,
                "cell_language": "python",
                "old_string": "a",
                "new_string": "b"
            }),
        };
        let (frames, kind) =
            encode_tool_start_frames(Some("."), &notebook, 6, "m-nb").expect("notebook");
        assert_eq!(kind, CursorTool::EditNotebook);
        assert!(frames
            .iter()
            .any(|frame| agent_wire::frame_kind(frame) == Some("exec")));
        let image = ToolUse {
            id: "call_img".into(),
            name: "GenerateImage".into(),
            arguments: json!({"description": "a red cube", "filename": "cube.png"}),
        };
        let (frames, kind) =
            encode_tool_start_frames(None, &image, 7, "m-img").expect("image");
        assert_eq!(kind, CursorTool::GenerateImage);
        assert_eq!(
            frames
                .iter()
                .filter_map(agent_wire::frame_kind)
                .collect::<Vec<_>>(),
            ["started"]
        );
    }

    #[test]
    fn complete_shell_maps_timeout_and_spawn_error() {
        let tool = ToolUse {
            id: "call_sh".into(),
            name: "Shell".into(),
            arguments: json!({"command": "sleep 9"}),
        };
        let mut timeout = ShellTranscript {
            timeout_ms: Some(30_000),
            finished: true,
            ..Default::default()
        };
        let (completed, text) = complete_shell_tool(Some("."), &tool, &timeout);
        assert!(text.contains("timeout"), "{text}");
        match completed.tool {
            Some(tool_call::Tool::ShellToolCall(shell)) => match shell.result.and_then(|r| r.result)
            {
                Some(shell_result::Result::Timeout(t)) => assert_eq!(t.timeout_ms, 30_000),
                other => panic!("expected timeout, {other:?}"),
            },
            other => panic!("expected shell, {other:?}"),
        }
        timeout.timeout_ms = None;
        timeout.spawn_error = Some("CreateProcess failed".into());
        let (_, text) = complete_shell_tool(Some("."), &tool, &timeout);
        assert!(text.contains("spawn error"), "{text}");
    }

    #[test]
    fn mcp_result_text_includes_image_placeholder() {
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "mcp-img".into(),
            message: Some(exec_client_message::Message::McpResult(
                crate::agent_proto::McpResult {
                    result: Some(crate::agent_proto::mcp_result::Result::Success(
                        crate::agent_proto::McpSuccess {
                            content: vec![crate::agent_proto::McpToolResultContentItem {
                                content: Some(
                                    crate::agent_proto::mcp_tool_result_content_item::Content::Image(
                                        crate::agent_proto::McpImageContent {
                                            data: vec![1, 2, 3],
                                            mime_type: "image/png".into(),
                                        },
                                    ),
                                ),
                            }],
                            is_error: false,
                        },
                    )),
                },
            )),
        };
        let text = exec_result_text(&exec);
        assert_eq!(text, "[image image/png]");
    }

    #[tokio::test]
    async fn loop_generate_image_emits_query_without_host_exec() {
        let run = LocalRun {
            request_id: "req-img".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "draw".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_img".into(),
                    name: "GenerateImage".into(),
                    arguments: json!({"description": "a blue square", "filename": "square.png"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "saved".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async {
            Some(ExecClientMessage {
                id: 1,
                exec_id: "1".into(),
                message: Some(exec_client_message::Message::WriteResult(
                    crate::agent_proto::WriteResult {
                        result: Some(write_result::Result::Success(
                            crate::agent_proto::WriteSuccess {
                                path: "square.png".into(),
                                lines_created: 0,
                                file_size: 0,
                                file_content_after_write: None,
                            },
                        )),
                    },
                )),
            })
        };
        let wait_ix = |_id: u32| async {
            Some(crate::agent_wire::InteractionResponse {
                id: 1,
                result: Some(
                    crate::agent_wire::interaction_response::Result::GenerateImageRequestResponse(
                        crate::agent_proto::GenerateImageRequestResponse {
                            result: Some(
                                crate::agent_proto::generate_image_request_response::Result::Approved(
                                    crate::agent_proto::GenerateImageApproved {
                                        description: "a blue square".into(),
                                    },
                                ),
                            ),
                        },
                    ),
                ),
            })
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, wait_ix, |msg| frames.push(msg))
            .await
            .expect("loop");
        assert!(frames
            .iter()
            .any(|msg| agent_wire::frame_kind(msg) == Some("query")));
        assert!(
            frames.iter().any(|msg| {
                matches!(
                    &msg.message,
                    Some(agent_wire::agent_server_message::Message::ExecServerMessage(exec))
                        if matches!(
                            exec.message,
                            Some(exec_server_message::Message::WriteArgs(ref args))
                                if args.file_text.is_empty() && args.path.ends_with("square.png")
                        )
                )
            }),
            "approved GenerateImage must emit empty writeArgs"
        );
        assert!(prompts.borrow()[1].contains("square.png"), "{}", prompts.borrow()[1]);
    }

    #[tokio::test]
    async fn loop_generate_image_missing_ix_is_error() {
        let run = LocalRun {
            request_id: "req-img-miss".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "draw".into(),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_img".into(),
                    name: "GenerateImage".into(),
                    arguments: json!({"description": "a red cube"}),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "no image".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let wait = |_id: u32| async { panic!("GenerateImage must not host-exec"); };
        run_injected_agent(&run, llm, wait, |_id| async { None }, |_| {})
            .await
            .expect("loop");
        assert!(
            prompts.borrow()[1].contains("response missing"),
            "{}",
            prompts.borrow()[1]
        );
    }

    #[tokio::test]
    async fn loop_edit_notebook_writes_cell() {
        let run = LocalRun {
            request_id: "req-nb".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "edit notebook".into(),
            workspace: Some(".".into()),
            ..Default::default()
        };
        let mut replies = VecDeque::from([
            ModelTurn {
                tools: vec![ToolUse {
                    id: "call_nb".into(),
                    name: "EditNotebook".into(),
                    arguments: json!({
                        "target_notebook": "notes.ipynb",
                        "cell_idx": 0,
                        "is_new_cell": false,
                        "cell_language": "python",
                        "old_string": "hi",
                        "new_string": "hello"
                    }),
                }],
                ..Default::default()
            },
            ModelTurn {
                text: "updated notebook".into(),
                ..Default::default()
            },
        ]);
        let prompts = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let prompts_llm = prompts.clone();
        let llm = move |prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| {
            prompts_llm.borrow_mut().push(prompt);
            let turn = replies.pop_front().expect("llm");
            async move { Ok(turn) }
        };
        let notebook = json!({
            "cells": [{"cell_type": "code", "source": ["print('hi')\n"]}],
        })
        .to_string();
        let mut execs = VecDeque::from([
            read_exec(1, &notebook),
            ExecClientMessage {
                id: 2,
                exec_id: "nb-write".into(),
                message: Some(exec_client_message::Message::WriteResult(
                    crate::agent_proto::WriteResult {
                        result: Some(write_result::Result::Success(
                            crate::agent_proto::WriteSuccess {
                                path: "notes.ipynb".into(),
                                lines_created: 8,
                                file_size: 80,
                                file_content_after_write: None,
                            },
                        )),
                    },
                )),
            },
        ]);
        let wait = move |_id: u32| {
            let next = execs.pop_front();
            async move { next }
        };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("notebook loop");
        match completed_edit_case(&frames) {
            Some(edit_result::Result::Success(ok)) => {
                assert!(ok.path.contains("notes.ipynb"), "{}", ok.path);
            }
            other => panic!("expected EditSuccess, got {other:?}"),
        }
        assert!(prompts.borrow()[1].contains("hello") || prompts.borrow()[1].contains("updated"), "{}", prompts.borrow()[1]);
    }

    #[test]
    fn apply_edit_keeps_utf8_bom() {
        let before = "\u{FEFF}fn main() {}\n";
        let after = apply_edit(before, "fn main() {}", "fn main() { 1 }", false).expect("edit");
        assert!(after.starts_with('\u{FEFF}'), "{after:?}");
        assert!(after.contains("fn main() { 1 }"), "{after}");
    }

    #[test]
    fn incomplete_native_json_does_not_fall_back_to_xml() {
        let turn = parse_model_turn(
            String::new(),
            "<tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"Cargo.toml\"}}</tool_call>"
                .into(),
            vec![crate::connect::ChatToolCall {
                id: "call_1".into(),
                name: "Read".into(),
                arguments: "{\"path\":".into(),
            }],
        );
        assert!(
            turn.tools.is_empty(),
            "incomplete native JSON must not execute XML fallback: {:?}",
            turn.tools
        );
    }

    #[test]
    fn glob_success_propagates_host_truncation() {
        let mut workspace_results = std::collections::HashMap::new();
        workspace_results.insert(
            "ws".into(),
            GrepUnionResult {
                result: Some(grep_union_result::Result::Files(
                    crate::agent_proto::GrepFilesResult {
                        files: vec!["a.rs".into(), "b.rs".into()],
                        total_files: 80,
                        client_truncated: true,
                        ripgrep_truncated: true,
                    },
                )),
            },
        );
        let exec = ExecClientMessage {
            id: 1,
            exec_id: "g-exec".into(),
            message: Some(exec_client_message::Message::GrepResult(GrepResult {
                result: Some(grep_result::Result::Success(GrepSuccess {
                    pattern: String::new(),
                    path: ".".into(),
                    output_mode: "files_with_matches".into(),
                    workspace_results,
                    active_editor_result: None,
                })),
            })),
        };
        let tool = ToolUse {
            id: "call_g".into(),
            name: "Glob".into(),
            arguments: json!({"glob_pattern": "*.rs"}),
        };
        let (completed, _) = complete_tool_call(Some("."), &tool, &exec, None, None);
        match completed.tool {
            Some(tool_call::Tool::GlobToolCall(glob)) => match glob.result.and_then(|r| r.result) {
                Some(glob_tool_result::Result::Success(ok)) => {
                    assert!(ok.client_truncated);
                    assert!(ok.ripgrep_truncated);
                    assert_eq!(ok.files.len(), 2);
                }
                other => panic!("expected glob success, {other:?}"),
            },
            other => panic!("expected glob, {other:?}"),
        }
    }

    #[test]
    fn shell_success_elides_over_tenk() {
        let tool = ToolUse {
            id: "call_sh".into(),
            name: "Shell".into(),
            arguments: json!({"command": "yes"}),
        };
        let transcript = ShellTranscript {
            stdout: "X".repeat(12_000),
            finished: true,
            ..Default::default()
        };
        let (completed, text) = complete_shell_tool(Some("."), &tool, &transcript);
        assert!(text.contains("..."), "{text}");
        match completed.tool {
            Some(tool_call::Tool::ShellToolCall(shell)) => match shell.result.and_then(|r| r.result)
            {
                Some(shell_result::Result::Success(ok)) => {
                    assert!(ok.elided_chars.unwrap_or(0) > 0);
                    assert!(ok.output_head.is_some());
                    assert!(ok.output_tail.is_some());
                }
                other => panic!("expected success, {other:?}"),
            },
            other => panic!("expected shell, {other:?}"),
        }
    }

    #[test]
    fn task_force_background_sets_run_in_background() {
        let task = ToolUse {
            id: "call_bg".into(),
            name: "Task".into(),
            arguments: json!({
                "description": "explore",
                "prompt": "look"
            }),
        };
        let (frames, kind) =
            encode_tool_start_frames_ex(None, &task, 8, "m-bg", Some("conv-parent"), true, None)
                .expect("task bg");
        assert_eq!(kind, CursorTool::Task);
        let exec = frames.iter().find_map(|f| match &f.message {
            Some(agent_wire::agent_server_message::Message::ExecServerMessage(msg)) => {
                msg.message.clone()
            }
            _ => None,
        });
        match exec {
            Some(exec_server_message::Message::SubagentArgs(args)) => {
                assert_eq!(args.run_in_background, Some(true));
                assert_eq!(args.parent_conversation_id.as_deref(), Some("conv-parent"));
            }
            other => panic!("expected subagentArgs, got {other:?}"),
        }
    }

    #[test]
    fn mcp_rejected_and_permission_text() {
        let rejected = crate::agent_proto::McpResult {
            result: Some(crate::agent_proto::mcp_result::Result::Rejected(
                crate::agent_proto::McpRejected {
                    reason: "user said no".into(),
                    is_readonly: false,
                },
            )),
        };
        assert!(mcp_result_text(&rejected).contains("rejected"));
        let denied = crate::agent_proto::McpResult {
            result: Some(crate::agent_proto::mcp_result::Result::PermissionDenied(
                crate::agent_proto::McpPermissionDenied {
                    error: "blocked".into(),
                    is_readonly: true,
                },
            )),
        };
        assert!(mcp_result_text(&denied).contains("permission denied"));
    }

    #[tokio::test]
    async fn background_completion_emits_user_message_appended() {
        let run = LocalRun {
            request_id: "bg-append".into(),
            model_id: "gb-grok-4.6".into(),
            user_text: "<agent_notification>\ndone\n</agent_notification>".into(),
            is_background_completion: true,
            ..Default::default()
        };
        let llm = |_prompt: String, _live: mpsc::UnboundedSender<LlmChunk>| async {
            Ok(ModelTurn {
                text: "noted".into(),
                ..Default::default()
            })
        };
        let wait = |_id: u32| async { None };
        let mut frames = Vec::new();
        run_injected_agent(&run, llm, wait, |_id| async { None }, |msg| frames.push(msg))
            .await
            .expect("bg");
        assert!(frames.iter().any(|frame| matches!(
            frame.message,
            Some(agent_wire::agent_server_message::Message::InteractionUpdate(
                agent_wire::InteractionUpdate {
                    message: Some(agent_wire::interaction_update::Message::UserMessageAppended(_)),
                }
            ))
        )));
    }
}
