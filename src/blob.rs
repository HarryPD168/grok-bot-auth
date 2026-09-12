//! Cursor KV blob store: sha256(base64(json)) id, memory + disk cache.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::store;

static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
struct MediaEntry {
    mime: String,
    bytes: Vec<u8>,
}

struct MediaStore {
    map: HashMap<String, MediaEntry>,
    order: VecDeque<String>,
    total: usize,
}

static MEDIA: OnceLock<Mutex<MediaStore>> = OnceLock::new();
const MAX_MEDIA_ITEM: usize = 8 * 1024 * 1024;
const MAX_MEDIA_TOTAL: usize = 32 * 1024 * 1024;
const MAX_MEDIA_KEYS: usize = 64;
static LOCAL_CONVERSATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static CLONE_LINEAGE: OnceLock<Mutex<HashMap<String, (String, String)>>> = OnceLock::new();
static CLONE_ORDER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
const MAX_CLONE_LINEAGE: usize = 5_000;

fn cache() -> &'static Mutex<HashMap<String, String>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn media() -> &'static Mutex<MediaStore> {
    MEDIA.get_or_init(|| {
        Mutex::new(MediaStore {
            map: HashMap::new(),
            order: VecDeque::new(),
            total: 0,
        })
    })
}

pub fn put_media(key: &str, mime: &str, bytes: Vec<u8>) -> Result<(), String> {
    if key.is_empty() {
        return Err("empty key".into());
    }
    if bytes.len() > MAX_MEDIA_ITEM {
        return Err("too large".into());
    }
    let mut store = media().lock().map_err(|_| "media lock poisoned")?;
    if let Some(old) = store.map.remove(key) {
        store.total = store.total.saturating_sub(old.bytes.len());
        store.order.retain(|k| k != key);
    }
    while store.total + bytes.len() > MAX_MEDIA_TOTAL || store.map.len() >= MAX_MEDIA_KEYS {
        let Some(oldest) = store.order.pop_front() else {
            break;
        };
        if let Some(entry) = store.map.remove(&oldest) {
            store.total = store.total.saturating_sub(entry.bytes.len());
        }
    }
    store.total += bytes.len();
    store.order.push_back(key.to_owned());
    store.map.insert(
        key.to_owned(),
        MediaEntry {
            mime: mime.to_owned(),
            bytes,
        },
    );
    Ok(())
}

pub fn get_media(key: &str) -> Option<(String, Vec<u8>)> {
    let store = media().lock().ok()?;
    let entry = store.map.get(key)?;
    Some((entry.mime.clone(), entry.bytes.clone()))
}

pub fn has_any_local_conversation() -> bool {
    local_conversations()
        .lock()
        .ok()
        .is_some_and(|set| !set.is_empty())
}

fn local_conversations() -> &'static Mutex<HashSet<String>> {
    LOCAL_CONVERSATIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn clone_lineage() -> &'static Mutex<HashMap<String, (String, String)>> {
    CLONE_LINEAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn clone_order() -> &'static Mutex<VecDeque<String>> {
    CLONE_ORDER.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn remember_conversation(id: &str) {
    if id.is_empty() {
        return;
    }
    if let Ok(mut set) = local_conversations().lock() {
        set.insert(id.to_owned());
    }
}

pub fn is_local_conversation(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    local_conversations()
        .lock()
        .ok()
        .is_some_and(|set| set.contains(id))
}

pub fn register_clone(new_id: &str, source_id: &str, source_request_id: &str) {
    if new_id.is_empty() {
        return;
    }
    if let (Ok(mut map), Ok(mut order)) = (clone_lineage().lock(), clone_order().lock()) {
        if map
            .insert(
                new_id.to_owned(),
                (source_id.to_owned(), source_request_id.to_owned()),
            )
            .is_none()
        {
            order.push_back(new_id.to_owned());
        }
        while map.len() > MAX_CLONE_LINEAGE {
            if let Some(oldest) = order.pop_front() {
                map.remove(&oldest);
            } else {
                break;
            }
        }
    }
    if is_local_conversation(source_id) {
        remember_conversation(new_id);
    }
}

pub fn clone_source(new_id: &str) -> Option<(String, String)> {
    clone_lineage()
        .lock()
        .ok()
        .and_then(|map| map.get(new_id).cloned())
}

fn engine() -> &'static base64::engine::GeneralPurpose {
    &base64::engine::general_purpose::STANDARD
}

/// JSON object → { blobId, blobData } matching CCursor `encodeBlob`.
pub fn encode_json(value: &serde_json::Value) -> (String, String) {
    let json = value.to_string();
    let blob_data = engine().encode(json.as_bytes());
    let blob_id = engine().encode(Sha256::digest(blob_data.as_bytes()));
    put(&blob_id, &blob_data);
    (blob_id, blob_data)
}

pub fn encode_role(role: &str, content: &str) -> (String, String) {
    encode_json(&serde_json::json!({ "role": role, "content": content }))
}

pub fn put_cas(blob_id: &str, blob_data: &str) -> Result<(), String> {
    put(blob_id, blob_data);
    Ok(())
}

pub fn put(blob_id: &str, blob_data: &str) {
    if let Ok(mut map) = cache().lock() {
        map.insert(blob_id.to_owned(), blob_data.to_owned());
    }
    let dir = store::data_dir().join("blobs");
    let _ = std::fs::create_dir_all(&dir);
    let safe: String = blob_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .take(80)
        .collect();
    let _ = std::fs::write(dir.join(format!("{safe}.b64")), blob_data.as_bytes());
}

pub fn get(blob_id: &str) -> Option<String> {
    if let Ok(map) = cache().lock() {
        if let Some(data) = map.get(blob_id) {
            return Some(data.clone());
        }
    }
    let dir = store::data_dir().join("blobs");
    let safe: String = blob_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .take(80)
        .collect();
    let data = std::fs::read_to_string(dir.join(format!("{safe}.b64"))).ok()?;
    put(blob_id, &data);
    Some(data)
}

pub fn decode_json(blob_data: &str) -> Option<serde_json::Value> {
    let bytes = engine().decode(blob_data).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn id_bytes(blob_id: &str) -> Vec<u8> {
    blob_id.as_bytes().to_vec()
}

pub fn id_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn hydrate_turns(blob_ids: &[Vec<u8>]) -> Vec<crate::compact::ChatTurn> {
    let mut turns = Vec::new();
    for id in blob_ids {
        let key = id_from_bytes(id);
        let Some(data) = get(&key) else {
            continue;
        };
        let Some(json) = decode_json(&data) else {
            continue;
        };
        let role = json
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user")
            .to_owned();
        let text = match json.get("content") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => continue,
        };
        if !text.is_empty() {
            turns.push(crate::compact::ChatTurn::new(role, text));
        }
    }
    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_roundtrip_keeps_role_and_content() {
        let (id, data) = encode_role("user", "GROUND_BLOB_ZX");
        assert!(!id.is_empty());
        put(&id, &data);
        let json = decode_json(&get(&id).expect("cached")).expect("json");
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "GROUND_BLOB_ZX");
        let turns = hydrate_turns(&[id_bytes(&id)]);
        assert!(turns.iter().any(|t| t.text.contains("GROUND_BLOB_ZX")));
    }

    #[test]
    fn media_put_get_roundtrip() {
        put_media("gba-test", "image/png", vec![1, 2, 3]).expect("put");
        let (mime, bytes) = get_media("gba-test").expect("media");
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, vec![1, 2, 3]);
        assert!(put_media("gba-huge", "image/png", vec![0; MAX_MEDIA_ITEM + 1]).is_err());
    }
}
