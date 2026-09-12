//! Disable Cursor 3.20 Agent WebSocket the same way CCursor does: rename the
//! Statsig gate literal in the local JS bundles. MITM on BootstrapStatsig is
//! not enough — `gateAllowsWebSocketCached` is set at process start, and live
//! logs still show `/agent/v1/run` WS upgrades after a successful Statsig patch.

use std::path::PathBuf;

use crate::error::{Error, Result};

const GATE: &str = "\"nal_websocket_client\"";
const OFF: &str = "\"__gb_off_nal_websocket_client\"";

/// Keep Auth from waiting on agent-exec (Cursor 3.20 ~10s deadlock).
/// Cursor updates overwrite this; coexist re-applies it. Do not revert on park.
pub fn ensure_always_local_early_auth() -> Result<String> {
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Ok("no LOCALAPPDATA".into());
    };
    let path =
        local.join("Programs/cursor/resources/app/extensions/cursor-always-local/package.json");
    if !path.exists() {
        return Ok("always-local package.json missing".into());
    }
    let source = std::fs::read_to_string(&path)?;
    let needle = r#""activationEvents":["onStartupFinished""#;
    let patched = r#""activationEvents":["*","onStartupFinished""#;
    if source.contains(r#""activationEvents":["*""#) {
        return Ok("always-local 已提前激活 Auth".into());
    }
    if !source.contains(needle) {
        return Ok("always-local activationEvents 不是预期格式，未改".into());
    }
    std::fs::write(&path, source.replacen(needle, patched, 1))?;
    Ok("always-local 已提前激活 Auth（Cursor 升级后需再应用）".into())
}

fn bundle_paths() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    let root = local.join("Programs/cursor/resources/app/extensions");
    vec![
        root.join("cursor-always-local/dist/main.js"),
        root.join("cursor-agent-host/dist/main.js"),
        root.join("cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs"),
    ]
}

/// Agent `Run` is bidi and Cursor still builds an HTTP/2 transport even when
/// the user set `cursor.general.disableHttp2`. Force those two factories to
/// HTTP/1.1 so the Node `http.request` hook can see `gb-*`.
/// Do **not** write the global `cursor.general.disableHttp2` setting — that
/// hits Plan & Usage / Dashboard, not just Agent.
const AGENT_HTTP1_PATCHES: &[(&str, &str)] = &[
    (
        "useHttp2:!r,maybeUseCppSpoofToken:!0,bidiTransport:this._bidiTransport,pingConfig:yield(0,N.getHttp2PingConfig)(this.host,N.Http2TransportCallSite.AGENT)",
        "useHttp2:!1,maybeUseCppSpoofToken:!0,bidiTransport:this._bidiTransport,pingConfig:yield(0,N.getHttp2PingConfig)(this.host,N.Http2TransportCallSite.AGENT)",
    ),
    (
        "agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!r,",
        "agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!1,",
    ),
    (
        "agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!0,",
        "agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!1,",
    ),
];

pub fn force_agent_http1(source: &str) -> (String, usize) {
    let mut out = source.to_owned();
    let mut count = 0;
    for (from, to) in AGENT_HTTP1_PATCHES {
        if out.contains(from) {
            out = out.replacen(from, to, 1);
            count += 1;
        }
    }
    (out, count)
}

pub fn unforce_agent_http1(source: &str) -> (String, usize) {
    let mut out = source.to_owned();
    let mut count = 0;
    for (from, to) in AGENT_HTTP1_PATCHES {
        if out.contains(to) {
            out = out.replacen(to, from, 1);
            count += 1;
        }
    }
    (out, count)
}

pub fn restore_gate_literal(source: &str) -> String {
    source.replace(OFF, GATE)
}

pub fn rewrite_gate_source(source: &str) -> (String, usize) {
    let mut count = 0;
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find(GATE) {
        let after = at + GATE.len();
        if rest[after..].starts_with('_') {
            out.push_str(&rest[..after]);
            rest = &rest[after..];
            continue;
        }
        out.push_str(&rest[..at]);
        out.push_str(OFF);
        rest = &rest[after..];
        count += 1;
    }
    out.push_str(rest);
    (out, count)
}

