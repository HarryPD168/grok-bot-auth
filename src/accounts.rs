//! Same-provider OAuth account pool. Slots are keyed by email.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PoolMode {
    /// Spread requests across unused accounts.
    #[default]
    Share,
    /// Stay on the current account until it reports quota failure, then the next.
    QuotaFirst,
}

impl PoolMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Share => "额度共享",
            Self::QuotaFirst => "额度优先",
        }
    }
}

pub trait SlotId {
    fn slot_id(&self) -> String;
    fn is_exhausted(&self) -> bool;
    fn mark_exhausted(&mut self, error: Option<String>);
    fn clear_exhausted(&mut self);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPool<T> {
    #[serde(default)]
    pub mode: PoolMode,
    #[serde(default)]
    pub active_id: String,
    #[serde(default)]
    pub share_cursor: usize,
    #[serde(default)]
    pub slots: Vec<T>,
}

impl<T> Default for AccountPool<T> {
    fn default() -> Self {
        Self {
            mode: PoolMode::Share,
            active_id: String::new(),
            share_cursor: 0,
            slots: Vec::new(),
        }
    }
}

impl<T: SlotId + Clone> AccountPool<T> {
    pub fn pick(&mut self) -> Option<T> {
        let index = pick_index(&self.slots, self.mode, &self.active_id, self.share_cursor)?;
        if self.mode == PoolMode::Share {
            self.share_cursor = index.saturating_add(1);
        }
        self.active_id = self.slots[index].slot_id();
        Some(self.slots[index].clone())
    }

    pub fn upsert(&mut self, slot: T) {
        let id = slot.slot_id();
        if let Some(existing) = self.slots.iter_mut().find(|item| item.slot_id() == id) {
            *existing = slot;
        } else {
            self.slots.push(slot);
        }
        self.active_id = id;
    }

    /// If every slot is marked exhausted, unstick the active (or first) token-bearing slot.
    pub fn recover_if_all_exhausted(&mut self) -> Option<T> {
        if self.slots.is_empty() {
            return None;
        }
        if self.slots.iter().any(|slot| !slot.is_exhausted()) {
            return None;
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.slot_id() == self.active_id)
            .unwrap_or(0);
        self.slots[index].clear_exhausted();
        self.active_id = self.slots[index].slot_id();
        Some(self.slots[index].clone())
    }

    pub fn update_slot(&mut self, slot: T) {
        let id = slot.slot_id();
        if let Some(existing) = self.slots.iter_mut().find(|item| item.slot_id() == id) {
            *existing = slot;
        } else {
            self.slots.push(slot);
            if self.active_id.is_empty() {
                self.active_id = id;
            }
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.slots.retain(|item| item.slot_id() != id);
        if self.active_id == id {
            self.active_id = self
                .slots
                .first()
                .map(SlotId::slot_id)
                .unwrap_or_default();
        }
    }

    pub fn activate(&mut self, id: &str) -> Option<T> {
        let slot = self.slots.iter().find(|item| item.slot_id() == id).cloned()?;
        self.active_id = id.to_owned();
        Some(slot)
    }

    pub fn mark_exhausted(&mut self, id: &str, error: Option<String>) {
        if let Some(slot) = self.slots.iter_mut().find(|item| item.slot_id() == id) {
            slot.mark_exhausted(error);
        }
    }

    pub fn reset_exhausted(&mut self) {
        for slot in &mut self.slots {
            slot.clear_exhausted();
        }
    }

    pub fn usable_count(&self) -> usize {
        self.slots.iter().filter(|item| !item.is_exhausted()).count()
    }
}

pub fn pick_index<T: SlotId>(
    slots: &[T],
    mode: PoolMode,
    active_id: &str,
    share_from: usize,
) -> Option<usize> {
    if slots.is_empty() {
        return None;
    }
    let usable: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| !slot.is_exhausted())
        .map(|(index, _)| index)
        .collect();
    if usable.is_empty() {
        return None;
    }
    match mode {
        PoolMode::QuotaFirst => {
            if let Some(index) = slots.iter().position(|slot| slot.slot_id() == active_id) {
                if !slots[index].is_exhausted() {
                    return Some(index);
                }
            }
            usable.into_iter().next()
        }
        PoolMode::Share => {
            if usable.len() == 1 {
                return Some(usable[0]);
            }
            let start = share_from % slots.len();
            usable
                .iter()
                .copied()
                .find(|index| *index >= start)
                .or_else(|| usable.into_iter().next())
        }
    }
}

pub fn is_quota_error(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("429")
        || text.contains("quota")
        || text.contains("rate limit")
        || text.contains("ratelimit")
        || text.contains("resource_exhausted")
        || text.contains("resource exhausted")
        || text.contains("insufficient")
        || text.contains("usage limit")
        || text.contains("too many requests")
        || text.contains("credit")
        || text.contains("billing")
        || text.contains("exceeded your")
        || text.contains("out of")
}

pub fn normalize_email(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone)]
pub struct PendingOauth {
    pub source: PendingKind,
    pub url: String,
    pub user_code: Option<String>,
    pub hint: String,
}

