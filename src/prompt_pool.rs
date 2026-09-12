//! Prompt-pool inject: prepend enabled pool entries onto the user turn
//! (CCursor prompt-profile / cursor-byok system extras, reduced).

pub fn apply_prompt_pool(entries: &[String], user: &str) -> String {
    let inject: Vec<&str> = entries
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .collect();
    if inject.is_empty() {
        return user.to_owned();
    }
    format!("{}\n\n{}", inject.join("\n"), user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_prompt_pool_prepends_entries() {
        let out = apply_prompt_pool(
            &[String::from("keep replies short"), String::from("  ")],
            "Reply with exactly: pong",
        );
        assert!(out.contains("keep replies short"));
        assert!(out.contains("Reply with exactly: pong"));
        assert!(out.starts_with("keep replies short"));
        assert!(out.len() > "Reply with exactly: pong".len());
    }

    #[test]
    fn apply_prompt_pool_empty_is_identity() {
        assert_eq!(apply_prompt_pool(&[], "hi"), "hi");
        assert_eq!(apply_prompt_pool(&[String::new()], "hi"), "hi");
    }
}
