//! CC Switch-style MCP list. Merge only servers we own into Cursor mcp.json.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};
use crate::store;

pub const OWNED_MARK: &str = "x-grok-bot-auth";
pub const MCP_AUTH_TOOL: &str = "mcp_auth";
pub const MCP_AUTH_DESCRIPTION: &str = "Authenticate an MCP server.";

const ALLOWED_STDIO_COMMANDS: &[&str] = &[
    "npx",
    "npx.cmd",
    "node",
    "node.exe",
    "python",
    "python.exe",
    "python3",
    "python3.exe",
    "uvx",
    "uvx.exe",
    "bun",
    "bun.exe",
    "deno",
    "deno.exe",
];

pub fn command_allowed(command: &str) -> bool {
    let name = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);
    ALLOWED_STDIO_COMMANDS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    #[default]
    Stdio,
    Http,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub transport: McpTransport,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub enabled: bool,
}

impl McpServer {
    pub fn preset(id: &str, name: &str, pkg: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport: McpTransport::Stdio,
            command: "npx".into(),
            args: vec!["-y".into(), pkg.into()],
            url: String::new(),
            enabled: false,
        }
    }
}

pub fn presets() -> Vec<McpServer> {
    vec![
        McpServer::preset("fetch", "fetch", "mcp-server-fetch"),
        McpServer::preset("time", "time", "@modelcontextprotocol/server-time"),
        McpServer::preset("memory", "memory", "@modelcontextprotocol/server-memory"),
        McpServer::preset(
            "sequential-thinking",
            "sequential-thinking",
            "@modelcontextprotocol/server-sequential-thinking",
        ),
        McpServer::preset("context7", "context7", "@upstash/context7-mcp"),
    ]
}

fn preset_tools(id: &str) -> Vec<Value> {
    match id {
        "fetch" => vec![json!({
            "name": "fetch",
            "description": "Fetch a URL and return the contents.",
            "inputSchema": {
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "max_length": { "type": "number" }
                }
            }
        })],
        "time" => vec![json!({
            "name": "get_current_time",
            "description": "Get the current time.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timezone": { "type": "string" }
                }
            }
        })],
        "memory" => vec![
            json!({
                "name": "create_entities",
                "description": "Create entities in the knowledge graph.",
                "inputSchema": {
                    "type": "object",
                    "required": ["entities"],
                    "properties": {
                        "entities": { "type": "array", "items": { "type": "object" } }
                    }
                }
            }),
            json!({
                "name": "search_nodes",
                "description": "Search knowledge graph nodes.",
                "inputSchema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" }
                    }
                }
            }),
        ],
        "sequential-thinking" => vec![json!({
            "name": "sequentialthinking",
            "description": "A detailed thinking step.",
            "inputSchema": {
                "type": "object",
                "required": ["thought", "nextThoughtNeeded", "thoughtNumber", "totalThoughts"],
                "properties": {
                    "thought": { "type": "string" },
                    "nextThoughtNeeded": { "type": "boolean" },
                    "thoughtNumber": { "type": "integer" },
                    "totalThoughts": { "type": "integer" }
                }
            }
        })],
        "context7" => vec![json!({
            "name": "resolve-library-id",
            "description": "Resolve a library name to a Context7 library ID.",
            "inputSchema": {
                "type": "object",
                "required": ["libraryName"],
                "properties": {
                    "libraryName": { "type": "string" }
                }
            }
        })],
        _ => Vec::new(),
    }
}

/// CCursor GetDynamicTools payload: namespaces with full inputSchema JSON.
pub fn catalog_json(namespace: Option<&str>, tool_name: Option<&str>, pattern: Option<&str>) -> String {
    let pat = pattern.map(str::trim).filter(|p| !p.is_empty());
    let mut namespaces = Vec::new();
    for server in presets() {
        if let Some(ns) = namespace.map(str::trim).filter(|n| !n.is_empty()) {
            if server.id != ns && server.name != ns {
                continue;
            }
        }
        let mut tools = preset_tools(&server.id);
        if let Some(name) = tool_name.map(str::trim).filter(|n| !n.is_empty()) {
            tools.retain(|tool| tool["name"].as_str() == Some(name));
        } else if let Some(re) = pat {
            let server_hit = pat_matches(Some(re), &server.id) || pat_matches(Some(re), &server.name);
            if !server_hit {
                tools.retain(|tool| {
                    tool["name"]
                        .as_str()
                        .is_some_and(|n| n.to_ascii_lowercase().contains(&re.to_ascii_lowercase()))
                });
            }
        }
        if tools.is_empty()
            && (tool_name.is_some_and(|n| !n.trim().is_empty())
                || pat.is_some_and(|re| {
                    !pat_matches(Some(re), &server.id) && !pat_matches(Some(re), &server.name)
                }))
        {
            continue;
        }
        namespaces.push(json!({
            "name": server.id,
            "source": "preset",
            "status": if server.enabled { "enabled" } else { "available" },
            "tools": tools
        }));
    }
    json!({
        "mode": if namespace.is_some() { "namespace" } else { "catalog" },
        "namespaces": namespaces
    })
    .to_string()
}

