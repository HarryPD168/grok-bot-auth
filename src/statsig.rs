//! Patch Cursor `BootstrapStatsig` so Agent chat uses HTTP Bidi+RunSSE.
//!
//! Cursor 3.20's `nal_websocket_client` gate sends composer over
//! `wss://api2.cursor.sh/agent/v1/run`. The model id is in WS frames after an
//! empty HTTP upgrade, so MITM never sees `gb-*`. Official WS then returns
//! `ERROR_BAD_MODEL_NAME` ("The model you chose is not available").
//! CCursor disables that gate in the JS bundle. We flip it in Statsig instead.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use prost::Message;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::connect::{frame_then_rest, split_first_frame};

pub const AGENT_WS_GATE: &str = "nal_websocket_client";
pub const AGENT_WS_POOL: &str = "nal_websocket_client_pool";

#[derive(Clone, PartialEq, Message)]
struct BootstrapStatsigResponse {
    #[prost(string, tag = "1")]
    config: String,
    #[prost(uint64, tag = "2")]
    generated_at_ms: u64,
}

pub fn patch_bootstrap_body(body: &[u8]) -> Option<Vec<u8>> {
    if let Some((_flags, payload, rest)) = split_first_frame(body) {
        let patched = patch_proto(payload)?;
        return Some(frame_then_rest(&patched, rest));
    }
    patch_proto(body)
}

fn patch_proto(payload: &[u8]) -> Option<Vec<u8>> {
    let mut message = BootstrapStatsigResponse::decode(payload).ok()?;
    let mut config: Value = serde_json::from_str(&message.config).ok()?;
    if !config.is_object() {
        return None;
    }
    disable_agent_websocket(&mut config);
    message.config = serde_json::to_string(&config).ok()?;
    Some(message.encode_to_vec())
}

pub fn disable_agent_websocket(config: &mut Value) {
    set_gate(config, AGENT_WS_GATE, false);
    set_pool_fallback(config);
}

fn set_gate(config: &mut Value, name: &str, enabled: bool) {
    let hashed = statsig_key(config, name);
    let Some(root) = config.as_object_mut() else {
        return;
    };
    let gates = root
        .entry("feature_gates")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(gates) = gates.as_object_mut() else {
        return;
    };
    let mut keys: Vec<String> = gates
        .iter()
        .filter(|(key, value)| {
            *key == name
                || *key == hashed.as_str()
                || value.get("name").and_then(Value::as_str) == Some(name)
                || value.get("name").and_then(Value::as_str) == Some(hashed.as_str())
        })
        .map(|(key, _)| key.clone())
        .collect();
    if !keys.iter().any(|key| key == &hashed) {
        keys.push(hashed.clone());
    }
    for key in keys {
        match gates.get_mut(&key) {
            Some(existing) => {
                if let Some(object) = existing.as_object_mut() {
                    object.insert("value".into(), json!(enabled));
                    object
                        .entry("name")
                        .or_insert_with(|| json!(key.clone()));
                }
            }
            None => {
                gates.insert(key.clone(), gate_value(&key, enabled));
            }
        }
    }
}

fn set_pool_fallback(config: &mut Value) {
    let hashed = statsig_key(config, AGENT_WS_POOL);
    let Some(root) = config.as_object_mut() else {
        return;
    };
    let configs = root
        .entry("dynamic_configs")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(configs) = configs.as_object_mut() else {
        return;
    };
    let mut keys: Vec<String> = configs
        .iter()
        .filter(|(key, value)| {
            *key == AGENT_WS_POOL
                || *key == hashed.as_str()
                || value.get("name").and_then(Value::as_str) == Some(AGENT_WS_POOL)
                || value.get("name").and_then(Value::as_str) == Some(hashed.as_str())
        })
        .map(|(key, _)| key.clone())
        .collect();
    if !keys.iter().any(|key| key == &hashed) {
        keys.push(hashed.clone());
    }
    for key in keys {
        let entry = configs.entry(key.clone()).or_insert_with(|| {
            json!({
                "name": key,
                "value": {},
                "rule_id": "local_enabled",
                "ruleID": "local_enabled"
            })
        });
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        let value = object
            .entry("value")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(value) = value.as_object_mut() {
            value.insert("fallbackOnColdPool".into(), json!(true));
        }
    }
}

fn statsig_key(config: &Value, name: &str) -> String {
    match config.get("hash_used").and_then(Value::as_str) {
        Some("djb2") => djb2(name),
        Some("sha256") => STANDARD.encode(Sha256::digest(name.as_bytes())),
        _ => name.to_owned(),
    }
}

fn djb2(value: &str) -> String {
    value
        .encode_utf16()
        .fold(0_u32, |hash, character| {
            hash.wrapping_mul(31).wrapping_add(u32::from(character))
        })
        .to_string()
}

fn gate_value(name: &str, enabled: bool) -> Value {
    json!({
        "name": name,
        "value": enabled,
        "rule_id": "local_enabled",
        "ruleID": "local_enabled",
        "group_name": "local_enabled",
        "groupName": "local_enabled",
        "secondary_exposures": [],
        "secondaryExposures": [],
        "undelegated_secondary_exposures": [],
        "undelegatedSecondaryExposures": [],
        "is_device_based": false,
        "isDeviceBased": false,
        "id_type": "userID",
        "idType": "userID"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::encode_connect_frame;

    #[test]
    fn djb2_matches_statsig_js_for_agent_ws_gate() {
        assert_eq!(djb2(AGENT_WS_GATE), "579159465");
        assert_eq!(djb2(AGENT_WS_POOL), "2362453234");
    }

    #[test]
    fn disables_hashed_gate_and_turns_on_http_fallback() {
        let mut config = json!({
            "hash_used": "djb2",
            "feature_gates": {
                "579159465": { "name": "579159465", "value": true }
            },
            "dynamic_configs": {
                "2362453234": {
                    "name": "2362453234",
                    "value": { "maxConnections": 1, "fallbackOnColdPool": false }
                }
            }
        });
        disable_agent_websocket(&mut config);
        assert_eq!(
            config["feature_gates"]["579159465"]["value"],
            json!(false)
        );
        assert_eq!(
            config["dynamic_configs"]["2362453234"]["value"]["fallbackOnColdPool"],
            json!(true)
        );
        assert_eq!(
            config["dynamic_configs"]["2362453234"]["value"]["maxConnections"],
            json!(1)
        );
    }

    #[test]
    fn inserts_raw_gate_when_hash_is_none() {
        let mut config = json!({
            "hash_used": "none",
            "feature_gates": {}
        });
        disable_agent_websocket(&mut config);
        assert_eq!(
            config["feature_gates"][AGENT_WS_GATE]["value"],
            json!(false)
        );
    }

    #[test]
    fn patches_connect_framed_bootstrap() {
        let config = json!({
            "hash_used": "djb2",
            "feature_gates": {
                "579159465": { "name": "579159465", "value": true }
            }
        });
        let message = BootstrapStatsigResponse {
            config: config.to_string(),
            generated_at_ms: 1,
        };
        let mut body = encode_connect_frame(&message.encode_to_vec());
        body.extend_from_slice(&[2, 0, 0, 0, 0]);
        let patched = patch_bootstrap_body(&body).expect("patch");
        let (_flags, payload, rest) = split_first_frame(&patched).expect("frame");
        assert_eq!(rest, [2, 0, 0, 0, 0]);
        let decoded = BootstrapStatsigResponse::decode(payload).unwrap();
        let value: Value = serde_json::from_str(&decoded.config).unwrap();
        assert_eq!(value["feature_gates"]["579159465"]["value"], json!(false));
    }
}
