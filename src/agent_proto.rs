//! Cursor `agent.v1` tool + exec frames. Field numbers from CCursor `agent_v1_pb`
//! / cursor-byok `agent_v1.proto` (ToolCall read=8 grep=5 ls=13 glob=4 shell=1 edit=12).

use std::collections::HashMap;

use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ToolCallStartedUpdate {
    #[prost(string, tag = "1")]
    pub call_id: String,
    #[prost(message, optional, tag = "2")]
    pub tool_call: Option<ToolCall>,
    #[prost(string, tag = "3")]
    pub model_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ToolCallCompletedUpdate {
    #[prost(string, tag = "1")]
    pub call_id: String,
    #[prost(message, optional, tag = "2")]
    pub tool_call: Option<ToolCall>,
    #[prost(string, tag = "3")]
    pub model_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct PartialToolCallUpdate {
    #[prost(string, tag = "1")]
    pub call_id: String,
    #[prost(message, optional, tag = "2")]
    pub tool_call: Option<ToolCall>,
    #[prost(string, tag = "3")]
    pub args_text_delta: String,
    #[prost(string, tag = "4")]
    pub model_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellToolCallStdoutDelta {
    #[prost(string, tag = "1")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellToolCallStderrDelta {
    #[prost(string, tag = "1")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellToolCallDelta {
    #[prost(oneof = "shell_tool_call_delta::Delta", tags = "1, 2")]
    pub delta: Option<shell_tool_call_delta::Delta>,
}

pub mod shell_tool_call_delta {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Delta {
        #[prost(message, tag = "1")]
        Stdout(super::ShellToolCallStdoutDelta),
        #[prost(message, tag = "2")]
        Stderr(super::ShellToolCallStderrDelta),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct EditToolCallDelta {
    #[prost(string, tag = "1")]
    pub stream_content_delta: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ToolCallDelta {
    #[prost(oneof = "tool_call_delta::Delta", tags = "1, 3")]
    pub delta: Option<tool_call_delta::Delta>,
}

pub mod tool_call_delta {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Delta {
        #[prost(message, tag = "1")]
        ShellToolCallDelta(super::ShellToolCallDelta),
        #[prost(message, tag = "3")]
        EditToolCallDelta(super::EditToolCallDelta),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ToolCallDeltaUpdate {
    #[prost(string, tag = "1")]
    pub call_id: String,
    #[prost(message, optional, tag = "2")]
    pub tool_call_delta: Option<ToolCallDelta>,
    #[prost(string, tag = "3")]
    pub model_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ToolCall {
    #[prost(oneof = "tool_call::Tool", tags = "1, 3, 4, 5, 8, 9, 12, 13, 14, 15, 17, 18, 19, 20, 21, 23, 25, 28, 37, 42, 44, 48")]
    pub tool: Option<tool_call::Tool>,
}

pub mod tool_call {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Tool {
        #[prost(message, tag = "1")]
        ShellToolCall(super::ShellToolCall),
        #[prost(message, tag = "3")]
        DeleteToolCall(super::DeleteToolCall),
        #[prost(message, tag = "4")]
        GlobToolCall(super::GlobToolCall),
        #[prost(message, tag = "5")]
        GrepToolCall(super::GrepToolCall),
        #[prost(message, tag = "8")]
        ReadToolCall(super::ReadToolCall),
        #[prost(message, tag = "9")]
        UpdateTodosToolCall(super::UpdateTodosToolCall),
        #[prost(message, tag = "12")]
        EditToolCall(super::EditToolCall),
        #[prost(message, tag = "13")]
        LsToolCall(super::LsToolCall),
        #[prost(message, tag = "14")]
        ReadLintsToolCall(super::ReadLintsToolCall),
        #[prost(message, tag = "15")]
        McpToolCall(super::McpToolCall),
        #[prost(message, tag = "17")]
        CreatePlanToolCall(super::CreatePlanToolCall),
        #[prost(message, tag = "18")]
        WebSearchToolCall(super::WebSearchToolCall),
        #[prost(message, tag = "19")]
        TaskToolCall(super::TaskToolCall),
        #[prost(message, tag = "20")]
        ListMcpResourcesToolCall(super::ListMcpResourcesToolCall),
        #[prost(message, tag = "21")]
        ReadMcpResourceToolCall(super::ReadMcpResourceToolCall),
        #[prost(message, tag = "23")]
        AskQuestionToolCall(super::AskQuestionToolCall),
        #[prost(message, tag = "25")]
        SwitchModeToolCall(super::SwitchModeToolCall),
        #[prost(message, tag = "28")]
        GenerateImageToolCall(super::GenerateImageToolCall),
        #[prost(message, tag = "37")]
        WebFetchToolCall(super::WebFetchToolCall),
        #[prost(message, tag = "42")]
        AwaitToolCall(super::AwaitToolCall),
        #[prost(message, tag = "44")]
        GetMcpToolsToolCall(super::GetMcpToolsToolCall),
        #[prost(message, tag = "48")]
        CommunicateUpdateToolCall(super::CommunicateUpdateToolCall),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecServerMessage {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "15")]
    pub exec_id: String,
    #[prost(oneof = "exec_server_message::Message", tags = "2, 3, 4, 5, 7, 8, 9, 11, 14, 17, 18, 28, 36")]
    pub message: Option<exec_server_message::Message>,
}

pub mod exec_server_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "2")]
        ShellArgs(super::ShellArgs),
        #[prost(message, tag = "3")]
        WriteArgs(super::WriteArgs),
        #[prost(message, tag = "4")]
        DeleteArgs(super::DeleteArgs),
        #[prost(message, tag = "5")]
        GrepArgs(super::GrepArgs),
        #[prost(message, tag = "7")]
        ReadArgs(super::ReadArgs),
        #[prost(message, tag = "8")]
        LsArgs(super::LsArgs),
        #[prost(message, tag = "9")]
        DiagnosticsArgs(super::DiagnosticsArgs),
        #[prost(message, tag = "11")]
        McpArgs(super::McpArgs),
        #[prost(message, tag = "14")]
        ShellStreamArgs(super::ShellArgs),
        #[prost(message, tag = "17")]
        ListMcpResourcesExecArgs(super::ListMcpResourcesExecArgs),
        #[prost(message, tag = "18")]
        ReadMcpResourceExecArgs(super::ReadMcpResourceExecArgs),
        #[prost(message, tag = "28")]
        SubagentArgs(super::SubagentArgs),
        #[prost(message, tag = "36")]
        McpStateExecArgs(super::McpStateExecArgs),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecClientMessage {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "15")]
    pub exec_id: String,
    #[prost(oneof = "exec_client_message::Message", tags = "2, 3, 4, 5, 7, 8, 9, 11, 14, 17, 18, 28, 36")]
    pub message: Option<exec_client_message::Message>,
}

pub mod exec_client_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "2")]
        ShellResult(super::ShellResult),
        #[prost(message, tag = "3")]
        WriteResult(super::WriteResult),
        #[prost(message, tag = "4")]
        DeleteResult(super::DeleteResult),
        #[prost(message, tag = "5")]
        GrepResult(super::GrepResult),
        #[prost(message, tag = "7")]
        ReadResult(super::ReadResult),
        #[prost(message, tag = "8")]
        LsResult(super::LsResult),
        #[prost(message, tag = "9")]
        DiagnosticsResult(super::DiagnosticsResult),
        #[prost(message, tag = "11")]
        McpResult(super::McpResult),
        #[prost(message, tag = "14")]
        ShellStream(super::ShellStream),
        #[prost(message, tag = "17")]
        ListMcpResourcesExecResult(super::ListMcpResourcesExecResult),
        #[prost(message, tag = "18")]
        ReadMcpResourceExecResult(super::ReadMcpResourceExecResult),
        #[prost(message, tag = "28")]
        SubagentResult(super::SubagentResult),
        #[prost(message, tag = "36")]
        McpStateExecResult(super::McpStateExecResult),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateExecArgs {
    #[prost(string, repeated, tag = "1")]
    pub server_identifiers: Vec<String>,
    #[prost(bool, tag = "2")]
    pub kick_only: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateExecResult {
    #[prost(oneof = "mcp_state_exec_result::Result", tags = "1, 2, 3")]
    pub result: Option<mcp_state_exec_result::Result>,
}

pub mod mcp_state_exec_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::McpStateSuccess),
        #[prost(message, tag = "2")]
        Error(super::McpStateError),
        #[prost(message, tag = "3")]
        Rejected(super::McpStateRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateSuccess {
    #[prost(message, repeated, tag = "1")]
    pub servers: Vec<McpStateServer>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpStateServer {
    #[prost(string, tag = "1")]
    pub server_name: String,
    #[prost(string, tag = "2")]
    pub server_identifier: String,
    #[prost(message, repeated, tag = "5")]
    pub tools: Vec<McpToolDefinition>,
    #[prost(string, optional, tag = "7")]
    pub status: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolDefinition {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(string, tag = "4")]
    pub provider_identifier: String,
    #[prost(string, tag = "5")]
    pub tool_name: String,
    #[prost(string, optional, tag = "6")]
    pub input_schema_json: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
    #[prost(int32, optional, tag = "4")]
    pub offset: Option<i32>,
    #[prost(uint32, optional, tag = "5")]
    pub limit: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadToolArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(int32, optional, tag = "2")]
    pub offset: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    pub limit: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<ReadToolArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<ReadToolResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadToolResult {
    #[prost(oneof = "read_tool_result::Result", tags = "1, 2")]
    pub result: Option<read_tool_result::Result>,
}

pub mod read_tool_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ReadToolSuccess),
        #[prost(message, tag = "2")]
        Error(super::ReadToolError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadToolError {
    #[prost(string, tag = "1")]
    pub error_message: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadToolSuccess {
    #[prost(bool, tag = "2")]
    pub is_empty: bool,
    #[prost(bool, tag = "3")]
    pub exceeded_limit: bool,
    #[prost(uint32, tag = "4")]
    pub total_lines: u32,
    #[prost(uint32, tag = "5")]
    pub file_size: u32,
    #[prost(string, tag = "7")]
    pub path: String,
    #[prost(oneof = "read_tool_success::Output", tags = "1, 6")]
    pub output: Option<read_tool_success::Output>,
}

pub mod read_tool_success {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Output {
        #[prost(string, tag = "1")]
        Content(String),
        #[prost(bytes, tag = "6")]
        Data(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadResult {
    #[prost(oneof = "read_result::Result", tags = "1, 2, 3, 4, 5, 6")]
    pub result: Option<read_result::Result>,
}

pub mod read_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ReadSuccess),
        #[prost(message, tag = "2")]
        Error(super::ReadError),
        #[prost(message, tag = "3")]
        Rejected(super::ReadRejected),
        #[prost(message, tag = "4")]
        FileNotFound(super::ReadFileNotFound),
        #[prost(message, tag = "5")]
        PermissionDenied(super::ReadPermissionDenied),
        #[prost(message, tag = "6")]
        InvalidFile(super::ReadInvalidFile),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadSuccess {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(int32, tag = "3")]
    pub total_lines: i32,
    #[prost(int64, tag = "4")]
    pub file_size: i64,
    #[prost(bool, tag = "6")]
    pub truncated: bool,
    #[prost(bool, tag = "8")]
    pub range_applied: bool,
    #[prost(oneof = "read_success::Output", tags = "2, 5")]
    pub output: Option<read_success::Output>,
}

pub mod read_success {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Output {
        #[prost(string, tag = "2")]
        Content(String),
        #[prost(bytes, tag = "5")]
        Data(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadRejected {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadFileNotFound {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadPermissionDenied {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadInvalidFile {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepArgs {
    #[prost(string, tag = "1")]
    pub pattern: String,
    #[prost(string, optional, tag = "2")]
    pub path: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub glob: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub output_mode: Option<String>,
    #[prost(int32, optional, tag = "5")]
    pub context_before: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub context_after: Option<i32>,
    #[prost(int32, optional, tag = "7")]
    pub context: Option<i32>,
    #[prost(bool, optional, tag = "8")]
    pub case_insensitive: Option<bool>,
    #[prost(string, optional, tag = "9")]
    pub r#type: Option<String>,
    #[prost(int32, optional, tag = "10")]
    pub head_limit: Option<i32>,
    #[prost(bool, optional, tag = "11")]
    pub multiline: Option<bool>,
    #[prost(string, tag = "14")]
    pub tool_call_id: String,
    #[prost(int32, optional, tag = "16")]
    pub offset: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<GrepArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<GrepResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepResult {
    #[prost(oneof = "grep_result::Result", tags = "1, 2")]
    pub result: Option<grep_result::Result>,
}

pub mod grep_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::GrepSuccess),
        #[prost(message, tag = "2")]
        Error(super::GrepError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepSuccess {
    #[prost(string, tag = "1")]
    pub pattern: String,
    #[prost(string, tag = "2")]
    pub path: String,
    #[prost(string, tag = "3")]
    pub output_mode: String,
    #[prost(map = "string, message", tag = "4")]
    pub workspace_results: HashMap<String, GrepUnionResult>,
    #[prost(message, optional, tag = "5")]
    pub active_editor_result: Option<GrepUnionResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepUnionResult {
    #[prost(oneof = "grep_union_result::Result", tags = "1, 2, 3")]
    pub result: Option<grep_union_result::Result>,
}

pub mod grep_union_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Count(super::GrepCountResult),
        #[prost(message, tag = "2")]
        Files(super::GrepFilesResult),
        #[prost(message, tag = "3")]
        Content(super::GrepContentResult),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepCountResult {
    #[prost(message, repeated, tag = "1")]
    pub counts: Vec<GrepFileCount>,
    #[prost(int32, tag = "2")]
    pub total_files: i32,
    #[prost(int32, tag = "3")]
    pub total_matches: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepFileCount {
    #[prost(string, tag = "1")]
    pub file: String,
    #[prost(int32, tag = "2")]
    pub count: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepFilesResult {
    #[prost(string, repeated, tag = "1")]
    pub files: Vec<String>,
    #[prost(int32, tag = "2")]
    pub total_files: i32,
    #[prost(bool, tag = "3")]
    pub client_truncated: bool,
    #[prost(bool, tag = "4")]
    pub ripgrep_truncated: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepContentResult {
    #[prost(message, repeated, tag = "1")]
    pub matches: Vec<GrepFileMatch>,
    #[prost(int32, tag = "2")]
    pub total_lines: i32,
    #[prost(int32, tag = "3")]
    pub total_matched_lines: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepFileMatch {
    #[prost(string, tag = "1")]
    pub file: String,
    #[prost(message, repeated, tag = "2")]
    pub matches: Vec<GrepContentMatch>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GrepContentMatch {
    #[prost(int32, tag = "1")]
    pub line_number: i32,
    #[prost(string, tag = "2")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GlobToolArgs {
    #[prost(string, optional, tag = "1")]
    pub target_directory: Option<String>,
    #[prost(string, tag = "2")]
    pub glob_pattern: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GlobToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<GlobToolArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<GlobToolResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GlobToolResult {
    #[prost(oneof = "glob_tool_result::Result", tags = "1, 2")]
    pub result: Option<glob_tool_result::Result>,
}

pub mod glob_tool_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::GlobToolSuccess),
        #[prost(message, tag = "2")]
        Error(super::GlobToolError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GlobToolError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GlobToolSuccess {
    #[prost(string, tag = "1")]
    pub pattern: String,
    #[prost(string, tag = "2")]
    pub path: String,
    #[prost(string, repeated, tag = "3")]
    pub files: Vec<String>,
    #[prost(int32, tag = "4")]
    pub total_files: i32,
    #[prost(bool, tag = "5")]
    pub client_truncated: bool,
    #[prost(bool, tag = "6")]
    pub ripgrep_truncated: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, repeated, tag = "2")]
    pub ignore: Vec<String>,
    #[prost(string, tag = "3")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<LsArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<LsResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsResult {
    #[prost(oneof = "ls_result::Result", tags = "1, 2, 3, 4")]
    pub result: Option<ls_result::Result>,
}

pub mod ls_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::LsSuccess),
        #[prost(message, tag = "2")]
        Error(super::LsError),
        #[prost(message, tag = "3")]
        Rejected(super::LsRejected),
        #[prost(message, tag = "4")]
        Timeout(super::LsTimeout),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct LsSuccess {
    #[prost(message, optional, tag = "1")]
    pub directory_tree_root: Option<LsDirectoryTreeNode>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsRejected {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsTimeout {
    #[prost(message, optional, tag = "1")]
    pub directory_tree_root: Option<LsDirectoryTreeNode>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsDirectoryTreeNode {
    #[prost(string, tag = "1")]
    pub abs_path: String,
    #[prost(message, repeated, tag = "2")]
    pub children_dirs: Vec<LsDirectoryTreeNode>,
    #[prost(message, repeated, tag = "3")]
    pub children_files: Vec<LsDirectoryFile>,
    #[prost(bool, tag = "4")]
    pub children_were_processed: bool,
    #[prost(int32, tag = "6")]
    pub num_files: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsDirectoryFile {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub file_text: String,
    #[prost(string, tag = "3")]
    pub tool_call_id: String,
    #[prost(bool, tag = "4")]
    pub return_file_content_after_write: bool,
    #[prost(bytes = "vec", tag = "5")]
    pub file_bytes: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteResult {
    #[prost(oneof = "write_result::Result", tags = "1, 3, 4, 5, 6")]
    pub result: Option<write_result::Result>,
}

pub mod write_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::WriteSuccess),
        #[prost(message, tag = "3")]
        PermissionDenied(super::WritePermissionDenied),
        #[prost(message, tag = "4")]
        NoSpace(super::WriteNoSpace),
        #[prost(message, tag = "5")]
        Error(super::WriteError),
        #[prost(message, tag = "6")]
        Rejected(super::WriteRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteSuccess {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(int32, tag = "2")]
    pub lines_created: i32,
    #[prost(int32, tag = "3")]
    pub file_size: i32,
    #[prost(string, optional, tag = "4")]
    pub file_content_after_write: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WritePermissionDenied {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "4")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteNoSpace {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WriteRejected {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct EditArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, optional, tag = "6")]
    pub stream_content: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct EditToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<EditArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<EditResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct EditResult {
    #[prost(oneof = "edit_result::Result", tags = "1, 2, 7")]
    pub result: Option<edit_result::Result>,
}

pub mod edit_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::EditSuccess),
        #[prost(message, tag = "2")]
        FileNotFound(super::EditFileNotFound),
        #[prost(message, tag = "7")]
        Error(super::EditError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct EditSuccess {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(int32, optional, tag = "3")]
    pub lines_added: Option<i32>,
    #[prost(int32, optional, tag = "4")]
    pub lines_removed: Option<i32>,
    #[prost(string, optional, tag = "5")]
    pub diff_string: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub before_full_file_content: Option<String>,
    #[prost(string, tag = "7")]
    pub after_full_file_content: String,
    #[prost(string, optional, tag = "8")]
    pub message: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct EditFileNotFound {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct EditError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellArgs {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "2")]
    pub working_directory: String,
    #[prost(int32, tag = "3")]
    pub timeout: i32,
    #[prost(string, tag = "4")]
    pub tool_call_id: String,
    #[prost(string, repeated, tag = "5")]
    pub simple_commands: Vec<String>,
    #[prost(message, optional, tag = "8")]
    pub parsing_result: Option<ShellCommandParsingResult>,
    #[prost(uint64, optional, tag = "10")]
    pub file_output_threshold_bytes: Option<u64>,
    #[prost(bool, tag = "11")]
    pub is_background: bool,
    #[prost(bool, tag = "12")]
    pub skip_approval: bool,
    #[prost(int32, tag = "13")]
    pub timeout_behavior: i32,
    #[prost(int32, optional, tag = "14")]
    pub hard_timeout: Option<i32>,
    #[prost(string, optional, tag = "15")]
    pub description: Option<String>,
    #[prost(bool, tag = "17")]
    pub close_stdin: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellCommandParsingResult {
    #[prost(bool, tag = "1")]
    pub parsing_failed: bool,
    #[prost(message, repeated, tag = "2")]
    pub executable_commands: Vec<ShellExecutableCommand>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellExecutableCommand {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub full_text: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<ShellArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<ShellResult>,
    #[prost(string, optional, tag = "3")]
    pub description: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellResult {
    #[prost(oneof = "shell_result::Result", tags = "1, 2, 3, 4, 5, 7")]
    pub result: Option<shell_result::Result>,
}

pub mod shell_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ShellSuccess),
        #[prost(message, tag = "2")]
        Failure(super::ShellFailure),
        #[prost(message, tag = "3")]
        Timeout(super::ShellTimeout),
        #[prost(message, tag = "4")]
        Rejected(super::ShellRejected),
        #[prost(message, tag = "5")]
        SpawnError(super::ShellSpawnError),
        #[prost(message, tag = "7")]
        PermissionDenied(super::ShellPermissionDenied),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellSuccess {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "2")]
    pub working_directory: String,
    #[prost(int32, tag = "3")]
    pub exit_code: i32,
    #[prost(string, tag = "5")]
    pub stdout: String,
    #[prost(string, tag = "6")]
    pub stderr: String,
    #[prost(string, optional, tag = "15")]
    pub output_head: Option<String>,
    #[prost(string, optional, tag = "16")]
    pub output_tail: Option<String>,
    #[prost(uint32, optional, tag = "17")]
    pub elided_chars: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellFailure {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "2")]
    pub working_directory: String,
    #[prost(int32, tag = "3")]
    pub exit_code: i32,
    #[prost(string, tag = "5")]
    pub stdout: String,
    #[prost(string, tag = "6")]
    pub stderr: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellRejected {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "3")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellTimeout {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "2")]
    pub working_directory: String,
    #[prost(int32, tag = "3")]
    pub timeout_ms: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellSpawnError {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "2")]
    pub working_directory: String,
    #[prost(string, tag = "3")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellPermissionDenied {
    #[prost(string, tag = "1")]
    pub command: String,
    #[prost(string, tag = "3")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStream {
    #[prost(oneof = "shell_stream::Event", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub event: Option<shell_stream::Event>,
}

pub mod shell_stream {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "1")]
        Stdout(super::ShellStreamStdout),
        #[prost(message, tag = "2")]
        Stderr(super::ShellStreamStderr),
        #[prost(message, tag = "3")]
        Exit(super::ShellStreamExit),
        #[prost(message, tag = "4")]
        Start(super::ShellStreamStart),
        #[prost(message, tag = "5")]
        Rejected(super::ShellRejected),
        #[prost(message, tag = "6")]
        PermissionDenied(super::ShellPermissionDenied),
        #[prost(message, tag = "7")]
        Backgrounded(super::ShellStreamBackgrounded),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStreamStdout {
    #[prost(string, tag = "1")]
    pub data: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStreamStderr {
    #[prost(string, tag = "1")]
    pub data: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStreamExit {
    #[prost(uint32, tag = "1")]
    pub code: u32,
    #[prost(string, tag = "2")]
    pub cwd: String,
    #[prost(int32, optional, tag = "6")]
    pub local_execution_time_ms: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStreamStart {}

#[derive(Clone, PartialEq, Message)]
pub struct ShellStreamBackgrounded {
    #[prost(uint32, tag = "1")]
    pub shell_id: u32,
    #[prost(string, tag = "2")]
    pub command: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpArgs {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(map = "string, message", tag = "2")]
    pub args: HashMap<String, prost_types::Value>,
    #[prost(string, tag = "3")]
    pub tool_call_id: String,
    #[prost(string, tag = "4")]
    pub provider_identifier: String,
    #[prost(string, tag = "5")]
    pub tool_name: String,
    #[prost(string, tag = "9")]
    pub server_identifier: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct OutputLocation {
    #[prost(string, tag = "1")]
    pub file_path: String,
    #[prost(int64, tag = "2")]
    pub size_bytes: i64,
    #[prost(int64, tag = "3")]
    pub line_count: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpTextContent {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(message, optional, tag = "2")]
    pub output_location: Option<OutputLocation>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolResultContentItem {
    #[prost(oneof = "mcp_tool_result_content_item::Content", tags = "1, 2")]
    pub content: Option<mcp_tool_result_content_item::Content>,
}

pub mod mcp_tool_result_content_item {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(message, tag = "1")]
        Text(super::McpTextContent),
        #[prost(message, tag = "2")]
        Image(super::McpImageContent),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct McpImageContent {
    #[prost(bytes = "vec", tag = "1")]
    pub data: Vec<u8>,
    #[prost(string, tag = "2")]
    pub mime_type: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpSuccess {
    #[prost(message, repeated, tag = "1")]
    pub content: Vec<McpToolResultContentItem>,
    #[prost(bool, tag = "2")]
    pub is_error: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
    #[prost(bool, tag = "2")]
    pub is_readonly: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpPermissionDenied {
    #[prost(string, tag = "1")]
    pub error: String,
    #[prost(bool, tag = "2")]
    pub is_readonly: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolNotFound {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, repeated, tag = "2")]
    pub available_tools: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpServerNotFound {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, repeated, tag = "2")]
    pub available_servers: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpApproved {}

#[derive(Clone, PartialEq, Message)]
pub struct McpResult {
    #[prost(oneof = "mcp_result::Result", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub result: Option<mcp_result::Result>,
}

pub mod mcp_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::McpSuccess),
        #[prost(message, tag = "2")]
        Error(super::McpError),
        #[prost(message, tag = "3")]
        Rejected(super::McpRejected),
        #[prost(message, tag = "4")]
        PermissionDenied(super::McpPermissionDenied),
        #[prost(message, tag = "5")]
        ToolNotFound(super::McpToolNotFound),
        #[prost(message, tag = "6")]
        ServerNotFound(super::McpServerNotFound),
        #[prost(message, tag = "7")]
        Approved(super::McpApproved),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolResult {
    #[prost(oneof = "mcp_tool_result::Result", tags = "1, 2, 3, 4")]
    pub result: Option<mcp_tool_result::Result>,
}

pub mod mcp_tool_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::McpSuccess),
        #[prost(message, tag = "2")]
        Error(super::McpToolError),
        #[prost(message, tag = "3")]
        Rejected(super::McpRejected),
        #[prost(message, tag = "4")]
        PermissionDenied(super::McpPermissionDenied),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolError {
    #[prost(string, tag = "1")]
    pub error: String,
    #[prost(string, tag = "2")]
    pub read_tool_def_reminder: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<McpArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<McpToolResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchArgs {
    #[prost(string, tag = "1")]
    pub search_term: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchReference {
    #[prost(string, tag = "1")]
    pub title: String,
    #[prost(string, tag = "2")]
    pub url: String,
    #[prost(string, tag = "3")]
    pub chunk: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchSuccess {
    #[prost(message, repeated, tag = "1")]
    pub references: Vec<WebSearchReference>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchResult {
    #[prost(oneof = "web_search_result::Result", tags = "1, 2, 3")]
    pub result: Option<web_search_result::Result>,
}

pub mod web_search_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::WebSearchSuccess),
        #[prost(message, tag = "2")]
        Error(super::WebSearchError),
        #[prost(message, tag = "3")]
        Rejected(super::WebSearchRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<WebSearchArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<WebSearchResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchRequestQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<WebSearchArgs>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchApproved {}

#[derive(Clone, PartialEq, Message)]
pub struct WebSearchRequestResponse {
    #[prost(oneof = "web_search_request_response::Result", tags = "1, 2")]
    pub result: Option<web_search_request_response::Result>,
}

pub mod web_search_request_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Approved(super::WebSearchApproved),
        #[prost(message, tag = "2")]
        Rejected(super::WebSearchRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct PlanTodoItem {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanArgs {
    #[prost(string, tag = "1")]
    pub plan: String,
    #[prost(message, repeated, tag = "2")]
    pub todos: Vec<PlanTodoItem>,
    #[prost(string, tag = "3")]
    pub overview: String,
    #[prost(string, tag = "4")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanSuccess {}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanResult {
    #[prost(string, tag = "3")]
    pub plan_uri: String,
    #[prost(oneof = "create_plan_result::Result", tags = "1, 2")]
    pub result: Option<create_plan_result::Result>,
}

pub mod create_plan_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::CreatePlanSuccess),
        #[prost(message, tag = "2")]
        Error(super::CreatePlanError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<CreatePlanArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<CreatePlanResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanRequestQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<CreatePlanArgs>,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct CreatePlanRequestResponse {
    #[prost(message, optional, tag = "1")]
    pub result: Option<CreatePlanResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentArgs {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(string, tag = "2")]
    pub subagent_type: String,
    #[prost(string, tag = "3")]
    pub model_id: String,
    #[prost(string, tag = "4")]
    pub prompt: String,
    #[prost(bool, tag = "5")]
    pub readonly: bool,
    #[prost(string, optional, tag = "6")]
    pub resume_agent_id: Option<String>,
    #[prost(bool, optional, tag = "7")]
    pub run_in_background: Option<bool>,
    #[prost(string, optional, tag = "9")]
    pub parent_conversation_id: Option<String>,
    #[prost(string, optional, tag = "15")]
    pub fork_agent_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentSuccess {
    #[prost(string, tag = "1")]
    pub agent_id: String,
    #[prost(string, optional, tag = "2")]
    pub final_message: Option<String>,
    #[prost(int32, tag = "3")]
    pub tool_call_count: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentResult {
    #[prost(oneof = "subagent_result::Result", tags = "1, 2")]
    pub result: Option<subagent_result::Result>,
}

pub mod subagent_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::SubagentSuccess),
        #[prost(message, tag = "2")]
        Error(super::SubagentError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct TaskSuccess {
    #[prost(string, optional, tag = "2")]
    pub agent_id: Option<String>,
    #[prost(bool, tag = "3")]
    pub is_background: bool,
    #[prost(string, optional, tag = "5")]
    pub result_suffix: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TaskError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct TaskResult {
    #[prost(oneof = "task_result::Result", tags = "1, 2")]
    pub result: Option<task_result::Result>,
}

pub mod task_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::TaskSuccess),
        #[prost(message, tag = "2")]
        Error(super::TaskError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct TaskArgs {
    #[prost(string, tag = "1")]
    pub description: String,
    #[prost(string, tag = "2")]
    pub prompt: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct TaskToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<TaskArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<TaskResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetMcpToolsArgs {
    #[prost(string, optional, tag = "1")]
    pub server: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub tool_name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub pattern: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetMcpToolsSuccess {
    #[prost(string, tag = "1")]
    pub catalog_json: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetMcpToolsResult {
    #[prost(oneof = "get_mcp_tools_result::Result", tags = "1")]
    pub result: Option<get_mcp_tools_result::Result>,
}

pub mod get_mcp_tools_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::GetMcpToolsSuccess),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GetMcpToolsToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<GetMcpToolsArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<GetMcpToolsResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteSuccess {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub deleted_file: String,
    #[prost(int64, tag = "3")]
    pub file_size: i64,
    #[prost(string, tag = "4")]
    pub prev_content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteFileNotFound {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteResult {
    #[prost(oneof = "delete_result::Result", tags = "1, 2, 7")]
    pub result: Option<delete_result::Result>,
}

pub mod delete_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::DeleteSuccess),
        #[prost(message, tag = "2")]
        FileNotFound(super::DeleteFileNotFound),
        #[prost(message, tag = "7")]
        Error(super::DeleteError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct DeleteToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<DeleteArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<DeleteResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TodoItem {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub content: String,
    #[prost(int32, tag = "3")]
    pub status: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct UpdateTodosArgs {
    #[prost(message, repeated, tag = "1")]
    pub todos: Vec<TodoItem>,
    #[prost(bool, tag = "2")]
    pub merge: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct UpdateTodosSuccess {
    #[prost(message, repeated, tag = "1")]
    pub todos: Vec<TodoItem>,
    #[prost(int32, tag = "2")]
    pub total_count: i32,
    #[prost(bool, tag = "3")]
    pub was_merge: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct UpdateTodosResult {
    #[prost(oneof = "update_todos_result::Result", tags = "1")]
    pub result: Option<update_todos_result::Result>,
}

pub mod update_todos_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::UpdateTodosSuccess),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct UpdateTodosToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<UpdateTodosArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<UpdateTodosResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchArgs {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchSuccess {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, tag = "2")]
    pub markdown: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchError {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchResult {
    #[prost(oneof = "web_fetch_result::Result", tags = "1, 2, 3")]
    pub result: Option<web_fetch_result::Result>,
}

pub mod web_fetch_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::WebFetchSuccess),
        #[prost(message, tag = "2")]
        Error(super::WebFetchError),
        #[prost(message, tag = "3")]
        Rejected(super::WebFetchRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<WebFetchArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<WebFetchResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchRequestQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<WebFetchArgs>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchApproved {}

#[derive(Clone, PartialEq, Message)]
pub struct WebFetchRequestResponse {
    #[prost(oneof = "web_fetch_request_response::Result", tags = "1, 2")]
    pub result: Option<web_fetch_request_response::Result>,
}

pub mod web_fetch_request_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Approved(super::WebFetchApproved),
        #[prost(message, tag = "2")]
        Rejected(super::WebFetchRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionOption {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub label: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionQuestion {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub prompt: String,
    #[prost(message, repeated, tag = "3")]
    pub options: Vec<AskQuestionOption>,
    #[prost(bool, tag = "4")]
    pub allow_multiple: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionArgs {
    #[prost(string, tag = "1")]
    pub title: String,
    #[prost(message, repeated, tag = "2")]
    pub questions: Vec<AskQuestionQuestion>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionInteractionQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<AskQuestionArgs>,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionAnswer {
    #[prost(string, tag = "1")]
    pub question_id: String,
    #[prost(string, repeated, tag = "2")]
    pub selected_option_ids: Vec<String>,
    #[prost(string, tag = "3")]
    pub freeform_text: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionSuccess {
    #[prost(message, repeated, tag = "1")]
    pub answers: Vec<AskQuestionAnswer>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionError {
    #[prost(string, tag = "1")]
    pub error_message: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionResult {
    #[prost(oneof = "ask_question_result::Result", tags = "1, 2")]
    pub result: Option<ask_question_result::Result>,
}

pub mod ask_question_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::AskQuestionSuccess),
        #[prost(message, tag = "2")]
        Error(super::AskQuestionError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionInteractionResponse {
    #[prost(message, optional, tag = "1")]
    pub result: Option<AskQuestionResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AskQuestionToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<AskQuestionArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<AskQuestionResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeArgs {
    #[prost(string, tag = "1")]
    pub target_mode_id: String,
    #[prost(string, optional, tag = "2")]
    pub explanation: Option<String>,
    #[prost(string, tag = "3")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeRequestQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<SwitchModeArgs>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeApproved {}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeRequestResponse {
    #[prost(oneof = "switch_mode_request_response::Result", tags = "1, 2")]
    pub result: Option<switch_mode_request_response::Result>,
}

pub mod switch_mode_request_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Approved(super::SwitchModeApproved),
        #[prost(message, tag = "2")]
        Rejected(super::SwitchModeRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeSuccess {
    #[prost(string, tag = "1")]
    pub from_mode_id: String,
    #[prost(string, tag = "2")]
    pub to_mode_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeResult {
    #[prost(oneof = "switch_mode_result::Result", tags = "1, 2, 3")]
    pub result: Option<switch_mode_result::Result>,
}

pub mod switch_mode_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::SwitchModeSuccess),
        #[prost(message, tag = "2")]
        Error(super::SwitchModeError),
        #[prost(message, tag = "3")]
        Rejected(super::SwitchModeRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct SwitchModeToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<SwitchModeArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<SwitchModeResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AwaitArgs {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(uint32, optional, tag = "2")]
    pub block_until_ms: Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub regex: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AwaitTaskStillRunning {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(uint64, tag = "2")]
    pub runtime_ms: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct AwaitError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AwaitResult {
    #[prost(oneof = "await_result::Result", tags = "2, 3")]
    pub result: Option<await_result::Result>,
}

pub mod await_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "2")]
        StillRunning(super::AwaitTaskStillRunning),
        #[prost(message, tag = "3")]
        Error(super::AwaitError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AwaitToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<AwaitArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<AwaitResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CommunicateUpdateArgs {
    #[prost(string, optional, tag = "1")]
    pub current_step: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub final_summary: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub completed_subtitle: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CommunicateUpdateSuccess {
    #[prost(string, tag = "1")]
    pub current_step: String,
    #[prost(uint32, tag = "3")]
    pub message_index: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct CommunicateUpdateResult {
    #[prost(oneof = "communicate_update_result::Result", tags = "1")]
    pub result: Option<communicate_update_result::Result>,
}

pub mod communicate_update_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::CommunicateUpdateSuccess),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct CommunicateUpdateToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<CommunicateUpdateArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<CommunicateUpdateResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct DiagnosticsArgs {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LintDiagnostic {
    #[prost(string, tag = "3")]
    pub message: String,
    #[prost(string, tag = "4")]
    pub source: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DiagnosticsSuccess {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(message, repeated, tag = "2")]
    pub diagnostics: Vec<LintDiagnostic>,
    #[prost(int32, tag = "3")]
    pub total_diagnostics: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct DiagnosticsError {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct DiagnosticsResult {
    #[prost(oneof = "diagnostics_result::Result", tags = "1, 2")]
    pub result: Option<diagnostics_result::Result>,
}

pub mod diagnostics_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::DiagnosticsSuccess),
        #[prost(message, tag = "2")]
        Error(super::DiagnosticsError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct FileDiagnostics {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(message, repeated, tag = "2")]
    pub diagnostics: Vec<LintDiagnostic>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadLintsToolArgs {
    #[prost(string, repeated, tag = "1")]
    pub paths: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadLintsToolSuccess {
    #[prost(message, repeated, tag = "1")]
    pub file_diagnostics: Vec<FileDiagnostics>,
    #[prost(int32, tag = "2")]
    pub total_files: i32,
    #[prost(int32, tag = "3")]
    pub total_diagnostics: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadLintsToolResult {
    #[prost(oneof = "read_lints_tool_result::Result", tags = "1")]
    pub result: Option<read_lints_tool_result::Result>,
}

pub mod read_lints_tool_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ReadLintsToolSuccess),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadLintsToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<ReadLintsToolArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<ReadLintsToolResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageArgs {
    #[prost(string, tag = "1")]
    pub description: String,
    #[prost(string, optional, tag = "2")]
    pub file_path: Option<String>,
    #[prost(string, repeated, tag = "5")]
    pub reference_image_paths: Vec<String>,
    #[prost(string, optional, tag = "6")]
    pub aspect_ratio: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageSuccess {
    #[prost(string, tag = "1")]
    pub file_path: String,
    #[prost(string, tag = "2")]
    pub image_data: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageResult {
    #[prost(oneof = "generate_image_result::Result", tags = "1, 2")]
    pub result: Option<generate_image_result::Result>,
}

pub mod generate_image_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::GenerateImageSuccess),
        #[prost(message, tag = "2")]
        Error(super::GenerateImageError),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<GenerateImageArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<GenerateImageResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageRequestQuery {
    #[prost(message, optional, tag = "1")]
    pub args: Option<GenerateImageArgs>,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageApproved {
    #[prost(string, tag = "1")]
    pub description: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GenerateImageRequestResponse {
    #[prost(oneof = "generate_image_request_response::Result", tags = "1, 2")]
    pub result: Option<generate_image_request_response::Result>,
}

pub mod generate_image_request_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Approved(super::GenerateImageApproved),
        #[prost(message, tag = "2")]
        Rejected(super::GenerateImageRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesExecArgs {
    #[prost(string, optional, tag = "1")]
    pub server: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpListedResource {
    #[prost(string, tag = "1")]
    pub uri: String,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub mime_type: Option<String>,
    #[prost(string, tag = "5")]
    pub server: String,
    #[prost(map = "string, string", tag = "6")]
    pub annotations: HashMap<String, String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesSuccess {
    #[prost(message, repeated, tag = "1")]
    pub resources: Vec<McpListedResource>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesError {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesRejected {
    #[prost(string, tag = "1")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesExecResult {
    #[prost(oneof = "list_mcp_resources_exec_result::Result", tags = "1, 2, 3")]
    pub result: Option<list_mcp_resources_exec_result::Result>,
}

pub mod list_mcp_resources_exec_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ListMcpResourcesSuccess),
        #[prost(message, tag = "2")]
        Error(super::ListMcpResourcesError),
        #[prost(message, tag = "3")]
        Rejected(super::ListMcpResourcesRejected),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ListMcpResourcesToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<ListMcpResourcesExecArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<ListMcpResourcesExecResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceExecArgs {
    #[prost(string, tag = "1")]
    pub server: String,
    #[prost(string, tag = "2")]
    pub uri: String,
    #[prost(string, optional, tag = "3")]
    pub download_path: Option<String>,
    #[prost(string, tag = "4")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceError {
    #[prost(string, tag = "1")]
    pub uri: String,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceRejected {
    #[prost(string, tag = "1")]
    pub uri: String,
    #[prost(string, tag = "2")]
    pub reason: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceNotFound {
    #[prost(string, tag = "1")]
    pub uri: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceSuccess {
    #[prost(string, tag = "1")]
    pub uri: String,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub mime_type: Option<String>,
    #[prost(map = "string, string", tag = "7")]
    pub annotations: HashMap<String, String>,
    #[prost(string, optional, tag = "8")]
    pub download_path: Option<String>,
    #[prost(message, optional, tag = "9")]
    pub output_location: Option<OutputLocation>,
    #[prost(oneof = "read_mcp_resource_success::Content", tags = "5, 6")]
    pub content: Option<read_mcp_resource_success::Content>,
}

pub mod read_mcp_resource_success {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(string, tag = "5")]
        Text(String),
        #[prost(bytes, tag = "6")]
        Blob(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceExecResult {
    #[prost(oneof = "read_mcp_resource_exec_result::Result", tags = "1, 2, 3, 4")]
    pub result: Option<read_mcp_resource_exec_result::Result>,
}

pub mod read_mcp_resource_exec_result {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Success(super::ReadMcpResourceSuccess),
        #[prost(message, tag = "2")]
        Error(super::ReadMcpResourceError),
        #[prost(message, tag = "3")]
        Rejected(super::ReadMcpResourceRejected),
        #[prost(message, tag = "4")]
        NotFound(super::ReadMcpResourceNotFound),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ReadMcpResourceToolCall {
    #[prost(message, optional, tag = "1")]
    pub args: Option<ReadMcpResourceExecArgs>,
    #[prost(message, optional, tag = "2")]
    pub result: Option<ReadMcpResourceExecResult>,
}
