use super::{AgentId, UsageAgent};
use crate::scan::incremental::ScanCursors;
use crate::usage::model::{TokenTotals, UsageEvent};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Reads Claude Code's local session logs. Default location is
/// `~/.claude/projects/<project-slug>/<session-id>.jsonl`, overridable via
/// `CLAUDE_CONFIG_DIR` (a comma-separated list of base dirs, each expected to
/// contain a `projects/` subfolder) — the same locations and layout the
/// open-source `ccusage` tool reads. No login/API key is involved.
pub struct ClaudeCodeAgent;

#[derive(Debug, Deserialize)]
struct RawEntry {
    session_id: Option<String>,
    timestamp: Option<String>,
    message: Option<RawMessage>,
    cost_usd: Option<f64>,
    request_id: Option<String>,
    #[serde(default, rename = "isApiErrorMessage")]
    is_api_error_message: bool,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    id: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

impl UsageAgent for ClaudeCodeAgent {
    fn id(&self) -> AgentId {
        AgentId::ClaudeCode
    }

    fn log_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Ok(dirs) = std::env::var("CLAUDE_CONFIG_DIR") {
            for d in dirs.split(',') {
                let d = d.trim();
                if !d.is_empty() {
                    roots.push(PathBuf::from(d).join("projects"));
                }
            }
        }
        if roots.is_empty() {
            if let Some(home) = dirs::home_dir() {
                roots.push(home.join(".claude").join("projects"));
            }
        }
        roots
    }

    fn discover_files(&self, _max_age_days: i64) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for root in self.log_roots() {
            let Ok(project_dirs) = std::fs::read_dir(&root) else {
                continue;
            };
            for project in project_dirs.flatten() {
                let path = project.path();
                if !path.is_dir() {
                    continue;
                }
                let Ok(entries) = std::fs::read_dir(&path) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        files.push(p);
                    }
                }
            }
        }
        files
    }

    fn parse_incremental(&self, path: &Path, cursors: &mut ScanCursors) -> Vec<UsageEvent> {
        let Ok(lines) = cursors.read_new_lines(path) else {
            return Vec::new();
        };

        let mut events = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(raw) = serde_json::from_str::<RawEntry>(line) else {
                continue;
            };
            if raw.is_api_error_message {
                continue;
            }
            let Some(message) = &raw.message else {
                continue;
            };
            let Some(usage) = &message.usage else {
                continue;
            };
            let Some(ts_str) = &raw.timestamp else {
                continue;
            };
            let Ok(timestamp) = DateTime::parse_from_rfc3339(ts_str) else {
                continue;
            };

            // Claude Code can rewrite the same assistant message across a
            // resumed/compacted session, so dedup on the combination of
            // message id, request id, and session id rather than summing
            // every line verbatim.
            let dedup_key = format!(
                "claude:{}:{}:{}",
                message.id.clone().unwrap_or_default(),
                raw.request_id.clone().unwrap_or_default(),
                raw.session_id.clone().unwrap_or_default(),
            );

            events.push(UsageEvent {
                agent: AgentId::ClaudeCode,
                timestamp: timestamp.with_timezone(&Utc),
                tokens: TokenTotals {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_creation_tokens: usage.cache_creation_input_tokens,
                    cache_read_tokens: usage.cache_read_input_tokens,
                },
                cost_usd: raw.cost_usd,
                dedup_key,
            });
        }
        events
    }
}
