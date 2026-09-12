//! Cursor 3.20 `/agent/v1/run` WebSocket frames (`agent.v1.AgentWebSocket*`).
//! Inner `client_message.message` is raw AgentClientMessage protobuf (ASCII `gb-`
//! is visible). HTTP BidiAppend hex-encodes the same payload.

use prost::Message;

use crate::agent_wire::{self, LocalRun};

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsHello {
    #[prost(int32, tag = "1")]
    pub protocol_version: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsRunIdentity {
    #[prost(string, tag = "1")]
    pub run_id: String,
    #[prost(string, tag = "2")]
    pub request_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsHeader {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsRunStart {
    #[prost(message, optional, tag = "1")]
    pub identity: Option<AgentWsRunIdentity>,
    #[prost(message, repeated, tag = "2")]
    pub headers: Vec<AgentWsHeader>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsClientMessage {
    #[prost(string, tag = "1")]
    pub run_id: String,
    #[prost(uint64, tag = "2")]
    pub sequence: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub message: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsHalfClose {
    #[prost(string, tag = "1")]
    pub run_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsCancelRun {
    #[prost(string, tag = "1")]
    pub run_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsClientFrame {
    #[prost(oneof = "agent_ws_client_frame::Frame", tags = "1, 2, 3, 4, 5")]
    pub frame: Option<agent_ws_client_frame::Frame>,
}

pub mod agent_ws_client_frame {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Frame {
        #[prost(message, tag = "1")]
        Hello(super::AgentWsHello),
        #[prost(message, tag = "2")]
        RunStart(super::AgentWsRunStart),
        #[prost(message, tag = "3")]
        ClientMessage(super::AgentWsClientMessage),
        #[prost(message, tag = "4")]
        HalfClose(super::AgentWsHalfClose),
        #[prost(message, tag = "5")]
        CancelRun(super::AgentWsCancelRun),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsRunAccepted {
    #[prost(message, optional, tag = "1")]
    pub identity: Option<AgentWsRunIdentity>,
    #[prost(message, repeated, tag = "2")]
    pub headers: Vec<AgentWsHeader>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsServerMessage {
    #[prost(string, tag = "1")]
    pub run_id: String,
    #[prost(uint64, tag = "2")]
    pub sequence: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub message: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsConnectError {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub raw_message: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsRunEnd {
    #[prost(message, optional, tag = "1")]
    pub identity: Option<AgentWsRunIdentity>,
    #[prost(message, repeated, tag = "2")]
    pub trailers: Vec<AgentWsHeader>,
    #[prost(message, optional, tag = "3")]
    pub error: Option<AgentWsConnectError>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsRunRejected {
    #[prost(message, optional, tag = "1")]
    pub identity: Option<AgentWsRunIdentity>,
    #[prost(message, optional, tag = "2")]
    pub error: Option<AgentWsConnectError>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AgentWsServerFrame {
    #[prost(oneof = "agent_ws_server_frame::Frame", tags = "1, 2, 3, 4, 5")]
    pub frame: Option<agent_ws_server_frame::Frame>,
}

pub mod agent_ws_server_frame {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Frame {
        #[prost(message, tag = "1")]
        Hello(super::AgentWsHello),
        #[prost(message, tag = "2")]
        RunAccepted(super::AgentWsRunAccepted),
        #[prost(message, tag = "3")]
        ServerMessage(super::AgentWsServerMessage),
        #[prost(message, tag = "4")]
        RunEnd(super::AgentWsRunEnd),
        #[prost(message, tag = "5")]
        RunRejected(super::AgentWsRunRejected),
    }
}

#[derive(Debug, Clone)]
pub enum ClientWs {
    Hello,
    RunStart(AgentWsRunIdentity, Vec<u8>),
    AgentMessage {
        run_id: String,
        run: Option<LocalRun>,
        exec: Option<crate::agent_proto::ExecClientMessage>,
        kv: Option<crate::agent_wire::KvClientMessage>,
        interaction: Option<crate::agent_wire::InteractionResponse>,
    },
    HalfClose(String),
    Cancel(String),
    Other,
}

pub fn decode_client_ws(bytes: &[u8]) -> ClientWs {
    let Ok(frame) = AgentWsClientFrame::decode(bytes) else {
        if agent_wire::find_gb_model(bytes).is_some() {
            let run = agent_wire::local_run_from_agent_payload("ws".into(), bytes);
            return ClientWs::AgentMessage {
                run_id: run
                    .as_ref()
                    .map(|item| item.request_id.clone())
                    .unwrap_or_else(|| "ws".into()),
                run,
                exec: None,
                kv: None,
                interaction: None,
            };
        }
        return ClientWs::Other;
    };
    match frame.frame {
        Some(agent_ws_client_frame::Frame::Hello(_)) => ClientWs::Hello,
        Some(agent_ws_client_frame::Frame::RunStart(start)) => {
            let identity = start.identity.unwrap_or_default();
            ClientWs::RunStart(identity, bytes.to_vec())
        }
        Some(agent_ws_client_frame::Frame::ClientMessage(message)) => {
            let request_id = if message.run_id.is_empty() {
                "ws".into()
            } else {
                message.run_id.clone()
            };
            let run = agent_wire::local_run_from_agent_payload(request_id, &message.message);
            let decoded =
                crate::agent_wire::AgentClientMessage::decode(message.message.as_slice()).ok();
            let exec = decoded.as_ref().and_then(|client| match &client.message {
                Some(crate::agent_wire::agent_client_message::Message::ExecClientMessage(exec)) => {
                    Some(exec.clone())
                }
                _ => None,
            });
            let kv = decoded.as_ref().and_then(|client| match &client.message {
                Some(crate::agent_wire::agent_client_message::Message::KvClientMessage(kv)) => {
                    Some(kv.clone())
                }
                _ => None,
            });
            let interaction = decoded.as_ref().and_then(|client| match &client.message {
                Some(crate::agent_wire::agent_client_message::Message::InteractionResponse(
                    resp,
                )) => Some(resp.clone()),
                _ => None,
            });
            ClientWs::AgentMessage {
                run_id: message.run_id,
                run,
                exec,
                kv,
                interaction,
            }
        }
        Some(agent_ws_client_frame::Frame::HalfClose(close)) => ClientWs::HalfClose(close.run_id),
        Some(agent_ws_client_frame::Frame::CancelRun(cancel)) => ClientWs::Cancel(cancel.run_id),
        None => ClientWs::Other,
    }
}

fn server_frame(frame: agent_ws_server_frame::Frame) -> Vec<u8> {
    AgentWsServerFrame { frame: Some(frame) }.encode_to_vec()
}

pub fn encode_run_accepted(identity: &AgentWsRunIdentity) -> Vec<u8> {
    server_frame(agent_ws_server_frame::Frame::RunAccepted(
        AgentWsRunAccepted {
            identity: Some(identity.clone()),
            headers: Vec::new(),
        },
    ))
}

pub fn encode_server_payload(run_id: &str, sequence: u64, payload: Vec<u8>) -> Vec<u8> {
    server_frame(agent_ws_server_frame::Frame::ServerMessage(
        AgentWsServerMessage {
            run_id: run_id.to_owned(),
            sequence,
            message: payload,
        },
    ))
}

pub fn encode_run_end(identity: &AgentWsRunIdentity, error: Option<(&str, i32)>) -> Vec<u8> {
    server_frame(agent_ws_server_frame::Frame::RunEnd(AgentWsRunEnd {
        identity: Some(identity.clone()),
        trailers: Vec::new(),
        error: error.map(|(message, code)| AgentWsConnectError {
            code,
            raw_message: message.to_owned(),
        }),
    }))
}

pub fn encode_local_ok(identity: &AgentWsRunIdentity, thinking: &str, text: &str) -> Vec<Vec<u8>> {
    let mut out = vec![encode_run_accepted(identity)];
    let mut seq = 0_u64;
    if !thinking.is_empty() {
        out.push(encode_server_payload(
            &identity.run_id,
            seq,
            agent_wire::raw_thinking_delta(thinking),
        ));
        seq += 1;
    }
    if !text.is_empty() {
        out.push(encode_server_payload(
            &identity.run_id,
            seq,
            agent_wire::raw_text_delta(text),
        ));
        seq += 1;
    }
    out.push(encode_server_payload(
        &identity.run_id,
        seq,
        agent_wire::raw_turn_ended(),
    ));
    out.push(encode_run_end(identity, None));
    out
}

pub fn encode_local_err(identity: &AgentWsRunIdentity, message: &str) -> Vec<Vec<u8>> {
    vec![
        encode_run_accepted(identity),
        encode_run_end(identity, Some((message, 14))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_wire::{
        agent_client_message, conversation_action, AgentClientMessage, AgentRunRequest,
        ConversationAction, ModelParameterValue, RequestedModel, UserMessage, UserMessageAction,
    };

    fn run_payload(model: &str, text: &str) -> Vec<u8> {
        AgentClientMessage {
            message: Some(agent_client_message::Message::RunRequest(AgentRunRequest {
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
                    parameters: vec![ModelParameterValue {
                        id: "effort".into(),
                        value: "xhigh".into(),
                    }],
                }),
                ..Default::default()
            })),
        }
        .encode_to_vec()
    }

    #[test]
    fn ws_client_message_keeps_ascii_gb() {
        let inner = run_payload("gb-p/xai-xAI/grok-4.6", "hi");
        let bytes = AgentWsClientFrame {
            frame: Some(agent_ws_client_frame::Frame::ClientMessage(
                AgentWsClientMessage {
                    run_id: "run-1".into(),
                    sequence: 0,
                    message: inner,
                },
            )),
        }
        .encode_to_vec();
        assert!(
            bytes.windows(3).any(|window| window == b"gb-"),
            "WS inner protobuf must show gb- unlike hex Bidi"
        );
        match decode_client_ws(&bytes) {
            ClientWs::AgentMessage { run_id, run, exec, .. } => {
                assert_eq!(run_id, "run-1");
                assert!(exec.is_none());
                let run = run.expect("run");
                assert_eq!(run.model_id, "gb-p/xai-xAI/grok-4.6");
                assert_eq!(run.user_text, "hi");
                assert_eq!(run.effort.as_deref(), Some("xhigh"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn local_ok_frames_are_server_proto() {
        let identity = AgentWsRunIdentity {
            run_id: "run-1".into(),
            request_id: "req-1".into(),
        };
        let frames = encode_local_ok(&identity, "think", "hello");
        assert!(frames.len() >= 3);
        let accepted = AgentWsServerFrame::decode(frames[0].as_slice()).unwrap();
        assert!(matches!(
            accepted.frame,
            Some(agent_ws_server_frame::Frame::RunAccepted(_))
        ));
        let ended = AgentWsServerFrame::decode(frames.last().unwrap().as_slice()).unwrap();
        assert!(matches!(
            ended.frame,
            Some(agent_ws_server_frame::Frame::RunEnd(_))
        ));
    }
}
