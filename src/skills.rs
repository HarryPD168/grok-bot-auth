//! Local skill discovery (CC Switch 3.3 first cut: scan + toggle, no marketplace).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub enabled: bool,
}

pub fn cursor_skills_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("skills")
}

pub fn scan_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join(".cursor").join("skills"),
        home.join(".grok").join("skills"),
        home.join(".agents").join("skills"),
        PathBuf::from(r"C:\Users\Harry_win10\Desktop\grok bot").join(".cursor").join("skills"),
    ]
}

pub fn scan() -> Vec<SkillEntry> {
    let mut out = Vec::new();
    let dest = cursor_skills_dir();
    for root in scan_dirs() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            let id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("skill")
                .to_owned();
            if out.iter().any(|item: &SkillEntry| item.id == id) {
                continue;
            }
            let body = std::fs::read_to_string(&skill_md).unwrap_or_default();
            let (name, description) = parse_skill_head(&body, &id);
            let enabled = dest.join(&id).join("SKILL.md").is_file();
            out.push(SkillEntry {
                id,
                name,
                description,
                path: path.display().to_string(),
                enabled,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_skill_head(body: &str, fallback: &str) -> (String, String) {
    let mut name = fallback.to_owned();
    let mut description = String::new();
    for line in body.lines().take(40) {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name:") {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                name = value.to_owned();
            }
        }
        if let Some(rest) = line.strip_prefix("description:") {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                description = value.chars().take(120).collect();
            }
        }
        if line.starts_with("# ") && name == fallback {
            name = line.trim_start_matches('#').trim().to_owned();
        }
    }
    (name, description)
}

pub fn set_enabled(entry: &SkillEntry, on: bool) -> Result<()> {
    let dest = cursor_skills_dir().join(&entry.id);
    if on {
        copy_dir(Path::new(&entry.path), &dest)?;
    } else if dest.exists() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_head_reads_yaml_name() {
        let (name, desc) = parse_skill_head(
            "---\nname: grok-bot-ctf\ndescription: Official CTF entry.\n---\n",
            "fallback",
        );
        assert_eq!(name, "grok-bot-ctf");
        assert!(desc.contains("CTF"));
    }
}
