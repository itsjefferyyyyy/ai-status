use super::model::UsageEvent;
use crate::agents::AgentId;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

const RETENTION_DAYS: i64 = 8;

/// In-memory source of truth for all agents' usage events, keyed by the
/// parser-assigned `dedup_key`. Ingesting an event with an existing key
/// *replaces* the stored one rather than skipping it: Claude Code's keys are
/// genuinely unique per message so this is a no-op there, but Codex's
/// `token_count` events are cumulative per-turn snapshots, so the "last one
/// wins" replace semantics are required to end up with each turn's final
/// total rather than its first (stale) one.
pub struct UsageStore {
    events: RwLock<HashMap<AgentId, HashMap<String, UsageEvent>>>,
}

impl Default for UsageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageStore {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
        }
    }

    pub fn ingest(&self, agent: AgentId, new_events: Vec<UsageEvent>) {
        let mut map = self.events.write().unwrap();
        let entry = map.entry(agent).or_default();
        for ev in new_events {
            entry.insert(ev.dedup_key.clone(), ev);
        }
        let cutoff = Utc::now() - Duration::days(RETENTION_DAYS);
        entry.retain(|_, ev| ev.timestamp >= cutoff);
    }

    pub fn events_for(&self, agent: AgentId) -> Vec<UsageEvent> {
        self.events
            .read()
            .unwrap()
            .get(&agent)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }
}
