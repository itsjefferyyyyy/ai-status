use crate::agents::{agent_for, AgentId};
use crate::scan::incremental::ScanCursors;
use crate::settings::{self, AgentSettings, LimitMode, Settings};
use crate::usage::blocks::compute_snapshot;
use crate::usage::limits;
use crate::usage::model::{UsageSnapshot, UsageWindowKind};
use crate::usage::store::UsageStore;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const MAX_AGE_DAYS: i64 = 8;
const POLL_INTERVAL_SECS: u64 = 60;

pub struct PollState {
    pub store: UsageStore,
    cursors: Mutex<HashMap<AgentId, ScanCursors>>,
}

impl PollState {
    pub fn new() -> Self {
        Self {
            store: UsageStore::new(),
            cursors: Mutex::new(HashMap::new()),
        }
    }
}

/// Re-scans enabled agents' logs (skipping disabled agents' I/O entirely),
/// recomputes snapshots for the visible windows, and emits `usage://updated`.
/// Shared by both the 60s background loop and the `get_usage` command's
/// immediate refresh-on-open.
pub fn refresh(app: &AppHandle, state: &PollState, settings: &Settings) -> Vec<UsageSnapshot> {
    {
        let mut cursors_guard = state.cursors.lock().unwrap();
        for id in AgentId::ALL {
            let Some(agent_settings) = settings.agents.get(&id) else {
                continue;
            };
            if !agent_settings.enabled {
                continue;
            }

            let agent = agent_for(id);
            let cursors = cursors_guard.entry(id).or_default();
            let files = agent.discover_files(MAX_AGE_DAYS);
            let mut new_events = Vec::new();
            for file in files {
                new_events.extend(agent.parse_incremental(&file, cursors));
            }
            if !new_events.is_empty() {
                state.store.ingest(id, new_events);
            }
        }
    }

    let now = Utc::now();
    let mut snapshots = Vec::new();
    for id in AgentId::ALL {
        let Some(agent_settings) = settings.agents.get(&id) else {
            continue;
        };
        if !agent_settings.enabled {
            continue;
        }
        let events = state.store.events_for(id);
        for window in UsageWindowKind::ALL {
            if !*settings.windows_visible.get(&window).unwrap_or(&false) {
                continue;
            }
            let mut snap = compute_snapshot(id, window, &events, now);
            apply_limit(&mut snap, agent_settings, window);
            snapshots.push(snap);
        }
    }

    let _ = app.emit("usage://updated", &snapshots);
    update_tray_title(app, state, settings, now);
    snapshots
}

const TRAY_ID: &str = "main-tray";

/// The tray title always reflects the primary agent's 5-hour window,
/// independent of `windows_visible` — it's a quick-glance surface, not a
/// second copy of the popover.
fn update_tray_title(
    app: &AppHandle,
    state: &PollState,
    settings: &Settings,
    now: chrono::DateTime<Utc>,
) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    let primary = settings.primary_agent;
    let Some(agent_settings) = settings.agents.get(&primary) else {
        let _ = tray.set_title(None::<&str>);
        return;
    };
    if !agent_settings.enabled {
        let _ = tray.set_title(None::<&str>);
        return;
    }

    let events = state.store.events_for(primary);
    let mut snap = compute_snapshot(primary, UsageWindowKind::FiveHour, &events, now);
    apply_limit(&mut snap, agent_settings, UsageWindowKind::FiveHour);

    let value = match snap.percent {
        Some(pct) => format!("{:.0}%", pct),
        None => format_compact_tokens(snap.totals.total()),
    };
    let _ = tray.set_title(Some(format!("{} · {}", primary.tray_letter(), value)));
}

fn format_compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn apply_limit(snap: &mut UsageSnapshot, agent_settings: &AgentSettings, window: UsageWindowKind) {
    let limit = match agent_settings.limit_mode {
        LimitMode::None => None,
        LimitMode::Custom => agent_settings.custom_limit,
        LimitMode::Auto => agent_settings
            .plan_tier
            .as_deref()
            .and_then(|tier| limits::limit_for(snap.agent, tier, window)),
    };
    snap.percent = limit.map(|l| {
        if l == 0 {
            0.0
        } else {
            (snap.totals.total() as f64 / l as f64) * 100.0
        }
    });
    snap.limit = limit;
}

pub fn spawn_poll_loop(app: AppHandle, state: Arc<PollState>) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let current_settings = settings::load(&app);
            refresh(&app, &state, &current_settings);
        }
    });
}
