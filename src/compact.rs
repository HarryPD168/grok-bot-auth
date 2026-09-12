//! History compaction lifted from cursor-byok / CCursor: keep the first user
//! turn, summarize the middle, keep a recent tail. Char budget, not tokens.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
    pub images: Vec<(String, String)>,
}

impl ChatTurn {
    pub fn new(role: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            text: text.into(),
            images: Vec::new(),
        }
    }
}

/// cursor-byok keeps 10% of the window free; local history uses a matching cap.
pub const OFFICIAL_CHAR_BUDGET: usize = 12_000;

pub fn official_budget() -> usize {
    OFFICIAL_CHAR_BUDGET
}

/// Apply prompt-pool to the new user turn, append, then compact the whole history.
pub fn prepare_outbound(
    pool: &[String],
    history: &[ChatTurn],
    user: &str,
    budget: usize,
) -> Vec<ChatTurn> {
    let user_text = crate::prompt_pool::apply_prompt_pool(pool, user);
    let mut turns = history.to_vec();
    turns.push(ChatTurn::new("user", user_text));
    compact_turns(&turns, budget)
}

pub fn compact_turns(turns: &[ChatTurn], budget: usize) -> Vec<ChatTurn> {
    if budget == 0 {
        return turns.iter().take(1).cloned().collect();
    }
    if total_chars(turns) <= budget {
        return turns.to_vec();
    }
    let keep_tail = keep_tail_count(budget, turns.len());
    let tail_start = turns.len().saturating_sub(keep_tail);
    let first_user = turns.iter().find(|turn| turn.role == "user").cloned();
    let middle = if tail_start > 1 {
        &turns[1..tail_start]
    } else {
        &[]
    };
    let summary = summarize_middle(middle);
    let mut out = Vec::new();
    if let Some(first) = first_user {
        let already = turns
            .get(tail_start..)
            .is_some_and(|tail| tail.iter().any(|turn| turn.text == first.text));
        if !already {
            out.push(first);
        }
    }
    if !summary.is_empty() {
        out.push(ChatTurn::new("assistant", summary));
    }
    out.extend(turns[tail_start..].iter().cloned());
    if total_chars(&out) > budget {
        let cap = budget / out.len().max(1);
        for turn in &mut out {
            if turn.text.chars().count() > cap {
                turn.text = turn.text.chars().take(cap).collect();
            }
        }
    }
    out
}

/// Larger windows keep a longer uncompressed tail (CCursor count policy).
fn keep_tail_count(budget: usize, n: usize) -> usize {
    let want = if budget < 40_000 {
        2
    } else if budget < 200_000 {
        4
    } else {
        6
    };
    want.min(n)
}

pub fn total_chars(turns: &[ChatTurn]) -> usize {
    turns.iter().map(|turn| turn.text.chars().count()).sum()
}

fn summarize_middle(middle: &[ChatTurn]) -> String {
    if middle.is_empty() {
        return String::new();
    }
    let snippets: Vec<String> = middle
        .iter()
        .map(|turn| turn.text.chars().take(80).collect())
        .collect();
    format!(
        "[compacted {} turns] {}",
        middle.len(),
        snippets.join(" | ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_outbound_injects_pool_then_compacts() {
        let history: Vec<ChatTurn> = (0..10)
            .map(|i| {
                ChatTurn::new(
                    if i % 2 == 0 { "user" } else { "assistant" },
                    format!("hist-{i} {}", "word ".repeat(30)),
                )
            })
            .collect();
        let before = history.iter().map(|t| t.text.len()).sum::<usize>() + "latest".len();
        let out = prepare_outbound(
            &[String::from("keep replies short")],
            &history,
            "latest",
            500,
        );
        let after = out.iter().map(|t| t.text.len()).sum::<usize>();
        assert!(after < before, "compacted {after} should be < {before}");
        assert!(
            out.iter()
                .any(|turn| turn.text.contains("keep replies short")),
            "prompt-pool text on outbound"
        );
        assert!(
            out.iter()
                .any(|turn| turn.role == "user" && turn.text.contains("latest")),
            "latest user kept"
        );
    }

    #[test]
    fn official_budget_is_cursor_byok_fallback() {
        assert_eq!(official_budget(), 12_000);
        assert_eq!(OFFICIAL_CHAR_BUDGET, 12_000);
    }

    #[test]
    fn compact_turns_shrinks_and_keeps_first_user() {
        let turns: Vec<ChatTurn> = (0..12)
            .map(|i| {
                ChatTurn::new(
                    if i % 2 == 0 { "user" } else { "assistant" },
                    format!("turn-{i} {}", "word ".repeat(40)),
                )
            })
            .collect();
        let before = turns.iter().map(|t| t.text.len()).sum::<usize>();
        let out = compact_turns(&turns, 400);
        let after = out.iter().map(|t| t.text.len()).sum::<usize>();
        assert!(after < before, "compacted {after} should be < {before}");
        assert!(
            out.iter()
                .any(|turn| turn.role == "user" && turn.text.starts_with("turn-0")),
            "first user turn kept"
        );
        assert!(
            out.iter().any(|turn| turn.text.contains("[compacted")),
            "middle summarized"
        );
    }
}
