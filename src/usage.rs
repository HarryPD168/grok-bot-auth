//! Local usage log for the overview page. Empty file = empty state, never fake hits.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRow {
    pub ts_ms: u64,
    pub source: String,
    pub model: String,
    pub latency_ms: u64,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default = "default_ok")]
    pub ok: bool,
}

fn default_ok() -> bool {
    true
}

#[derive(Debug, Clone, Default)]
pub struct UsageBar {
    pub start_ms: u64,
    pub label: String,
    pub count: u64,
    pub ok: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct UsageSummary {
    pub rows: usize,
    pub ok: usize,
    pub err: usize,
    pub tokens: u64,
    pub latency_ms: u64,
    pub bars: Vec<UsageBar>,
    pub by_day: Vec<(String, u64)>,
    pub by_model: Vec<(String, u64)>,
    pub recent: Vec<UsageRow>,
}

pub fn log_path() -> std::path::PathBuf {
    store::data_dir().join("usage.jsonl")
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn append(row: &UsageRow) -> Result<()> {
    std::fs::create_dir_all(store::data_dir())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())?;
    writeln!(file, "{}", serde_json::to_string(row)?)?;
    trim_if_huge()?;
    Ok(())
}

fn trim_if_huge() -> Result<()> {
    let path = log_path();
    let meta = std::fs::metadata(&path)?;
    if meta.len() < 2_000_000 {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let keep: String = raw
        .lines()
        .rev()
        .take(4000)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(path, keep)?;
    Ok(())
}

pub fn load_since(since_ms: u64) -> Vec<UsageRow> {
    let Ok(raw) = std::fs::read_to_string(log_path()) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<UsageRow>(line).ok())
        .filter(|row| row.ts_ms >= since_ms)
        .collect()
}

pub fn summarize(since_ms: u64) -> UsageSummary {
    let until = now_ms();
    let since = since_ms.min(until);
    let span = until.saturating_sub(since);
    let step = if span <= 2 * 3_600_000 {
        10 * 60_000
    } else if span <= 48 * 3_600_000 {
        3_600_000
    } else {
        86_400_000
    };
    summarize_window(since, until.max(since + 1), step)
}

pub fn summarize_window(since_ms: u64, until_ms: u64, step_ms: u64) -> UsageSummary {
    let step = step_ms.max(60_000);
    let rows = load_since(since_ms);
    let keys = span_keys(since_ms, until_ms, step);
    let mut bars: Vec<UsageBar> = keys
        .iter()
        .map(|(start, label)| UsageBar {
            start_ms: *start,
            label: label.clone(),
            count: 0,
            ok: 0,
            tokens: 0,
        })
        .collect();
    let mut models = std::collections::BTreeMap::<String, u64>::new();
    let mut ok = 0;
    let mut err = 0;
    let mut tokens = 0;
    let mut latency = 0;
    for row in &rows {
        if let Some(bar) = bars.iter_mut().rev().find(|bar| row.ts_ms >= bar.start_ms) {
            bar.count += 1;
            if row.ok {
                bar.ok += 1;
            }
            bar.tokens += row.tokens;
        }
        *models.entry(row.model.clone()).or_default() += 1;
        if row.ok {
            ok += 1;
        } else {
            err += 1;
        }
        tokens += row.tokens;
        latency += row.latency_ms;
    }
    let by_day = bars
        .iter()
        .map(|bar| (bar.label.clone(), bar.count))
        .collect();
    UsageSummary {
        rows: rows.len(),
        ok,
        err,
        tokens,
        latency_ms: latency,
        bars,
        by_day,
        by_model: models.into_iter().collect(),
        recent: rows.into_iter().rev().take(12).collect(),
    }
}

/// Inclusive-start buckets covering [since, until). Always returns at least one bar.
pub fn span_keys(since_ms: u64, until_ms: u64, step_ms: u64) -> Vec<(u64, String)> {
    let step = step_ms.max(1);
    let end = until_ms.max(since_ms + step);
    let mut keys = Vec::new();
    let mut t = since_ms - (since_ms % step);
    while t < end {
        keys.push((t, bucket_label(t, step)));
        t = t.saturating_add(step);
        if keys.len() > 120 {
            break;
        }
    }
    if keys.is_empty() {
        keys.push((since_ms, bucket_label(since_ms, step)));
    }
    keys
}

pub fn stamp_label(ts_ms: u64) -> String {
    bucket_label(ts_ms, 86_400_000)
}

fn bucket_label(ts_ms: u64, step_ms: u64) -> String {
    let secs = (ts_ms / 1000) as i64;
    let Ok(t) = time::OffsetDateTime::from_unix_timestamp(secs) else {
        return ts_ms.to_string();
    };
    if step_ms >= 86_400_000 {
        format!("{:02}-{:02}", u8::from(t.month()), t.day())
    } else {
        format!("{:02}:{:02}", t.hour(), t.minute())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_empty_when_missing_file() {
        let sum = summarize(u64::MAX);
        assert_eq!(sum.rows, 0);
    }

    #[test]
    fn month_span_always_has_trend_bars() {
        let keys = span_keys(0, 30 * 86_400_000, 86_400_000);
        assert!(keys.len() >= 28, "got {}", keys.len());
        assert!(keys.len() <= 32, "got {}", keys.len());
        let hour = span_keys(0, 3_600_000, 10 * 60_000);
        assert_eq!(hour.len(), 6);
    }
}
