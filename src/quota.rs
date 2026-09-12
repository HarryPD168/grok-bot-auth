//! Grok Bot (sand) weekly usage from DashboardService/GetSandUsageStatus.
//! Cursor Models / Other Models planUsage is ignored.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub plan_label: Option<String>,
    pub remaining_percent: Option<f64>,
    pub used_percent: Option<f64>,
    pub remaining: Option<i64>,
    pub limit: Option<i64>,
    pub reset_at_ms: Option<u64>,
    pub cycle_start_ms: Option<u64>,
    pub display_message: Option<String>,
    pub updated_at_ms: u64,
    /// True only when the snapshot is Grok Bot's own weekly/included bucket.
    /// Cursor Models / Other Models must not mark Grok Bot as exhausted.
    #[serde(default)]
    pub grok_scoped: bool,
}

impl QuotaSnapshot {
    pub fn limit_reached(&self) -> bool {
        self.grok_exhausted()
    }

    pub fn grok_exhausted(&self) -> bool {
        if !self.grok_scoped {
            return false;
        }
        self.remaining_percent
            .map(|n| n <= 0.0)
            .or_else(|| self.remaining.map(|n| n <= 0))
            .unwrap_or(false)
    }

    pub fn summary(&self, now_ms: u64) -> String {
        let mut parts = Vec::new();
        if let Some(plan) = &self.plan_label {
            if !plan.is_empty() {
                parts.push(plan.clone());
            }
        }
        if let Some(used) = self.used_percent {
            parts.push(format!("已用 {:.0}%", used));
        }
        if let Some(pct) = self.remaining_percent {
            parts.push(format!("剩余 {:.0}%", pct));
        } else if let (Some(remaining), Some(limit)) = (self.remaining, self.limit) {
            if limit > 0 {
                parts.push(format!("剩余 {remaining}/{limit}"));
            }
        }
        if let Some(reset) = self.reset_at_ms {
            parts.push(format_reset(reset, now_ms));
        }
        if parts.is_empty() {
            self.display_message.clone().unwrap_or_else(|| "额度未拉取".into())
        } else {
            parts.join("  ·  ")
        }
    }
}

pub fn format_reset(reset_at_ms: u64, now_ms: u64) -> String {
    if reset_at_ms <= now_ms {
        return "已到重置点".into();
    }
    let secs = (reset_at_ms - now_ms) / 1000;
    if secs < 90 {
        "即将重置".into()
    } else if secs < 3600 {
        format!("{} 分钟后重置", (secs + 30) / 60)
    } else if secs < 86400 {
        format!("{:.1} 小时后重置", secs as f64 / 3600.0)
    } else {
        format!("{:.1} 天后重置", secs as f64 / 86400.0)
    }
}

fn as_u64_ms(value: &Value) -> Option<u64> {
    let n = value.as_u64().or_else(|| value.as_i64().map(|v| v.max(0) as u64))?;
    if n == 0 {
        return None;
    }
    Some(if n > 10_000_000_000 { n } else { n.saturating_mul(1000) })
}

fn as_f64(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_i64().map(|n| n as f64))
}

fn as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_u64().map(|n| n as i64))
}

fn child<'a>(root: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| root.get(*key))
}

fn usage_from_node(node: &Value) -> (Option<f64>, Option<f64>, Option<i64>, Option<i64>) {
    let remaining = as_i64(child(node, &["remaining"]).unwrap_or(&Value::Null));
    let limit = as_i64(child(node, &["limit", "included"]).unwrap_or(&Value::Null));
    let used_percent = as_f64(
        child(
            node,
            &[
                "percentUsed",
                "percent_used",
                "usedPercent",
                "used_percent",
                "totalPercentUsed",
                "total_percent_used",
            ],
        )
        .unwrap_or(&Value::Null),
    );
    let remaining_percent = as_f64(
        child(node, &["remainingPercent", "remaining_percent"]).unwrap_or(&Value::Null),
    )
    .or_else(|| used_percent.map(|used| (100.0 - used).clamp(0.0, 100.0)))
    .or_else(|| match (remaining, limit) {
        (Some(left), Some(cap)) if cap > 0 => {
            Some(((left as f64) / (cap as f64) * 100.0).clamp(0.0, 100.0))
        }
        _ => None,
    });
    (used_percent, remaining_percent, remaining, limit)
}

fn looks_like_grok_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.contains("grok") && lower.contains("bot"))
        || lower == "grok bot"
        || lower.contains("grokbot")
}

