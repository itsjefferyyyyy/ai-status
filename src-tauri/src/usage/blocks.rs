use super::model::{TokenTotals, UsageEvent, UsageSnapshot, UsageWindowKind};
use crate::agents::AgentId;
use chrono::{DateTime, Duration, Timelike, Utc};

pub fn compute_snapshot(
    agent: AgentId,
    window: UsageWindowKind,
    events: &[UsageEvent],
    now: DateTime<Utc>,
) -> UsageSnapshot {
    match window {
        UsageWindowKind::FiveHour => compute_five_hour_block(agent, events, now),
        UsageWindowKind::Daily | UsageWindowKind::Weekly => {
            compute_rolling_window(agent, window, events, now)
        }
    }
}

/// Mirrors ccusage's 5-hour billing block semantics: a block starts at the
/// floor-to-hour of its first event, runs 5 hours, and a new block begins
/// whenever either the block itself would exceed 5 hours or more than 5 hours
/// have elapsed since the previous event (a "gap" block). A block is active
/// while its most recent event is under 5 hours old and the block hasn't
/// reached its end time yet.
fn compute_five_hour_block(
    agent: AgentId,
    events: &[UsageEvent],
    now: DateTime<Utc>,
) -> UsageSnapshot {
    let mut sorted: Vec<&UsageEvent> = events.iter().filter(|e| e.agent == agent).collect();
    sorted.sort_by_key(|e| e.timestamp);

    struct Block {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        last_event: DateTime<Utc>,
        totals: TokenTotals,
    }

    let mut blocks: Vec<Block> = Vec::new();
    for ev in &sorted {
        let needs_new_block = match blocks.last() {
            None => true,
            Some(b) => {
                (ev.timestamp - b.start) > Duration::hours(5)
                    || (ev.timestamp - b.last_event) > Duration::hours(5)
            }
        };
        if needs_new_block {
            let start = floor_to_hour(ev.timestamp);
            blocks.push(Block {
                start,
                end: start + Duration::hours(5),
                last_event: ev.timestamp,
                totals: TokenTotals::default(),
            });
        }
        let b = blocks.last_mut().expect("just pushed if needed");
        b.totals.add(&ev.tokens);
        b.last_event = ev.timestamp;
    }

    match blocks.last() {
        None => UsageSnapshot {
            agent,
            window: UsageWindowKind::FiveHour,
            totals: TokenTotals::default(),
            window_start_ms: now.timestamp_millis(),
            window_end_ms: None,
            is_active: false,
            limit: None,
            percent: None,
            estimated_cost_usd: None,
            cost_incomplete: false,
            updated_at_ms: now.timestamp_millis(),
        },
        Some(b) => {
            let is_active = (now - b.last_event) < Duration::hours(5) && now < b.end;
            UsageSnapshot {
                agent,
                window: UsageWindowKind::FiveHour,
                totals: b.totals,
                window_start_ms: b.start.timestamp_millis(),
                window_end_ms: Some(b.end.timestamp_millis()),
                is_active,
                limit: None,
                percent: None,
                estimated_cost_usd: None,
                cost_incomplete: false,
                updated_at_ms: now.timestamp_millis(),
            }
        }
    }
}

/// Daily/Weekly are simple trailing sums (last 24h / last 7d), not calendar
/// periods — Anthropic's actual reset schedule for these windows isn't
/// officially documented, so a trailing window avoids implying a specific
/// reset time we can't back up. The UI should label Weekly as "last 7 days".
fn compute_rolling_window(
    agent: AgentId,
    window: UsageWindowKind,
    events: &[UsageEvent],
    now: DateTime<Utc>,
) -> UsageSnapshot {
    let start = now - window.duration();
    let mut totals = TokenTotals::default();
    for ev in events.iter().filter(|e| e.agent == agent) {
        if ev.timestamp >= start && ev.timestamp <= now {
            totals.add(&ev.tokens);
        }
    }
    UsageSnapshot {
        agent,
        window,
        totals,
        window_start_ms: start.timestamp_millis(),
        window_end_ms: None,
        is_active: true,
        limit: None,
        percent: None,
        estimated_cost_usd: None,
        cost_incomplete: false,
        updated_at_ms: now.timestamp_millis(),
    }
}

fn floor_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(dt)
}
