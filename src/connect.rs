use std::io::Read;

use bytes::Bytes;
use flate2::read::GzDecoder;

use crate::error::{Error, Result};

const COMPRESSED: u8 = 0x01;

pub fn maybe_gunzip(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut out = Vec::new();
        if GzDecoder::new(bytes).read_to_end(&mut out).is_ok() && !out.is_empty() {
            return out;
        }
    }
    bytes.to_vec()
}

/// HTTP gzip and Connect flag 0x01 gzip, so `gb-*` is visible to scanners.
pub fn prepare_cursor_body(body: &[u8]) -> Vec<u8> {
    let body = maybe_gunzip(body);
    if body.len() < 5 {
        return body;
    }
    let flags = body[0];
    let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    if flags & COMPRESSED == 0 || length > MAX_CONNECT_FRAME || body.len() < 5 + length {
        return body;
    }
    let inflated = maybe_gunzip(&body[5..5 + length]);
    let mut out = encode_connect_frame(&inflated);
    out.extend_from_slice(&body[5 + length..]);
    out
}
pub const MAX_CONNECT_FRAME: usize = 8 * 1024 * 1024;
pub const MAX_CONNECT_BUFFER: usize = 16 * 1024 * 1024;

pub fn take_connect_frames(buffered: &mut Vec<u8>) -> Result<Vec<Bytes>> {
    let mut frames = Vec::new();
    loop {
        if buffered.len() < 5 {
            break;
        }
        let flags = buffered[0];
        let length =
            u32::from_be_bytes([buffered[1], buffered[2], buffered[3], buffered[4]]) as usize;
        if flags & COMPRESSED != 0 {
            return Err(Error::Msg(
                "compressed Connect frames are not supported".into(),
            ));
        }
        if length > MAX_CONNECT_FRAME {
            return Err(Error::Msg(format!(
                "Connect frame too large: {length} bytes"
            )));
        }
        if buffered.len() < 5 + length {
            if buffered.len() > MAX_CONNECT_BUFFER {
                return Err(Error::Msg(
                    "Connect buffer exceeded cap while waiting for frame".into(),
                ));
            }
            break;
        }
        let payload = Bytes::copy_from_slice(&buffered[5..5 + length]);
        buffered.drain(..5 + length);
        frames.push(payload);
    }
    Ok(frames)
}

pub const END_STREAM_FLAG: u8 = 0x02;

pub fn encode_connect_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(0);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn encode_end_stream() -> Vec<u8> {
    encode_end_stream_payload(b"{}")
}

pub fn encode_error_end_stream(code: &str, message: &str) -> Vec<u8> {
    let payload = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
            "details": [{
                "type": "aiserver.v1.ErrorDetails",
                "debug": {
                    "error": "ERROR_UNSPECIFIED",
                    "details": { "detail": message }
                }
            }]
        }
    });
    encode_end_stream_payload(&payload.to_string().into_bytes())
}

fn encode_end_stream_payload(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(END_STREAM_FLAG);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// First uncompressed Connect data frame + leftover (trailers).
pub fn split_first_frame(body: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    if body.len() < 5 {
        return None;
    }
    let flags = body[0];
    if flags & COMPRESSED != 0 {
        return None;
    }
    let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    if length > MAX_CONNECT_FRAME || body.len() < 5 + length {
        return None;
    }
    Some((flags, &body[5..5 + length], &body[5 + length..]))
}

pub fn frame_then_rest(payload: &[u8], rest: &[u8]) -> Vec<u8> {
    let mut out = encode_connect_frame(payload);
    out.extend_from_slice(rest);
    out
}

pub fn unwrap_connect(body: &[u8]) -> (bool, &[u8]) {
    if body.len() >= 5 {
        let flags = body[0];
        let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        if flags & COMPRESSED == 0
            && length == body.len().saturating_sub(5)
            && length <= MAX_CONNECT_FRAME
        {
            return (true, &body[5..]);
        }
    }
    (false, body)
}

#[derive(Debug, Clone, Default)]
pub struct ChatToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Default)]
pub struct ChatDelta {
    pub thinking: String,
    pub text: String,
    pub error: Option<String>,
    pub tools: Vec<ChatToolCall>,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub reasoning_tokens: i32,
}