fn grok_usage_node(body: &Value) -> Option<&Value> {
    for key in [
        "grokBotUsage",
        "grok_bot_usage",
        "grokBotWeeklyUsage",
        "grokBotIncludedUsage",
        "includedGrokBotUsage",
    ] {
        if let Some(node) = body.get(key) {
            return Some(node);
        }
    }
    for key in [
        "namedModelUsage",
        "named_model_usage",
        "modelUsage",
        "model_usage",
        "usages",
        "entitlements",
    ] {
        if let Some(items) = body.get(key).and_then(|v| v.as_array()) {
            for item in items {
                let name = child(item, &["name", "displayName", "display_name", "title", "label"])
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if looks_like_grok_name(name) {
                    return Some(item);
                }
            }
        }
    }
    None
}

fn proto_timestamp_ms(value: &Value) -> Option<u64> {
    if let Some(seconds) = value.get("seconds").and_then(|v| v.as_i64().or_else(|| {
        v.as_u64().map(|n| n as i64)
    })) {
        return Some((seconds.max(0) as u64).saturating_mul(1000));
    }
    if let Some(text) = value.as_str() {
        if let Ok(parsed) =
            time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        {
            return Some((parsed.unix_timestamp().max(0) as u64).saturating_mul(1000));
        }
        return None;
    }
    as_u64_ms(value)
}

fn parse_sand_usage(body: &Value, now_ms: u64) -> QuotaSnapshot {
    let used_percent = as_f64(
        child(body, &["usagePercent", "usage_percent"]).unwrap_or(&Value::Null),
    );
    let remaining_percent = used_percent.map(|used| (100.0 - used).clamp(0.0, 100.0));
    let has_available = body
        .get("hasAvailableUsage")
        .or_else(|| body.get("has_available_usage"))
        .and_then(|v| v.as_bool());
    let included_zero = body
        .get("includedLimitZero")
        .or_else(|| body.get("included_limit_zero"))
        .and_then(|v| v.as_bool())
        == Some(true);
    QuotaSnapshot {
        plan_label: Some("Grok Bot".into()),
        remaining_percent,
        used_percent,
        remaining: None,
        limit: None,
        reset_at_ms: child(
            body,
            &[
                "nextResetTimestampUtc",
                "next_reset_timestamp_utc",
            ],
        )
        .and_then(proto_timestamp_ms),
        cycle_start_ms: child(body, &["currentPeriodStart", "current_period_start"])
            .and_then(proto_timestamp_ms),
        display_message: None,
        updated_at_ms: now_ms,
        grok_scoped: true,
        // stash availability in remaining: 0 if no usage left
        // grok_exhausted uses remaining_percent / remaining
    }
    .with_sand_flags(has_available, included_zero, used_percent)
}

impl QuotaSnapshot {
    fn with_sand_flags(
        mut self,
        has_available: Option<bool>,
        included_zero: bool,
        used_percent: Option<f64>,
    ) -> Self {
        if has_available == Some(false) || included_zero || used_percent.map(|n| n >= 100.0) == Some(true)
        {
            self.remaining_percent = Some(0.0);
        }
        self
    }
}

/// Grok Bot weekly usage only. Cursor planUsage is never shown or used to exhaust.
pub fn parse_xai_credits(body: &Value, now_ms: u64) -> QuotaSnapshot {
    let remaining = as_f64(
        child(
            body,
            &["remaining", "remainingCredits", "remaining_credits", "credits"],
        )
        .unwrap_or(&Value::Null),
    );
    let limit = as_f64(
        child(body, &["limit", "total", "included"]).unwrap_or(&Value::Null),
    );
    let used_percent = as_f64(
        child(body, &["percentUsed", "usedPercent", "used_percent"]).unwrap_or(&Value::Null),
    )
    .or_else(|| match (remaining, limit) {
        (Some(left), Some(cap)) if cap > 0.0 => Some(((cap - left) / cap * 100.0).clamp(0.0, 100.0)),
        _ => None,
    });
    let remaining_percent = used_percent.map(|used| (100.0 - used).clamp(0.0, 100.0));
    QuotaSnapshot {
        plan_label: Some("xAI".into()),
        remaining_percent,
        used_percent,
        remaining: remaining.map(|n| n as i64),
        limit: limit.map(|n| n as i64),
        reset_at_ms: None,
        cycle_start_ms: None,
        display_message: None,
        updated_at_ms: now_ms,
        grok_scoped: false,
    }
}

