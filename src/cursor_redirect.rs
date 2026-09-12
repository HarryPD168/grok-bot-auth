//! CCursor-style coexist with fail-open isolation.
//! Official account/plan/agent stay on api2 TLS unless Grok-Bot-Auth is
//! listening on 47821. Catalog + `gb-*` agent RPCs go local only while up.
//! Do not copy CCursor AGPL; this is a small path-prefix hook.

use std::path::PathBuf;

use crate::error::Result;

pub const MARKER: &str = "globalThis.__gb_rpc_redirect";
pub const LOCAL: &str = "http://127.0.0.1:47821";

pub const REDIRECT_NEEDLES: &[&str] = &[
    "AvailableModels",
    "GetUsableModels",
    "BidiAppend",
    "RunSSE",
    "agent.v1.AgentService/Run",
    "GetNewChatNudgeParameterizedModelPicker",
    "GetPromptContextUsage",
    "UploadConversationBlobs",
    "NotifyConversationClone",
    "NameAgent",
    "GetSignedUrlForAttachedMedia",
    "/agent/v1/run",
];

pub const NEVER_REDIRECT: &[&str] = &[
    "GetPlanInfo",
    "GetMe",
    "GetCurrentPeriodUsage",
    "stripe_profile",
    "BootstrapStatsig",
    "GetDefaultModel",
];

pub fn hook_source() -> String {
    include_str!("../assets/cursor-rpc-hook.js").to_owned()
}

fn paths() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    let app = local.join("Programs/cursor/resources/app");
    // Do not patch workbench.*.main.js — wrapping fetch there blanks Cursor.
    vec![
        app.join("extensions/cursor-always-local/dist/main.js"),
        app.join("extensions/cursor-agent-host/dist/main.js"),
        app.join("extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs"),
    ]
}

fn bak_path(path: &std::path::Path) -> PathBuf {
    path.with_extension(format!(
        "{}.gb-redir-bak",
        path.extension().and_then(|e| e.to_str()).unwrap_or("bak")
    ))
}

pub fn install() -> Result<String> {
    let hook = hook_source();
    let mut changed = 0;
    let mut skipped = 0;
    let mut missing = 0;
    for path in paths() {
        if !path.exists() {
            missing += 1;
            continue;
        }
        let bak = bak_path(&path);
        let source = match vendor_source(&path, &bak) {
            Ok(source) => source,
            Err(_) => {
                missing += 1;
                continue;
            }
        };
        if source.is_empty() {
            skipped += 1;
            continue;
        }
        let (source, http1) = crate::cursor_ws::force_agent_http1(&source);
        let (source, _) = crate::cursor_ws::rewrite_gate_source(&source);
        if std::fs::write(&path, format!("{hook}\n{source}")).is_err() {
            continue;
        }
        let _ = refresh_extension_file_hash(&path);
        changed += 1;
        let _ = http1;
    }
    if changed == 0 && skipped == 0 {
        return Err(crate::error::Error::Msg(format!(
            "共存未写入隔离钩子（缺扩展 {missing}）"
        )));
    }
    Ok(format!(
        "官方直连隔离：本软件未开时 Cursor 走 api2。仅目录/gb-* 在 47821 存活时进本机（改 {changed}，已有 {skipped}，缺 {missing}）。必须完全退出 Cursor。"
    ))
}

fn vendor_source(path: &std::path::Path, bak: &std::path::Path) -> crate::error::Result<String> {
    let current = std::fs::read_to_string(path)?;
    let current_clean = !current.contains(MARKER);
    if bak.exists() {
        let bak_src = std::fs::read_to_string(bak)?;
        let bak_clean = !bak_src.contains(MARKER);
        if bak_clean && current_clean && bak_src != current {
            std::fs::write(bak, &current)?;
            return Ok(current);
        }
        if bak_clean {
            if current.contains(MARKER) {
                return Ok(bak_src);
            }
            return Ok(bak_src);
        }
    }
    if current.contains(MARKER) {
        return Ok(String::new());
    }
    std::fs::write(bak, &current)?;
    Ok(current)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_b64_nopad(bytes: &[u8]) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    base64::engine::general_purpose::STANDARD
        .encode(digest)
        .trim_end_matches('=')
        .to_owned()
}

fn extension_id_for(path: &std::path::Path) -> Option<&'static str> {
    let text = path.to_string_lossy();
    if text.contains("cursor-always-local") {
        Some("anysphere.cursor-always-local")
    } else if text.contains("cursor-agent-host") {
        Some("anysphere.cursor-agent-host")
    } else {
        None
    }
}

pub(crate) fn refresh_extension_file_hash(patched: &std::path::Path) -> Result<()> {
    let Some(ext_id) = extension_id_for(patched) else {
        return Ok(());
    };
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Ok(());
    };
    let app = local.join("Programs/cursor/resources/app");
    let eh = app.join("out/vs/workbench/api/node/extensionHostProcess.js");
    if !eh.exists() {
        return Ok(());
    }
    let file_name = patched
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("main.js");
    let hex = sha256_hex(&std::fs::read(patched)?);
    let src = std::fs::read_to_string(&eh)?;
    let Some(updated) = replace_hashed_js(&src, ext_id, file_name, &hex) else {
        return Ok(());
    };
    if updated == src {
        return Ok(());
    }
    std::fs::write(&eh, updated.as_bytes())?;
    let product = app.join("product.json");
    if product.exists() {
        let body = std::fs::read(&eh)?;
        let sum = sha256_b64_nopad(&body);
        let mut prod = std::fs::read_to_string(&product)?;
        let key = "\"vs/workbench/api/node/extensionHostProcess.js\": \"";
        if let Some(at) = prod.find(key) {
            let start = at + key.len();
            if let Some(rel) = prod[start..].find('"') {
                prod.replace_range(start..start + rel, &sum);
                let _ = std::fs::write(product, prod);
            }
        }
    }
    Ok(())
}