fn connect_error_text(err: &serde_json::Value) -> String {
    let code = err.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let message = err.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let debug = err
        .pointer("/details/0/debug/error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let detail = err
        .pointer("/details/0/debug/details/detail")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mut parts = Vec::new();
    if !code.is_empty() {
        parts.push(code);
    }
    if !debug.is_empty() && debug != message {
        parts.push(debug);
    }
    if !detail.is_empty() {
        parts.push(detail);
    } else if !message.is_empty() && message != "Error" {
        parts.push(message);
    }
    if parts.is_empty() {
        "stream error".into()
    } else {
        parts.join(" / ")
    }
}

pub fn parse_connect_payload(payload: &[u8]) -> ChatDelta {
    let trimmed = String::from_utf8_lossy(payload);
    let trimmed = trimmed.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return ChatDelta::default();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return ChatDelta::default();
    };
    if let Some(err) = value.get("error") {
        return ChatDelta {
            error: Some(connect_error_text(err)),
            ..ChatDelta::default()
        };
    }
    let inner = value
        .get("streamUnifiedChatResponse")
        .or_else(|| value.get("stream_unified_chat_response"))
        .unwrap_or(&value);
    ChatDelta {
        thinking: inner
            .pointer("/thinking/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
        text: inner
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
        error: None,
        tools: Vec::new(),
        ..ChatDelta::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_complete_envelopes_and_keeps_tail() {
        let mut buffered = vec![0, 0, 0, 0, 2, b'{', b'}', 0, 0, 0, 0, 4];
        let frames = take_connect_frames(&mut buffered).unwrap();
        assert_eq!(frames, vec![Bytes::from_static(b"{}")]);
        assert_eq!(buffered, vec![0, 0, 0, 0, 4]);
        buffered.extend_from_slice(b"abcd");
        let frames = take_connect_frames(&mut buffered).unwrap();
        assert_eq!(frames, vec![Bytes::from_static(b"abcd")]);
        assert!(buffered.is_empty());
    }

    #[test]
    fn prepare_cursor_body_unzips_http_and_connect_flag() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let inner = b"xxgb-grok-4.6[effort=high]";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(inner).unwrap();
        let gz = encoder.finish().unwrap();
        assert_eq!(maybe_gunzip(&gz), inner);
        let mut framed = vec![COMPRESSED];
        framed.extend_from_slice(&(gz.len() as u32).to_be_bytes());
        framed.extend_from_slice(&gz);
        let out = prepare_cursor_body(&framed);
        assert!(out.windows(inner.len()).any(|w| w == inner) || out == inner);
        assert!(crate::agent_wire::find_gb_model(&prepare_cursor_body(&gz)).is_some());
    }

    #[test]
    fn parse_thinking_then_text() {
        let delta = parse_connect_payload(br#"{"thinking":{"text":"plan"}}"#);
        assert_eq!(delta.thinking, "plan");
        let delta = parse_connect_payload(br#"{"text":"hello"}"#);
        assert_eq!(delta.text, "hello");
    }

    #[test]
    fn parse_not_logged_in_details() {
        let payload = br#"{"error":{"code":"unauthenticated","message":"Error","details":[{"type":"aiserver.v1.ErrorDetails","debug":{"error":"ERROR_NOT_LOGGED_IN","details":{"detail":"If you are logged in, try logging out and back in."}}}]}}"#;
        let delta = parse_connect_payload(payload);
        let error = delta.error.expect("error");
        assert!(error.contains("unauthenticated"), "{error}");
        assert!(error.contains("ERROR_NOT_LOGGED_IN"), "{error}");
        assert!(error.contains("logging out"), "{error}");
    }

    #[test]
    fn parse_permission_denied_keeps_codes() {
        let payload = br#"{"error":{"code":"permission_denied","message":"This model is unavailable for Grok Bot inference","details":[{"type":"aiserver.v1.ErrorDetails","debug":{"error":"ERROR_NOT_HIGH_ENOUGH_PERMISSIONS"}}]}}"#;
        let error = parse_connect_payload(payload).error.expect("error");
        assert!(error.contains("permission_denied"), "{error}");
        assert!(
            error.contains("ERROR_NOT_HIGH_ENOUGH_PERMISSIONS"),
            "{error}"
        );
        assert!(
            error.contains("unavailable for Grok Bot inference"),
            "{error}"
        );
    }

    #[test]
    fn rejects_huge_claimed_length() {
        let mut buffered = vec![0, 0x7f, 0xff, 0xff, 0xff];
        assert!(take_connect_frames(&mut buffered).is_err());
    }
}