pub fn disable_agent_websocket() -> Result<String> {
    let mut changed = 0;
    let mut skipped = 0;
    let mut missing = 0;
    for path in bundle_paths() {
        if !path.exists() {
            missing += 1;
            continue;
        }
        let source = std::fs::read_to_string(&path)?;
        if source.contains(OFF) {
            skipped += 1;
            continue;
        }
        let (patched, count) = rewrite_gate_source(&source);
        if count == 0 {
            return Err(Error::Msg(format!(
                "no {} gate in {}",
                GATE,
                path.display()
            )));
        }
        let bak = path.with_extension("js.gb-bak");
        let bak = if path.extension().and_then(|e| e.to_str()) == Some("cjs") {
            path.with_extension("cjs.gb-bak")
        } else {
            bak
        };
        if !bak.exists() {
            std::fs::write(&bak, &source)?;
        }
        std::fs::write(&path, patched.as_bytes())?;
        changed += 1;
    }
    Ok(format!(
        "Agent WebSocket 闸已关掉（改 {changed} 个 Cursor 文件，已是关闭 {skipped}，缺失 {missing}）。必须完全退出 Cursor。"
    ))
}

pub fn restore_agent_websocket() -> Result<String> {
    let mut restored = 0;
    for path in bundle_paths() {
        let bak = if path.extension().and_then(|e| e.to_str()) == Some("cjs") {
            path.with_extension("cjs.gb-bak")
        } else {
            path.with_extension("js.gb-bak")
        };
        if bak.exists() {
            std::fs::copy(&bak, &path)?;
            let _ = crate::cursor_redirect::refresh_extension_file_hash(&path);
            restored += 1;
        }
    }
    Ok(format!(
        "已从备份恢复 {restored} 个 Cursor 文件。完全退出 Cursor 后官方 WebSocket 会回来。"
    ))
}

/// After park/restore, bak may still contain Agent HTTP/1 leftovers.
/// Only scrub when the isolation hook is gone so a live coexist install is not undone.
pub fn scrub_transport_leftovers() -> Result<String> {
    let mut scrubbed = 0;
    for path in bundle_paths() {
        if !path.exists() {
            continue;
        }
        let source = std::fs::read_to_string(&path)?;
        if source.contains(crate::cursor_redirect::MARKER) {
            continue;
        }
        let (undone, n_http1) = unforce_agent_http1(&source);
        let restored = restore_gate_literal(&undone);
        if restored != source {
            std::fs::write(&path, restored.as_bytes())?;
            let _ = crate::cursor_redirect::refresh_extension_file_hash(&path);
            scrubbed += 1;
            let _ = n_http1;
        }
    }
    Ok(format!(
        "已清除 {scrubbed} 个文件里的 Agent HTTP/1 / WebSocket 闸残留"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_gate_but_not_pool_config() {
        let src = r#"t.AGENT_WEBSOCKET_FEATURE_GATE="nal_websocket_client",t.AGENT_WEBSOCKET_POOL_CONFIG="nal_websocket_client_pool""#;
        let (out, n) = rewrite_gate_source(src);
        assert_eq!(n, 1);
        assert!(out.contains(OFF));
        assert!(out.contains("\"nal_websocket_client_pool\""));
        assert!(!out.contains("GATE=\"nal_websocket_client\""));
    }

    #[test]
    fn already_off_is_stable() {
        let src = r#"GATE="__gb_off_nal_websocket_client""#;
        let (out, n) = rewrite_gate_source(src);
        assert_eq!(n, 0);
        assert_eq!(out, src);
    }

    #[test]
    fn force_agent_http1_only_touches_agent_factories() {
        let src = concat!(
            "useHttp2:!r,maybeUseCppSpoofToken:!0,bidiTransport:this._bidiTransport,pingConfig:yield(0,N.getHttp2PingConfig)(this.host,N.Http2TransportCallSite.AGENT)",
            "useHttp2:!r&&i.includes(\".cursor.sh\")",
            "agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!r,pingConfig:1}"
        );
        let (out, n) = force_agent_http1(src);
        assert_eq!(n, 2);
        assert!(
            out.contains("Http2TransportCallSite.AGENT")
                && out.contains("useHttp2:!1,maybeUseCppSpoofToken:!0")
        );
        assert!(out.contains("useHttp2:!r&&i.includes"));
        assert!(out.contains("agenticComposerTransport:this.bidiTransportFactory.createTransport({baseUrl:s,useHttp2:!1,"));
        let (back, n2) = unforce_agent_http1(&out);
        assert_eq!(n2, 2);
        assert_eq!(back, src);
    }
}
