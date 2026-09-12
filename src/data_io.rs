//! Backup / export / import of grok-bot-auth prefs. Tokens stay in files, not dumped to UI.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::store;

pub fn backup_dir() -> PathBuf {
    store::data_dir().join("backups")
}

pub fn export_prefs() -> Result<PathBuf> {
    std::fs::create_dir_all(backup_dir())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = backup_dir().join(format!("prefs-{stamp}.json"));
    let src = store::data_dir().join("prefs.json");
    if src.is_file() {
        std::fs::copy(&src, &dest)?;
    } else {
        std::fs::write(&dest, b"{}")?;
    }
    Ok(dest)
}

pub fn import_prefs(path: &std::path::Path) -> Result<()> {
    let raw = std::fs::read_to_string(path)?;
    let parsed: crate::store::Prefs = serde_json::from_str(&raw)?;
    store::save_prefs_sync(&parsed)?;
    Ok(())
}

pub fn clear_providers_file() -> Result<()> {
    let path = store::data_dir().join("providers.json");
    if path.exists() {
        std::fs::write(path, b"[]")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_dir_is_under_data_dir() {
        let dir = backup_dir();
        assert!(dir.ends_with("backups"));
    }
}
