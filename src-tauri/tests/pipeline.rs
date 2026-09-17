//! End-to-end tests over the fixture files in `tests/fixtures/`: parse ->
//! store ingest (dedup) -> window computation. Hand-computed expected sums
//! are documented inline next to each fixture's contents.

use ai_status_lib::agents::{agent_for, AgentId};
use ai_status_lib::scan::incremental::ScanCursors;
use ai_status_lib::usage::blocks::compute_snapshot;
use ai_status_lib::usage::model::UsageWindowKind;
use ai_status_lib::usage::pricing::{estimate_cost, PricingCache};
use ai_status_lib::usage::store::UsageStore;
use chrono::DateTime;

fn fixture_path(rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures")).join(rel)
}

#[test]
fn claude_code_dedups_and_blocks_correctly() {
    let path = fixture_path("claude/projects/demo-project/session1.jsonl");
    let agent = agent_for(AgentId::ClaudeCode);
    let mut cursors = ScanCursors::new();

    // The fixture has 4 lines; line 3 is a byte-for-byte duplicate of line 1
    // (simulating Claude Code rewriting a message on a resumed session).
    let raw_events = agent.parse_incremental(&path, &mut cursors);
    assert_eq!(
        raw_events.len(),
        4,
        "parser should emit one event per valid line, dedup happens at the store"
    );

    let store = UsageStore::new();
    store.ingest(AgentId::ClaudeCode, raw_events);
    let events = store.events_for(AgentId::ClaudeCode);
    assert_eq!(
        events.len(),
        3,
        "store should collapse the duplicate dedup_key"
    );

    let total_input: u64 = events.iter().map(|e| e.tokens.input_tokens).sum();
    let total_output: u64 = events.iter().map(|e| e.tokens.output_tokens).sum();
    assert_eq!(total_input, 100 + 200 + 50);
    assert_eq!(total_output, 50 + 100 + 20);

    // Re-scanning the same file with the same cursors should yield nothing new.
    let more = agent.parse_incremental(&path, &mut cursors);
    assert!(more.is_empty());

    // now = 2026-09-16T16:30:00Z: the 4th line (16:00Z) is >5h after the
    // first block's start (10:00Z floor), so it opens a second block.
    let now = DateTime::parse_from_rfc3339("2026-09-16T16:30:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let five_hour = compute_snapshot(AgentId::ClaudeCode, UsageWindowKind::FiveHour, &events, now);
    assert_eq!(
        five_hour.totals.input_tokens, 50,
        "latest block should only contain the 16:00Z event"
    );
    assert_eq!(five_hour.totals.output_tokens, 20);
    assert!(five_hour.is_active);
    assert_eq!(
        five_hour.window_start_ms,
        DateTime::parse_from_rfc3339("2026-09-16T16:00:00Z")
            .unwrap()
            .timestamp_millis()
    );

    let daily = compute_snapshot(AgentId::ClaudeCode, UsageWindowKind::Daily, &events, now);
    assert_eq!(
        daily.totals.input_tokens,
        100 + 200 + 50,
        "all events fall within the trailing 24h"
    );
    assert_eq!(daily.totals.output_tokens, 50 + 100 + 20);
}

#[test]
fn codex_keeps_latest_cumulative_snapshot_per_turn() {
    let path = fixture_path("codex/sessions/2026/09/16/rollout-sess-a.jsonl");
    let agent = agent_for(AgentId::Codex);
    let mut cursors = ScanCursors::new();

    let raw_events = agent.parse_incremental(&path, &mut cursors);
    // 3 token_count lines parsed (2 for turn t1, 1 for t2); the 4th line is a
    // non-token_count event and must be skipped entirely.
    assert_eq!(raw_events.len(), 3);

    let store = UsageStore::new();
    store.ingest(AgentId::Codex, raw_events);
    let events = store.events_for(AgentId::Codex);
    // t1's two cumulative snapshots collapse to one (latest wins); t2 adds one more.
    assert_eq!(events.len(), 2);

    let total_input: u64 = events.iter().map(|e| e.tokens.input_tokens).sum();
    let total_output: u64 = events.iter().map(|e| e.tokens.output_tokens).sum();
    assert_eq!(
        total_input,
        120 + 30,
        "t1 should use its latest cumulative value (120), not 50+120"
    );
    assert_eq!(total_output, 60 + 10);

    let now = DateTime::parse_from_rfc3339("2026-09-16T11:30:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let five_hour = compute_snapshot(AgentId::Codex, UsageWindowKind::FiveHour, &events, now);
    assert_eq!(five_hour.totals.input_tokens, 150);
    assert_eq!(five_hour.totals.output_tokens, 70);
    assert!(five_hour.is_active);
}

#[test]
fn fee_calculator_estimates_cost_across_priced_and_unpriced_events() {
    // Claude Code side: one event with a known model and no first-party
    // cost (priced via the reference table), one with an explicit cost_usd
    // (used as-is regardless of its model's table price), one with no model
    // at all (can't be priced, must flag the total as incomplete).
    let claude_path = fixture_path("claude/projects/pricing-project/session1.jsonl");
    let claude_agent = agent_for(AgentId::ClaudeCode);
    let mut claude_cursors = ScanCursors::new();
    let claude_events = claude_agent.parse_incremental(&claude_path, &mut claude_cursors);
    assert_eq!(claude_events.len(), 3);

    let cache = PricingCache::new();
    let claude_result = estimate_cost(&claude_events, &cache);
    // claude-sonnet-4-5: 1_000_000 input @ $3/M + 200_000 output @ $15/M = 3.0 + 3.0
    // claude-opus-4-1 event carries its own cost_usd (2.5), used verbatim
    // no-model event contributes nothing and flips `incomplete`
    assert!((claude_result.total_usd - (3.0 + 3.0 + 2.5)).abs() < 1e-9);
    assert!(claude_result.incomplete);

    // Codex side: the model arrives on an earlier `session_meta` line and
    // must still apply to a later `token_count` line that doesn't repeat it.
    let codex_path = fixture_path("codex/sessions/2026/09/16/rollout-sess-pricing.jsonl");
    let codex_agent = agent_for(AgentId::Codex);
    let mut codex_cursors = ScanCursors::new();
    let codex_events = codex_agent.parse_incremental(&codex_path, &mut codex_cursors);
    assert_eq!(codex_events.len(), 1);
    assert_eq!(codex_events[0].model.as_deref(), Some("gpt-5-2025-08-07"));

    let codex_result = estimate_cost(&codex_events, &cache);
    // gpt-5: 1_000_000 input @ $1.25/M + 100_000 output @ $10/M = 1.25 + 1.0
    assert!((codex_result.total_usd - (1.25 + 1.0)).abs() < 1e-9);
    assert!(!codex_result.incomplete);
}
