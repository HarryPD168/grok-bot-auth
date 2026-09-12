use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::providers::Provider;
use crate::sand::{Account, SandFamily};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub imported: Vec<String>,
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub catalog: Vec<SandFamily>,
    #[serde(default)]
    pub coexist: bool,
    #[serde(default)]
    pub preferred_model: Option<String>,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub prompt_pool: Vec<String>,
    #[serde(default = "default_compact_budget")]
    pub compact_budget: usize,
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default = "default_context")]
    pub context: String,
    #[serde(default)]
    pub mcp: Vec<crate::mcp::McpServer>,
    #[serde(default)]
    pub grok_maps: Vec<crate::providers::ModelMap>,
    #[serde(default = "default_api_port")]
    pub api_port: u16,
    #[serde(default = "default_mitm_port")]
    pub mitm_port: u16,
    #[serde(default)]
    pub outbound_proxy: String,
}

fn default_compact_budget() -> usize {
    crate::compact::OFFICIAL_CHAR_BUDGET
}

fn default_effort() -> String {
    "high".into()
}

fn default_context() -> String {
    String::new()
}

fn default_api_port() -> u16 {
    47821
}

fn default_mitm_port() -> u16 {
    47822
}

pub fn save_prefs_sync(prefs: &Prefs) -> Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::write(prefs_path(), serde_json::to_vec_pretty(prefs)?)?;
    Ok(())
}

pub fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok-bot-auth")
}

pub fn save_bytes_cache(name: &str, content_type: &str, bytes: &[u8]) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{name}.ctype")), content_type);
    let _ = std::fs::write(dir.join(format!("{name}.bin")), bytes);
}

pub fn load_bytes_cache(name: &str) -> Option<(String, Vec<u8>)> {
    let dir = data_dir();
    let content_type = std::fs::read_to_string(dir.join(format!("{name}.ctype"))).ok()?;
    let bytes = std::fs::read(dir.join(format!("{name}.bin"))).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some((content_type, bytes))
}

fn account_path() -> PathBuf {
    data_dir().join("account.json")
}

pub async fn load_account() -> Result<Option<Account>> {
    match tokio::fs::read_to_string(account_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(raw) => {
            let account: Account = serde_json::from_str(&raw)?;
            if account.access_token.is_empty() || account.machine_id.is_empty() {
                return Ok(None);
            }
            Ok(Some(account))
        }
    }
}

pub async fn save_account(account: &Account) -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(account_path(), serde_json::to_vec_pretty(account)?).await?;
    Ok(())
}

pub async fn clear_account() -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(account_path(), b"{}").await?;
    Ok(())
}

fn grok_pool_path() -> PathBuf {
    data_dir().join("accounts.json")
}

pub async fn load_grok_pool() -> Result<crate::accounts::AccountPool<Account>> {
    match tokio::fs::read_to_string(grok_pool_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut pool = crate::accounts::AccountPool::default();
            if let Some(mut account) = load_account().await? {
                crate::sand::fill_account_identity(&mut account);
                pool.upsert(account);
            }
            Ok(pool)
        }
        Err(error) => Err(error.into()),
        Ok(raw) => {
            let mut pool: crate::accounts::AccountPool<Account> =
                serde_json::from_str(&raw).unwrap_or_default();
            for slot in &mut pool.slots {
                crate::sand::fill_account_identity(slot);
            }
            if pool.slots.is_empty() {
                if let Some(mut account) = load_account().await? {
                    crate::sand::fill_account_identity(&mut account);
                    pool.upsert(account);
                }
            }
            if pool.active_id.is_empty() {
                if let Some(first) = pool.slots.first() {
                    pool.active_id = crate::sand::account_id(first);
                }
            }
            Ok(pool)
        }
    }
}

pub async fn save_grok_pool(pool: &crate::accounts::AccountPool<Account>) -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(grok_pool_path(), serde_json::to_vec_pretty(pool)?).await?;
    if let Some(active) = pool
        .slots
        .iter()
        .find(|slot| crate::sand::account_id(slot) == pool.active_id)
        .or_else(|| pool.slots.first())
    {
        save_account(active).await?;
    } else {
        clear_account().await?;
    }
    Ok(())
}

fn prefs_path() -> PathBuf {
    data_dir().join("prefs.json")
}

pub async fn load_prefs() -> Result<Prefs> {
    match tokio::fs::read_to_string(prefs_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Prefs::default()),
        Err(error) => Err(error.into()),
        Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
    }
}

