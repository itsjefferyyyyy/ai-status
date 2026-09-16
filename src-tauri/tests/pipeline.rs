//! End-to-end tests over the fixture files in `tests/fixtures/`: parse ->
//! store ingest (dedup) -> window computation. Hand-computed expected sums
//! are documented inline next to each fixture's contents.

use ai_status_lib::agents::{agent_for, AgentId};
use ai_status_lib::scan::incremental::ScanCursors;
use ai_status_lib::usage::blocks::compute_snapshot;
use ai_status_lib::usage::model::UsageWindowKind;
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
