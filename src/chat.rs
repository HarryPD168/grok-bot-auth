use prost::Message;

use crate::connect::encode_connect_frame;
use crate::sand::family_from_model_id;

#[derive(Clone, PartialEq, Message)]
pub struct ConversationMessage {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(int32, tag = "2")]
    pub r#type: i32,
    #[prost(string, tag = "13")]
    pub bubble_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ModelDetails {
    #[prost(string, optional, tag = "1")]
    pub model_name: Option<String>,
    #[prost(bool, optional, tag = "3")]
    pub enable_ghost_mode: Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub enable_slow_pool: Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub max_mode: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct EnvironmentInfo {
    #[prost(string, optional, tag = "1")]
    pub exthost_platform: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub exthost_arch: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub local_timestamp: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StreamUnifiedChatRequest {
    #[prost(message, repeated, tag = "1")]
    pub conversation: Vec<ConversationMessage>,
    #[prost(message, optional, tag = "5")]
    pub model_details: Option<ModelDetails>,
    #[prost(bool, tag = "22")]
    pub is_chat: bool,
    #[prost(string, tag = "23")]
    pub conversation_id: String,
    #[prost(bool, tag = "27")]
    pub is_agentic: bool,
    #[prost(bool, tag = "37")]
    pub allow_model_fallbacks: bool,
    #[prost(bool, tag = "48")]
    pub should_disable_tools: bool,
    #[prost(int32, optional, tag = "49")]
    pub thinking_level: Option<i32>,
    #[prost(message, optional, tag = "26")]
    pub environment_info: Option<EnvironmentInfo>,
    #[prost(bool, tag = "33")]
    pub use_unified_chat_prompt: bool,
    #[prost(bool, optional, tag = "50")]
    pub should_use_chat_prompt: Option<bool>,
    #[prost(bool, tag = "69")]
    pub force_is_not_dev: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct Thinking {
    #[prost(string, tag = "1")]
    pub text: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct StreamUnifiedChatResponse {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(message, optional, tag = "25")]
    pub thinking: Option<Thinking>,
}

const HUMAN: i32 = 1;
const THINKING_MEDIUM: i32 = 1;
const THINKING_HIGH: i32 = 2;

#[derive(Clone, PartialEq, Message)]
pub struct GetChatRequest {
    #[prost(message, repeated, tag = "2")]
    pub conversation: Vec<ConversationMessage>,
    #[prost(message, optional, tag = "7")]
    pub model_details: Option<ModelDetails>,
    #[prost(string, tag = "9")]
    pub request_id: String,
    #[prost(string, tag = "15")]
    pub conversation_id: String,
    #[prost(bool, optional, tag = "30")]
    pub allow_model_fallbacks: Option<bool>,
}

pub fn encode_get_chat_request(model: &str, message: &str, fast: bool) -> Vec<u8> {
    let family = family_from_model_id(model);
    let id = uuid::Uuid::new_v4().to_string();
    let req = GetChatRequest {
        conversation: vec![ConversationMessage {
            text: message.to_owned(),
            r#type: HUMAN,
            bubble_id: uuid::Uuid::new_v4().to_string(),
        }],
        model_details: Some(ModelDetails {
            model_name: Some(family),
            enable_ghost_mode: Some(false),
            enable_slow_pool: Some(!fast),
            max_mode: Some(false),
        }),
        request_id: id.clone(),
        conversation_id: id,
        allow_model_fallbacks: Some(true),
    };
    encode_connect_frame(&req.encode_to_vec())
}

pub fn encode_stream_request(model: &str, message: &str, effort: Option<&str>, fast: bool) -> Vec<u8> {
    let family = family_from_model_id(model);
    let thinking_level = match effort.unwrap_or("high") {
        "low" | "medium" => Some(THINKING_MEDIUM),
        "high" | "xhigh" | "max" => Some(THINKING_HIGH),
        _ => Some(THINKING_HIGH),
    };
    let req = StreamUnifiedChatRequest {
        conversation: vec![ConversationMessage {
            text: message.to_owned(),
            r#type: HUMAN,
            bubble_id: uuid::Uuid::new_v4().to_string(),
        }],
        model_details: Some(ModelDetails {
            model_name: Some(family),
            enable_ghost_mode: Some(false),
            enable_slow_pool: Some(!fast),
            max_mode: Some(false),
        }),
        is_chat: true,
        conversation_id: uuid::Uuid::new_v4().to_string(),
        is_agentic: false,
        allow_model_fallbacks: true,
        should_disable_tools: true,
        thinking_level,
        environment_info: Some(EnvironmentInfo {
            exthost_platform: Some("win32".into()),
            exthost_arch: Some("x64".into()),
            local_timestamp: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis().to_string())
                    .unwrap_or_default(),
            ),
        }),
        use_unified_chat_prompt: true,
        should_use_chat_prompt: Some(true),
        force_is_not_dev: true,
    };
    encode_connect_frame(&req.encode_to_vec())
}

pub fn decode_stream_payload(payload: &[u8]) -> (String, String) {
    if let Ok(msg) = StreamUnifiedChatResponse::decode(payload) {
        let thinking = msg.thinking.map(|t| t.text).unwrap_or_default();
        return (thinking, msg.text);
    }
    let json = crate::connect::parse_connect_payload(payload);
    (json.thinking, json.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frame_starts_with_connect_envelope() {
        let bytes = encode_stream_request("grok-4.6", "hi", Some("high"), false);
        assert_eq!(bytes[0], 0);
        let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
        assert_eq!(len, bytes.len() - 5);
        assert!(len > 8);
    }
}