pub async fn save_prefs(prefs: &Prefs) -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(prefs_path(), serde_json::to_vec_pretty(prefs)?).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceShort {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub grok_bot_token: String,
    pub expires_at_ms: Option<u64>,
}

fn inference_short_path() -> PathBuf {
    data_dir().join("inference-short.json")
}

fn renewal_credential_path() -> PathBuf {
    data_dir().join("renewal-credential.txt")
}

pub async fn load_inference_short() -> Result<Option<InferenceShort>> {
    match tokio::fs::read_to_string(inference_short_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(raw) => {
            let short: InferenceShort = serde_json::from_str(&raw)?;
            if short.grok_bot_token.is_empty() {
                return Ok(None);
            }
            Ok(Some(short))
        }
    }
}

pub async fn save_inference_short(short: &InferenceShort) -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(inference_short_path(), serde_json::to_vec_pretty(short)?).await?;
    Ok(())
}

pub async fn load_renewal_credential() -> Result<Option<String>> {
    match tokio::fs::read_to_string(renewal_credential_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(raw) => {
            let value = raw.trim().to_owned();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxStream {
    pub base_url: String,
    pub network_token: String,
}

fn box_stream_path() -> PathBuf {
    data_dir().join("box-stream.json")
}

pub async fn load_box_stream() -> Result<Option<BoxStream>> {
    match tokio::fs::read_to_string(box_stream_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(raw) => {
            let cfg: BoxStream = serde_json::from_str(&raw)?;
            if cfg.base_url.is_empty() || cfg.network_token.is_empty() {
                Ok(None)
            } else {
                Ok(Some(cfg))
            }
        }
    }
}

fn providers_path() -> PathBuf {
    data_dir().join("providers.json")
}

pub async fn load_providers() -> Result<Vec<Provider>> {
    match tokio::fs::read_to_string(providers_path()).await {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
        Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
    }
}

pub async fn save_providers(providers: &[Provider]) -> Result<()> {
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::write(providers_path(), serde_json::to_vec_pretty(providers)?).await?;
    Ok(())
}

pub fn grok_bot_bearer(short: &InferenceShort) -> Result<&str> {
    if short.grok_bot_token.is_empty() {
        return Err(Error::Msg(
            "Stream fail-closed: inference grokBotToken missing".into(),
        ));
    }
    Ok(short.grok_bot_token.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_bot_bearer_fail_closed_when_empty() {
        let short = InferenceShort {
            access_token: "session-shaped".into(),
            grok_bot_token: String::new(),
            expires_at_ms: None,
        };
        assert!(grok_bot_bearer(&short).is_err());
    }

    #[test]
    fn provider_file_does_not_clear_grok_bot_account_shape() {
        let account = crate::sand::Account {
            access_token: "cursor-session".into(),
            machine_id: "m".into(),
            display_name: "bot".into(),
            ..crate::sand::Account::default()
        };
        let encoded = serde_json::to_string(&account).expect("account");
        assert!(encoded.contains("cursor-session"));
        let mut providers = Vec::new();
        crate::providers::upsert_provider(
            &mut providers,
            crate::providers::Provider {
                id: "openai-1".into(),
                kind: crate::providers::ProviderKind::Openai,
                label: "OpenAI".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_key: "sk-test".into(),
                ..crate::providers::Provider::default()
            },
        );
        crate::providers::upsert_provider(
            &mut providers,
            crate::providers::Provider {
                id: "xai-1".into(),
                kind: crate::providers::ProviderKind::Xai,
                label: "xAI".into(),
                base_url: "https://api.x.ai/v1".into(),
                api_key: "xai-test".into(),
                ..crate::providers::Provider::default()
            },
        );
        assert_eq!(providers.len(), 2);
        let round: crate::sand::Account = serde_json::from_str(&encoded).expect("reload");
        assert_eq!(round.access_token, "cursor-session");
        assert_eq!(round.machine_id, "m");
    }

    #[test]
    fn grok_bot_bearer_returns_grok_field_not_access() {
        let short = InferenceShort {
            access_token: "session-shaped".into(),
            grok_bot_token: "grok-bot-shaped".into(),
            expires_at_ms: Some(1),
        };
        assert_eq!(grok_bot_bearer(&short).expect("present"), "grok-bot-shaped");
    }
}
