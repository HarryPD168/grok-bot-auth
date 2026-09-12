use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::error::{Error, Result};

const PROXY_URL: &str = "http.proxy";
const PROXY_SUPPORT: &str = "http.proxySupport";
const DISABLE_HTTP2: &str = "cursor.general.disableHttp2";
const SYSTEM_CERTS: &str = "http.experimental.systemCertificatesV2";
/// Never a proxy URL. Old builds wrote 47822 here and Cursor account/plan RPCs died.
const KERBEROS_SPN: &str = "http.proxyKerberosServicePrincipal";

const CLEAR_KEYS: [&str; 5] = [
    PROXY_URL,
    PROXY_SUPPORT,
    DISABLE_HTTP2,
    SYSTEM_CERTS,
    KERBEROS_SPN,
];

fn settings_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Msg("APPDATA is missing".into()))?;
    Ok(appdata.join("Cursor/User/settings.json"))
}

fn read() -> Result<BTreeMap<String, Value>> {
    let path = settings_path()?;
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    if data.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    json5::from_str(&data).map_err(|error| Error::Msg(format!("parse Cursor settings: {error}")))
}

fn write(settings: &BTreeMap<String, Value>) -> Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(settings)?;
    std::fs::write(path, [bytes.as_slice(), b"\n"].concat())?;
    Ok(())
}

/// Cursor MITM 反代 keys. Does not touch Clash / outbound HTTP proxy.
pub fn apply_cursor_mitm(settings: &mut BTreeMap<String, Value>, proxy_url: &str) {
    settings.insert(PROXY_URL.into(), Value::String(proxy_url.into()));
    settings.insert(PROXY_SUPPORT.into(), Value::String("on".into()));
    settings.insert(DISABLE_HTTP2.into(), Value::Bool(true));
    settings.insert(SYSTEM_CERTS.into(), Value::Bool(true));
    settings.remove(KERBEROS_SPN);
}

pub fn strip_cursor_mitm(settings: &mut BTreeMap<String, Value>) -> bool {
    let before = settings.len();
    for key in CLEAR_KEYS {
        settings.remove(key);
    }
    settings.len() != before
}

pub fn ensure_disable_http2() -> Result<()> {
    let mut settings = read()?;
    settings.insert(DISABLE_HTTP2.into(), Value::Bool(true));
    write(&settings)
}

pub fn write_proxy_settings(proxy_url: &str) -> Result<()> {
    let mut settings = read()?;
    apply_cursor_mitm(&mut settings, proxy_url);
    write(&settings)
}

pub fn clear_proxy_settings() -> Result<()> {
    let mut settings = read()?;
    if strip_cursor_mitm(&mut settings) {
        write(&settings)?;
    }
    Ok(())
}

pub fn current_proxy() -> Result<Option<String>> {
    Ok(read()?
        .get(PROXY_URL)
        .and_then(Value::as_str)
        .map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mitm_does_not_write_kerberos_spn() {
        let mut settings = BTreeMap::new();
        settings.insert(
            KERBEROS_SPN.into(),
            Value::String("http://127.0.0.1:47822".into()),
        );
        apply_cursor_mitm(&mut settings, "http://127.0.0.1:47822");
        assert_eq!(
            settings.get(PROXY_URL).and_then(Value::as_str),
            Some("http://127.0.0.1:47822")
        );
        assert!(!settings.contains_key(KERBEROS_SPN));
        assert_eq!(
            settings.get(PROXY_SUPPORT).and_then(Value::as_str),
            Some("on")
        );
    }

    #[test]
    fn strip_removes_leftover_kerberos() {
        let mut settings = BTreeMap::new();
        apply_cursor_mitm(&mut settings, "http://127.0.0.1:47822");
        settings.insert(KERBEROS_SPN.into(), Value::String("http://127.0.0.1:47822".into()));
        assert!(strip_cursor_mitm(&mut settings));
        assert!(settings.is_empty());
    }
}
