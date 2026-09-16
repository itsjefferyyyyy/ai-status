use super::{AgentId, UsageAgent};
use crate::scan::incremental::ScanCursors;
use crate::usage::model::{TokenTotals, UsageEvent};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Reads Codex CLI's local session logs, stored as
/// `~/.codex/sessions/YYYY/MM/DD/rollout-<session-id>.jsonl` (and a sibling
/// `archived_sessions/` tree for rotated-out sessions), overridable via
/// `CODEX_HOME` (comma-separated base dirs). No login/API key is involved.
///
/// Codex's `token_count` events carry *cumulative* per-turn totals rather
/// than deltas, so — unlike Claude Code — summing every line would overcount.
/// The dedup key here is `(session file, turn id)`; combined with
/// `UsageStore`'s replace-on-existing-key semantics, only the latest
/// (largest) cumulative snapshot per turn is kept.
///
/// Field names have drifted across Codex versions (`prompt_tokens` vs
/// `input_tokens`, etc.), so token fields are looked up by trying a short
/// list of known aliases rather than a single fixed schema. The exact set of
/// keys/nesting should be re-verified against real `~/.codex/sessions` output
/// during development (see the plan's verification step comparing against
/// `ccusage`'s own Codex adapter).
pub struct CodexAgent;

const MAX_SCAN_DEPTH_DAYS_DEFAULT: i64 = 8;

impl UsageAgent for CodexAgent {
    fn id(&self) -> AgentId {
        AgentId::Codex
    }

    fn log_roots(&self) -> Vec<PathBuf> {
        let mut bases = Vec::new();
        if let Ok(dirs) = std::env::var("CODEX_HOME") {
            for d in dirs.split(',') {
                let d = d.trim();
                if !d.is_empty() {
                    bases.push(PathBuf::from(d));
                }
            }
        }
        if bases.is_empty() {
            if let Some(home) = dirs::home_dir() {
                bases.push(home.join(".codex"));
            }
        }

        let mut roots = Vec::new();
        for base in bases {
            roots.push(base.join("sessions"));
            roots.push(base.join("archived_sessions"));
        }
        roots
    }

    fn discover_files(&self, max_age_days: i64) -> Vec<PathBuf> {
        let max_age_days = if max_age_days > 0 {
            max_age_days
        } else {
            MAX_SCAN_DEPTH_DAYS_DEFAULT
        };
        let cutoff_date = (Utc::now() - chrono::Duration::days(max_age_days)).date_naive();
        let mut files = Vec::new();
        for root in self.log_roots() {
            walk_dated_tree(&root, cutoff_date, &mut files);
        }
        files
    }

    fn parse_incremental(&self, path: &Path, cursors: &mut ScanCursors) -> Vec<UsageEvent> {
        let Ok(lines) = cursors.read_new_lines(path) else {
            return Vec::new();
        };

        let session_key = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown-session")
            .to_string();

        let mut events = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(payload) = v.get("payload") else {
                continue;
            };
            if payload.get("type").and_then(Value::as_str) != Some("token_count") {
                continue;
            }

            let turn_id = payload
                .get("turn_id")
                .or_else(|| payload.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("0")
                .to_string();

            // Token fields may be nested under `info`/`last_token_usage`/
            // `total_token_usage`, or sit directly on the payload.
            let usage_obj = payload
                .get("info")
                .or_else(|| payload.get("last_token_usage"))
                .or_else(|| payload.get("total_token_usage"))
                .unwrap_or(payload);

            let tokens = TokenTotals {
                input_tokens: extract_u64(usage_obj, &["input_tokens", "prompt_tokens"]),
                output_tokens: extract_u64(usage_obj, &["output_tokens", "completion_tokens"]),
                cache_creation_tokens: extract_u64(usage_obj, &["cache_creation_input_tokens"]),
                cache_read_tokens: extract_u64(
                    usage_obj,
                    &["cached_input_tokens", "cache_read_input_tokens"],
                ),
            };

            if tokens.total() == 0 {
                continue;
            }

            let timestamp = v
                .get("timestamp")
                .and_then(parse_timestamp)
                .unwrap_or_else(Utc::now);

            events.push(UsageEvent {
                agent: AgentId::Codex,
                timestamp,
                tokens,
                cost_usd: None,
                dedup_key: format!("codex:{session_key}:{turn_id}"),
            });
        }
        events
    }
}

fn extract_u64(v: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        if let Some(n) = v.get(*key).and_then(Value::as_u64) {
            return n;
        }
    }
    0
}

fn parse_timestamp(v: &Value) -> Option<DateTime<Utc>> {
    if let Some(s) = v.as_str() {
        return DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc));
    }
    if let Some(n) = v.as_i64() {
        return Utc.timestamp_millis_opt(n).single();
    }
    None
}

/// Walks a `root/YYYY/MM/DD/*.jsonl` tree, skipping date directories older
/// than `cutoff_date` so a full 8-day scan doesn't have to touch years of
/// history every poll tick.
fn walk_dated_tree(root: &Path, cutoff_date: NaiveDate, out: &mut Vec<PathBuf>) {
    let Ok(years) = std::fs::read_dir(root) else {
        return;
    };
    for year_entry in years.flatten() {
        let year_path = year_entry.path();
        let Ok(months) = std::fs::read_dir(&year_path) else {
            continue;
        };
        for month_entry in months.flatten() {
            let month_path = month_entry.path();
            let Ok(days) = std::fs::read_dir(&month_path) else {
                continue;
            };
            for day_entry in days.flatten() {
                let day_path = day_entry.path();
                if let Some(date) = parse_date_dir(&year_path, &month_path, &day_path) {
                    if date < cutoff_date {
                        continue;
                    }
                }
                let Ok(files) = std::fs::read_dir(&day_path) else {
                    continue;
                };
                for file_entry in files.flatten() {
                    let p = file_entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        out.push(p);
                    }
                }
            }
        }
    }
}

fn parse_date_dir(year_dir: &Path, month_dir: &Path, day_dir: &Path) -> Option<NaiveDate> {
    let y: i32 = year_dir.file_name()?.to_str()?.parse().ok()?;
    let m: u32 = month_dir.file_name()?.to_str()?.parse().ok()?;
    let d: u32 = day_dir.file_name()?.to_str()?.parse().ok()?;
    NaiveDate::from_ymd_opt(y, m, d)
}
