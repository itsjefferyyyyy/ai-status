use crate::agents::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageWindowKind {
    FiveHour,
    Daily,
    Weekly,
}

impl UsageWindowKind {
    pub const ALL: [UsageWindowKind; 3] = [
        UsageWindowKind::FiveHour,
        UsageWindowKind::Daily,
        UsageWindowKind::Weekly,
    ];

    pub fn duration(&self) -> chrono::Duration {
        match self {
            UsageWindowKind::FiveHour => chrono::Duration::hours(5),
            UsageWindowKind::Daily => chrono::Duration::hours(24),
            UsageWindowKind::Weekly => chrono::Duration::days(7),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

impl TokenTotals {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }

    pub fn add(&mut self, other: &TokenTotals) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
    }
}

/// A single normalized usage record, already deduplicated at the agent-parser
/// level via `dedup_key` (see agents::claude_code / agents::codex for what
/// that key means per agent — the two agents have very different dedup
/// semantics, which is why this key is opaque here).
#[derive(Debug, Clone)]
pub struct UsageEvent {
    pub agent: AgentId,
    pub timestamp: DateTime<Utc>,
    pub tokens: TokenTotals,
    pub cost_usd: Option<f64>,
    pub dedup_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub agent: AgentId,
    pub window: UsageWindowKind,
    pub totals: TokenTotals,
    pub window_start_ms: i64,
    /// Only set for FiveHour, whose block has a real reset time. Daily/Weekly
    /// are rolling trailing windows with no fixed reset, so this is None for
    /// them by design (see usage/blocks.rs).
    pub window_end_ms: Option<i64>,
    pub is_active: bool,
    /// Only present when the agent's limit_mode is Auto or Custom (see settings.rs).
    pub limit: Option<u64>,
    pub percent: Option<f64>,
    pub updated_at_ms: i64,
}