pub fn parse_period_usage(body: &Value, now_ms: u64) -> QuotaSnapshot {
    if body.get("usagePercent").is_some()
        || body.get("usage_percent").is_some()
        || body.get("hasAvailableUsage").is_some()
        || body.get("has_available_usage").is_some()
    {
        return parse_sand_usage(body, now_ms);
    }
    let grok_node = grok_usage_node(body);
    if let Some(node) = grok_node {
        let (used_percent, remaining_percent, remaining, limit) = usage_from_node(node);
        return QuotaSnapshot {
            plan_label: Some("Grok Bot".into()),
            remaining_percent,
            used_percent,
            remaining,
            limit,
            reset_at_ms: child(node, &["resetsAt", "resetAt", "reset_at", "nextResetTimestampUtc"])
                .and_then(proto_timestamp_ms)
                .or_else(|| as_u64_ms(child(node, &["resetsAt", "resetAt"]).unwrap_or(&Value::Null))),
            cycle_start_ms: None,
            display_message: None,
            updated_at_ms: now_ms,
            grok_scoped: true,
        };
    }
    QuotaSnapshot {
        display_message: Some("未返回 Grok Bot 周用量".into()),
        updated_at_ms: now_ms,
        grok_scoped: false,
        ..QuotaSnapshot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_plan_usage_is_ignored() {
        let body = json!({
            "billingCycleStart": 1_700_000_000,
            "billingCycleEnd": 1_702_592_000,
            "displayMessage": "Ultra",
            "planUsage": {
                "remaining": 0,
                "limit": 10000,
                "totalPercentUsed": 100.0
            }
        });
        let snap = parse_period_usage(&body, 1_701_000_000_000);
        assert!(!snap.grok_scoped);
        assert!(snap.used_percent.is_none());
        assert!(!snap.grok_exhausted());
        assert!(snap.summary(1_701_000_000_000).contains("未返回 Grok Bot 周用量"));
    }

    #[test]
    fn sand_usage_percent_is_grok_bot_weekly() {
        let body = json!({
            "usagePercent": 33.0,
            "hasAvailableUsage": true,
            "includedLimitZero": false,
            "nextResetTimestampUtc": { "seconds": 1_800_000_000 }
        });
        let snap = parse_period_usage(&body, 0);
        assert!(snap.grok_scoped);
        assert_eq!(snap.used_percent, Some(33.0));
        assert_eq!(snap.remaining_percent, Some(67.0));
        assert!(!snap.grok_exhausted());
        assert!(snap.summary(0).contains("已用 33%"));
    }

    #[test]
    fn iso_reset_is_days_not_thousands() {
        let body = json!({
            "usagePercent": 33.0,
            "hasAvailableUsage": true,
            "nextResetTimestampUtc": "2026-09-14T00:00:00.000Z"
        });
        let now_ms = 1_789_171_200_000; // 2026-09-12 UTC
        let snap = parse_period_usage(&body, now_ms);
        let reset = snap.reset_at_ms.expect("reset");
        let days = (reset.saturating_sub(now_ms)) / 86_400_000;
        assert!(days <= 5, "reset days={days} reset_at={reset}");
        assert!(!snap.summary(now_ms).contains("2742"));
    }

    #[test]
    fn grok_bot_bucket_not_cursor_models() {
        let body = json!({
            "planUsage": { "remaining": 0, "limit": 100, "totalPercentUsed": 100 },
            "namedModelUsage": [
                { "name": "Cursor Models", "percentUsed": 99 },
                { "name": "Other Models", "percentUsed": 100 },
                { "name": "Grok Bot", "percentUsed": 33, "resetsAt": 1_800_000_000 }
            ]
        });
        let snap = parse_period_usage(&body, 0);
        assert!(snap.grok_scoped);
        assert_eq!(snap.used_percent, Some(33.0));
        assert_eq!(snap.remaining_percent, Some(67.0));
        assert!(!snap.grok_exhausted());
        assert!(!snap.limit_reached());
    }

    #[test]
    fn grok_bot_zero_remaining_is_exhausted() {
        let body = json!({
            "grokBotUsage": { "percentUsed": 100, "remaining": 0, "limit": 10 }
        });
        let snap = parse_period_usage(&body, 0);
        assert!(snap.grok_scoped);
        assert!(snap.grok_exhausted());
    }
}