pub fn catalog_json_auth(
    namespace: Option<&str>,
    tool_name: Option<&str>,
    pattern: Option<&str>,
    _supports_mcp_auth: bool,
) -> String {
    catalog_json(namespace, tool_name, pattern)
}

fn pat_matches(pat: Option<&str>, text: &str) -> bool {
    let Some(pat) = pat else {
        return true;
    };
    text.to_ascii_lowercase()
        .contains(&pat.to_ascii_lowercase())
}

pub fn cursor_mcp_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("mcp.json")
}

pub fn owned_ids_path() -> PathBuf {
    store::data_dir().join("mcp-owned.json")
}

pub fn merge_into_cursor(servers: &[McpServer]) -> Result<()> {
    let path = cursor_mcp_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut root: Value = match std::fs::read_to_string(&path) {
        Ok(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw).unwrap_or(json!({})),
        _ => json!({}),
    };
    if !root.is_object() {
        root = json!({});
    }
    let map = root.as_object_mut().unwrap();
    let servers_obj = map
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| Error::Msg("mcp.json mcpServers is not an object".into()))?;
    let mut owned: BTreeMap<String, bool> = load_owned();
    for id in owned.keys().cloned().collect::<Vec<_>>() {
        if !servers.iter().any(|s| s.id == id) || servers.iter().any(|s| s.id == id && !s.enabled) {
            servers_obj.remove(&id);
            owned.remove(&id);
        }
    }
    for server in servers.iter().filter(|s| s.enabled) {
        let mut entry = Map::new();
        match server.transport {
            McpTransport::Stdio => {
                entry.insert("command".into(), json!(server.command));
                entry.insert("args".into(), json!(server.args));
            }
            McpTransport::Http | McpTransport::Sse => {
                entry.insert("url".into(), json!(server.url));
            }
        }
        entry.insert(OWNED_MARK.into(), json!(true));
        servers_obj.insert(server.id.clone(), Value::Object(entry));
        owned.insert(server.id.clone(), true);
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&root)?)?;
    save_owned(&owned)?;
    Ok(())
}

fn load_owned() -> BTreeMap<String, bool> {
    std::fs::read_to_string(owned_ids_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_owned(owned: &BTreeMap<String, bool>) -> Result<()> {
    std::fs::create_dir_all(store::data_dir())?;
    std::fs::write(owned_ids_path(), serde_json::to_vec_pretty(owned)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_json_includes_input_schema() {
        let raw = catalog_json(None, None, None);
        assert!(raw.contains("inputSchema"), "{raw}");
        assert!(raw.contains("create_entities"), "{raw}");
        let one = catalog_json(Some("memory"), Some("create_entities"), None);
        assert!(one.contains("create_entities"));
        assert!(!one.contains("fetch"));
    }

    #[test]
    fn presets_match_cc_switch_common_set() {
        let ids: Vec<_> = presets().into_iter().map(|s| s.id).collect();
        assert!(ids.contains(&"memory".into()));
        assert!(ids.contains(&"context7".into()));
        assert!(ids.contains(&"sequential-thinking".into()));
    }

    #[test]
    fn merge_upserts_only_enabled_and_marks_owned() {
        let dir = std::env::temp_dir().join(format!("gba-mcp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mcp.json");
        std::fs::write(&path, r#"{"mcpServers":{"keep-me":{"command":"echo"}}}"#).unwrap();
        let mut servers = presets();
        servers[2].enabled = true; // memory
        let mut root: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let map = root["mcpServers"].as_object_mut().unwrap();
        map.insert(
            "memory".into(),
            json!({"command":"npx","args":["-y","@modelcontextprotocol/server-memory"], OWNED_MARK: true}),
        );
        assert!(map.contains_key("keep-me"));
        assert!(map["memory"][OWNED_MARK].as_bool().unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }
}
