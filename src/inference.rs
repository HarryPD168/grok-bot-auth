//! aiserver.v1.InferenceService/Stream — field numbers from
//! cursor-byok-custom/protocols/cursor/aiserver_v1.proto (InferenceStreamRequest var lDf).
use prost::Message;

use crate::catalog::INJECT_PREFIX;
use crate::connect::encode_connect_frame;
use crate::sand::family_from_model_id;

const ROLE_USER: i32 = 1;
const ROLE_ASSISTANT: i32 = 2;
const ROLE_SYSTEM: i32 = 4;

#[derive(Clone, PartialEq, Message)]
pub struct InferenceModelParameterValue {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceRequestedModel {
    #[prost(string, tag = "1")]
    pub model_id: String,
    #[prost(bool, tag = "2")]
    pub max_mode: bool,
    #[prost(message, repeated, tag = "3")]
    pub parameters: Vec<InferenceModelParameterValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceCoreMessage {
    #[prost(int32, tag = "1")]
    pub role: i32,
    #[prost(oneof = "inference_core_message::Content", tags = "2, 3, 6")]
    pub content: Option<inference_core_message::Content>,
}

pub mod inference_core_message {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Content {
        #[prost(string, tag = "2")]
        Text(String),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceStreamRequest {
    #[prost(message, repeated, tag = "1")]
    pub messages: Vec<InferenceCoreMessage>,
    #[prost(message, repeated, tag = "2")]
    pub tools: Vec<InferenceAgentTool>,
    #[prost(message, optional, tag = "7")]
    pub requested_model: Option<InferenceRequestedModel>,
    #[prost(string, optional, tag = "5")]
    pub model_id: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub invocation_id: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub conversation_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceAgentTool {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(message, optional, tag = "3")]
    pub parameters: Option<prost_types::Struct>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceTextStreamPart {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(bool, tag = "2")]
    pub is_final: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceThinkingStreamPart {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(string, optional, tag = "2")]
    pub signature: Option<String>,
    #[prost(bool, tag = "3")]
    pub is_final: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceStreamError {
    #[prost(string, tag = "1")]
    pub message: String,
    #[prost(string, tag = "2")]
    pub code: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceUsageInfo {
    #[prost(int32, tag = "1")]
    pub prompt_tokens: i32,
    #[prost(int32, tag = "2")]
    pub completion_tokens: i32,
    #[prost(int32, optional, tag = "3")]
    pub total_tokens: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceStreamResponse {
    #[prost(oneof = "inference_stream_response::Response", tags = "1, 2, 3, 8, 9")]
    pub response: Option<inference_stream_response::Response>,
}

pub mod inference_stream_response {
    use super::{
        InferenceStreamError, InferenceTextStreamPart, InferenceThinkingStreamPart,
        InferenceUsageInfo,
    };

    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Response {
        #[prost(message, tag = "1")]
        TextPart(InferenceTextStreamPart),
        #[prost(message, tag = "2")]
        ToolCallPart(super::InferenceToolCallStub),
        #[prost(message, tag = "3")]
        Usage(InferenceUsageInfo),
        #[prost(message, tag = "8")]
        Error(InferenceStreamError),
        #[prost(message, tag = "9")]
        ThinkingPart(InferenceThinkingStreamPart),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct InferenceToolCallStub {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(string, tag = "2")]
    pub tool_name: String,
    #[prost(string, tag = "3")]
    pub args: String,
    #[prost(bool, tag = "4")]
    pub is_complete: bool,
}

#[derive(Debug, Default, Clone)]
pub struct InferenceDelta {
    pub thinking: String,
    pub text: String,
    pub error: Option<String>,
    pub tool_id: String,
    pub tool_name: String,
    pub tool_args: String,
    pub tool_complete: bool,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
}

pub fn sand_model_id(model: &str) -> String {
    let stripped = model.strip_prefix(INJECT_PREFIX).unwrap_or(model);
    family_from_model_id(stripped)
}

pub fn encode_inference_request(
    model: &str,
    message: &str,
    effort: Option<&str>,
    fast: bool,
) -> Vec<u8> {
    encode_inference_request_knobs(model, message, effort, fast, None)
}

pub fn encode_inference_request_knobs(
    model: &str,
    message: &str,
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> Vec<u8> {
    encode_connect_frame(&encode_inference_payload_knobs(
        model, message, effort, fast, context,
    ))
}

pub fn encode_inference_payload(
    model: &str,
    message: &str,
    effort: Option<&str>,
    fast: bool,
) -> Vec<u8> {
    encode_inference_payload_knobs(model, message, effort, fast, None)
}

pub fn encode_inference_payload_knobs(
    model: &str,
    message: &str,
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> Vec<u8> {
    encode_inference_payload_turns(
        model,
        &[crate::compact::ChatTurn::new("user", message)],
        effort,
        fast,
        context,
    )
}

pub fn encode_inference_request_turns(
    model: &str,
    turns: &[crate::compact::ChatTurn],
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> Vec<u8> {
    encode_inference_request_turns_mode(
        model,
        turns,
        effort,
        fast,
        context,
        1,
        false,
        crate::providers::ToolFlags::default(),
    )
}

pub fn encode_inference_request_turns_mode(
    model: &str,
    turns: &[crate::compact::ChatTurn],
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
    mode: i32,
    is_subagent: bool,
    flags: crate::providers::ToolFlags,
) -> Vec<u8> {
    encode_connect_frame(&encode_inference_payload_turns_mode(
        model, turns, effort, fast, context, mode, is_subagent, flags,
    ))
}

pub fn encode_inference_payload_turns(
    model: &str,
    turns: &[crate::compact::ChatTurn],
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
) -> Vec<u8> {
    encode_inference_payload_turns_mode(
        model,
        turns,
        effort,
        fast,
        context,
        1,
        false,
        crate::providers::ToolFlags::default(),
    )
}

pub fn encode_inference_payload_turns_mode(
    model: &str,
    turns: &[crate::compact::ChatTurn],
    effort: Option<&str>,
    fast: bool,
    context: Option<&str>,
    mode: i32,
    is_subagent: bool,
    flags: crate::providers::ToolFlags,
) -> Vec<u8> {
    let family = sand_model_id(model);
    let details = crate::sand::model_details_for_knobs(&family, effort, fast, context);
    let id = uuid::Uuid::new_v4().to_string();
    let messages = turns
        .iter()
        .map(|turn| InferenceCoreMessage {
            role: match turn.role.as_str() {
                "assistant" => ROLE_ASSISTANT,
                "system" => ROLE_SYSTEM,
                _ => ROLE_USER,
            },
            content: Some(inference_core_message::Content::Text(turn.text.clone())),
        })
        .collect();
    InferenceStreamRequest {
        messages,
        tools: inference_host_tools_for(mode, is_subagent, flags),
        requested_model: Some(InferenceRequestedModel {
            model_id: family,
            max_mode: false,
            parameters: details
                .parameters
                .into_iter()
                .map(|p| InferenceModelParameterValue {
                    id: p.id,
                    value: p.value,
                })
                .collect(),
        }),
        model_id: None,
        invocation_id: Some(id.clone()),
        conversation_id: Some(id),
    }
    .encode_to_vec()
}

pub fn inference_host_tools_for(
    mode: i32,
    is_subagent: bool,
    flags: crate::providers::ToolFlags,
) -> Vec<InferenceAgentTool> {
    let Some(array) = crate::providers::filter_host_tools(
        crate::providers::cursor_host_tools_openai(),
        mode,
        is_subagent,
        flags,
    )
    .as_array()
    .cloned()
    else {
        return Vec::new();
    };
    array
        .into_iter()
        .filter_map(|tool| {
            let function = tool.get("function")?;
            Some(InferenceAgentTool {
                name: function.get("name")?.as_str()?.to_owned(),
                description: function
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                parameters: function.get("parameters").and_then(json_to_struct),
            })
        })
        .collect()
}

fn json_to_struct(value: &serde_json::Value) -> Option<prost_types::Struct> {
    match json_to_prost_value(value).kind? {
        prost_types::value::Kind::StructValue(s) => Some(s),
        _ => None,
    }
}

fn json_to_prost_value(value: &serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match value {
        serde_json::Value::Null => Some(Kind::NullValue(0)),
        serde_json::Value::Bool(flag) => Some(Kind::BoolValue(*flag)),
        serde_json::Value::Number(number) => {
            Some(Kind::NumberValue(number.as_f64().unwrap_or(0.0)))
        }
        serde_json::Value::String(text) => Some(Kind::StringValue(text.clone())),
        serde_json::Value::Array(items) => Some(Kind::ListValue(prost_types::ListValue {
            values: items.iter().map(json_to_prost_value).collect(),
        })),
        serde_json::Value::Object(map) => Some(Kind::StructValue(prost_types::Struct {
            fields: map
                .iter()
                .map(|(key, item)| (key.clone(), json_to_prost_value(item)))
                .collect(),
        })),
    };
    prost_types::Value { kind }
}

pub fn decode_inference_payload(payload: &[u8]) -> InferenceDelta {
    if let Ok(msg) = InferenceStreamResponse::decode(payload) {
        return match msg.response {
            Some(inference_stream_response::Response::TextPart(part)) => InferenceDelta {
                text: part.text,
                ..InferenceDelta::default()
            },
            Some(inference_stream_response::Response::ThinkingPart(part)) => InferenceDelta {
                thinking: part.text,
                ..InferenceDelta::default()
            },
            Some(inference_stream_response::Response::Error(err)) => InferenceDelta {
                error: Some(if err.message.is_empty() {
                    err.code
                } else {
                    err.message
                }),
                ..InferenceDelta::default()
            },
            Some(inference_stream_response::Response::ToolCallPart(part)) => InferenceDelta {
                tool_id: part.tool_call_id,
                tool_name: part.tool_name,
                tool_args: part.args,
                tool_complete: part.is_complete,
                ..InferenceDelta::default()
            },
            Some(inference_stream_response::Response::Usage(usage)) => InferenceDelta {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                ..InferenceDelta::default()
            },
            _ => InferenceDelta::default(),
        };
    }
    crate::connect::parse_connect_payload(payload).into()
}

impl From<crate::connect::ChatDelta> for InferenceDelta {
    fn from(delta: crate::connect::ChatDelta) -> Self {
        Self {
            thinking: delta.thinking,
            text: delta.text,
            error: delta.error,
            prompt_tokens: delta.prompt_tokens,
            completion_tokens: delta.completion_tokens,
            ..Self::default()
        }
    }
}

pub fn history_to_messages(
    system: Option<&str>,
    user: &str,
    prior_assistant: Option<&str>,
) -> Vec<InferenceCoreMessage> {
    let mut messages = Vec::new();
    if let Some(system) = system.filter(|s| !s.is_empty()) {
        messages.push(InferenceCoreMessage {
            role: ROLE_SYSTEM,
            content: Some(inference_core_message::Content::Text(system.to_owned())),
        });
    }
    if let Some(assistant) = prior_assistant.filter(|s| !s.is_empty()) {
        messages.push(InferenceCoreMessage {
            role: ROLE_ASSISTANT,
            content: Some(inference_core_message::Content::Text(assistant.to_owned())),
        });
    }
    messages.push(InferenceCoreMessage {
        role: ROLE_USER,
        content: Some(inference_core_message::Content::Text(user.to_owned())),
    });
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::split_first_frame;

    #[test]
    fn encodes_user_text_and_grok_parameters() {
        let framed = encode_inference_request("gb-grok-4.6", "ping", Some("high"), true);
        assert_eq!(framed[0], 0);
        let (_flags, payload, rest) = split_first_frame(&framed).expect("connect frame");
        assert!(rest.is_empty());
        let req = InferenceStreamRequest::decode(payload).expect("proto");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, ROLE_USER);
        assert_eq!(
            req.messages[0].content,
            Some(inference_core_message::Content::Text("ping".into()))
        );
        let model = req.requested_model.expect("requested_model");
        assert_eq!(model.model_id, "grok-4.6");
        assert!(!model.model_id.starts_with("gb-"));
        assert!(
            req.conversation_id
                .as_deref()
                .is_some_and(|id| !id.is_empty()),
            "api2 requires conversation_id on Stream"
        );
        assert!(
            req.invocation_id
                .as_deref()
                .is_some_and(|id| !id.is_empty()),
            "invocation_id should be set with conversation_id"
        );
        let map: std::collections::BTreeMap<_, _> = model
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(map.get("effort"), Some(&"high"));
        assert_eq!(map.get("fast"), Some(&"true"));
        assert!(
            req.tools.iter().any(|tool| tool.name == "Read"),
            "Grok Stream must advertise Cursor host tools"
        );
    }

    #[test]
    fn decode_tool_call_part_is_not_dropped() {
        let payload = InferenceStreamResponse {
            response: Some(inference_stream_response::Response::ToolCallPart(
                InferenceToolCallStub {
                    tool_call_id: "c1".into(),
                    tool_name: "Read".into(),
                    args: r#"{"path":"Cargo.toml"}"#.into(),
                    is_complete: true,
                },
            )),
        }
        .encode_to_vec();
        let delta = decode_inference_payload(&payload);
        assert_eq!(delta.tool_name, "Read");
        assert_eq!(delta.tool_args, r#"{"path":"Cargo.toml"}"#);
        assert!(delta.tool_complete);
    }

    #[test]
    fn encode_available_family_does_not_invent_context_param() {
        let framed = encode_inference_request_knobs(
            "gb-grok-4.6",
            "ping",
            Some("low"),
            false,
            Some("128000"),
        );
        let (_flags, payload, _) = split_first_frame(&framed).expect("connect frame");
        let req = InferenceStreamRequest::decode(payload).expect("proto");
        let model = req.requested_model.expect("requested_model");
        let map: std::collections::BTreeMap<_, _> = model
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(map.get("effort"), Some(&"low"));
        assert_eq!(map.get("fast"), Some(&"false"));
        assert!(!map.contains_key("context"));
        let pooled = crate::prompt_pool::apply_prompt_pool(
            &[String::from("pool-entry")],
            "Reply with exactly: pong",
        );
        let framed = encode_inference_request("grok-4.6", &pooled, Some("high"), false);
        let (_flags, payload, _) = split_first_frame(&framed).expect("frame");
        let req = InferenceStreamRequest::decode(payload).expect("proto");
        match &req.messages[0].content {
            Some(inference_core_message::Content::Text(text)) => {
                assert!(text.contains("pool-entry"));
                assert!(text.contains("Reply with exactly: pong"));
            }
            other => panic!("expected text {other:?}"),
        }
    }

    #[test]
    fn decodes_text_and_thinking_parts() {
        let text = InferenceStreamResponse {
            response: Some(inference_stream_response::Response::TextPart(
                InferenceTextStreamPart {
                    text: "pong".into(),
                    is_final: true,
                },
            )),
        }
        .encode_to_vec();
        assert_eq!(decode_inference_payload(&text).text, "pong");
        let think = InferenceStreamResponse {
            response: Some(inference_stream_response::Response::ThinkingPart(
                InferenceThinkingStreamPart {
                    text: "plan".into(),
                    signature: None,
                    is_final: false,
                },
            )),
        }
        .encode_to_vec();
        assert_eq!(decode_inference_payload(&think).thinking, "plan");
    }

    #[test]
    fn decodes_usage_info() {
        let payload = InferenceStreamResponse {
            response: Some(inference_stream_response::Response::Usage(
                InferenceUsageInfo {
                    prompt_tokens: 40500,
                    completion_tokens: 120,
                    total_tokens: Some(40620),
                },
            )),
        }
        .encode_to_vec();
        let delta = decode_inference_payload(&payload);
        assert_eq!(delta.prompt_tokens, 40500);
        assert_eq!(delta.completion_tokens, 120);
    }

    #[test]
    fn strips_inject_prefix() {
        assert_eq!(sand_model_id("gb-claude-opus-4-6"), "claude-opus-4-6");
        assert_eq!(sand_model_id("grok-4.6"), "grok-4.6");
        assert_eq!(
            sand_model_id("gb-grok-4.6[effort=xhigh,fast=true]"),
            "grok-4.6"
        );
        assert_eq!(
            sand_model_id("gb-p/xai-X/grok-4.6[effort=high,fast=true]"),
            "grok-4.6"
        );
    }

    #[test]
    fn encode_turns_keeps_compacted_history_and_knobs() {
        let history: Vec<crate::compact::ChatTurn> = (0..10)
            .map(|i| {
                crate::compact::ChatTurn::new(
                    if i % 2 == 0 { "user" } else { "assistant" },
                    format!("turn-{i} {}", "word ".repeat(40)),
                )
            })
            .collect();
        let turns = crate::compact::prepare_outbound(
            &[String::from("pool-entry")],
            &history,
            "Reply with exactly: pong",
            500,
        );
        let framed = encode_inference_request_turns(
            "gb-grok-4.6",
            &turns,
            Some("medium"),
            false,
            Some("128000"),
        );
        let (_flags, payload, _) = split_first_frame(&framed).expect("frame");
        let req = InferenceStreamRequest::decode(payload).expect("proto");
        assert!(
            req.messages.len() > 1,
            "compacted history is multiple messages"
        );
        let texts: Vec<String> = req
            .messages
            .iter()
            .filter_map(|m| match &m.content {
                Some(inference_core_message::Content::Text(t)) => Some(t.clone()),
                _ => None,
            })
            .collect();
        let joined = texts.join("\n");
        assert!(joined.contains("pool-entry"));
        assert!(joined.contains("Reply with exactly: pong"));
        let model = req.requested_model.expect("requested_model");
        let map: std::collections::BTreeMap<_, _> = model
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(map.get("effort"), Some(&"medium"));
        assert_eq!(map.get("fast"), Some(&"false"));
        assert!(!map.contains_key("context"));
    }
}
