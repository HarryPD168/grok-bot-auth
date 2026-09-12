//! CC Switch-style version check. No silent exe overwrite. No PowerShell.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::error::Result;

pub const CHANGELOG: &str = include_str!("../CHANGELOG.md");

pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn changelog() -> &'static str {
    CHANGELOG
}

#[derive(Debug, Clone, Default)]
pub struct VersionReport {
    pub app: String,
    pub remote: Option<String>,
    pub cursor: Option<String>,
    pub notice: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: Option<String>,
    name: Option<String>,
}

pub fn detect_cursor_version() -> Option<String> {
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    CACHED.get_or_init(find_cursor).clone()
}

fn find_cursor() -> Option<String> {
    let home = dirs::home_dir()?;
    let local = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
    let candidates = [
        local.as_ref().map(|p| p.join("Programs").join("cursor").join("Cursor.exe")),
        local.as_ref().map(|p| p.join("cursor").join("Cursor.exe")),
        Some(home.join("AppData").join("Local").join("Programs").join("cursor").join("Cursor.exe")),
    ];
    for path in candidates.into_iter().flatten() {
        if path.is_file() {
            return Some("已安装".into());
        }
    }
    None
}

pub async fn check_remote(http: &reqwest::Client) -> Result<VersionReport> {
    let mut report = VersionReport {
        app: app_version().into(),
        cursor: detect_cursor_version(),
        ..VersionReport::default()
    };
    let url = std::env::var("GROK_BOT_AUTH_LATEST")
        .unwrap_or_else(|_| {
            "https://api.github.com/repos/xai-org/grok-bot-auth/releases/latest".into()
        });
    // Default probe is a known public latest endpoint shape. This app has no
    // release channel of its own; 404/mismatch is reported honestly.
    match http
        .get(&url)
        .header("user-agent", "Grok-Bot-Auth")
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let body = response.text().await.unwrap_or_default();
            if let Ok(rel) = serde_json::from_str::<GithubRelease>(&body) {
                report.remote = rel.tag_name.or(rel.name);
                report.notice = match &report.remote {
                    Some(tag) => format!("远端 {tag} · 本地 {}", report.app),
                    None => "远端清单没有 tag".into(),
                };
            } else if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                report.remote = value
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned);
                report.notice = format!("已读取 latest.json · 本地 {}", report.app);
            } else {
                report.notice = "未配置本应用发布源（latest.json 无法解析）".into();
            }
        }
        Ok(response) => {
            report.notice = format!(
                "当前 {} 为本机构建。没有独立发布源（HTTP {}）。更新说明见下方日志。",
                report.app,
                response.status().as_u16()
            );
        }
        Err(error) => {
            report.notice = format!("检查失败：{error}");
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_is_cargo_semver() {
        assert!(!app_version().is_empty());
        assert!(app_version().contains('.'));
        assert!(changelog().contains(app_version()));
    }
}