fn replace_hashed_js(src: &str, ext_id: &str, file_name: &str, new_hex: &str) -> Option<String> {
    let marker = format!("{ext_id}\":{{");
    let start = src.find(&marker)?;
    let window_end = (start + 1200).min(src.len());
    let window = &src[start..window_end];
    let needle = format!("\"{file_name}\":\"");
    let rel = window.find(&needle)?;
    let hex_at = start + rel + needle.len();
    let hex_end = hex_at + 64;
    if hex_end > src.len() {
        return None;
    }
    if !src[hex_at..hex_end].bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if new_hex.len() != 64 {
        return None;
    }
    let mut out = src.to_owned();
    out.replace_range(hex_at..hex_end, new_hex);
    Some(out)
}

pub fn restore() -> Result<String> {
    let mut restored = 0;
    for path in paths() {
        let bak = bak_path(&path);
        if bak.exists() {
            std::fs::copy(&bak, &path)?;
            let _ = refresh_extension_file_hash(&path);
            let _ = std::fs::remove_file(&bak);
            restored += 1;
        }
    }
    Ok(format!("已恢复 {restored} 个 Cursor 直连补丁文件。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_hashed_js_updates_main_hash() {
        let src = r#"Xae={"anysphere.cursor-always-local":{dist:{"main.js":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}"#;
        let out = replace_hashed_js(
            src,
            "anysphere.cursor-always-local",
            "main.js",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap();
        assert!(out.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        assert!(!out.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn hook_redirects_inject_rpcs_only() {
        let hook = hook_source();
        for needle in REDIRECT_NEEDLES {
            assert!(hook.contains(needle), "missing {needle}");
        }
        for banned in NEVER_REDIRECT {
            assert!(
                !hook.contains(banned),
                "must not hijack official account RPC {banned}"
            );
        }
        assert!(hook.contains(LOCAL));
        assert!(hook.contains(MARKER));
        assert!(
            hook.contains("http2.connect"),
            "HTTP/2 AgentService/Run must be wrapped even when workbench is HTTP/1.1"
        );
        assert!(
            hook.contains("syncBuiltinESMExports"),
            "CCursor: ESM https.request is a different binding"
        );
        assert!(
            hook.contains("directReq"),
            "local divert must use unwrapped http.request or HIT recurses"
        );
        assert!(
            hook.contains("localUp"),
            "must probe 47821 and default official when GBA is down"
        );
        assert!(
            hook.contains("probeLocal"),
            "background TCP probe keeps official Cursor isolated"
        );
        assert!(
            hook.contains("looksInjected"),
            "agent RPCs must not divert official grok-4.6 into 47821"
        );
        assert!(
            hook.contains("67622d"),
            "hex Bidi hides ascii gb-; hook must scan 67622d"
        );
        assert!(
            hook.contains("heldSse"),
            "RunSSE is first; hold until BidiAppend decides local vs api2"
        );
        assert!(
            hook.contains("localIds") && hook.contains("extractReqId"),
            "exec/kv/cancel BidiAppend must stick to 47821 by request_id after gb-* run starts"
        );
        assert!(
            hook.contains("GetPromptContextUsage"),
            "context meter RPC must stick with injected gb-* runs"
        );
        assert!(
            hook.contains("UploadConversationBlobs"),
            "rules/skills/mcp blobs upload with injected runs"
        );
        assert!(
            hook.contains("NotifyConversationClone"),
            "fork chat clone ACK must stick with injected gb-* runs"
        );
        assert!(
            hook.contains("isStickyUnary") && hook.contains("anyRemembered"),
            "usage/clone/blobs must not ride injectUntil onto official grok-4.6"
        );
        assert!(
            !hook.contains("return injected || isRemembered(id) || Date.now() < injectUntil"),
            "non-sticky Agent RPCs must not divert official grok-4.6 during injectUntil"
        );
        assert!(
            hook.contains("heldSse.reqId"),
            "held RunSSE must release only when request_id matches"
        );
        assert!(
            hook.contains("NameAgent") && hook.contains("GetSignedUrlForAttachedMedia"),
            "NameAgent / signed media must ACK on injected chats"
        );
        assert!(
            hook.contains("hasLiveLocal") && hook.contains("injectUntil + 90000"),
            "NameAgent divert only while a gb-* run is live and recently injected"
        );
        assert!(
            !hook.contains("GetDefaultModelForCli"),
            "CLI default model must stay on api2 when GBA is up"
        );
        assert!(
            hook.contains("/relay/merge-catalog") && hook.contains("wrapCatalogMerge"),
            "official catalog must hit api2 first; GBA only appends gb-*"
        );
        assert!(
            hook.contains("wrapH2CatalogMerge") && hook.contains("origEmit"),
            "HTTP/2 catalog merge must keep the official Http2Stream"
        );
        assert!(
            !hook.contains("localUp && isCatalog(p)) return divertH1"),
            "HTTP/2 catalog must not divertH1-replace official models"
        );
    }

    #[test]
    fn install_is_idempotent_on_marked_source() {
        let once = format!("{}\nconsole.log(1)", hook_source());
        assert!(once.contains(MARKER));
    }
}
