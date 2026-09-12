//! Cursor Agent BidiAppend / RunSSE wire (protocol shape from cursor-byok handlers + agent_v1.proto).
use std::collections::{HashMap, VecDeque};
use std::path::Path;

use prost::Message;
use sha2::Digest;

use crate::catalog::{injected_id, INJECT_PREFIX};
use crate::connect::{encode_connect_frame, unwrap_connect};

#[derive(Clone, PartialEq, Message)]
pub struct BidiRequestId {
    #[prost(string, tag = "1")]
    pub request_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct BidiAppendRequest {
    #[prost(string, tag = "1")]
    pub data: String,
    #[prost(message, optional, tag = "2")]
    pub request_id: Option<BidiRequestId>,
    #[prost(int64, tag = "3")]
    pub append_seqno: i64,
    #[prost(bytes = "vec", tag = "4")]
    pub data_binary: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ModelParameterValue {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct RequestedModel {
    #[prost(string, tag = "1")]
    pub model_id: String,
    #[prost(bool, tag = "2")]
    pub max_mode: bool,
    #[prost(message, repeated, tag = "3")]
    pub parameters: Vec<ModelParameterValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ModelDetails {
    #[prost(string, tag = "1")]
    pub model_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct UserMessage {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(string, tag = "2")]
    pub message_id: String,
    #[prost(message, optional, tag = "3")]
    pub selected_context: Option<SelectedContext>,
    #[prost(int32, tag = "4")]
    pub mode: i32,
    #[prost(bool, optional, tag = "5")]
    pub is_simulated_msg: Option<bool>,
    #[prost(string, optional, tag = "8")]
    pub rich_text: Option<String>,
    #[prost(int32, optional, tag = "9")]
    pub simulated_msg_reason: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "18")]
    pub text_blob_id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedContext {
    #[prost(message, repeated, tag = "1")]
    pub selected_images: Vec<SelectedImage>,
    #[prost(message, optional, tag = "2")]
    pub invocation_context: Option<InvocationContext>,
    #[prost(string, repeated, tag = "3")]
    pub extra_context: Vec<String>,
    #[prost(message, repeated, tag = "5")]
    pub code_selections: Vec<SelectedCodeSelection>,
    #[prost(message, repeated, tag = "7")]
    pub terminal_selections: Vec<SelectedTerminalSelection>,
    #[prost(message, repeated, tag = "9")]
    pub external_links: Vec<SelectedExternalLink>,
    #[prost(message, repeated, tag = "12")]
    pub cursor_commands: Vec<SelectedCursorCommand>,
    #[prost(message, repeated, tag = "13")]
    pub documentations: Vec<SelectedDocumentation>,
    #[prost(message, repeated, tag = "16")]
    pub extra_context_entries: Vec<ExtraContextEntry>,
    #[prost(message, repeated, tag = "22")]
    pub selected_subagents: Vec<SelectedSubagent>,
    #[prost(message, repeated, tag = "24")]
    pub selected_browsers: Vec<SelectedBrowser>,
    #[prost(message, repeated, tag = "26")]
    pub selected_skills: Vec<AgentSkill>,
    #[prost(message, optional, tag = "27")]
    pub recent_agents_context: Option<RecentAgentsContext>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ExtraContextEntry {
    #[prost(oneof = "extra_context_entry::DataOrBlobId", tags = "1, 2")]
    pub data_or_blob_id: Option<extra_context_entry::DataOrBlobId>,
}

pub mod extra_context_entry {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum DataOrBlobId {
        #[prost(string, tag = "1")]
        Data(String),
        #[prost(bytes, tag = "2")]
        BlobId(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedCodeSelection {
    #[prost(string, tag = "1")]
    pub content: String,
    #[prost(string, tag = "2")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedTerminalSelection {
    #[prost(string, tag = "1")]
    pub content: String,
    #[prost(string, optional, tag = "2")]
    pub title: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub path: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedCursorCommand {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedDocumentation {
    #[prost(string, tag = "1")]
    pub doc_id: String,
    #[prost(string, tag = "2")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedExternalLink {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(string, optional, tag = "5")]
    pub filename: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedSubagent {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedBrowser {
    #[prost(string, tag = "2")]
    pub url: String,
    #[prost(string, optional, tag = "3")]
    pub page_title: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct RecentAgent {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub path: String,
    #[prost(string, optional, tag = "3")]
    pub overview: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct RecentAgentsContext {
    #[prost(message, repeated, tag = "1")]
    pub recent_agents: Vec<RecentAgent>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InvocationContext {
    #[prost(oneof = "invocation_context::Data", tags = "3")]
    pub data: Option<invocation_context::Data>,
}

pub mod invocation_context {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Data {
        #[prost(message, tag = "3")]
        IdeState(super::IdeState),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct IdeState {
    #[prost(message, repeated, tag = "1")]
    pub visible_files: Vec<IdeFile>,
    #[prost(message, repeated, tag = "2")]
    pub recently_viewed_files: Vec<IdeFile>,
}

#[derive(Clone, PartialEq, Message)]
pub struct IdeFile {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, optional, tag = "2")]
    pub relative_path: Option<String>,
    #[prost(int32, tag = "4")]
    pub total_lines: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedImage {
    #[prost(string, tag = "2")]
    pub uuid: String,
    #[prost(string, tag = "3")]
    pub path: String,
    #[prost(string, tag = "7")]
    pub mime_type: String,
    #[prost(oneof = "selected_image::DataOrBlobId", tags = "1, 8, 9")]
    pub data_or_blob_id: Option<selected_image::DataOrBlobId>,
}

pub mod selected_image {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum DataOrBlobId {
        #[prost(bytes, tag = "1")]
        BlobId(Vec<u8>),
        #[prost(bytes, tag = "8")]
        Data(Vec<u8>),
        #[prost(message, tag = "9")]
        BlobIdWithData(super::SelectedImageBlob),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct SelectedImageBlob {
    #[prost(bytes = "vec", tag = "1")]
    pub blob_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryText {
    #[prost(string, tag = "1")]
    pub text: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryImage {
    #[prost(string, tag = "1")]
    pub data: String,
    #[prost(string, optional, tag = "2")]
    pub mime_type: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryReasoning {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(string, optional, tag = "2")]
    pub signature: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryRedactedReasoning {
    #[prost(string, tag = "1")]
    pub data: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryUserContent {
    #[prost(oneof = "conversation_history_user_content::Content", tags = "1, 2")]
    pub content: Option<conversation_history_user_content::Content>,
}

pub mod conversation_history_user_content {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(message, tag = "1")]
        Text(super::ConversationHistoryText),
        #[prost(message, tag = "2")]
        Image(super::ConversationHistoryImage),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryUserMessage {
    #[prost(message, repeated, tag = "1")]
    pub content: Vec<ConversationHistoryUserContent>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryAssistantContent {
    #[prost(oneof = "conversation_history_assistant_content::Content", tags = "1, 2, 3, 4")]
    pub content: Option<conversation_history_assistant_content::Content>,
}

pub mod conversation_history_assistant_content {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(message, tag = "1")]
        Text(super::ConversationHistoryText),
        #[prost(message, tag = "2")]
        Reasoning(super::ConversationHistoryReasoning),
        #[prost(message, tag = "3")]
        RedactedReasoning(super::ConversationHistoryRedactedReasoning),
        #[prost(message, tag = "4")]
        ToolCall(super::ConversationHistoryToolCall),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryToolCall {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(string, tag = "2")]
    pub tool_name: String,
    #[prost(string, tag = "3")]
    pub args_json: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryAssistantMessage {
    #[prost(message, repeated, tag = "1")]
    pub content: Vec<ConversationHistoryAssistantContent>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryMessage {
    #[prost(oneof = "conversation_history_message::Message", tags = "1, 2, 3")]
    pub message: Option<conversation_history_message::Message>,
}

pub mod conversation_history_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        User(super::ConversationHistoryUserMessage),
        #[prost(message, tag = "2")]
        Assistant(super::ConversationHistoryAssistantMessage),
        #[prost(message, tag = "3")]
        Tool(super::ConversationHistoryToolMessage),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryToolResultContent {
    #[prost(oneof = "conversation_history_tool_result_content::Content", tags = "1, 2")]
    pub content: Option<conversation_history_tool_result_content::Content>,
}

pub mod conversation_history_tool_result_content {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(message, tag = "1")]
        Text(super::ConversationHistoryText),
        #[prost(message, tag = "2")]
        Image(super::ConversationHistoryImage),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistoryToolMessage {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(string, tag = "2")]
    pub tool_name: String,
    #[prost(message, repeated, tag = "3")]
    pub content: Vec<ConversationHistoryToolResultContent>,
    #[prost(bool, optional, tag = "4")]
    pub is_error: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationHistory {
    #[prost(message, repeated, tag = "1")]
    pub messages: Vec<ConversationHistoryMessage>,
}

#[derive(Clone, PartialEq, Message)]
pub struct RequestContextEnv {
    #[prost(string, tag = "1")]
    pub os_version: String,
    #[prost(string, repeated, tag = "2")]
    pub workspace_paths: Vec<String>,
    #[prost(string, tag = "3")]
    pub shell: String,
    #[prost(bool, tag = "5")]
    pub sandbox_enabled: bool,
    #[prost(string, tag = "7")]
    pub terminals_folder: String,
    #[prost(string, tag = "8")]
    pub agent_shared_notes_folder: String,
    #[prost(string, tag = "9")]
    pub agent_conversation_notes_folder: String,
    #[prost(string, tag = "10")]
    pub time_zone: String,
    #[prost(string, tag = "11")]
    pub project_folder: String,
    #[prost(string, tag = "12")]
    pub agent_transcripts_folder: String,
    #[prost(string, optional, tag = "13")]
    pub artifacts_folder: Option<String>,
    #[prost(bool, optional, tag = "14")]
    pub sandbox_supported: Option<bool>,
    #[prost(string, optional, tag = "21")]
    pub process_working_directory: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct RequestContext {
    #[prost(message, repeated, tag = "2")]
    pub rules: Vec<CursorRule>,
    #[prost(message, optional, tag = "4")]
    pub env: Option<RequestContextEnv>,
    #[prost(message, repeated, tag = "11")]
    pub git_repos: Vec<GitRepoInfo>,
    #[prost(message, repeated, tag = "13")]
    pub project_layouts: Vec<LsDirectoryTreeNode>,
    #[prost(message, repeated, tag = "14")]
    pub mcp_instructions: Vec<McpInstructions>,
    #[prost(string, optional, tag = "16")]
    pub cloud_rule: Option<String>,
    #[prost(bool, optional, tag = "17")]
    pub web_search_enabled: Option<bool>,
    #[prost(map = "string, string", tag = "20")]
    pub file_contents: HashMap<String, String>,
    #[prost(message, repeated, tag = "22")]
    pub custom_subagents: Vec<CustomSubagent>,
    #[prost(bool, optional, tag = "24")]
    pub web_fetch_enabled: Option<bool>,
    #[prost(message, repeated, tag = "29")]
    pub agent_skills: Vec<AgentSkill>,
    #[prost(bool, optional, tag = "32")]
    pub supports_mcp_auth: Option<bool>,
    #[prost(message, optional, tag = "34")]
    pub mcp_meta_tool_options: Option<McpMetaToolOptions>,
    #[prost(bool, optional, tag = "35")]
    pub read_lints_enabled: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CursorRule {
    #[prost(string, tag = "1")]
    pub full_path: String,
    #[prost(string, tag = "2")]
    pub content: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsDirectoryTreeNode {
    #[prost(string, tag = "1")]
    pub abs_path: String,
    #[prost(message, repeated, tag = "2")]
    pub children_dirs: Vec<LsDirectoryTreeNode>,
    #[prost(message, repeated, tag = "3")]
    pub children_files: Vec<LsTreeFile>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LsTreeFile {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpInstructions {
    #[prost(string, tag = "1")]
    pub server_name: String,
    #[prost(string, tag = "2")]
    pub instructions: String,
    #[prost(string, tag = "3")]
    pub server_identifier: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct CustomSubagent {
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub description: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpMetaToolOptions {
    #[prost(bool, tag = "1")]
    pub enabled: bool,
    #[prost(message, repeated, tag = "2")]
    pub mcp_descriptors: Vec<McpDescriptor>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpDescriptor {
    #[prost(string, tag = "1")]
    pub server_name: String,
    #[prost(string, tag = "2")]
    pub server_identifier: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GitRepoInfo {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(string, tag = "2")]
    pub status: String,
    #[prost(string, tag = "3")]
    pub branch_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentSkill {
    #[prost(string, tag = "1")]
    pub full_path: String,
    #[prost(string, tag = "2")]
    pub content: String,
    #[prost(string, tag = "3")]
    pub description: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct UserMessageAction {
    #[prost(message, optional, tag = "1")]
    pub user_message: Option<UserMessage>,
    #[prost(message, optional, tag = "2")]
    pub request_context: Option<RequestContext>,
    #[prost(message, repeated, tag = "4")]
    pub prepend_user_messages: Vec<UserMessage>,
    #[prost(message, optional, tag = "6")]
    pub interrupted_pending_tool_call_resolutions:
        Option<InterruptedPendingToolCallResolutions>,
    #[prost(message, optional, tag = "7")]
    pub conversation_history: Option<ConversationHistory>,
}

#[derive(Clone, PartialEq, Message)]
pub struct RequestContextPartReferences {
    #[prost(bytes = "vec", tag = "1")]
    pub rules_blob_id: Vec<u8>,
    #[prost(uint32, tag = "2")]
    pub rules_byte_length: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub skills_blob_id: Vec<u8>,
    #[prost(uint32, tag = "4")]
    pub skills_byte_length: u32,
    #[prost(bytes = "vec", tag = "5")]
    pub subagents_blob_id: Vec<u8>,
    #[prost(uint32, tag = "6")]
    pub subagents_byte_length: u32,
    #[prost(bytes = "vec", tag = "7")]
    pub mcps_blob_id: Vec<u8>,
    #[prost(uint32, tag = "8")]
    pub mcps_byte_length: u32,
    #[prost(message, optional, tag = "9")]
    pub dynamic_context: Option<RequestContext>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationAction {
    #[prost(message, optional, tag = "17")]
    pub request_context_parts: Option<RequestContextPartReferences>,
    #[prost(
        oneof = "conversation_action::Action",
        tags = "1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 13, 14, 16, 18, 19"
    )]
    pub action: Option<conversation_action::Action>,
}

pub mod conversation_action {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Action {
        #[prost(message, tag = "1")]
        UserMessageAction(super::UserMessageAction),
        #[prost(message, tag = "2")]
        ResumeAction(super::ResumeAction),
        #[prost(message, tag = "3")]
        CancelAction(super::CancelAction),
        #[prost(message, tag = "4")]
        SummarizeAction(super::SummarizeAction),
        #[prost(message, tag = "5")]
        ShellCommandAction(super::ShellCommandAction),
        #[prost(message, tag = "6")]
        StartPlanAction(super::StartPlanAction),
        #[prost(message, tag = "7")]
        ExecutePlanAction(super::ExecutePlanAction),
        #[prost(message, tag = "8")]
        AsyncAskQuestionCompletionAction(super::AsyncAskQuestionCompletionAction),
        #[prost(message, tag = "10")]
        CancelSubagentAction(super::CancelSubagentAction),
        #[prost(message, tag = "12")]
        BackgroundTaskCompletionAction(super::BackgroundTaskCompletionAction),
        #[prost(message, tag = "13")]
        BackgroundShellAction(super::BackgroundShellAction),
        #[prost(message, tag = "14")]
        BackgroundSubagentAction(super::BackgroundSubagentAction),
        #[prost(message, tag = "16")]
        SubscriptionNotificationAction(super::SubscriptionNotificationAction),
        #[prost(message, tag = "18")]
        GoalContinuationAction(super::GoalContinuationAction),
        #[prost(message, tag = "19")]
        InjectContextAction(super::InjectContextAction),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct InjectContextAction {}

#[derive(Clone, PartialEq, Message)]
pub struct ShellCommand {
    #[prost(string, tag = "1")]
    pub command: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ShellCommandAction {
    #[prost(message, optional, tag = "1")]
    pub shell_command: Option<ShellCommand>,
    #[prost(string, tag = "2")]
    pub exec_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct StartPlanAction {
    #[prost(message, optional, tag = "1")]
    pub user_message: Option<UserMessage>,
    #[prost(message, optional, tag = "2")]
    pub request_context: Option<RequestContext>,
    #[prost(bool, tag = "3")]
    pub is_spec: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct AsyncAskQuestionCompletionAction {
    #[prost(string, tag = "1")]
    pub original_tool_call_id: String,
    #[prost(message, optional, tag = "2")]
    pub original_args: Option<crate::agent_proto::AskQuestionArgs>,
    #[prost(message, optional, tag = "3")]
    pub result: Option<crate::agent_proto::AskQuestionResult>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CancelSubagentAction {
    #[prost(string, tag = "1")]
    pub subagent_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct BackgroundShellAction {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct BackgroundSubagentAction {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubscriptionNotificationAction {
    #[prost(message, repeated, tag = "1")]
    pub notifications: Vec<UserMessage>,
    #[prost(message, optional, tag = "2")]
    pub request_context: Option<RequestContext>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GoalContinuationAction {}

#[derive(Clone, PartialEq, Message)]
pub struct PrewarmRequest {
    #[prost(message, optional, tag = "1")]
    pub model_details: Option<ModelDetails>,
    #[prost(string, optional, tag = "2")]
    pub conversation_id: Option<String>,
    #[prost(message, optional, tag = "9")]
    pub requested_model: Option<RequestedModel>,
}

#[derive(Clone, PartialEq, Message)]
pub struct NameAgentRequest {
    #[prost(string, tag = "1")]
    pub user_message: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct NameAgentResponse {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetDefaultModelForCliRequest {}

#[derive(Clone, PartialEq, Message)]
pub struct GetDefaultModelForCliResponse {
    #[prost(message, optional, tag = "1")]
    pub model: Option<ModelDetails>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetSignedUrlForAttachedMediaRequest {
    #[prost(string, optional, tag = "1")]
    pub key: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub mime_type: Option<String>,
    #[prost(string, tag = "3")]
    pub conversation_id: String,
    #[prost(int64, optional, tag = "4")]
    pub content_length_bytes: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetSignedUrlForAttachedMediaResponse {
    #[prost(string, tag = "1")]
    pub key: String,
    #[prost(string, tag = "2")]
    pub post_url: String,
    #[prost(string, tag = "3")]
    pub get_url: String,
    #[prost(int64, tag = "4")]
    pub expires_at_unix_ms: i64,
    #[prost(int64, tag = "5")]
    pub refresh_after_unix_ms: i64,
    #[prost(map = "string, string", tag = "6")]
    pub post_fields: HashMap<String, String>,
    #[prost(string, tag = "7")]
    pub put_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SummarizeAction {}

#[derive(Clone, PartialEq, Message)]
pub struct ExecutePlanAction {
    #[prost(message, optional, tag = "1")]
    pub request_context: Option<RequestContext>,
    #[prost(string, optional, tag = "3")]
    pub plan_file_uri: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub plan_file_content: Option<String>,
    #[prost(int32, tag = "5")]
    pub execution_mode: i32,
    #[prost(string, optional, tag = "6")]
    pub kickoff_message_id: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub plan_id: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub plan_file_path: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResumeAction {
    #[prost(message, optional, tag = "2")]
    pub request_context: Option<RequestContext>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CancelAction {
    #[prost(string, tag = "1")]
    pub reason: String,
    #[prost(message, optional, tag = "3")]
    pub interrupted_pending_tool_call_resolutions:
        Option<InterruptedPendingToolCallResolutions>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InterruptedPendingToolCallResolution {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(oneof = "interrupted_pending_tool_call_resolution::Resolution", tags = "2, 3")]
    pub resolution: Option<interrupted_pending_tool_call_resolution::Resolution>,
}

pub mod interrupted_pending_tool_call_resolution {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Resolution {
        #[prost(message, tag = "2")]
        ShellResult(crate::agent_proto::ShellResult),
        #[prost(message, tag = "3")]
        TaskResult(crate::agent_proto::TaskResult),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct InterruptedPendingToolCallResolutions {
    #[prost(message, repeated, tag = "1")]
    pub resolutions: Vec<InterruptedPendingToolCallResolution>,
}

#[derive(Clone, PartialEq, Message)]
pub struct BackgroundTaskCompletion {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(int32, tag = "2")]
    pub kind: i32,
    #[prost(int32, tag = "3")]
    pub status: i32,
    #[prost(string, tag = "4")]
    pub title: String,
    #[prost(string, optional, tag = "5")]
    pub detail: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub output_path: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub thread_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct BackgroundTaskCompletionAction {
    #[prost(message, repeated, tag = "1")]
    pub completions: Vec<BackgroundTaskCompletion>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PromptTokenBreakdownCategory {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub label: String,
    #[prost(uint32, tag = "3")]
    pub estimated_tokens: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct PromptTokenBreakdownSnapshot {
    #[prost(uint32, tag = "1")]
    pub total_used_tokens: u32,
    #[prost(uint32, tag = "2")]
    pub max_tokens: u32,
    #[prost(message, repeated, tag = "3")]
    pub categories: Vec<PromptTokenBreakdownCategory>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationTokenDetails {
    #[prost(uint32, tag = "1")]
    pub used_tokens: u32,
    #[prost(uint32, tag = "2")]
    pub max_tokens: u32,
    #[prost(message, optional, tag = "3")]
    pub breakdown: Option<PromptTokenBreakdownSnapshot>,
    #[prost(message, optional, tag = "4")]
    pub prompt_context_usage_tree: Option<PromptContextUsageTree>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub prompt_context_usage_snapshot_blob_id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PromptContextNode {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, optional, tag = "2")]
    pub parent_id: Option<String>,
    #[prost(string, tag = "3")]
    pub kind: String,
    #[prost(string, tag = "4")]
    pub label: String,
    #[prost(string, tag = "5")]
    pub category_id: String,
    #[prost(uint32, tag = "6")]
    pub estimated_tokens: u32,
    #[prost(uint32, tag = "7")]
    pub character_count: u32,
    #[prost(bool, tag = "9")]
    pub content_available: bool,
    #[prost(string, optional, tag = "12")]
    pub inline_content: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PromptContextUsageTree {
    #[prost(uint32, tag = "1")]
    pub schema_version: u32,
    #[prost(message, repeated, tag = "2")]
    pub nodes: Vec<PromptContextNode>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PromptContextUsageSnapshot {
    #[prost(message, optional, tag = "1")]
    pub prompt_context_usage_tree: Option<PromptContextUsageTree>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub root_prompt_messages_json: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetPromptContextUsageRequest {
    #[prost(string, tag = "1")]
    pub conversation_id: String,
    #[prost(bytes = "vec", tag = "2")]
    pub snapshot_blob_id: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetPromptContextUsageResponse {
    #[prost(message, optional, tag = "1")]
    pub snapshot: Option<PromptContextUsageSnapshot>,
}

#[derive(Clone, PartialEq, Message)]
pub struct BlobEntry {
    #[prost(bytes = "vec", tag = "1")]
    pub id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct UploadConversationBlobsRequest {
    #[prost(string, tag = "1")]
    pub conversation_id: String,
    #[prost(message, repeated, tag = "2")]
    pub blobs: Vec<BlobEntry>,
    #[prost(int32, tag = "3")]
    pub chunk_index: i32,
    #[prost(int32, tag = "4")]
    pub total_chunks: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct UploadConversationBlobsResponse {}

#[derive(Clone, PartialEq, Message)]
pub struct NotifyConversationCloneRequest {
    #[prost(string, tag = "1")]
    pub conversation_id: String,
    #[prost(string, tag = "2")]
    pub source_conversation_id: String,
    #[prost(string, tag = "3")]
    pub source_request_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct NotifyConversationCloneResponse {}

#[derive(Clone, PartialEq, Message)]
pub struct McpTools {
    #[prost(message, repeated, tag = "1")]
    pub mcp_tools: Vec<McpToolDef>,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpToolDef {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(string, tag = "3")]
    pub provider_identifier: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConversationStateStructure {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub root_prompt_messages_json: Vec<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub todos: Vec<Vec<u8>>,
    #[prost(string, repeated, tag = "4")]
    pub pending_tool_calls: Vec<String>,
    #[prost(message, optional, tag = "5")]
    pub token_details: Option<ConversationTokenDetails>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub summary: Option<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "8")]
    pub turns: Vec<Vec<u8>>,
    #[prost(string, repeated, tag = "9")]
    pub previous_workspace_uris: Vec<String>,
    #[prost(int32, optional, tag = "10")]
    pub mode: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "11")]
    pub summary_archive: Option<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "13")]
    pub summary_archives: Vec<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub plan: Option<Vec<u8>>,
    #[prost(string, optional, tag = "19")]
    pub active_branch_name: Option<String>,
    #[prost(message, repeated, tag = "21")]
    pub tracked_git_repo_branches: Vec<TrackedGitRepo>,
    #[prost(string, optional, tag = "22")]
    pub agent_type: Option<String>,
    #[prost(map = "string, bytes", tag = "12")]
    pub file_states: HashMap<String, Vec<u8>>,
    #[prost(map = "string, message", tag = "16")]
    pub subagent_states: HashMap<String, SubagentPersistedState>,
    #[prost(string, repeated, tag = "18")]
    pub read_paths: Vec<String>,
    #[prost(map = "string, message", tag = "20")]
    pub plans: HashMap<String, PlanRegistryEntry>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PlanRegistryEntry {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub path: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentPersistedState {
    #[prost(string, optional, tag = "5")]
    pub model_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TrackedGitRepo {
    #[prost(string, tag = "1")]
    pub repo_path: String,
    #[prost(string, tag = "2")]
    pub branch_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetBlobArgs {
    #[prost(bytes = "vec", tag = "1")]
    pub blob_id: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetBlobResult {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub blob_data: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SetBlobArgs {
    #[prost(bytes = "vec", tag = "1")]
    pub blob_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub blob_data: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SetBlobResult {}

#[derive(Clone, PartialEq, Message)]
pub struct KvClientMessage {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(oneof = "kv_client_message::Message", tags = "2, 3")]
    pub message: Option<kv_client_message::Message>,
}

pub mod kv_client_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "2")]
        GetBlobResult(super::GetBlobResult),
        #[prost(message, tag = "3")]
        SetBlobResult(super::SetBlobResult),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct KvServerMessage {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(oneof = "kv_server_message::Message", tags = "2, 3")]
    pub message: Option<kv_server_message::Message>,
}

pub mod kv_server_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "2")]
        GetBlobArgs(super::GetBlobArgs),
        #[prost(message, tag = "3")]
        SetBlobArgs(super::SetBlobArgs),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct InteractionQuery {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(oneof = "interaction_query::Query", tags = "2, 3, 4, 7, 8, 9, 10, 11, 12, 13, 14")]
    pub query: Option<interaction_query::Query>,
}

pub mod interaction_query {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Query {
        #[prost(message, tag = "2")]
        WebSearchRequestQuery(crate::agent_proto::WebSearchRequestQuery),
        #[prost(message, tag = "3")]
        AskQuestionInteractionQuery(crate::agent_proto::AskQuestionInteractionQuery),
        #[prost(message, tag = "4")]
        SwitchModeRequestQuery(crate::agent_proto::SwitchModeRequestQuery),
        #[prost(message, tag = "7")]
        CreatePlanRequestQuery(crate::agent_proto::CreatePlanRequestQuery),
        #[prost(message, tag = "8")]
        SetupVmEnvironmentArgs(super::SetupVmEnvironmentArgs),
        #[prost(message, tag = "9")]
        WebFetchRequestQuery(crate::agent_proto::WebFetchRequestQuery),
        #[prost(message, tag = "10")]
        PrManagementRequestQuery(super::PrManagementRequestQuery),
        #[prost(message, tag = "11")]
        McpAuthRequestQuery(super::McpAuthRequestQuery),
        #[prost(message, tag = "12")]
        GenerateImageRequestQuery(crate::agent_proto::GenerateImageRequestQuery),
        #[prost(message, tag = "13")]
        ReplaceEnvArgs(super::ReplaceEnvArgs),
        #[prost(message, tag = "14")]
        ConnectScmRequestQuery(super::ConnectScmRequestQuery),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct SetupVmEnvironmentArgs {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SetupVmEnvironmentResult {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct PrManagementRequestQuery {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct PrManagementResult {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpAuthRequestQuery {
    #[prost(string, tag = "1")]
    pub server_identifier: String,
    #[prost(string, tag = "2")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct McpAuthRequestResponse {
    #[prost(string, optional, tag = "2")]
    pub rejected_reason: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReplaceEnvArgs {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ReplaceEnvResult {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConnectScmRequestQuery {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ConnectScmRequestResponse {
    #[prost(string, tag = "1")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct InteractionResponse {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(oneof = "interaction_response::Result", tags = "2, 3, 4, 7, 8, 9, 10, 11, 12, 13, 14")]
    pub result: Option<interaction_response::Result>,
}

pub mod interaction_response {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "2")]
        WebSearchRequestResponse(crate::agent_proto::WebSearchRequestResponse),
        #[prost(message, tag = "3")]
        AskQuestionInteractionResponse(crate::agent_proto::AskQuestionInteractionResponse),
        #[prost(message, tag = "4")]
        SwitchModeRequestResponse(crate::agent_proto::SwitchModeRequestResponse),
        #[prost(message, tag = "7")]
        CreatePlanRequestResponse(crate::agent_proto::CreatePlanRequestResponse),
        #[prost(message, tag = "8")]
        SetupVmEnvironmentResult(super::SetupVmEnvironmentResult),
        #[prost(message, tag = "9")]
        WebFetchRequestResponse(crate::agent_proto::WebFetchRequestResponse),
        #[prost(message, tag = "10")]
        PrManagementResult(super::PrManagementResult),
        #[prost(message, tag = "11")]
        McpAuthRequestResponse(super::McpAuthRequestResponse),
        #[prost(message, tag = "12")]
        GenerateImageRequestResponse(crate::agent_proto::GenerateImageRequestResponse),
        #[prost(message, tag = "13")]
        ReplaceEnvResult(super::ReplaceEnvResult),
        #[prost(message, tag = "14")]
        ConnectScmRequestResponse(super::ConnectScmRequestResponse),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentRunRequest {
    #[prost(message, optional, tag = "1")]
    pub conversation_state: Option<ConversationStateStructure>,
    #[prost(message, optional, tag = "2")]
    pub action: Option<ConversationAction>,
    #[prost(message, optional, tag = "3")]
    pub model_details: Option<ModelDetails>,
    #[prost(message, optional, tag = "4")]
    pub mcp_tools: Option<McpTools>,
    #[prost(string, optional, tag = "5")]
    pub conversation_id: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub custom_system_prompt: Option<String>,
    #[prost(message, optional, tag = "9")]
    pub requested_model: Option<RequestedModel>,
    #[prost(string, optional, tag = "11")]
    pub subagent_type_name: Option<String>,
    #[prost(message, repeated, tag = "20")]
    pub subagent_model_overrides: Vec<SubagentModelOverride>,
    #[prost(bool, optional, tag = "27")]
    pub client_supports_prompt_context_usage_rpc: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SubagentModelOverride {
    #[prost(string, tag = "1")]
    pub subagent_type: String,
    #[prost(oneof = "subagent_model_override::Selection", tags = "2, 3, 4")]
    pub selection: Option<subagent_model_override::Selection>,
}

pub mod subagent_model_override {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Selection {
        #[prost(message, tag = "2")]
        Model(super::RequestedModel),
        #[prost(bool, tag = "3")]
        Inherit(bool),
        #[prost(bool, tag = "4")]
        Disabled(bool),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentClientMessage {
    #[prost(oneof = "agent_client_message::Message", tags = "1, 2, 3, 4, 5, 6, 7, 8")]
    pub message: Option<agent_client_message::Message>,
}

pub mod agent_client_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        RunRequest(super::AgentRunRequest),
        #[prost(message, tag = "2")]
        ExecClientMessage(crate::agent_proto::ExecClientMessage),
        #[prost(message, tag = "3")]
        KvClientMessage(super::KvClientMessage),
        #[prost(message, tag = "4")]
        ConversationAction(super::ConversationAction),
        #[prost(message, tag = "5")]
        ExecClientControlMessage(super::ExecClientControlMessage),
        #[prost(message, tag = "6")]
        InteractionResponse(super::InteractionResponse),
        #[prost(message, tag = "7")]
        ClientHeartbeat(super::ClientHeartbeat),
        #[prost(message, tag = "8")]
        PrewarmRequest(super::PrewarmRequest),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecClientControlMessage {
    #[prost(oneof = "exec_client_control_message::Message", tags = "1, 2, 3")]
    pub message: Option<exec_client_control_message::Message>,
}

pub mod exec_client_control_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        StreamClose(super::ExecClientStreamClose),
        #[prost(message, tag = "2")]
        Throw(super::ExecClientThrow),
        #[prost(message, tag = "3")]
        Heartbeat(super::ExecClientHeartbeat),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecClientStreamClose {
    #[prost(uint32, tag = "1")]
    pub id: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecClientThrow {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "2")]
    pub error: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecClientHeartbeat {
    #[prost(uint32, tag = "1")]
    pub id: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ClientHeartbeat {}

#[derive(Clone, PartialEq, Message)]
pub struct TextDeltaUpdate {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(bool, tag = "2")]
    pub is_server_notice: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct ThinkingDeltaUpdate {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(int32, optional, tag = "2")]
    pub thinking_style: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TurnEndedUpdate {
    #[prost(int64, optional, tag = "1")]
    pub input_tokens: Option<i64>,
    #[prost(int64, optional, tag = "2")]
    pub output_tokens: Option<i64>,
    #[prost(int64, optional, tag = "3")]
    pub cache_read_tokens: Option<i64>,
    #[prost(int64, optional, tag = "4")]
    pub cache_write_tokens: Option<i64>,
    #[prost(int64, optional, tag = "5")]
    pub reasoning_tokens: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
pub struct HeartbeatUpdate {}

#[derive(Clone, PartialEq, Message)]
pub struct TokenDeltaUpdate {
    #[prost(int32, tag = "1")]
    pub tokens: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct InteractionUpdate {
    #[prost(
        oneof = "interaction_update::Message",
        tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17"
    )]
    pub message: Option<interaction_update::Message>,
}

pub mod interaction_update {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        TextDelta(super::TextDeltaUpdate),
        #[prost(message, tag = "2")]
        ToolCallStarted(crate::agent_proto::ToolCallStartedUpdate),
        #[prost(message, tag = "3")]
        ToolCallCompleted(crate::agent_proto::ToolCallCompletedUpdate),
        #[prost(message, tag = "4")]
        ThinkingDelta(super::ThinkingDeltaUpdate),
        #[prost(message, tag = "5")]
        ThinkingCompleted(super::ThinkingCompletedUpdate),
        #[prost(message, tag = "6")]
        UserMessageAppended(super::UserMessageAppendedUpdate),
        #[prost(message, tag = "7")]
        PartialToolCall(crate::agent_proto::PartialToolCallUpdate),
        #[prost(message, tag = "9")]
        Summary(super::SummaryUpdate),
        #[prost(message, tag = "10")]
        SummaryStarted(super::SummaryStartedUpdate),
        #[prost(message, tag = "11")]
        SummaryCompleted(super::SummaryCompletedUpdate),
        #[prost(message, tag = "8")]
        TokenDelta(super::TokenDeltaUpdate),
        #[prost(message, tag = "13")]
        Heartbeat(super::HeartbeatUpdate),
        #[prost(message, tag = "14")]
        TurnEnded(super::TurnEndedUpdate),
        #[prost(message, tag = "15")]
        ToolCallDelta(crate::agent_proto::ToolCallDeltaUpdate),
        #[prost(message, tag = "16")]
        StepStarted(super::StepStartedUpdate),
        #[prost(message, tag = "17")]
        StepCompleted(super::StepCompletedUpdate),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct UserMessageAppendedUpdate {
    #[prost(message, optional, tag = "1")]
    pub user_message: Option<UserMessage>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SummaryUpdate {
    #[prost(string, tag = "1")]
    pub summary: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct SummaryStartedUpdate {}

#[derive(Clone, PartialEq, Message)]
pub struct SummaryCompletedUpdate {
    #[prost(string, optional, tag = "1")]
    pub hook_message: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StepStartedUpdate {
    #[prost(uint64, tag = "1")]
    pub step_id: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct StepCompletedUpdate {
    #[prost(uint64, tag = "1")]
    pub step_id: u64,
    #[prost(int64, tag = "2")]
    pub step_duration_ms: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ThinkingCompletedUpdate {
    #[prost(int32, tag = "1")]
    pub thinking_duration_ms: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecServerAbort {
    #[prost(uint32, tag = "1")]
    pub id: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ExecServerControlMessage {
    #[prost(oneof = "exec_server_control_message::Message", tags = "1")]
    pub message: Option<exec_server_control_message::Message>,
}

pub mod exec_server_control_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        Abort(super::ExecServerAbort),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentServerMessage {
    #[prost(oneof = "agent_server_message::Message", tags = "1, 2, 3, 4, 5, 7")]
    pub message: Option<agent_server_message::Message>,
}

pub mod agent_server_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "1")]
        InteractionUpdate(super::InteractionUpdate),
        #[prost(message, tag = "2")]
        ExecServerMessage(crate::agent_proto::ExecServerMessage),
        #[prost(message, tag = "3")]
        ConversationCheckpointUpdate(super::ConversationStateStructure),
        #[prost(message, tag = "4")]
        KvServerMessage(super::KvServerMessage),
        #[prost(message, tag = "5")]
        ExecServerControlMessage(super::ExecServerControlMessage),
        #[prost(message, tag = "7")]
        InteractionQuery(super::InteractionQuery),
    }
}

#[derive(Debug, Clone)]
pub struct LocalRun {
    pub request_id: String,
    pub model_id: String,
    pub user_text: String,
    pub effort: Option<String>,
    pub fast: bool,
    pub workspace: Option<String>,
    pub os_version: String,
    pub shell: String,
    pub attached_files: Vec<(String, String)>,
    pub history: String,
    pub system_prompt: String,
    pub used_tokens: u32,
    pub max_tokens: u32,
    pub history_blob_ids: Vec<Vec<u8>>,
    pub mode: i32,
    pub conversation_id: Option<String>,
    pub subagent_type_name: Option<String>,
    pub time_zone: String,
    pub images: Vec<(String, String)>,
    pub history_images: Vec<(String, String)>,
    pub git_repos: Vec<(String, String)>,
    pub is_background_completion: bool,
    pub dynamic_tool_transition: bool,
    pub extra_prompt: String,
    pub web_search: bool,
    pub web_fetch: bool,
    pub read_lints: bool,
    pub mcp_meta_enabled: bool,
    pub supports_mcp_auth: bool,
    pub subagent_overrides: Vec<(String, SubagentOverride)>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentOverride {
    Inherit,
    Disabled,
    Model(String),
}

impl LocalRun {
    pub fn tool_flags(&self) -> crate::providers::ToolFlags {
        crate::providers::ToolFlags {
            web_search: self.web_search,
            web_fetch: self.web_fetch,
            read_lints: self.read_lints,
        }
    }
}

impl Default for LocalRun {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            model_id: String::new(),
            user_text: String::new(),
            effort: None,
            fast: false,
            workspace: None,
            os_version: String::new(),
            shell: String::new(),
            attached_files: Vec::new(),
            history: String::new(),
            system_prompt: String::new(),
            used_tokens: 0,
            max_tokens: 0,
            history_blob_ids: Vec::new(),
            mode: 1,
            conversation_id: None,
            subagent_type_name: None,
            time_zone: String::new(),
            images: Vec::new(),
            history_images: Vec::new(),
            git_repos: Vec::new(),
            is_background_completion: false,
            dynamic_tool_transition: false,
            extra_prompt: String::new(),
            web_search: true,
            web_fetch: true,
            read_lints: true,
            mcp_meta_enabled: false,
            supports_mcp_auth: false,
            subagent_overrides: Vec::new(),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedBidi {
    pub request_id: String,
    pub seqno: i64,
    pub run: Option<LocalRun>,
    pub heartbeat: bool,
    pub exec: Option<crate::agent_proto::ExecClientMessage>,
    pub cancel: bool,
    pub stream_close: Option<u32>,
    pub throw: Option<(u32, String)>,
    pub kv: Option<KvClientMessage>,
    pub interaction: Option<InteractionResponse>,
    pub queued_user: Option<String>,
    pub queued_background: bool,
}

#[derive(Debug, Clone, Default)]
pub struct QueuedUser {
    pub text: String,
    pub is_background: bool,
}

pub fn queue_user(
    map: &mut HashMap<String, VecDeque<QueuedUser>>,
    request_id: &str,
    text: String,
    is_background: bool,
) {
    if text.is_empty() {
        return;
    }
    map.entry(request_id.to_owned())
        .or_default()
        .push_back(QueuedUser {
            text,
            is_background,
        });
}

pub fn drain_queued(
    map: &mut HashMap<String, VecDeque<QueuedUser>>,
    request_id: &str,
) -> Option<QueuedUser> {
    let q = map.remove(request_id)?;
    if q.is_empty() {
        return None;
    }
    let mut text = String::new();
    let mut is_background = false;
    for item in q {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&item.text);
        is_background |= item.is_background;
    }
    Some(QueuedUser {
        text,
        is_background,
    })
}

pub fn ascii_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.is_empty() || text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for index in (0..bytes.len()).step_by(2) {
        let hi = from_hex(bytes[index])?;
        let lo = from_hex(bytes[index + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn body_mentions_injected(body: &[u8], enabled: &[String]) -> bool {
    enabled.iter().any(|id| {
        if id.is_empty() || id == "default" {
            return false;
        }
        let needle = injected_id(id);
        contains_bytes_or_hex(body, needle.as_bytes())
    })
}

pub fn contains_bytes_or_hex(buf: &[u8], text: &[u8]) -> bool {
    if text.is_empty() {
        return false;
    }
    if buf.windows(text.len()).any(|window| window == text) {
        return true;
    }
    let hex = ascii_hex(text);
    buf.windows(hex.len())
        .any(|window| window.eq_ignore_ascii_case(hex.as_bytes()))
}

pub fn decode_unary<M: Message + Default>(body: &[u8]) -> Option<M> {
    let (_framed, payload) = unwrap_connect(body);
    M::decode(payload).ok()
}

pub fn find_gb_model(buf: &[u8]) -> Option<String> {
    let start = buf.windows(3).position(|window| window == b"gb-")?;
    let rest = &buf[start..];
    let end = rest
        .iter()
        .position(|byte| {
            !(byte.is_ascii_alphanumeric()
                || matches!(
                    *byte,
                    b'-' | b'_' | b'.' | b'/' | b':' | b'[' | b']' | b'=' | b','
                ))
        })
        .unwrap_or(rest.len());
    let text = std::str::from_utf8(&rest[..end]).ok()?;
    (!text.is_empty()).then(|| text.to_owned())
}

pub fn scan_gb_model(body: &[u8]) -> Option<String> {
    if let Some(found) = find_gb_model(body) {
        return Some(found);
    }
    if let Some(request) = decode_unary::<BidiAppendRequest>(body) {
        if !request.data.is_empty() {
            if let Some(payload) = decode_hex(&request.data) {
                if let Some(found) = find_gb_model(&payload) {
                    return Some(found);
                }
            }
        }
        if let Some(found) = find_gb_model(&request.data_binary) {
            return Some(found);
        }
    }
    let hex = b"67622d";
    let start = body
        .windows(hex.len())
        .position(|window| window.eq_ignore_ascii_case(hex))?;
    let slice = &body[start..];
    let take = slice
        .iter()
        .take_while(|byte| byte.is_ascii_hexdigit())
        .count();
    let take = take - (take % 2);
    let decoded = decode_hex(std::str::from_utf8(&slice[..take]).ok()?)?;
    find_gb_model(&decoded)
}

pub fn decode_bidi_append(body: &[u8]) -> Option<DecodedBidi> {
    let request = decode_unary::<BidiAppendRequest>(body)?;
    let request_id = request.request_id.as_ref()?.request_id.clone();
    if request_id.is_empty() {
        return None;
    }
    let payload = if !request.data.is_empty() {
        decode_hex(&request.data)?
    } else if !request.data_binary.is_empty() {
        request.data_binary.clone()
    } else {
        return Some(DecodedBidi {
            request_id,
            seqno: request.append_seqno,
            run: None,
            heartbeat: false,
            exec: None,
            cancel: false,
            stream_close: None,
            throw: None,
            kv: None,
            interaction: None,
            queued_user: None,
            queued_background: false,
        });
    };
    let client = AgentClientMessage::decode(payload.as_slice()).ok();
    let inner = client.as_ref().and_then(|msg| msg.message.as_ref());
    let heartbeat = matches!(
        inner,
        Some(agent_client_message::Message::ClientHeartbeat(_))
            | Some(agent_client_message::Message::ExecClientControlMessage(
                ExecClientControlMessage {
                    message: Some(exec_client_control_message::Message::Heartbeat(_)),
                }
            ))
    );
    let exec = match inner {
        Some(agent_client_message::Message::ExecClientMessage(exec)) => Some(exec.clone()),
        _ => None,
    };
    let kv = match inner {
        Some(agent_client_message::Message::KvClientMessage(kv)) => Some(kv.clone()),
        _ => None,
    };
    let interaction = match inner {
        Some(agent_client_message::Message::InteractionResponse(resp)) => Some(resp.clone()),
        _ => None,
    };
    let throw = match inner {
        Some(agent_client_message::Message::ExecClientControlMessage(
            ExecClientControlMessage {
                message: Some(exec_client_control_message::Message::Throw(thrown)),
            },
        )) => Some((thrown.id, thrown.error.clone())),
        _ => None,
    };
    let cancel = match inner {
        Some(agent_client_message::Message::RunRequest(run)) => run_is_cancel(run),
        Some(agent_client_message::Message::ConversationAction(action)) => matches!(
            action.action.as_ref(),
            Some(conversation_action::Action::CancelAction(_))
        ),
        _ => false,
    };
    let queued_user = match inner {
        Some(agent_client_message::Message::ConversationAction(action)) => {
            conversation_action_user_text(action)
        }
        _ => None,
    };
    let stream_close = match inner {
        Some(agent_client_message::Message::ExecClientControlMessage(
            ExecClientControlMessage {
                message: Some(exec_client_control_message::Message::StreamClose(close)),
            },
        )) => Some(close.id),
        _ => None,
    };
    Some(DecodedBidi {
        request_id: request_id.clone(),
        seqno: request.append_seqno,
        run: local_run_from_agent_payload(request_id, &payload),
        heartbeat,
        exec,
        cancel,
        stream_close,
        throw,
        kv,
        interaction,
        queued_user,
        queued_background: false,
    })
}

pub fn local_run_from_agent_payload(request_id: String, payload: &[u8]) -> Option<LocalRun> {
    if let Ok(client) = AgentClientMessage::decode(payload) {
        match client.message {
            Some(agent_client_message::Message::RunRequest(run)) => {
                if run_is_cancel(&run) || run_is_inject(&run) {
                    return None;
                }
                return Some(local_run_from_request(request_id, &run, payload));
            }
            Some(agent_client_message::Message::ClientHeartbeat(_))
            | Some(agent_client_message::Message::ExecClientMessage(_))
            | Some(agent_client_message::Message::ExecClientControlMessage(_))
            | Some(agent_client_message::Message::KvClientMessage(_))
            | Some(agent_client_message::Message::InteractionResponse(_))
            | Some(agent_client_message::Message::ConversationAction(_))
            | Some(agent_client_message::Message::PrewarmRequest(_)) => return None,
            None => {}
        }
    }
    if let Ok(run) = AgentRunRequest::decode(payload) {
        return Some(local_run_from_request(request_id, &run, payload));
    }
    None
}

fn run_is_cancel(run: &AgentRunRequest) -> bool {
    matches!(
        run.action.as_ref().and_then(|action| action.action.as_ref()),
        Some(conversation_action::Action::CancelAction(_))
    )
}

fn run_is_inject(run: &AgentRunRequest) -> bool {
    matches!(
        run.action.as_ref().and_then(|action| action.action.as_ref()),
        Some(conversation_action::Action::InjectContextAction(_))
            | Some(conversation_action::Action::CancelSubagentAction(_))
    )
}

fn execute_plan_user_text(plan: &ExecutePlanAction) -> String {
    let content = plan.plan_file_content.clone().unwrap_or_default();
    if !content.is_empty() {
        return format!("Execute the following plan:\n\n{content}");
    }
    let uri = plan
        .plan_file_uri
        .clone()
        .filter(|value| !value.is_empty())
        .or_else(|| plan.plan_file_path.clone().filter(|value| !value.is_empty()))
        .unwrap_or_default();
    if uri.is_empty() {
        "Execute the current plan.".into()
    } else {
        format!("Execute the plan at {uri}")
    }
}

fn local_run_from_request(request_id: String, run: &AgentRunRequest, payload: &[u8]) -> LocalRun {
    let mut model_id = run
        .requested_model
        .as_ref()
        .map(|model| model.model_id.as_str())
        .filter(|id| !id.is_empty())
        .or_else(|| {
            run.model_details
                .as_ref()
                .map(|model| model.model_id.as_str())
                .filter(|id| !id.is_empty())
        })
        .unwrap_or("")
        .to_owned();
    if model_id.is_empty() {
        if let Some(found) = find_gb_model(payload) {
            model_id = found;
        }
    }
    let conversation = run.action.as_ref();
    let user_action = conversation.and_then(|action| action.action.as_ref());
    let user_text = user_action
        .and_then(|action| match action {
            conversation_action::Action::UserMessageAction(user) => {
                user.user_message.as_ref().map(user_message_text)
            }
            conversation_action::Action::ExecutePlanAction(plan) => {
                Some(execute_plan_user_text(plan))
            }
            conversation_action::Action::SummarizeAction(_) => Some(
                "Summarize the conversation so far, focusing on decisions, remaining tasks, and important file paths.".into(),
            ),
            conversation_action::Action::BackgroundTaskCompletionAction(done) => {
                Some(format_background_completions(done))
            }
            conversation_action::Action::ShellCommandAction(cmd) => {
                Some(format_shell_command_action(cmd))
            }
            conversation_action::Action::StartPlanAction(plan) => plan
                .user_message
                .as_ref()
                .map(user_message_text)
                .filter(|text| !text.trim().is_empty())
                .or_else(|| Some("Create a plan for the current task.".into())),
            conversation_action::Action::AsyncAskQuestionCompletionAction(done) => {
                Some(format_async_ask_completion(done))
            }
            conversation_action::Action::BackgroundShellAction(shell) => Some(format!(
                "<agent_notification>\nbackground shell tool_call_id: {}\n</agent_notification>",
                shell.tool_call_id
            )),
            conversation_action::Action::BackgroundSubagentAction(sub) => Some(format!(
                "<agent_notification>\nbackground subagent tool_call_id: {}\n</agent_notification>",
                sub.tool_call_id
            )),
            conversation_action::Action::SubscriptionNotificationAction(note) => {
                Some(format_subscription_notifications(note))
            }
            conversation_action::Action::GoalContinuationAction(_) => {
                Some("Continue working on the current goal.".into())
            }
            _ => None,
        })
        .unwrap_or_default();
    let interrupted = user_action.and_then(|action| match action {
        conversation_action::Action::UserMessageAction(user) => {
            user.interrupted_pending_tool_call_resolutions.as_ref()
        }
        conversation_action::Action::CancelAction(cancel) => {
            cancel.interrupted_pending_tool_call_resolutions.as_ref()
        }
        _ => None,
    });
    let user_text = {
        let extra = format_interrupted(interrupted);
        if extra.is_empty() {
            user_text
        } else if user_text.is_empty() {
            extra
        } else {
            format!("{user_text}\n\n{extra}")
        }
    };
    let ctx = user_action
        .and_then(|action| match action {
            conversation_action::Action::UserMessageAction(user) => user.request_context.as_ref(),
            conversation_action::Action::ResumeAction(resume) => resume.request_context.as_ref(),
            conversation_action::Action::ExecutePlanAction(plan) => plan.request_context.as_ref(),
            conversation_action::Action::StartPlanAction(plan) => plan.request_context.as_ref(),
            conversation_action::Action::SubscriptionNotificationAction(note) => {
                note.request_context.as_ref()
            }
            _ => None,
        })
        .or_else(|| {
            conversation
                .and_then(|action| action.request_context_parts.as_ref())
                .and_then(|parts| parts.dynamic_context.as_ref())
        });
    let workspace = ctx.and_then(|ctx| ctx.env.as_ref()).and_then(|env| {
        if !env.project_folder.is_empty() {
            Some(env.project_folder.clone())
        } else {
            env.workspace_paths.first().cloned()
        }
    });
    let os_version = ctx
        .and_then(|ctx| ctx.env.as_ref())
        .map(|env| env.os_version.clone())
        .unwrap_or_default();
    let shell = ctx
        .and_then(|ctx| ctx.env.as_ref())
        .map(|env| env.shell.clone())
        .unwrap_or_default();
    let time_zone = ctx
        .and_then(|ctx| ctx.env.as_ref())
        .map(|env| env.time_zone.clone())
        .unwrap_or_default();
    let git_repos = ctx
        .map(|ctx| {
            ctx.git_repos
                .iter()
                .filter(|repo| !repo.path.is_empty())
                .map(|repo| (repo.path.clone(), repo.branch_name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let attached_files = ctx
        .map(|ctx| {
            ctx.file_contents
                .iter()
                .map(|(path, body)| (path.clone(), body.clone()))
                .collect()
        })
        .unwrap_or_default();
    let (effort, fast) = parameters_from(run.requested_model.as_ref());
    let history = user_action
        .and_then(|action| match action {
            conversation_action::Action::UserMessageAction(user) => Some(format_history(user)),
            _ => None,
        })
        .unwrap_or_default();
    let mode = user_action
        .and_then(|action| match action {
            conversation_action::Action::UserMessageAction(user) => {
                user.user_message.as_ref().map(|msg| msg.mode)
            }
            conversation_action::Action::StartPlanAction(plan) => {
                plan.user_message.as_ref().map(|msg| msg.mode).or(Some(3))
            }
            _ => None,
        })
        .filter(|mode| *mode != 0)
        .or_else(|| {
            matches!(
                user_action,
                Some(conversation_action::Action::StartPlanAction(_))
            )
            .then_some(3)
        })
        .unwrap_or(1);
    let images = user_action
        .and_then(|action| match action {
            conversation_action::Action::UserMessageAction(user) => user.user_message.as_ref(),
            _ => None,
        })
        .map(selected_images)
        .unwrap_or_default();
    let history_images = match user_action {
        Some(conversation_action::Action::UserMessageAction(user)) => history_images(user),
        _ => Vec::new(),
    };
    let is_background_completion = matches!(
        user_action,
        Some(conversation_action::Action::BackgroundTaskCompletionAction(_))
            | Some(conversation_action::Action::AsyncAskQuestionCompletionAction(_))
            | Some(conversation_action::Action::BackgroundShellAction(_))
            | Some(conversation_action::Action::BackgroundSubagentAction(_))
            | Some(conversation_action::Action::SubscriptionNotificationAction(_))
    );
    let dynamic_tool_transition = run
        .mcp_tools
        .as_ref()
        .is_some_and(|tools| !tools.mcp_tools.is_empty());
    let mut system_prompt = run.custom_system_prompt.clone().unwrap_or_default();
    let parts_text = hydrate_context_parts(conversation);
    if !parts_text.is_empty() {
        if !system_prompt.is_empty() {
            system_prompt.push('\n');
        }
        system_prompt.push_str(&parts_text);
    }
    let (used_tokens, max_tokens) = run
        .conversation_state
        .as_ref()
        .and_then(|state| state.token_details.as_ref())
        .map(|details| (details.used_tokens, details.max_tokens))
        .unwrap_or((0, 0));
    let extra_ctx = format_git_and_skills(ctx, max_tokens);
    if !extra_ctx.is_empty() {
        if !system_prompt.is_empty() {
            system_prompt.push('\n');
        }
        system_prompt.push_str(&extra_ctx);
    }
    let history_blob_ids = run
        .conversation_state
        .as_ref()
        .map(|state| {
            if !state.root_prompt_messages_json.is_empty() {
                state.root_prompt_messages_json.clone()
            } else {
                state.turns.clone()
            }
        })
        .unwrap_or_default();
    let user_msg = user_msg_from_action(user_action);
    LocalRun {
        request_id,
        model_id,
        user_text,
        effort,
        fast,
        workspace,
        os_version,
        shell,
        attached_files,
        history,
        system_prompt,
        used_tokens,
        max_tokens,
        history_blob_ids,
        mode,
        conversation_id: {
            let id = run
                .conversation_id
                .clone()
                .filter(|id| !id.is_empty());
            if let Some(id) = id.as_deref() {
                crate::blob::remember_conversation(id);
            }
            id
        },
        subagent_type_name: run
            .subagent_type_name
            .clone()
            .filter(|name| !name.is_empty()),
        time_zone,
        images,
        history_images,
        git_repos,
        is_background_completion,
        dynamic_tool_transition: dynamic_tool_transition
            || ctx
                .and_then(|ctx| ctx.mcp_meta_tool_options.as_ref())
                .is_some_and(|opt| opt.enabled && !opt.mcp_descriptors.is_empty()),
        extra_prompt: format_selected_and_request(ctx, user_msg),
        web_search: ctx.and_then(|c| c.web_search_enabled).unwrap_or(true),
        web_fetch: ctx.and_then(|c| c.web_fetch_enabled).unwrap_or(true),
        read_lints: ctx.and_then(|c| c.read_lints_enabled).unwrap_or(true),
        mcp_meta_enabled: ctx
            .and_then(|c| c.mcp_meta_tool_options.as_ref())
            .is_some_and(|opt| opt.enabled),
        supports_mcp_auth: ctx.and_then(|c| c.supports_mcp_auth).unwrap_or(false),
        subagent_overrides: decode_subagent_overrides(&run.subagent_model_overrides),
        cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}

fn decode_subagent_overrides(list: &[SubagentModelOverride]) -> Vec<(String, SubagentOverride)> {
    list.iter()
        .filter(|item| !item.subagent_type.is_empty())
        .filter_map(|item| {
            let sel = match &item.selection {
                Some(subagent_model_override::Selection::Model(model))
                    if !model.model_id.is_empty() =>
                {
                    SubagentOverride::Model(model.model_id.clone())
                }
                Some(subagent_model_override::Selection::Inherit(true)) => {
                    SubagentOverride::Inherit
                }
                Some(subagent_model_override::Selection::Disabled(true)) => {
                    SubagentOverride::Disabled
                }
                _ => return None,
            };
            Some((item.subagent_type.clone(), sel))
        })
        .collect()
}

fn history_images(action: &UserMessageAction) -> Vec<(String, String)> {
    let Some(hist) = &action.conversation_history else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in &hist.messages {
        match &item.message {
            Some(conversation_history_message::Message::User(user)) => {
                for part in &user.content {
                    if let Some(conversation_history_user_content::Content::Image(image)) =
                        &part.content
                    {
                        push_history_image(&mut out, image);
                    }
                }
            }
            Some(conversation_history_message::Message::Tool(tool)) => {
                for part in &tool.content {
                    if let Some(conversation_history_tool_result_content::Content::Image(image)) =
                        &part.content
                    {
                        push_history_image(&mut out, image);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn push_history_image(out: &mut Vec<(String, String)>, image: &ConversationHistoryImage) {
    if image.data.is_empty() {
        return;
    }
    let mime = image
        .mime_type
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("image/png");
    out.push((mime.to_owned(), image.data.clone()));
}

fn user_msg_from_action<'a>(
    action: Option<&'a conversation_action::Action>,
) -> Option<&'a UserMessage> {
    match action {
        Some(conversation_action::Action::UserMessageAction(user)) => user.user_message.as_ref(),
        Some(conversation_action::Action::StartPlanAction(plan)) => plan.user_message.as_ref(),
        _ => None,
    }
}

fn user_message_text(user: &UserMessage) -> String {
    if !user.text.trim().is_empty() {
        return user.text.clone();
    }
    if let Some(id) = user.text_blob_id.as_ref().filter(|id| !id.is_empty()) {
        let text = hydrate_blob_utf8(id);
        if !text.trim().is_empty() {
            return text;
        }
    }
    user.rich_text.clone().unwrap_or_default()
}

fn format_selected_and_request(
    ctx: Option<&RequestContext>,
    user: Option<&UserMessage>,
) -> String {
    let mut out = String::new();
    if let Some(user) = user {
        if let Some(sel) = user.selected_context.as_ref() {
            out.push_str(&format_selected_context(sel));
        }
    }
    if let Some(ctx) = ctx {
        out.push_str(&format_request_extras(ctx));
    }
    out
}

fn format_selected_context(sel: &SelectedContext) -> String {
    let mut out = String::new();
    if let Some(invocation_context::Data::IdeState(ide)) =
        sel.invocation_context.as_ref().and_then(|ctx| ctx.data.as_ref())
    {
        if !ide.visible_files.is_empty() || !ide.recently_viewed_files.is_empty() {
            out.push_str("<ide_state>\n");
            for file in &ide.visible_files {
                out.push_str(&format!("visible: {}\n", file.path));
            }
            for file in &ide.recently_viewed_files {
                out.push_str(&format!("recent: {}\n", file.path));
            }
            out.push_str("</ide_state>\n");
        }
    }
    let mut pending_blobs = 0u32;
    let mut extras: Vec<String> = sel.extra_context.clone();
    for entry in &sel.extra_context_entries {
        match &entry.data_or_blob_id {
            Some(extra_context_entry::DataOrBlobId::Data(data)) if !data.is_empty() => {
                extras.push(data.clone());
            }
            Some(extra_context_entry::DataOrBlobId::BlobId(id)) if !id.is_empty() => {
                let text = hydrate_blob_utf8(id);
                if text.is_empty() {
                    pending_blobs += 1;
                } else {
                    extras.push(text);
                }
            }
            _ => {}
        }
    }
    if !extras.is_empty() {
        out.push_str("<extra_context>\n");
        for item in extras {
            out.push_str(&clip_utf8(&item, 4_000));
            out.push('\n');
        }
        out.push_str("</extra_context>\n");
    }
    if pending_blobs > 0 {
        out.push_str(&format!(
            "<extra_context_pending blob_count=\"{pending_blobs}\" />\n"
        ));
    }
    if !sel.code_selections.is_empty() {
        out.push_str("<code_selections>\n");
        for sel in &sel.code_selections {
            out.push_str(&format!(
                "<code path=\"{}\">\n{}\n</code>\n",
                sel.path,
                clip_utf8(&sel.content, 4_000)
            ));
        }
        out.push_str("</code_selections>\n");
    }
    if !sel.terminal_selections.is_empty() {
        out.push_str("<terminal_selections>\n");
        for term in &sel.terminal_selections {
            out.push_str(&clip_utf8(&term.content, 2_000));
            out.push('\n');
        }
        out.push_str("</terminal_selections>\n");
    }
    if !sel.cursor_commands.is_empty() {
        out.push_str("<cursor_commands>\n");
        for cmd in &sel.cursor_commands {
            out.push_str(&format!("{}: {}\n", cmd.name, clip_utf8(&cmd.content, 2_000)));
        }
        out.push_str("</cursor_commands>\n");
    }
    if !sel.documentations.is_empty() {
        out.push_str("<attached_docs>\n");
        for doc in &sel.documentations {
            out.push_str(&format!("{} {}\n", doc.doc_id, doc.name));
        }
        out.push_str("</attached_docs>\n");
    }
    if !sel.external_links.is_empty() {
        out.push_str("<external_links>\n");
        for link in &sel.external_links {
            out.push_str(&link.url);
            out.push('\n');
        }
        out.push_str("</external_links>\n");
    }
    if !sel.selected_subagents.is_empty() {
        out.push_str("<attached_subagents>\n");
        for agent in &sel.selected_subagents {
            out.push_str(&agent.name);
            out.push('\n');
        }
        out.push_str("</attached_subagents>\n");
    }
    if !sel.selected_browsers.is_empty() {
        out.push_str("<attached_browsers>\n");
        for browser in &sel.selected_browsers {
            out.push_str(&browser.url);
            out.push('\n');
        }
        out.push_str("</attached_browsers>\n");
    }
    if !sel.selected_skills.is_empty() {
        out.push_str("<manually_attached_skills>\n");
        for skill in &sel.selected_skills {
            out.push_str(&format!(
                "<skill path=\"{}\">{}</skill>\n",
                skill.full_path,
                clip_utf8(&skill.content, 8_000)
            ));
        }
        out.push_str("</manually_attached_skills>\n");
    }
    if let Some(recent) = &sel.recent_agents_context {
        if !recent.recent_agents.is_empty() {
            out.push_str("<recent_agents>\n");
            for agent in &recent.recent_agents {
                out.push_str(&format!("{} {}\n", agent.name, agent.path));
            }
            out.push_str("</recent_agents>\n");
        }
    }
    out
}

fn format_request_extras(ctx: &RequestContext) -> String {
    let mut out = String::new();
    if !ctx.rules.is_empty() {
        out.push_str("<rules>\n");
        for rule in &ctx.rules {
            out.push_str(&format!(
                "<rule path=\"{}\">{}</rule>\n",
                rule.full_path,
                clip_utf8(&rule.content, 4_000)
            ));
        }
        out.push_str("</rules>\n");
    }
    if let Some(cloud) = ctx.cloud_rule.as_ref().filter(|s| !s.is_empty()) {
        out.push_str("<cloud_rule>\n");
        out.push_str(&clip_utf8(cloud, 2_000));
        out.push_str("\n</cloud_rule>\n");
    }
    if !ctx.project_layouts.is_empty() {
        out.push_str("<attached_folders>\n");
        for node in &ctx.project_layouts {
            out.push_str(&format_layout(node, 0));
        }
        out.push_str("</attached_folders>\n");
    }
    if !ctx.mcp_instructions.is_empty() {
        out.push_str("<mcp_instructions>\n");
        for inst in &ctx.mcp_instructions {
            out.push_str(&format!(
                "{} ({}): {}\n",
                inst.server_name,
                inst.server_identifier,
                clip_utf8(&inst.instructions, 2_000)
            ));
        }
        out.push_str("</mcp_instructions>\n");
    }
    if !ctx.custom_subagents.is_empty() {
        out.push_str("<custom_subagents>\n");
        for agent in &ctx.custom_subagents {
            out.push_str(&format!("{}: {}\n", agent.name, agent.description));
        }
        out.push_str("</custom_subagents>\n");
    }
    if let Some(meta) = &ctx.mcp_meta_tool_options {
        if meta.enabled && !meta.mcp_descriptors.is_empty() {
            out.push_str("<mcp_namespaces>\n");
            for desc in &meta.mcp_descriptors {
                out.push_str(&format!("{} {}\n", desc.server_identifier, desc.server_name));
            }
            out.push_str("</mcp_namespaces>\n");
        }
    }
    out
}

fn format_layout(node: &LsDirectoryTreeNode, depth: usize) -> String {
    if depth > 4 {
        return String::new();
    }
    let pad = "  ".repeat(depth);
    let mut out = format!("{pad}{}/\n", node.abs_path);
    for file in node.children_files.iter().take(40) {
        out.push_str(&format!("{pad}  {}\n", file.name));
    }
    for child in node.children_dirs.iter().take(20) {
        out.push_str(&format_layout(child, depth + 1));
    }
    out
}

fn selected_images(user: &UserMessage) -> Vec<(String, String)> {
    let Some(ctx) = user.selected_context.as_ref() else {
        return Vec::new();
    };
    ctx.selected_images
        .iter()
        .filter_map(|image| {
            let mime = if image.mime_type.is_empty() {
                "image/png".into()
            } else {
                image.mime_type.clone()
            };
            let bytes = match &image.data_or_blob_id {
                Some(selected_image::DataOrBlobId::Data(data)) if !data.is_empty() => data.clone(),
                Some(selected_image::DataOrBlobId::BlobIdWithData(blob)) if !blob.data.is_empty() => {
                    blob.data.clone()
                }
                Some(selected_image::DataOrBlobId::BlobId(id)) if !id.is_empty() => {
                    let key = String::from_utf8_lossy(id);
                    crate::blob::get(key.as_ref()).and_then(|data| {
                        base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            data.as_bytes(),
                        )
                        .ok()
                    })?
                }
                _ => return None,
            };
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
            Some((mime, b64))
        })
        .collect()
}

fn conversation_action_user_text(action: &ConversationAction) -> Option<String> {
    match action.action.as_ref()? {
        conversation_action::Action::UserMessageAction(user) => {
            user.user_message.as_ref().map(user_message_text).filter(|t| !t.trim().is_empty())
        }
        conversation_action::Action::BackgroundTaskCompletionAction(done) => {
            Some(format_background_completions(done))
        }
        conversation_action::Action::ShellCommandAction(cmd) => {
            Some(format_shell_command_action(cmd))
        }
        conversation_action::Action::StartPlanAction(plan) => plan
            .user_message
            .as_ref()
            .map(|msg| msg.text.clone())
            .filter(|text| !text.trim().is_empty()),
        conversation_action::Action::AsyncAskQuestionCompletionAction(done) => {
            Some(format_async_ask_completion(done))
        }
        conversation_action::Action::BackgroundShellAction(shell) => Some(format!(
            "<agent_notification>\nbackground shell tool_call_id: {}\n</agent_notification>",
            shell.tool_call_id
        )),
        conversation_action::Action::BackgroundSubagentAction(sub) => Some(format!(
            "<agent_notification>\nbackground subagent tool_call_id: {}\n</agent_notification>",
            sub.tool_call_id
        )),
        conversation_action::Action::SubscriptionNotificationAction(note) => {
            Some(format_subscription_notifications(note))
        }
        conversation_action::Action::GoalContinuationAction(_) => {
            Some("Continue working on the current goal.".into())
        }
        _ => None,
    }
}

fn format_shell_command_action(cmd: &ShellCommandAction) -> String {
    let command = cmd
        .shell_command
        .as_ref()
        .map(|c| c.command.as_str())
        .filter(|c| !c.is_empty())
        .unwrap_or("(empty command)");
    format!(
        "The user asked to run this shell command. Use the Shell tool and report the result.\n\n```\n{command}\n```"
    )
}

fn format_async_ask_completion(done: &AsyncAskQuestionCompletionAction) -> String {
    let answers = match done.result.as_ref().and_then(|r| r.result.as_ref()) {
        Some(crate::agent_proto::ask_question_result::Result::Success(ok)) => ok
            .answers
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
            .join("\n"),
        Some(crate::agent_proto::ask_question_result::Result::Error(err)) => {
            err.error_message.clone()
        }
        None => "no answers".into(),
    };
    format!(
        "<agent_notification>\nasync AskQuestion completed\ntool_call_id: {}\n{answers}\n</agent_notification>",
        done.original_tool_call_id
    )
}

fn format_subscription_notifications(note: &SubscriptionNotificationAction) -> String {
    let texts: Vec<String> = note
        .notifications
        .iter()
        .map(|msg| msg.text.trim())
        .filter(|text| !text.is_empty())
        .map(|text| text.to_owned())
        .collect();
    format!(
        "<system_reminder>\nDo not reiterate or repeat the contents of this agent notification to the user unless asked to do so.\n</system_reminder>\n\n<agent_notification>\n{}\n</agent_notification>",
        if texts.is_empty() {
            "subscription notification".into()
        } else {
            texts.join("\n\n")
        }
    )
}

pub fn name_agent_title(user_message: &str) -> String {
    let line = user_message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Chat")
        .trim();
    let mut out = String::new();
    for ch in line.chars() {
        if out.chars().count() >= 48 {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        "Chat".into()
    } else {
        out
    }
}

fn format_background_completions(done: &BackgroundTaskCompletionAction) -> String {
    if done.completions.is_empty() {
        return "<agent_notification>\nbackground task completed\n</agent_notification>".into();
    }
    let blocks: Vec<String> = done
        .completions
        .iter()
        .map(|item| {
            format!(
                "kind: {}\ntask_id: {}\nstatus: {}\ntitle: {}\noutput_path: {}\nthread_id: {}\nresponse:\n<response>\n{}\n</response>",
                item.kind,
                item.task_id,
                item.status,
                item.title,
                item.output_path.clone().unwrap_or_default(),
                item.thread_id.clone().unwrap_or_default(),
                item.detail.clone().unwrap_or_else(|| "No output".into())
            )
        })
        .collect();
    format!(
        "<system_reminder>\nDo not reiterate or repeat the contents of this agent notification to the user unless asked to do so.\n</system_reminder>\n\n<agent_notification>\n{}\n</agent_notification>",
        blocks.join("\n\n")
    )
}

fn format_interrupted(list: Option<&InterruptedPendingToolCallResolutions>) -> String {
    let Some(list) = list else {
        return String::new();
    };
    if list.resolutions.is_empty() {
        return String::new();
    }
    let mut out = String::from("<interrupted_tool_results>\n");
    for item in &list.resolutions {
        out.push_str("tool_call_id: ");
        out.push_str(&item.tool_call_id);
        out.push('\n');
        match &item.resolution {
            Some(interrupted_pending_tool_call_resolution::Resolution::ShellResult(result)) => {
                out.push_str(&shell_result_preview(result));
            }
            Some(interrupted_pending_tool_call_resolution::Resolution::TaskResult(result)) => {
                match &result.result {
                    Some(crate::agent_proto::task_result::Result::Success(ok)) => {
                        out.push_str(ok.result_suffix.as_deref().unwrap_or("task completed"));
                    }
                    Some(crate::agent_proto::task_result::Result::Error(err)) => {
                        out.push_str(&err.error);
                    }
                    None => {}
                }
            }
            None => {}
        }
        out.push('\n');
    }
    out.push_str("</interrupted_tool_results>");
    out
}

fn hydrate_context_parts(action: Option<&ConversationAction>) -> String {
    let Some(parts) = action.and_then(|action| action.request_context_parts.as_ref()) else {
        return String::new();
    };
    let mut out = String::new();
    append_blob_section(&mut out, "rules", &parts.rules_blob_id);
    append_blob_section(&mut out, "agent_skills", &parts.skills_blob_id);
    append_blob_section(&mut out, "attached_subagents", &parts.subagents_blob_id);
    append_blob_section(&mut out, "mcp_instructions", &parts.mcps_blob_id);
    out
}

fn append_blob_section(out: &mut String, tag: &str, id: &[u8]) {
    let text = hydrate_blob_utf8(id);
    if text.is_empty() {
        return;
    }
    out.push('<');
    out.push_str(tag);
    out.push_str(">\n");
    out.push_str(&clip_utf8(&text, 8_000));
    out.push_str("\n</");
    out.push_str(tag);
    out.push_str(">\n");
}

pub fn hydrate_blob_utf8(id: &[u8]) -> String {
    if id.is_empty() {
        return String::new();
    }
    let key = crate::blob::id_from_bytes(id);
    let Some(data) = crate::blob::get(&key) else {
        return String::new();
    };
    if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data.as_bytes())
    {
        if let Ok(text) = String::from_utf8(bytes) {
            if !text.is_empty() {
                return text;
            }
        }
    }
    crate::blob::decode_json(&data)
        .map(|value| value.to_string())
        .unwrap_or(data)
}

fn shell_result_preview(result: &crate::agent_proto::ShellResult) -> String {
    match &result.result {
        Some(crate::agent_proto::shell_result::Result::Success(ok)) => {
            clip_utf8(&format!("{}\n{}", ok.stdout, ok.stderr), 4_000)
        }
        Some(crate::agent_proto::shell_result::Result::Failure(fail)) => {
            format!("exit {}\n{}", fail.exit_code, clip_utf8(&fail.stdout, 2_000))
        }
        Some(crate::agent_proto::shell_result::Result::Rejected(rej)) => {
            format!("rejected: {}", rej.reason)
        }
        Some(crate::agent_proto::shell_result::Result::Timeout(_)) => "timeout".into(),
        Some(crate::agent_proto::shell_result::Result::SpawnError(err)) => {
            format!("spawn error: {}", err.error)
        }
        Some(crate::agent_proto::shell_result::Result::PermissionDenied(denied)) => {
            format!("permission denied: {}", denied.error)
        }
        None => "(empty shell result)".into(),
    }
}

fn format_git_and_skills(ctx: Option<&RequestContext>, token_limit: u32) -> String {
    let Some(ctx) = ctx else {
        return String::new();
    };
    let mut out = String::new();
    if !ctx.git_repos.is_empty() {
        out.push_str("<git_status>\n");
        for repo in &ctx.git_repos {
            out.push_str(&repo.path);
            out.push(' ');
            out.push_str(&repo.branch_name);
            out.push('\n');
            out.push_str(&clip_utf8(&repo.status, 2_000));
            out.push('\n');
        }
        out.push_str("</git_status>\n");
    }
    if !ctx.agent_skills.is_empty() {
        out.push_str(&build_agent_skills_section(&ctx.agent_skills, token_limit));
        out.push('\n');
    }
    out
}

const SKILL_CATALOG_BUDGET_PERCENT: f64 = 0.02;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 480;
const MIN_SKILL_DESCRIPTION_CHARS: usize = 24;

fn estimate_skill_tokens(text: &str) -> u32 {
    text.len().div_ceil(4) as u32
}

fn skill_leaf_name(full_path: &str) -> String {
    let normalized = full_path.replace('\\', "/");
    let segments: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
    if let Some(idx) = segments.iter().rposition(|s| *s == "SKILL.md") {
        if idx > 0 {
            return segments[idx - 1].to_owned();
        }
    }
    segments.last().unwrap_or(&"Skill").to_string()
}

fn skill_directory(full_path: &str) -> String {
    let normalized = full_path.replace('\\', "/");
    let markers = [
        "/.cursor/skills/",
        "/.cursor/skills-cursor/",
        "/.agents/skills/",
        "/.claude/skills/",
        "/.codex/skills/",
        "/.claude/plugins/",
    ];
    for marker in markers {
        if let Some(index) = normalized.find(marker) {
            return normalized[..index + marker.len() - 1].to_owned();
        }
    }
    match normalized.rfind('/') {
        Some(slash) => normalized[..slash].to_owned(),
        None => normalized,
    }
}

fn shorten_skill_description(description: &str, max_len: usize) -> String {
    let normalized: String = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= max_len {
        return normalized;
    }
    let keep = max_len.saturating_sub(3);
    let mut end = keep.min(normalized.len());
    while end > 0 && !normalized.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", normalized[..end].trim_end())
}

fn render_agent_skills_section(
    skills: &[(String, Option<String>)],
    omitted: Option<(usize, Vec<String>)>,
) -> String {
    let mut entries = String::new();
    for (path, description) in skills {
        entries.push_str("<agent_skill fullPath=\"");
        entries.push_str(&xml_escape_attr(path));
        entries.push_str("\">");
        if let Some(desc) = description {
            entries.push_str(&xml_escape_attr(desc));
        }
        entries.push_str("</agent_skill>\n");
    }
    let (scope, omitted_notice) = match omitted {
        Some((count, dirs)) if count > 0 => (
            "Use the skills listed below. If a later task specifically requires discovering more skills, additional skills may exist in the directories shown in this section.",
            format!(
                "\nAdditional skills omitted from this initial list ({count}). Directories containing omitted skills: {}.",
                dirs.join(", ")
            ),
        ),
        _ => ("Only use skills listed below.", String::new()),
    };
    format!(
        "<agent_skills>\nSkills the agent can use. Use the Read tool with the provided absolute path to fetch full contents.\nWhen users ask you to perform tasks, check if any of the available skills below can help complete the task more effectively. To use a skill, read the skill file at the provided absolute path using the Read tool, then follow the instructions within. When a skill is relevant, read and follow it IMMEDIATELY as your first action. {scope}\n\n{entries}{omitted_notice}\n</agent_skills>"
    )
}

fn xml_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// CCursor `buildAgentSkillsSection`: 2% token budget, shrink then omit.
pub fn build_agent_skills_section(skills: &[AgentSkill], agent_token_limit: u32) -> String {
    let skills: Vec<(String, Option<String>)> = skills
        .iter()
        .filter(|skill| !skill.full_path.is_empty())
        .map(|skill| {
            (
                skill.full_path.clone(),
                (!skill.description.is_empty()).then(|| skill.description.clone()),
            )
        })
        .collect();
    if skills.is_empty() {
        return String::new();
    }
    let limit = if agent_token_limit == 0 {
        DEFAULT_CONTEXT_TOKENS
    } else {
        agent_token_limit
    };
    let budget = ((limit as f64) * SKILL_CATALOG_BUDGET_PERCENT).floor() as u32;
    let original = render_agent_skills_section(&skills, None);
    if estimate_skill_tokens(&original) <= budget {
        return original;
    }
    let is_protected = |path: &str| {
        matches!(skill_leaf_name(path).as_str(), "canvas" | "env-setup")
    };
    let longest = skills
        .iter()
        .filter(|(path, _)| !is_protected(path))
        .filter_map(|(_, desc)| desc.as_ref().map(|d| d.len()))
        .max()
        .unwrap_or(0);
    if longest > 80 {
        let mut best: Option<String> = None;
        let mut low = MIN_SKILL_DESCRIPTION_CHARS;
        let mut high = longest.saturating_sub(1).min(MAX_SKILL_DESCRIPTION_CHARS);
        while low <= high {
            let candidate_len = (low + high) / 2;
            let candidate: Vec<(String, Option<String>)> = skills
                .iter()
                .map(|(path, desc)| {
                    if is_protected(path) {
                        (path.clone(), desc.clone())
                    } else {
                        let shortened = desc
                            .as_deref()
                            .map(|d| shorten_skill_description(d, candidate_len))
                            .filter(|d| !d.is_empty());
                        (path.clone(), shortened)
                    }
                })
                .collect();
            let section = render_agent_skills_section(&candidate, None);
            if estimate_skill_tokens(&section) <= budget {
                best = Some(section);
                low = candidate_len + 1;
            } else {
                high = candidate_len.saturating_sub(1);
            }
        }
        if let Some(section) = best {
            return section;
        }
    }
    let path_only: Vec<(String, Option<String>)> = skills
        .iter()
        .map(|(path, desc)| {
            if is_protected(path) {
                (path.clone(), desc.clone())
            } else {
                (path.clone(), None)
            }
        })
        .collect();
    let path_only_section = render_agent_skills_section(&path_only, None);
    if estimate_skill_tokens(&path_only_section) <= budget {
        return path_only_section;
    }
    let optional_indices: Vec<usize> = skills
        .iter()
        .enumerate()
        .filter(|(_, (path, _))| !is_protected(path))
        .map(|(i, _)| i)
        .collect();
    for retained_optional in (0..=optional_indices.len()).rev() {
        let retained: std::collections::HashSet<usize> = optional_indices
            .iter()
            .take(retained_optional)
            .copied()
            .collect();
        let listed: Vec<(String, Option<String>)> = path_only
            .iter()
            .enumerate()
            .filter(|(index, (path, _))| is_protected(path) || retained.contains(index))
            .map(|(_, item)| item.clone())
            .collect();
        let omitted_skills: Vec<&(String, Option<String>)> = skills
            .iter()
            .enumerate()
            .filter(|(index, (path, _))| !is_protected(path) && !retained.contains(index))
            .map(|(_, item)| item)
            .collect();
        let mut directories = Vec::new();
        for (path, _) in &omitted_skills {
            let dir = skill_directory(path);
            if !directories.contains(&dir) {
                directories.push(dir);
            }
            if directories.len() == 5 {
                break;
            }
        }
        let section = render_agent_skills_section(
            &listed,
            Some((omitted_skills.len(), directories)),
        );
        if estimate_skill_tokens(&section) <= budget || retained_optional == 0 {
            return section;
        }
    }
    path_only_section
}

fn format_history(action: &UserMessageAction) -> String {
    let mut out = String::new();
    for msg in &action.prepend_user_messages {
        if !msg.text.is_empty() {
            out.push_str("User:\n");
            out.push_str(&clip_utf8_tail(&msg.text, 8_000));
            out.push_str("\n\n");
        }
    }
    let Some(hist) = &action.conversation_history else {
        return out;
    };
    for item in &hist.messages {
        match &item.message {
            Some(conversation_history_message::Message::User(user)) => {
                let text = history_user_text(user);
                if !text.is_empty() {
                    out.push_str("User:\n");
                    out.push_str(&clip_utf8_tail(&text, 8_000));
                    out.push_str("\n\n");
                }
            }
            Some(conversation_history_message::Message::Assistant(assistant)) => {
                let text = history_assistant_text(assistant);
                if !text.is_empty() {
                    out.push_str("Assistant:\n");
                    out.push_str(&clip_utf8_tail(&text, 8_000));
                    out.push_str("\n\n");
                }
            }
            Some(conversation_history_message::Message::Tool(tool)) => {
                let text = history_tool_text(tool);
                if !text.is_empty() {
                    out.push_str("Tool ");
                    out.push_str(&tool.tool_name);
                    out.push_str(":\n");
                    out.push_str(&clip_utf8_tail(&text, 8_000));
                    out.push_str("\n\n");
                }
            }
            None => {}
        }
    }
    out
}

fn history_user_text(user: &ConversationHistoryUserMessage) -> String {
    user.content
        .iter()
        .filter_map(|part| match &part.content {
            Some(conversation_history_user_content::Content::Text(text)) => {
                Some(text.text.clone())
            }
            Some(conversation_history_user_content::Content::Image(image)) => {
                Some(format_history_image(image))
            }
            None => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn history_assistant_text(assistant: &ConversationHistoryAssistantMessage) -> String {
    assistant
        .content
        .iter()
        .filter_map(|part| match &part.content {
            Some(conversation_history_assistant_content::Content::Text(text)) => {
                Some(text.text.clone())
            }
            Some(conversation_history_assistant_content::Content::Reasoning(think)) => {
                Some(format!("[reasoning]\n{}", think.text))
            }
            Some(conversation_history_assistant_content::Content::RedactedReasoning(_)) => {
                Some("[redacted reasoning]".into())
            }
            Some(conversation_history_assistant_content::Content::ToolCall(call)) => {
                Some(format!("{}({})", call.tool_name, call.args_json))
            }
            None => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn history_tool_text(tool: &ConversationHistoryToolMessage) -> String {
    tool.content
        .iter()
        .filter_map(|part| match &part.content {
            Some(conversation_history_tool_result_content::Content::Text(text)) => {
                Some(text.text.clone())
            }
            Some(conversation_history_tool_result_content::Content::Image(image)) => {
                Some(format_history_image(image))
            }
            None => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_history_image(image: &ConversationHistoryImage) -> String {
    let mime = image
        .mime_type
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("image/png");
    format!("[image {mime}]")
}

pub fn clip_utf8(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_owned();
    }
    let mut end = max_bytes;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &body[..end])
}

/// Keep the tail (Cursor dumps rules first, the user question last).
/// Official shell interleaved output: head 5k + tail 5k when over 10k chars.
pub fn clip_utf8_head_tail(body: &str, head: usize, tail: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    if chars.len() <= head.saturating_add(tail) {
        return body.to_owned();
    }
    let start: String = chars.iter().take(head).collect();
    let end: String = chars.iter().skip(chars.len() - tail).collect();
    format!("{start}\n...\n{end}")
}

pub fn clip_utf8_tail(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_owned();
    }
    let keep = max_bytes.saturating_sub('…'.len_utf8());
    let mut start = body.len().saturating_sub(keep);
    while start < body.len() && !body.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &body[start..])
}

/// Keep a short head (toolkit) and the tail (user + latest tool results).
pub fn budget_prompt(prompt: &str, max_bytes: usize) -> String {
    budget_keep_user(prompt, max_bytes)
}

pub const USER_MARK: &str = "\n\nUser:\n";
/// CCursor scaffold: system + preamble user + current `<user_query>`.
pub const PREAMBLE_MARK: &str = "\n\nPreamble:\n";

const PREAMBLE_TAGS: &[&str] = &[
    "user_info",
    "agent_transcripts",
    "ide_state",
    "rules",
    "cloud_rule",
    "agent_skills",
    "manually_attached_skills",
    "attached_docs",
    "cursor_commands",
    "mcp_instructions",
    "mcp_namespaces",
    "extra_context",
    "extra_context_pending",
    "code_selections",
    "past_chats",
    "terminal_selections",
    "attached_files",
    "attached_folders",
    "external_links",
    "attached_subagents",
    "attached_browsers",
    "recent_agents",
    "custom_subagents",
    "git_status",
];

/// Reorder known XML scaffold tags to match CCursor `buildPreambleUserMessage`.
pub fn assemble_preamble(src: &str) -> String {
    let mut remain = src.to_owned();
    let mut out = String::new();
    for tag in PREAMBLE_TAGS {
        loop {
            let Some((section, next)) = take_xml_section(&remain, tag) else {
                break;
            };
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&section);
            remain = next;
        }
    }
    let leftover = remain.trim();
    if !leftover.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(leftover);
    }
    out
}

fn take_xml_section(src: &str, tag: &str) -> Option<(String, String)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = src.find(&open)?;
    let after_open = &src[start + open.len()..];
    if let Some(gt) = after_open.find('>') {
        if after_open[..gt].ends_with('/') {
            let end = start + open.len() + gt + 1;
            let section = src[start..end].trim().to_owned();
            let mut remain = String::new();
            remain.push_str(src[..start].trim());
            remain.push('\n');
            remain.push_str(src[end..].trim_start());
            return Some((section, remain));
        }
    }
    let rest = &src[start..];
    let rel = rest.find(&close)?;
    let end = start + rel + close.len();
    let section = src[start..end].trim().to_owned();
    let mut remain = String::new();
    remain.push_str(src[..start].trim());
    remain.push('\n');
    remain.push_str(src[end..].trim_start());
    Some((section, remain))
}

/// Keep toolkit + the User: block, then the latest tool results. The original
/// question sits between toolkit and `<tool_result>` and must not fall out.
pub fn budget_keep_user(prompt: &str, max_bytes: usize) -> String {
    if prompt.len() <= max_bytes {
        return prompt.to_owned();
    }
    let Some(user_at) = prompt.find(USER_MARK) else {
        let head_len = 4_000.min(max_bytes / 4).max(512);
        let tail_len = max_bytes.saturating_sub(head_len + 8);
        let head = clip_utf8(prompt, head_len);
        let tail = clip_utf8_tail(prompt, tail_len);
        return format!("{head}\n{tail}");
    };
    let user_body_at = user_at + USER_MARK.len();
    let tool_at = prompt[user_body_at..]
        .find("\n\n<tool_result")
        .map(|rel| user_body_at + rel)
        .unwrap_or(prompt.len());
    let mut head = if let Some(preamble_at) = prompt.find(PREAMBLE_MARK) {
        if preamble_at < user_at {
            let sys = clip_utf8(&prompt[..preamble_at + PREAMBLE_MARK.len()], 4_000);
            let preamble_budget = (max_bytes / 4).max(2_048);
            let preamble = clip_utf8(
                &prompt[preamble_at + PREAMBLE_MARK.len()..user_at],
                preamble_budget,
            );
            format!("{sys}{preamble}{USER_MARK}")
        } else {
            prompt[..user_body_at].to_owned()
        }
    } else {
        prompt[..user_body_at].to_owned()
    };
    if head.len() > max_bytes / 2 {
        head = clip_utf8(&head, max_bytes / 2);
        if !head.contains("User:") {
            head.push_str(USER_MARK);
        }
    }
    let user = clip_utf8_tail(&prompt[user_body_at..tool_at], max_bytes / 2);
    let used = head.len() + user.len();
    let leftover = max_bytes.saturating_sub(used.saturating_add(8));
    let tools = if leftover == 0 {
        String::new()
    } else {
        clip_utf8_tail(&prompt[tool_at..], leftover)
    };
    format!("{head}{user}{tools}")
}

pub fn prompt_with_workspace(run: &LocalRun) -> String {
    let mut out = String::new();
    if let Some(root) = run.workspace.as_deref().filter(|path| !path.is_empty()) {
        out.push_str("Workspace: ");
        out.push_str(root);
        out.push('\n');
        if let Some(tree) = list_workspace_tree(Path::new(root), 3, 80) {
            out.push_str("Project files:\n");
            out.push_str(&tree);
            out.push('\n');
        }
    }
    for (path, body) in run.attached_files.iter().take(20) {
        let clipped = clip_utf8(body, 8000);
        out.push_str("\n<file path=\"");
        out.push_str(path);
        out.push_str("\">\n");
        out.push_str(&clipped);
        out.push_str("\n</file>\n");
    }
    if !out.is_empty() {
        out.push_str("\nUser:\n");
    }
    out.push_str(&run.user_text);
    out
}

fn list_workspace_tree(root: &Path, depth: usize, max_files: usize) -> Option<String> {
    if !root.is_dir() {
        return None;
    }
    let mut lines = Vec::new();
    fn walk(dir: &Path, prefix: &str, depth: usize, max_files: usize, lines: &mut Vec<String>) {
        if lines.len() >= max_files || depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut names: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
        names.sort_by_key(|entry| entry.file_name());
        for entry in names {
            if lines.len() >= max_files {
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                || matches!(
                    name.as_ref(),
                    "node_modules" | "target" | "dist" | ".git" | "build"
                )
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                lines.push(format!("{prefix}{name}/"));
                walk(&path, &format!("{prefix}  "), depth - 1, max_files, lines);
            } else {
                lines.push(format!("{prefix}{name}"));
            }
        }
    }
    walk(root, "", depth, max_files, &mut lines);
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn knobs_from_id(id: &str) -> (Option<String>, Option<bool>) {
    let Some(open) = id.find('[') else {
        return (None, None);
    };
    let Some(close) = id[open + 1..].find(']') else {
        return (None, None);
    };
    let mut effort = None;
    let mut fast = None;
    for part in id[open + 1..open + 1 + close].split(',') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let value = kv.next().unwrap_or("").trim();
        match key {
            "effort" | "reasoning" if !value.is_empty() && value != "none" => {
                effort = Some(value.to_owned());
            }
            "fast" => fast = Some(value == "true"),
            _ => {}
        }
    }
    (effort, fast)
}

fn parameters_from(model: Option<&RequestedModel>) -> (Option<String>, bool) {
    let mut effort = None;
    let mut fast = true;
    if let Some(model) = model {
        let (id_effort, id_fast) = knobs_from_id(&model.model_id);
        effort = id_effort;
        if let Some(flag) = id_fast {
            fast = flag;
        }
        for parameter in &model.parameters {
            match parameter.id.as_str() {
                "effort" | "reasoning" => {
                    if parameter.value != "none" && !parameter.value.is_empty() {
                        effort = Some(parameter.value.clone());
                    }
                }
                "fast" => fast = parameter.value == "true",
                _ => {}
            }
        }
    }
    (effort, fast)
}

pub fn decode_run_sse_id(body: &[u8]) -> Option<String> {
    let id = decode_unary::<BidiRequestId>(body)?.request_id;
    (!id.is_empty()).then_some(id)
}

pub fn is_injected_model(model_id: &str, enabled: &[String]) -> bool {
    if model_id.starts_with(INJECT_PREFIX) {
        return true;
    }
    enabled.iter().any(|id| injected_id(id) == model_id)
}

/// Connect bidi `AgentService/Run` body: frames of `AgentClientMessage`.
pub fn decode_connect_agent_run(body: &[u8]) -> Option<LocalRun> {
    let mut rest = body;
    while let Some((flags, payload, next)) = crate::connect::split_first_frame(rest) {
        if flags & crate::connect::END_STREAM_FLAG != 0 {
            break;
        }
        if let Some(run) = local_run_from_agent_payload("agent-run".into(), payload) {
            if !run.model_id.is_empty() {
                return Some(run);
            }
        }
        if next.is_empty() || next.len() >= rest.len() {
            break;
        }
        rest = next;
    }
    local_run_from_agent_payload("agent-run".into(), body).filter(|run| !run.model_id.is_empty())
}

#[allow(dead_code)]
pub fn encode_agent_message(message: &AgentServerMessage) -> Vec<u8> {
    encode_connect_frame(&message.encode_to_vec())
}

fn interaction(message: interaction_update::Message) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::InteractionUpdate(
            InteractionUpdate {
                message: Some(message),
            },
        )),
    }
}

pub fn raw_text_delta(text: &str) -> Vec<u8> {
    interaction(interaction_update::Message::TextDelta(TextDeltaUpdate {
        text: text.to_owned(),
        is_server_notice: false,
    }))
    .encode_to_vec()
}

pub fn raw_thinking_delta(text: &str) -> Vec<u8> {
    interaction(interaction_update::Message::ThinkingDelta(
        ThinkingDeltaUpdate {
            text: text.to_owned(),
            thinking_style: Some(1),
        },
    ))
    .encode_to_vec()
}

pub fn raw_turn_ended() -> Vec<u8> {
    interaction(interaction_update::Message::TurnEnded(TurnEndedUpdate {
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
    }))
    .encode_to_vec()
}

pub fn raw_token_delta(tokens: i32) -> Vec<u8> {
    interaction(interaction_update::Message::TokenDelta(TokenDeltaUpdate {
        tokens,
    }))
    .encode_to_vec()
}

pub fn encode_token_delta(tokens: i32) -> Vec<u8> {
    encode_connect_frame(&raw_token_delta(tokens))
}

pub fn has_token_delta(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::TokenDelta(_)),
                }
            ))
        )
    })
}

pub fn has_thinking_delta(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::ThinkingDelta(_)),
                }
            ))
        )
    })
}

pub fn turn_ended_input(messages: &[AgentServerMessage]) -> Option<i64> {
    messages
        .iter()
        .rev()
        .find_map(|message| match &message.message {
            Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                message: Some(interaction_update::Message::TurnEnded(ended)),
            })) => ended.input_tokens,
            _ => None,
        })
}

pub fn encode_text_delta(text: &str) -> Vec<u8> {
    encode_connect_frame(&raw_text_delta(text))
}

pub fn encode_thinking_delta(text: &str) -> Vec<u8> {
    encode_connect_frame(&raw_thinking_delta(text))
}

pub fn encode_turn_ended() -> Vec<u8> {
    encode_connect_frame(&raw_turn_ended())
}

pub fn encode_end_stream_ok() -> Vec<u8> {
    crate::connect::encode_end_stream()
}

pub fn encode_end_stream_error(code: &str, message: &str) -> Vec<u8> {
    crate::connect::encode_error_end_stream(code, message)
}

pub fn encode_thinking_completed(duration_ms: i32) -> Vec<u8> {
    encode_connect_frame(
        &interaction(interaction_update::Message::ThinkingCompleted(
            ThinkingCompletedUpdate {
                thinking_duration_ms: duration_ms,
            },
        ))
        .encode_to_vec(),
    )
}

pub fn encode_local_completion(thinking: &str, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    if !thinking.is_empty() {
        out.extend_from_slice(&encode_thinking_delta(thinking));
        out.extend_from_slice(&encode_thinking_completed(0));
    }
    if !text.is_empty() {
        out.extend_from_slice(&encode_text_delta(text));
    }
    out.extend_from_slice(&encode_turn_ended());
    out.extend_from_slice(&encode_end_stream_ok());
    out
}

pub fn encode_heartbeat() -> Vec<u8> {
    encode_connect_frame(
        &interaction(interaction_update::Message::Heartbeat(HeartbeatUpdate {})).encode_to_vec(),
    )
}

pub fn server_partial_tool(
    call_id: &str,
    tool: crate::agent_proto::ToolCall,
    model_call_id: &str,
) -> AgentServerMessage {
    interaction(interaction_update::Message::PartialToolCall(
        crate::agent_proto::PartialToolCallUpdate {
            call_id: call_id.to_owned(),
            tool_call: Some(tool),
            args_text_delta: String::new(),
            model_call_id: model_call_id.to_owned(),
        },
    ))
}

pub fn server_shell_stdout_delta(
    call_id: &str,
    content: &str,
    model_call_id: &str,
) -> AgentServerMessage {
    shell_delta(call_id, model_call_id, true, content)
}

pub fn server_shell_stderr_delta(
    call_id: &str,
    content: &str,
    model_call_id: &str,
) -> AgentServerMessage {
    shell_delta(call_id, model_call_id, false, content)
}

pub fn server_edit_stream_delta(
    call_id: &str,
    content: &str,
    model_call_id: &str,
) -> AgentServerMessage {
    interaction(interaction_update::Message::ToolCallDelta(
        crate::agent_proto::ToolCallDeltaUpdate {
            call_id: call_id.to_owned(),
            tool_call_delta: Some(crate::agent_proto::ToolCallDelta {
                delta: Some(
                    crate::agent_proto::tool_call_delta::Delta::EditToolCallDelta(
                        crate::agent_proto::EditToolCallDelta {
                            stream_content_delta: content.to_owned(),
                        },
                    ),
                ),
            }),
            model_call_id: model_call_id.to_owned(),
        },
    ))
}

fn shell_delta(
    call_id: &str,
    model_call_id: &str,
    stdout: bool,
    content: &str,
) -> AgentServerMessage {
    let inner = if stdout {
        crate::agent_proto::shell_tool_call_delta::Delta::Stdout(
            crate::agent_proto::ShellToolCallStdoutDelta {
                content: content.to_owned(),
            },
        )
    } else {
        crate::agent_proto::shell_tool_call_delta::Delta::Stderr(
            crate::agent_proto::ShellToolCallStderrDelta {
                content: content.to_owned(),
            },
        )
    };
    interaction(interaction_update::Message::ToolCallDelta(
        crate::agent_proto::ToolCallDeltaUpdate {
            call_id: call_id.to_owned(),
            tool_call_delta: Some(crate::agent_proto::ToolCallDelta {
                delta: Some(
                    crate::agent_proto::tool_call_delta::Delta::ShellToolCallDelta(
                        crate::agent_proto::ShellToolCallDelta { delta: Some(inner) },
                    ),
                ),
            }),
            model_call_id: model_call_id.to_owned(),
        },
    ))
}

pub fn has_tool_call_delta(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::ToolCallDelta(_)),
                }
            ))
        )
    })
}

pub fn has_turn_ended(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::TurnEnded(_)),
                }
            ))
        )
    })
}

pub fn frame_kind(message: &AgentServerMessage) -> Option<&'static str> {
    match &message.message {
        Some(agent_server_message::Message::ExecServerMessage(_)) => Some("exec"),
        Some(agent_server_message::Message::ConversationCheckpointUpdate(_)) => Some("checkpoint"),
        Some(agent_server_message::Message::KvServerMessage(_)) => Some("kv"),
        Some(agent_server_message::Message::InteractionQuery(_)) => Some("query"),
        Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
            message: Some(kind),
        })) => Some(match kind {
            interaction_update::Message::TextDelta(_) => "text",
            interaction_update::Message::ToolCallStarted(_) => "started",
            interaction_update::Message::ToolCallCompleted(_) => "completed",
            interaction_update::Message::ThinkingDelta(_) => "thinking",
            interaction_update::Message::ThinkingCompleted(_) => "thinking_done",
            interaction_update::Message::UserMessageAppended(_) => "user_appended",
            interaction_update::Message::PartialToolCall(_) => "partial",
            interaction_update::Message::TokenDelta(_) => "token",
            interaction_update::Message::Summary(_) => "summary",
            interaction_update::Message::SummaryStarted(_) => "summary_started",
            interaction_update::Message::SummaryCompleted(_) => "summary_completed",
            interaction_update::Message::Heartbeat(_) => "heartbeat",
            interaction_update::Message::TurnEnded(_) => "turn_ended",
            interaction_update::Message::ToolCallDelta(_) => "delta",
            interaction_update::Message::StepStarted(_) => "step_started",
            interaction_update::Message::StepCompleted(_) => "step_completed",
        }),
        _ => None,
    }
}

pub fn server_tool_started(
    call_id: &str,
    tool: crate::agent_proto::ToolCall,
    model_call_id: &str,
) -> AgentServerMessage {
    interaction(interaction_update::Message::ToolCallStarted(
        crate::agent_proto::ToolCallStartedUpdate {
            call_id: call_id.to_owned(),
            tool_call: Some(tool),
            model_call_id: model_call_id.to_owned(),
        },
    ))
}

pub fn server_tool_completed(
    call_id: &str,
    tool: crate::agent_proto::ToolCall,
    model_call_id: &str,
) -> AgentServerMessage {
    interaction(interaction_update::Message::ToolCallCompleted(
        crate::agent_proto::ToolCallCompletedUpdate {
            call_id: call_id.to_owned(),
            tool_call: Some(tool),
            model_call_id: model_call_id.to_owned(),
        },
    ))
}

pub fn server_exec(
    id: u32,
    exec_id: &str,
    message: crate::agent_proto::exec_server_message::Message,
) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::ExecServerMessage(
            crate::agent_proto::ExecServerMessage {
                id,
                exec_id: exec_id.to_owned(),
                message: Some(message),
            },
        )),
    }
}

pub fn encode_server(message: &AgentServerMessage) -> Vec<u8> {
    encode_connect_frame(&message.encode_to_vec())
}

pub const SIMULATED_MSG_BACKGROUND_TASK: i32 = 3;

pub fn user_message_appended(text: &str, mode: i32) -> AgentServerMessage {
    interaction(interaction_update::Message::UserMessageAppended(
        UserMessageAppendedUpdate {
            user_message: Some(UserMessage {
                text: text.to_owned(),
                message_id: uuid::Uuid::new_v4().to_string(),
                mode,
                is_simulated_msg: Some(true),
                simulated_msg_reason: Some(SIMULATED_MSG_BACKGROUND_TASK),
                ..Default::default()
            }),
        },
    ))
}

pub fn exec_server_abort(id: u32) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::ExecServerControlMessage(
            ExecServerControlMessage {
                message: Some(exec_server_control_message::Message::Abort(ExecServerAbort {
                    id,
                })),
            },
        )),
    }
}

#[derive(Clone, Default)]
pub struct CheckpointExtras {
    pub mode: i32,
    pub git_repos: Vec<(String, String)>,
    pub todos: Vec<Vec<u8>>,
    pub pending_tool_calls: Vec<String>,
    pub summary_archives: Vec<Vec<u8>>,
    pub read_paths: Vec<String>,
    pub file_states: HashMap<String, Vec<u8>>,
    pub plans: HashMap<String, PlanRegistryEntry>,
    pub subagent_states: HashMap<String, SubagentPersistedState>,
    pub plan: Option<Vec<u8>>,
}

pub fn apply_checkpoint_extras(frame: &mut AgentServerMessage, extras: &CheckpointExtras) {
    let Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) =
        frame.message.as_mut()
    else {
        return;
    };
    if extras.mode != 0 {
        state.mode = Some(extras.mode);
    }
    if !extras.todos.is_empty() {
        state.todos = extras.todos.clone();
    }
    if !extras.pending_tool_calls.is_empty() {
        state.pending_tool_calls = extras.pending_tool_calls.clone();
    }
    if !extras.summary_archives.is_empty() {
        state.summary_archives = extras.summary_archives.clone();
        state.summary_archive = extras.summary_archives.last().cloned();
    }
    if !extras.git_repos.is_empty() {
        state.tracked_git_repo_branches = extras
            .git_repos
            .iter()
            .map(|(path, branch)| TrackedGitRepo {
                repo_path: path.clone(),
                branch_name: branch.clone(),
            })
            .collect();
        state.active_branch_name = extras
            .git_repos
            .first()
            .map(|(_, branch)| branch.clone())
            .filter(|branch| !branch.is_empty());
    }
    if !extras.read_paths.is_empty() {
        state.read_paths = extras.read_paths.clone();
    }
    if !extras.file_states.is_empty() {
        state.file_states = extras.file_states.clone();
    }
    if !extras.plans.is_empty() {
        state.plans = extras.plans.clone();
    }
    if !extras.subagent_states.is_empty() {
        state.subagent_states = extras.subagent_states.clone();
    }
    if extras.plan.is_some() {
        state.plan = extras.plan.clone();
    }
}

pub fn kv_set_blob(id: u32, blob_id: &str, blob_data: &str) -> AgentServerMessage {
    crate::blob::put(blob_id, blob_data);
    AgentServerMessage {
        message: Some(agent_server_message::Message::KvServerMessage(
            KvServerMessage {
                id,
                message: Some(kv_server_message::Message::SetBlobArgs(SetBlobArgs {
                    blob_id: crate::blob::id_bytes(blob_id),
                    blob_data: blob_data.as_bytes().to_vec(),
                })),
            },
        )),
    }
}

pub fn kv_get_blob(id: u32, blob_id: &[u8]) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::KvServerMessage(
            KvServerMessage {
                id,
                message: Some(kv_server_message::Message::GetBlobArgs(GetBlobArgs {
                    blob_id: blob_id.to_vec(),
                })),
            },
        )),
    }
}

pub fn interaction_query(id: u32, query: interaction_query::Query) -> AgentServerMessage {
    AgentServerMessage {
        message: Some(agent_server_message::Message::InteractionQuery(
            InteractionQuery {
                id,
                query: Some(query),
            },
        )),
    }
}

pub fn checkpoint_with_prompt(
    used: u32,
    max: u32,
    workspace: Option<&str>,
    blobs: &[Vec<u8>],
    prompt: &str,
) -> AgentServerMessage {
    let mut frame = checkpoint_with_blobs(used, max, workspace, blobs);
    if let Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) =
        frame.message.as_mut()
    {
        if let Some(details) = state.token_details.as_mut() {
            let cats = breakdown_categories_from_prompt(prompt, used);
            if let Some(breakdown) = details.breakdown.as_mut() {
                breakdown.categories = cats;
            }
        }
    }
    frame
}

pub fn checkpoint_with_blobs(
    used: u32,
    max: u32,
    workspace: Option<&str>,
    blobs: &[Vec<u8>],
) -> AgentServerMessage {
    let mut frame = checkpoint_frame(used, max, workspace);
    if let Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) =
        frame.message.as_mut()
    {
        state.root_prompt_messages_json = blobs.to_vec();
        state.turns = blobs.to_vec();
    }
    frame
}

pub const DEFAULT_CONTEXT_TOKENS: u32 = 256_000;
const AGENT_MODE_AGENT: i32 = 1;

pub fn context_usage_tree(used: u32, max: u32) -> PromptContextUsageTree {
    let _ = max;
    PromptContextUsageTree {
        schema_version: 1,
        nodes: vec![PromptContextNode {
            id: "conversation".into(),
            parent_id: None,
            kind: "conversation".into(),
            label: "Conversation".into(),
            category_id: "conversation".into(),
            estimated_tokens: used,
            character_count: used.saturating_mul(4),
            content_available: true,
            inline_content: None,
        }],
    }
}

pub fn put_usage_snapshot(tree: &PromptContextUsageTree) -> Vec<u8> {
    let snapshot = PromptContextUsageSnapshot {
        prompt_context_usage_tree: Some(tree.clone()),
        root_prompt_messages_json: Vec::new(),
    };
    let bytes = snapshot.encode_to_vec();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let id = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        sha2::Sha256::digest(data.as_bytes()),
    );
    crate::blob::put(&id, &data);
    crate::blob::id_bytes(&id)
}

pub fn signed_media_response(key: &str) -> GetSignedUrlForAttachedMediaResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let url = format!("http://127.0.0.1:47821/gba-media/{key}");
    GetSignedUrlForAttachedMediaResponse {
        key: key.to_owned(),
        post_url: url.clone(),
        get_url: url.clone(),
        expires_at_unix_ms: now.saturating_add(3_600_000),
        refresh_after_unix_ms: now.saturating_add(3_000_000),
        post_fields: HashMap::new(),
        put_url: url,
    }
}

pub fn load_usage_snapshot(blob_id: &[u8]) -> Option<PromptContextUsageSnapshot> {
    let id = crate::blob::id_from_bytes(blob_id);
    let data = crate::blob::get(&id)?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data.as_bytes())
        .ok()?;
    PromptContextUsageSnapshot::decode(bytes.as_slice()).ok()
}

pub fn breakdown_categories_from_prompt(prompt: &str, used: u32) -> Vec<PromptTokenBreakdownCategory> {
    let section = |tag: &str| -> u32 {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let Some(start) = prompt.find(&open) else {
            return 0;
        };
        let rest = &prompt[start..];
        let end = rest.find(&close).unwrap_or(rest.len());
        (rest[..end].len().div_ceil(4)) as u32
    };
    let tools = section("tools").max((prompt.split("Tools:").next().unwrap_or("").len().div_ceil(4)) as u32);
    let rules = section("rules") + section("cursor_rules_context");
    let skills = section("agent_skills") + section("manually_attached_skills");
    let mcp = section("mcp_instructions") + section("dynamic_tools");
    let subagents = section("attached_subagents");
    let system = (prompt.len().div_ceil(4)) as u32;
    let classified = tools
        .saturating_add(rules)
        .saturating_add(skills)
        .saturating_add(mcp)
        .saturating_add(subagents);
    let conversation = used.max(system).saturating_sub(classified);
    [
        ("system_prompt", "System prompt", system.saturating_sub(classified).min(used)),
        ("tools", "Tool definitions", tools),
        ("rules", "Rules", rules),
        ("skills", "Skills", skills),
        ("mcp", "MCP", mcp),
        ("subagents", "Subagents", subagents),
        ("conversation", "Conversation", conversation.max(1)),
    ]
    .into_iter()
    .filter(|(_, _, tokens)| *tokens > 0)
    .map(|(id, label, estimated_tokens)| PromptTokenBreakdownCategory {
        id: id.into(),
        label: label.into(),
        estimated_tokens,
    })
    .collect()
}

fn token_details_with_usage(used: u32, max: u32) -> ConversationTokenDetails {
    token_details_with_categories(used, max, Vec::new())
}

fn token_details_with_categories(
    used: u32,
    max: u32,
    mut categories: Vec<PromptTokenBreakdownCategory>,
) -> ConversationTokenDetails {
    if categories.is_empty() {
        categories.push(PromptTokenBreakdownCategory {
            id: "conversation".into(),
            label: "Conversation".into(),
            estimated_tokens: used,
        });
    }
    let tree = context_usage_tree(used, max);
    let snapshot_id = put_usage_snapshot(&tree);
    ConversationTokenDetails {
        used_tokens: used,
        max_tokens: max,
        breakdown: Some(PromptTokenBreakdownSnapshot {
            total_used_tokens: used,
            max_tokens: max,
            categories,
        }),
        prompt_context_usage_tree: Some(tree),
        prompt_context_usage_snapshot_blob_id: Some(snapshot_id),
    }
}

pub fn checkpoint_frame(used: u32, max: u32, workspace: Option<&str>) -> AgentServerMessage {
    let max = if max == 0 {
        DEFAULT_CONTEXT_TOKENS
    } else {
        max
    };
    let used = used.min(max).max(1);
    let previous_workspace_uris = workspace
        .filter(|path| !path.is_empty())
        .map(|path| {
            if path.starts_with("file:") {
                path.to_owned()
            } else {
                format!("file:///{}", path.replace('\\', "/"))
            }
        })
        .into_iter()
        .collect();
    AgentServerMessage {
        message: Some(agent_server_message::Message::ConversationCheckpointUpdate(
            ConversationStateStructure {
                root_prompt_messages_json: Vec::new(),
                turns: Vec::new(),
                token_details: Some(token_details_with_usage(used, max)),
                previous_workspace_uris,
                mode: Some(AGENT_MODE_AGENT),
                agent_type: Some("ide".into()),
                ..Default::default()
            },
        )),
    }
}

pub fn has_checkpoint(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::ConversationCheckpointUpdate(
                _
            ))
        )
    })
}

pub fn has_partial_tool_call(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::PartialToolCall(_)),
                }
            ))
        )
    })
}

pub fn checkpoint_used(messages: &[AgentServerMessage]) -> Option<u32> {
    messages
        .iter()
        .rev()
        .find_map(|message| match &message.message {
            Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) => state
                .token_details
                .as_ref()
                .map(|details| details.used_tokens),
            _ => None,
        })
}

pub fn decode_connect_server_messages(bytes: &[u8]) -> Vec<AgentServerMessage> {
    let mut rest = bytes;
    let mut out = Vec::new();
    while let Some((flags, payload, next)) = crate::connect::split_first_frame(rest) {
        if flags & crate::connect::END_STREAM_FLAG != 0 {
            break;
        }
        if let Ok(message) = AgentServerMessage::decode(payload) {
            out.push(message);
        }
        if next.is_empty() || next.len() >= rest.len() {
            break;
        }
        rest = next;
    }
    out
}

pub fn has_tool_call_started(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::ToolCallStarted(_)),
                }
            ))
        )
    })
}

pub fn completed_shell_result(
    messages: &[AgentServerMessage],
) -> Option<crate::agent_proto::shell_result::Result> {
    messages
        .iter()
        .rev()
        .find_map(|message| match &message.message {
            Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                message: Some(interaction_update::Message::ToolCallCompleted(update)),
            })) => match &update.tool_call.as_ref()?.tool {
                Some(crate::agent_proto::tool_call::Tool::ShellToolCall(shell)) => {
                    shell.result.as_ref()?.result.clone()
                }
                _ => None,
            },
            _ => None,
        })
}

pub fn has_tool_call_completed(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::InteractionUpdate(
                InteractionUpdate {
                    message: Some(interaction_update::Message::ToolCallCompleted(_)),
                }
            ))
        )
    })
}

pub fn has_exec_server(messages: &[AgentServerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.message,
            Some(agent_server_message::Message::ExecServerMessage(_))
        )
    })
}

pub fn exec_server_ids(messages: &[AgentServerMessage]) -> Vec<u32> {
    messages
        .iter()
        .filter_map(|message| match &message.message {
            Some(agent_server_message::Message::ExecServerMessage(exec)) => Some(exec.id),
            _ => None,
        })
        .collect()
}

pub fn text_deltas(messages: &[AgentServerMessage]) -> String {
    let mut out = String::new();
    for message in messages {
        if let Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
            message: Some(interaction_update::Message::TextDelta(delta)),
        })) = &message.message
        {
            out.push_str(&delta.text);
        }
    }
    out
}

pub fn encode_run_sse_request(request_id: &str) -> Vec<u8> {
    encode_connect_frame(
        &BidiRequestId {
            request_id: request_id.into(),
        }
        .encode_to_vec(),
    )
}

pub fn encode_bidi_run(
    request_id: &str,
    model: &str,
    text: &str,
    workspace: Option<&str>,
) -> Vec<u8> {
    let env = workspace.map(|path| RequestContextEnv {
        os_version: "win32".into(),
        workspace_paths: vec![path.into()],
        shell: "powershell".into(),
        project_folder: path.into(),
        ..Default::default()
    });
    let run = AgentRunRequest {
        action: Some(ConversationAction {
            action: Some(conversation_action::Action::UserMessageAction(
                UserMessageAction {
                    user_message: Some(UserMessage { text: text.into(), ..Default::default() }),
                    request_context: Some(RequestContext {
                        env,
                        file_contents: HashMap::new(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        }),
        model_details: None,
        requested_model: Some(RequestedModel {
            model_id: model.into(),
            max_mode: false,
            parameters: vec![
                ModelParameterValue {
                    id: "effort".into(),
                    value: "high".into(),
                },
                ModelParameterValue {
                    id: "fast".into(),
                    value: "true".into(),
                },
            ],
        }),
        ..Default::default()
    };
    let client = AgentClientMessage {
        message: Some(agent_client_message::Message::RunRequest(run)),
    };
    let append = BidiAppendRequest {
        data: ascii_hex(&client.encode_to_vec()),
        request_id: Some(BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: 0,
        data_binary: Vec::new(),
    };
    encode_connect_frame(&append.encode_to_vec())
}

pub fn encode_exec_client_bidi(
    request_id: &str,
    exec: &crate::agent_proto::ExecClientMessage,
) -> Vec<u8> {
    let client = AgentClientMessage {
        message: Some(agent_client_message::Message::ExecClientMessage(
            exec.clone(),
        )),
    };
    let append = BidiAppendRequest {
        data: ascii_hex(&client.encode_to_vec()),
        request_id: Some(BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: 1,
        data_binary: Vec::new(),
    };
    encode_connect_frame(&append.encode_to_vec())
}

pub fn encode_throw_bidi(request_id: &str, exec_id: u32, error: &str) -> Vec<u8> {
    let client = AgentClientMessage {
        message: Some(agent_client_message::Message::ExecClientControlMessage(
            ExecClientControlMessage {
                message: Some(exec_client_control_message::Message::Throw(
                    ExecClientThrow {
                        id: exec_id,
                        error: error.into(),
                    },
                )),
            },
        )),
    };
    let append = BidiAppendRequest {
        data: ascii_hex(&client.encode_to_vec()),
        request_id: Some(BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: 2,
        data_binary: Vec::new(),
    };
    encode_connect_frame(&append.encode_to_vec())
}

pub fn encode_stream_close_bidi(request_id: &str, exec_id: u32) -> Vec<u8> {
    let client = AgentClientMessage {
        message: Some(agent_client_message::Message::ExecClientControlMessage(
            ExecClientControlMessage {
                message: Some(exec_client_control_message::Message::StreamClose(
                    ExecClientStreamClose { id: exec_id },
                )),
            },
        )),
    };
    let append = BidiAppendRequest {
        data: ascii_hex(&client.encode_to_vec()),
        request_id: Some(BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: 2,
        data_binary: Vec::new(),
    };
    encode_connect_frame(&append.encode_to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::encode_connect_frame;
    use std::collections::HashMap;

    fn framed_bidi(model: &str, text: &str) -> Vec<u8> {
        let run = AgentRunRequest {
            action: Some(ConversationAction {
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage { text: text.into(), ..Default::default() }),
                        request_context: None,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            model_details: None,
            requested_model: Some(RequestedModel {
                model_id: model.into(),
                max_mode: false,
                parameters: vec![
                    ModelParameterValue {
                        id: "effort".into(),
                        value: "high".into(),
                    },
                    ModelParameterValue {
                        id: "fast".into(),
                        value: "true".into(),
                    },
                ],
            }),
            ..Default::default()
        };
        let client = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        };
        let hex = ascii_hex(&client.encode_to_vec());
        let append = BidiAppendRequest {
            data: hex,
            request_id: Some(BidiRequestId {
                request_id: "req-1".into(),
            }),
            append_seqno: 0,
            data_binary: Vec::new(),
        };
        encode_connect_frame(&append.encode_to_vec())
    }

    #[test]
    fn hex_bidi_hides_ascii_gb_but_decode_finds_model() {
        let body = framed_bidi("gb-grok-4.6", "hello from cursor");
        assert!(
            !body.windows(3).any(|window| window == b"gb-"),
            "hex-encoded Bidi must not contain raw gb-"
        );
        assert!(body_mentions_injected(&body, &["grok-4.6".into()]));
        let decoded = decode_bidi_append(&body).expect("decode");
        assert_eq!(decoded.request_id, "req-1");
        let run = decoded.run.expect("run");
        assert_eq!(run.model_id, "gb-grok-4.6");
        assert_eq!(run.user_text, "hello from cursor");
        assert!(scan_gb_model(&body).unwrap().starts_with("gb-grok-4.6"));
        assert_eq!(run.effort.as_deref(), Some("high"));
        assert!(run.fast);
        assert!(is_injected_model(&run.model_id, &["grok-4.6".into()]));
        assert!(
            local_run_from_agent_payload("req-1".into(), b"xxgb-grok-4.6[effort=high]").is_none(),
            "substring gb- must not invent a local run"
        );
    }

    fn framed_agent_run(model: &str, text: &str) -> Vec<u8> {
        let run = AgentRunRequest {
            action: Some(ConversationAction {
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage { text: text.into(), ..Default::default() }),
                        request_context: None,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            model_details: None,
            requested_model: Some(RequestedModel {
                model_id: model.into(),
                max_mode: false,
                parameters: vec![
                    ModelParameterValue {
                        id: "effort".into(),
                        value: "high".into(),
                    },
                    ModelParameterValue {
                        id: "fast".into(),
                        value: "true".into(),
                    },
                ],
            }),
            ..Default::default()
        };
        let client = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        };
        encode_connect_frame(&client.encode_to_vec())
    }

    #[test]
    fn connect_run_frame_finds_injected_model() {
        let body = framed_agent_run("gb-grok-4.6", "hi");
        let run = decode_connect_agent_run(&body).expect("run");
        assert_eq!(run.model_id, "gb-grok-4.6");
        assert_eq!(run.user_text, "hi");
        assert_eq!(run.effort.as_deref(), Some("high"));
        assert!(run.fast);
        assert!(is_injected_model(&run.model_id, &["grok-4.6".into()]));
        let official = framed_agent_run("grok-4.6", "hi");
        let official_run = decode_connect_agent_run(&official).unwrap();
        assert!(!is_injected_model(
            &official_run.model_id,
            &["grok-4.6".into()]
        ));
    }

    #[test]
    fn clip_utf8_head_tail_keeps_both_ends() {
        let body = format!("{}{}", "H".repeat(6000), "T".repeat(6000));
        let clipped = clip_utf8_head_tail(&body, 5000, 5000);
        assert!(clipped.starts_with("HHHH"));
        assert!(clipped.ends_with("TTTT"));
        assert!(clipped.contains("\n...\n"));
        assert!(clipped.len() < body.len());
        assert_eq!(clip_utf8_head_tail("short", 5000, 5000), "short");
    }

    #[test]
    fn clip_utf8_does_not_panic_on_multibyte_boundary() {
        let mut body = "a".repeat(7999);
        body.push('你');
        body.push_str("more");
        let clipped = clip_utf8(&body, 8000);
        assert!(clipped.ends_with('…'));
        assert!(clipped.is_char_boundary(clipped.len() - '…'.len_utf8()));
        let mut long = "HEAD-".repeat(8000);
        long.push_str("TAIL_QUESTION");
        let tail = clip_utf8_tail(&long, 40);
        assert!(tail.contains("TAIL_QUESTION"));
        assert!(tail.starts_with('…'));
        assert!(!tail.starts_with("HEAD-"));
        let budget = budget_prompt(&format!("{}{}", "TOOLKIT ".repeat(200), long), 8_000);
        assert!(budget.contains("TAIL_QUESTION"));
        assert!(budget.len() <= 8_200);
        let mut with_user = String::from("TOOLKIT\n\nUser:\nUNIQUE_USER_QUESTION_ZX9");
        with_user.push_str(&"\n\n<tool_result name=\"Read\">".repeat(80));
        with_user.push_str(&"X".repeat(80_000));
        with_user.push_str("</tool_result>\n");
        let kept = budget_keep_user(&with_user, 12_000);
        assert!(
            kept.contains("UNIQUE_USER_QUESTION_ZX9"),
            "user question must survive tool-result shrink"
        );
        assert!(kept.contains("User:"));
        assert!(kept.len() <= 12_400);
    }

    #[test]
    fn token_delta_frame_is_interaction_field_8() {
        let bytes = encode_token_delta(3);
        let messages = decode_connect_server_messages(&bytes);
        assert!(has_token_delta(&messages));
        match &messages[0].message {
            Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                message: Some(interaction_update::Message::TokenDelta(delta)),
            })) => assert_eq!(delta.tokens, 3),
            other => panic!("expected token delta, got {other:?}"),
        }
    }

    #[test]
    fn official_model_is_not_local() {
        let body = framed_bidi("claude-opus-4-6", "hi");
        let run = decode_bidi_append(&body).unwrap().run.unwrap();
        assert!(!is_injected_model(&run.model_id, &["grok-4.6".into()]));
        assert!(!body_mentions_injected(&body, &["grok-4.6".into()]));
    }

    #[test]
    fn official_grok_is_not_third_party_or_injected() {
        let enabled = vec!["grok-4.6".into(), "p/xai-X/grok-4.6".into()];
        assert!(
            !is_injected_model("grok-4.6", &enabled),
            "unsuffixed grok-4.6 is Cursor official, not gb-*"
        );
        assert!(is_injected_model("gb-grok-4.6", &enabled));
        assert!(is_injected_model("gb-p/xai-X/grok-4.6", &enabled));
        let official = framed_bidi("grok-4.6", "hi");
        let run = decode_bidi_append(&official).unwrap().run.unwrap();
        assert_eq!(run.model_id, "grok-4.6");
        assert!(!is_injected_model(&run.model_id, &enabled));
        assert!(!body_mentions_injected(&official, &enabled));
        assert!(scan_gb_model(&official).is_none());
        let mentioned = framed_bidi("grok-4.6", "compare with gb-grok-4.6 later");
        let mentioned_run = decode_bidi_append(&mentioned).unwrap().run.unwrap();
        assert_eq!(mentioned_run.model_id, "grok-4.6");
        assert!(!is_injected_model(&mentioned_run.model_id, &enabled));
    }

    #[test]
    fn checkpoint_frame_is_server_field_3_with_256k() {
        let frame = checkpoint_frame(40500, 256000, Some(r"C:\ws"));
        match frame.message {
            Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                let details = state.token_details.expect("token_details");
                assert_eq!(details.used_tokens, 40500);
                assert_eq!(details.max_tokens, 256000);
                assert_eq!(state.mode, Some(1));
                assert_eq!(state.agent_type.as_deref(), Some("ide"));
            }
            other => panic!("expected checkpoint, got {other:?}"),
        }
    }

    #[test]
    fn checkpoint_with_blobs_sets_root_prompt_ids() {
        let (id, data) = crate::blob::encode_role("user", "CKPT_BLOB_ZX9");
        crate::blob::put(&id, &data);
        let bytes = crate::blob::id_bytes(&id);
        let frame = checkpoint_with_blobs(12, 256000, Some(r"C:\ws"), &[bytes.clone()]);
        match frame.message {
            Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                assert_eq!(state.root_prompt_messages_json, vec![bytes.clone()]);
                assert_eq!(state.turns, vec![bytes], "Cursor hydrates resume from turns tag 8");
            }
            other => panic!("expected checkpoint, got {other:?}"),
        }
        let kv = kv_set_blob(1, &id, &data);
        assert_eq!(frame_kind(&kv), Some("kv"));
    }

    fn framed_client(request_id: &str, client: AgentClientMessage) -> Vec<u8> {
        let append = BidiAppendRequest {
            data: ascii_hex(&client.encode_to_vec()),
            request_id: Some(BidiRequestId {
                request_id: request_id.into(),
            }),
            append_seqno: 2,
            data_binary: Vec::new(),
        };
        encode_connect_frame(&append.encode_to_vec())
    }

    #[test]
    fn decode_bidi_kv_and_interaction() {
        let body = framed_client(
            "req-kv",
            AgentClientMessage {
                message: Some(agent_client_message::Message::KvClientMessage(
                    KvClientMessage {
                        id: 7,
                        message: Some(kv_client_message::Message::SetBlobResult(SetBlobResult {})),
                    },
                )),
            },
        );
        let decoded = decode_bidi_append(&body).expect("bidi");
        assert_eq!(decoded.kv.as_ref().map(|k| k.id), Some(7));
        let body = framed_client(
            "req-ix",
            AgentClientMessage {
                message: Some(agent_client_message::Message::InteractionResponse(
                    InteractionResponse {
                        id: 3,
                        result: Some(interaction_response::Result::WebSearchRequestResponse(
                            crate::agent_proto::WebSearchRequestResponse {
                                result: Some(
                                    crate::agent_proto::web_search_request_response::Result::Approved(
                                        crate::agent_proto::WebSearchApproved {},
                                    ),
                                ),
                            },
                        )),
                    },
                )),
            },
        );
        let decoded = decode_bidi_append(&body).expect("bidi");
        assert_eq!(decoded.interaction.as_ref().map(|i| i.id), Some(3));
    }

    #[test]
    fn conversation_state_turns_hydrate_when_rpm_empty() {
        let (id, data) = crate::blob::encode_role("user", "TURN_BLOB_ZX9");
        crate::blob::put(&id, &data);
        let bytes = crate::blob::id_bytes(&id);
        let run = AgentRunRequest {
            conversation_state: Some(ConversationStateStructure {
                turns: vec![bytes],
                ..Default::default()
            }),
            action: Some(ConversationAction {
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage { text: "now".into(), ..Default::default() }),
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
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        }
        .encode_to_vec();
        let local = local_run_from_agent_payload("t".into(), &payload).expect("run");
        assert_eq!(local.history_blob_ids.len(), 1);
        let prompt = crate::agent_loop::toolkit_prompt(&local);
        assert!(prompt.contains("TURN_BLOB_ZX9"), "{prompt}");
    }

    #[test]
    fn history_prepend_is_decoded_into_local_run() {
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
        let run = AgentRunRequest {
            action: Some(ConversationAction {
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage { text: "now".into(), ..Default::default() }),
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
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        }
        .encode_to_vec();
        let local = local_run_from_agent_payload("h".into(), &payload).expect("run");
        assert_eq!(local.user_text, "now");
        assert!(local.history.contains("PREV_TURN_ZX9"));
        let prompt = crate::agent_loop::toolkit_prompt(&local);
        assert!(prompt.contains("PREV_TURN_ZX9"));
        assert!(prompt.contains("now"));
    }

    #[test]
    fn request_context_parts_dynamic_context_fills_env_and_files() {
        let mut files = HashMap::new();
        files.insert("foo.rs".into(), "fn main() {}".into());
        let run = AgentRunRequest {
            action: Some(ConversationAction {
                request_context_parts: Some(RequestContextPartReferences {
                    dynamic_context: Some(RequestContext {
                        env: Some(RequestContextEnv {
                            os_version: "win32".into(),
                            workspace_paths: vec![r"C:\ws".into()],
                            shell: "powershell".into(),
                            project_folder: r"C:\ws".into(),
                            ..Default::default()
                        }),
                        file_contents: files,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage {
                            text: "see foo".into(),
                            ..Default::default()
                        }),
                        request_context: None,
                        ..Default::default()
                    },
                )),
            }),
            requested_model: Some(RequestedModel {
                model_id: "gb-grok-4.6".into(),
                max_mode: false,
                parameters: Vec::new(),
            }),
            ..Default::default()
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        }
        .encode_to_vec();
        let local = local_run_from_agent_payload("req-parts".into(), &payload).expect("decode");
        assert_eq!(local.workspace.as_deref(), Some(r"C:\ws"));
        assert_eq!(local.os_version, "win32");
        assert_eq!(local.shell, "powershell");
        assert_eq!(local.attached_files.len(), 1);
        assert_eq!(local.attached_files[0].0, "foo.rs");
    }

    #[test]
    fn completion_frames_end_with_connect_trailer() {
        let bytes = encode_local_completion("think", "answer");
        assert_eq!(bytes[0], 0);
        assert!(bytes.windows(5).any(|window| window[0] == 0x02));
        let messages = decode_connect_server_messages(&bytes);
        let thinking = messages.iter().find_map(|message| match &message.message {
            Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                message: Some(interaction_update::Message::ThinkingDelta(delta)),
            })) => Some(delta),
            _ => None,
        });
        assert_eq!(thinking.map(|d| d.thinking_style), Some(Some(1)));
    }

    #[test]
    fn bidi_cancel_action_is_not_a_new_run() {
        let body = framed_client(
            "req-stop",
            AgentClientMessage {
                message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                    action: Some(ConversationAction {
                        action: Some(conversation_action::Action::CancelAction(CancelAction {
                            reason: "user_stopped_generation".into(),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }),
                    requested_model: Some(RequestedModel {
                        model_id: "gb-grok-4.6".into(),
                        max_mode: false,
                        parameters: Vec::new(),
                    }),
                    ..Default::default()
                })),
            },
        );
        let decoded = decode_bidi_append(&body).expect("bidi");
        assert!(decoded.cancel);
        assert!(
            decoded.run.is_none(),
            "stop must not start a second agent loop"
        );
    }

    #[test]
    fn bidi_decodes_exec_client_message_field_2() {
        let exec = crate::agent_proto::ExecClientMessage {
            id: 7,
            exec_id: "call-1-exec".into(),
            message: Some(
                crate::agent_proto::exec_client_message::Message::ReadResult(
                    crate::agent_proto::ReadResult {
                        result: Some(crate::agent_proto::read_result::Result::Success(
                            crate::agent_proto::ReadSuccess {
                                path: "README.md".into(),
                                total_lines: 1,
                                file_size: 4,
                                truncated: false,
                                range_applied: false,
                                output: Some(crate::agent_proto::read_success::Output::Content(
                                    "ping".into(),
                                )),
                            },
                        )),
                    },
                ),
            ),
        };
        let body = encode_exec_client_bidi("req-exec", &exec);
        let decoded = decode_bidi_append(&body).expect("decode");
        assert_eq!(decoded.request_id, "req-exec");
        assert!(decoded.run.is_none());
        let got = decoded.exec.expect("exec");
        assert_eq!(got.id, 7);
        assert!(matches!(
            got.message,
            Some(crate::agent_proto::exec_client_message::Message::ReadResult(_))
        ));
    }

    #[test]
    fn bidi_decodes_stream_close_control() {
        let body = encode_stream_close_bidi("req-close", 3);
        let decoded = decode_bidi_append(&body).expect("decode");
        assert_eq!(decoded.request_id, "req-close");
        assert!(decoded.exec.is_none());
        assert!(!decoded.cancel);
        assert_eq!(decoded.stream_close, Some(3));
    }

    #[test]
    fn bidi_decodes_throw_control_is_not_run_cancel() {
        let body = encode_throw_bidi("req-throw", 9, "host aborted");
        let decoded = decode_bidi_append(&body).expect("decode");
        assert_eq!(decoded.request_id, "req-throw");
        assert!(!decoded.cancel);
        assert_eq!(decoded.throw.as_ref().map(|(id, _)| *id), Some(9));
        assert_eq!(
            decoded.throw.as_ref().map(|(_, err)| err.as_str()),
            Some("host aborted")
        );
    }

    #[test]
    fn execute_plan_action_becomes_user_text() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::ExecutePlanAction(
                        ExecutePlanAction {
                            plan_file_content: Some("# Auth\n1. login".into()),
                            plan_file_uri: Some("file:///.cursor/plans/auth.plan.md".into()),
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
        let local = local_run_from_agent_payload("plan".into(), &payload).expect("run");
        assert!(local.user_text.contains("Execute the following plan"));
        assert!(local.user_text.contains("1. login"));
    }

    #[test]
    fn summarize_action_starts_a_run_with_summarize_prompt() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::SummarizeAction(
                        SummarizeAction {},
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
        let local = local_run_from_agent_payload("sum".into(), &payload).expect("run");
        assert!(local.user_text.to_ascii_lowercase().contains("summarize"));
    }

    #[test]
    fn inject_context_is_not_a_new_run() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::InjectContextAction(
                        InjectContextAction {},
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
        assert!(local_run_from_agent_payload("inj".into(), &payload).is_none());
    }

    #[test]
    fn history_tool_message_is_formatted() {
        let history = ConversationHistory {
            messages: vec![ConversationHistoryMessage {
                message: Some(conversation_history_message::Message::Tool(
                    ConversationHistoryToolMessage {
                        tool_call_id: "call_1".into(),
                        tool_name: "Read".into(),
                        content: vec![ConversationHistoryToolResultContent {
                            content: Some(conversation_history_tool_result_content::Content::Text(
                                ConversationHistoryText {
                                    text: "TOOL_HIST_ZX9".into(),
                                },
                            )),
                        }],
                        is_error: None,
                    },
                )),
            }],
        };
        let run = AgentRunRequest {
            action: Some(ConversationAction {
                action: Some(conversation_action::Action::UserMessageAction(
                    UserMessageAction {
                        user_message: Some(UserMessage {
                            text: "now".into(),
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
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(run)),
        }
        .encode_to_vec();
        let local = local_run_from_agent_payload("h".into(), &payload).expect("run");
        assert!(local.history.contains("TOOL_HIST_ZX9"), "{}", local.history);
        assert!(local.history.contains("Read"), "{}", local.history);
    }

    #[test]
    fn usage_snapshot_roundtrip_keeps_conversation_node() {
        let tree = context_usage_tree(40500, 256000);
        let id = put_usage_snapshot(&tree);
        let loaded = load_usage_snapshot(&id).expect("snapshot");
        let node = loaded
            .prompt_context_usage_tree
            .as_ref()
            .and_then(|t| t.nodes.first())
            .expect("node");
        assert_eq!(node.estimated_tokens, 40500);
        assert_eq!(node.category_id, "conversation");
        let frame = checkpoint_frame(40500, 256000, None);
        match frame.message {
            Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                let details = state.token_details.expect("token_details");
                assert!(details.prompt_context_usage_tree.is_some());
                assert!(details.prompt_context_usage_snapshot_blob_id.is_some());
            }
            other => panic!("expected checkpoint, {other:?}"),
        }
    }

    #[test]
    fn queued_conversation_action_is_not_a_run() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::ConversationAction(
                ConversationAction {
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: "queued later".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
            )),
        }
        .encode_to_vec();
        assert!(local_run_from_agent_payload("q".into(), &payload).is_none());
        let body = framed_client(
            "req-q",
            AgentClientMessage {
                message: Some(agent_client_message::Message::ConversationAction(
                    ConversationAction {
                        action: Some(conversation_action::Action::UserMessageAction(
                            UserMessageAction {
                                user_message: Some(UserMessage {
                                    text: "queued later".into(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        )),
                        ..Default::default()
                    },
                )),
            },
        );
        let decoded = decode_bidi_append(&body).expect("bidi");
        assert_eq!(decoded.queued_user.as_deref(), Some("queued later"));
        assert!(decoded.run.is_none());
    }

    #[test]
    fn background_task_completion_becomes_user_text() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(
                        conversation_action::Action::BackgroundTaskCompletionAction(
                            BackgroundTaskCompletionAction {
                                completions: vec![BackgroundTaskCompletion {
                                    task_id: "t1".into(),
                                    kind: 2,
                                    status: 1,
                                    title: "explore".into(),
                                    detail: Some("found Cargo.toml".into()),
                                    ..Default::default()
                                }],
                            },
                        ),
                    ),
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
        let local = local_run_from_agent_payload("bg".into(), &payload).expect("run");
        assert!(local.user_text.contains("agent_notification"));
        assert!(local.user_text.contains("found Cargo.toml"));
    }

    #[test]
    fn rules_blob_hydrates_into_system_prompt() {
        crate::blob::put("rules-blob-1", &base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            b"<always>\nno secrets\n</always>",
        ));
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    request_context_parts: Some(RequestContextPartReferences {
                        rules_blob_id: b"rules-blob-1".to_vec(),
                        ..Default::default()
                    }),
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: "hi".into(),
                                mode: 2,
                                ..Default::default()
                            }),
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
        let local = local_run_from_agent_payload("rules".into(), &payload).expect("run");
        assert_eq!(local.mode, 2);
        assert!(local.system_prompt.contains("no secrets"), "{}", local.system_prompt);
        let prompt = crate::agent_loop::toolkit_prompt(&local);
        assert!(prompt.contains("Mode: Ask"), "{prompt}");
    }

    #[test]
    fn history_image_and_reasoning_are_formatted() {
        let history = ConversationHistory {
            messages: vec![
                ConversationHistoryMessage {
                    message: Some(conversation_history_message::Message::User(
                        ConversationHistoryUserMessage {
                            content: vec![ConversationHistoryUserContent {
                                content: Some(conversation_history_user_content::Content::Image(
                                    ConversationHistoryImage {
                                        data: "abc".into(),
                                        mime_type: Some("image/png".into()),
                                    },
                                )),
                            }],
                        },
                    )),
                },
                ConversationHistoryMessage {
                    message: Some(conversation_history_message::Message::Assistant(
                        ConversationHistoryAssistantMessage {
                            content: vec![ConversationHistoryAssistantContent {
                                content: Some(
                                    conversation_history_assistant_content::Content::Reasoning(
                                        ConversationHistoryReasoning {
                                            text: "think-hard".into(),
                                            signature: None,
                                        },
                                    ),
                                ),
                            }],
                        },
                    )),
                },
            ],
        };
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                conversation_id: Some("conv-img".into()),
                subagent_type_name: Some("explore".into()),
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: "see pic".into(),
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
        let local = local_run_from_agent_payload("hist-img".into(), &payload).expect("run");
        assert_eq!(local.conversation_id.as_deref(), Some("conv-img"));
        assert_eq!(local.subagent_type_name.as_deref(), Some("explore"));
        assert!(local.history.contains("[image image/png]"), "{}", local.history);
        assert!(
            local
                .history_images
                .iter()
                .any(|(mime, data)| mime == "image/png" && data == "abc"),
            "{:?}",
            local.history_images
        );
        assert!(
            local.images.is_empty(),
            "current selected images stay separate from history"
        );
        assert!(local.history.contains("[reasoning]"), "{}", local.history);
        assert!(local.history.contains("think-hard"), "{}", local.history);
        assert!(crate::blob::is_local_conversation("conv-img"));
    }

    #[test]
    fn notify_clone_registers_lineage() {
        crate::blob::remember_conversation("src-conv");
        crate::blob::register_clone("new-conv", "src-conv", "req-1");
        assert_eq!(
            crate::blob::clone_source("new-conv"),
            Some(("src-conv".into(), "req-1".into()))
        );
        assert!(crate::blob::is_local_conversation("new-conv"));
        let frame = user_message_appended("bg done", 1);
        assert!(matches!(
            frame.message,
            Some(agent_server_message::Message::InteractionUpdate(InteractionUpdate {
                message: Some(interaction_update::Message::UserMessageAppended(_)),
            }))
        ));
        let abort = exec_server_abort(9);
        assert!(matches!(
            abort.message,
            Some(agent_server_message::Message::ExecServerControlMessage(_))
        ));
        let mut ck = checkpoint_frame(10, 256_000, Some(r"C:\ws"));
        apply_checkpoint_extras(
            &mut ck,
            &CheckpointExtras {
                mode: 2,
                git_repos: vec![("C:\\repo".into(), "main".into())],
                todos: vec![b"todo".to_vec()],
                pending_tool_calls: vec!["{\"role\":\"assistant\"}".into()],
                summary_archives: vec![b"sum".to_vec()],
                read_paths: vec!["C:\\ws\\Cargo.toml".into()],
                file_states: {
                    let mut map = HashMap::new();
                    map.insert("C:\\ws\\Cargo.toml".into(), Vec::new());
                    map
                },
                plans: {
                    let mut map = HashMap::new();
                    map.insert(
                        "auth".into(),
                        PlanRegistryEntry {
                            id: "auth".into(),
                            path: "file:///.cursor/plans/auth.plan.md".into(),
                        },
                    );
                    map
                },
                subagent_states: {
                    let mut map = HashMap::new();
                    map.insert(
                        "call_task".into(),
                        SubagentPersistedState {
                            model_id: Some("gb-grok-4.6".into()),
                        },
                    );
                    map
                },
                plan: Some(b"plan-body".to_vec()),
            },
        );
        match ck.message {
            Some(agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                assert_eq!(state.mode, Some(2));
                assert_eq!(state.active_branch_name.as_deref(), Some("main"));
                assert_eq!(state.tracked_git_repo_branches[0].repo_path, "C:\\repo");
                assert_eq!(state.todos.len(), 1);
                assert_eq!(state.pending_tool_calls.len(), 1);
                assert_eq!(state.summary_archives.len(), 1);
                assert_eq!(state.read_paths, vec!["C:\\ws\\Cargo.toml"]);
                assert!(state.file_states.contains_key("C:\\ws\\Cargo.toml"));
                assert_eq!(
                    state.plans.get("auth").map(|p| p.path.as_str()),
                    Some("file:///.cursor/plans/auth.plan.md")
                );
                assert_eq!(
                    state
                        .subagent_states
                        .get("call_task")
                        .and_then(|s| s.model_id.as_deref()),
                    Some("gb-grok-4.6")
                );
                assert_eq!(state.plan.as_deref(), Some(b"plan-body".as_slice()));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn shell_command_action_becomes_user_text() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::ShellCommandAction(
                        ShellCommandAction {
                            shell_command: Some(ShellCommand {
                                command: "cargo test".into(),
                            }),
                            exec_id: "sh-1".into(),
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
        let local = local_run_from_agent_payload("sh".into(), &payload).expect("run");
        assert!(local.user_text.contains("cargo test"), "{}", local.user_text);
        assert!(local.user_text.contains("Shell tool"), "{}", local.user_text);
    }

    #[test]
    fn start_plan_action_sets_plan_mode() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::StartPlanAction(
                        StartPlanAction {
                            user_message: Some(UserMessage {
                                text: "plan the auth rewrite".into(),
                                ..Default::default()
                            }),
                            is_spec: false,
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
        let local = local_run_from_agent_payload("plan".into(), &payload).expect("run");
        assert_eq!(local.mode, 3);
        assert!(local.user_text.contains("auth rewrite"), "{}", local.user_text);
    }

    #[test]
    fn cancel_subagent_and_prewarm_are_not_runs() {
        let cancel = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::CancelSubagentAction(
                        CancelSubagentAction {
                            subagent_id: "child-1".into(),
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
        assert!(local_run_from_agent_payload("csub".into(), &cancel).is_none());
        let prewarm = AgentClientMessage {
            message: Some(agent_client_message::Message::PrewarmRequest(
                PrewarmRequest {
                    conversation_id: Some("conv".into()),
                    requested_model: Some(RequestedModel {
                        model_id: "gb-grok-4.6".into(),
                        max_mode: false,
                        parameters: Vec::new(),
                    }),
                    ..Default::default()
                },
            )),
        }
        .encode_to_vec();
        assert!(local_run_from_agent_payload("pw".into(), &prewarm).is_none());
        assert_eq!(name_agent_title("Fix the login\nmore"), "Fix the login");
    }

    #[test]
    fn selected_context_and_text_blob_hydrate() {
        crate::blob::put(
            "user-blob-1",
            &base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"from blob"),
        );
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: String::new(),
                                text_blob_id: Some(b"user-blob-1".to_vec()),
                                selected_context: Some(SelectedContext {
                                    code_selections: vec![SelectedCodeSelection {
                                        path: "src/lib.rs".into(),
                                        content: "fn main() {}".into(),
                                    }],
                                    extra_context: vec!["note-a".into()],
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }),
                            request_context: Some(RequestContext {
                                rules: vec![CursorRule {
                                    full_path: "AGENTS.md".into(),
                                    content: "no secrets".into(),
                                }],
                                web_search_enabled: Some(false),
                                mcp_instructions: vec![McpInstructions {
                                    server_name: "memory".into(),
                                    instructions: "use entities".into(),
                                    server_identifier: "memory".into(),
                                }],
                                ..Default::default()
                            }),
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
        let local = local_run_from_agent_payload("sel".into(), &payload).expect("run");
        assert_eq!(local.user_text, "from blob");
        assert!(!local.web_search);
        assert!(local.extra_prompt.contains("fn main()"), "{}", local.extra_prompt);
        assert!(local.extra_prompt.contains("no secrets"), "{}", local.extra_prompt);
        assert!(local.extra_prompt.contains("use entities"), "{}", local.extra_prompt);
        let prompt = crate::agent_loop::toolkit_prompt(&local);
        assert!(prompt.contains("code_selections"), "{prompt}");
    }

    #[test]
    fn skill_catalog_fits_two_percent_budget() {
        let skills: Vec<AgentSkill> = (0..80)
            .map(|i| AgentSkill {
                full_path: format!("C:/skills/skill-{i}/SKILL.md"),
                description: "x".repeat(400),
                content: "BODY".repeat(200),
                ..Default::default()
            })
            .collect();
        let section = build_agent_skills_section(&skills, 8_000);
        let full = build_agent_skills_section(&skills, 10_000_000);
        let tokens = section.len().div_ceil(4) as u32;
        assert!(
            section.len() * 5 < full.len(),
            "2% budget must shrink catalog {} vs {}",
            section.len(),
            full.len()
        );
        assert!(tokens < 400, "omitted catalog still compact, got {tokens}");
        assert!(section.contains("<agent_skills>"));
        assert!(
            !section.contains("BODYBODY"),
            "catalog must not dump skill bodies"
        );
        assert!(section.contains("omitted") || section.contains("fullPath="));
    }

    #[test]
    fn signed_media_response_has_local_urls() {
        let resp = signed_media_response("gba-abc");
        assert_eq!(resp.key, "gba-abc");
        assert!(resp.post_url.contains("/gba-media/gba-abc"), "{}", resp.post_url);
        assert_eq!(resp.post_url, resp.get_url);
        assert_eq!(resp.put_url, resp.get_url);
        assert!(resp.expires_at_unix_ms > 0);
    }

    #[test]
    fn subagent_overrides_and_mcp_auth_decode() {
        let payload = AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
                action: Some(ConversationAction {
                    action: Some(conversation_action::Action::UserMessageAction(
                        UserMessageAction {
                            user_message: Some(UserMessage {
                                text: "go".into(),
                                ..Default::default()
                            }),
                            request_context: Some(RequestContext {
                                supports_mcp_auth: Some(true),
                                ..Default::default()
                            }),
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
                subagent_model_overrides: vec![SubagentModelOverride {
                    subagent_type: "explore".into(),
                    selection: Some(subagent_model_override::Selection::Model(RequestedModel {
                        model_id: "gb-grok-4".into(),
                        max_mode: false,
                        parameters: Vec::new(),
                    })),
                }],
                ..Default::default()
            })),
        }
        .encode_to_vec();
        let local = local_run_from_agent_payload("ov".into(), &payload).expect("run");
        assert!(local.supports_mcp_auth);
        assert_eq!(
            local.subagent_overrides,
            vec![("explore".into(), SubagentOverride::Model("gb-grok-4".into()))]
        );
    }

    #[test]
    fn assemble_preamble_orders_ccursor_tags() {
        let src = "<code_selections>\nfn x\n</code_selections>\n<user_info>\nOS\n</user_info>\n<rules>\nr\n</rules>\n";
        let ordered = assemble_preamble(src);
        let info = ordered.find("<user_info>").expect("info");
        let rules = ordered.find("<rules>").expect("rules");
        let code = ordered.find("<code_selections>").expect("code");
        assert!(info < rules && rules < code, "{ordered}");
        let pending = assemble_preamble(
            "<code_selections>c</code_selections>\n<extra_context_pending blob_count=\"2\" />\n",
        );
        let pending_at = pending.find("<extra_context_pending").expect("pending");
        let code_at = pending.find("<code_selections>").expect("code");
        assert!(pending_at < code_at, "{pending}");
    }
}