#[derive(Debug, Clone)]
pub enum PendingKind {
    Grok { login_id: String },
    Xai { device_code: String },
    Portal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Slot {
        id: String,
        exhausted: bool,
    }

    impl SlotId for Slot {
        fn slot_id(&self) -> String {
            self.id.clone()
        }
        fn is_exhausted(&self) -> bool {
            self.exhausted
        }
        fn mark_exhausted(&mut self, _error: Option<String>) {
            self.exhausted = true;
        }
        fn clear_exhausted(&mut self) {
            self.exhausted = false;
        }
    }

    fn slot(id: &str, exhausted: bool) -> Slot {
        Slot {
            id: id.into(),
            exhausted,
        }
    }

    #[test]
    fn recover_if_all_exhausted_unsticks_active() {
        let mut pool = AccountPool {
            mode: PoolMode::QuotaFirst,
            active_id: "b@x.com".into(),
            share_cursor: 0,
            slots: vec![slot("a@x.com", true), slot("b@x.com", true)],
        };
        let recovered = pool.recover_if_all_exhausted().expect("recover");
        assert_eq!(recovered.slot_id(), "b@x.com");
        assert!(!pool.slots[1].exhausted);
        assert!(pool.pick().is_some());
    }

    #[test]
    fn quota_first_stays_on_active_until_exhausted() {
        let slots = vec![slot("a@x.com", false), slot("b@x.com", false)];
        assert_eq!(
            pick_index(&slots, PoolMode::QuotaFirst, "b@x.com", 0),
            Some(1)
        );
        let slots = vec![slot("a@x.com", false), slot("b@x.com", true)];
        assert_eq!(
            pick_index(&slots, PoolMode::QuotaFirst, "b@x.com", 0),
            Some(0)
        );
    }

    #[test]
    fn share_walks_usable_accounts() {
        let mut pool = AccountPool {
            mode: PoolMode::Share,
            active_id: "a@x.com".into(),
            share_cursor: 0,
            slots: vec![
                slot("a@x.com", false),
                slot("b@x.com", false),
                slot("c@x.com", true),
            ],
        };
        assert_eq!(pool.pick().unwrap().id, "a@x.com");
        assert_eq!(pool.pick().unwrap().id, "b@x.com");
        assert_eq!(pool.pick().unwrap().id, "a@x.com");
    }

    #[test]
    fn upsert_replaces_same_email() {
        let mut pool = AccountPool::default();
        pool.upsert(slot("a@x.com", false));
        pool.upsert(slot("a@x.com", true));
        assert_eq!(pool.slots.len(), 1);
        assert!(pool.slots[0].exhausted);
        assert_eq!(pool.active_id, "a@x.com");
    }

    #[test]
    fn quota_error_detects_http_429_and_billing() {
        assert!(is_quota_error("provider HTTP 429: rate limit"));
        assert!(is_quota_error("You exceeded your current quota"));
        assert!(!is_quota_error("ERROR_NOT_HIGH_ENOUGH_PERMISSIONS"));
        assert!(!is_quota_error("model you chose is not available"));
    }
}
