pub mod claude_code;
pub mod codex;

use crate::scan::incremental::ScanCursors;
use crate::usage::model::UsageEvent;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentId {
    ClaudeCode,
    Codex,
}

impl AgentId {
    pub const ALL: [AgentId; 2] = [AgentId::ClaudeCode, AgentId::Codex];

    pub fn label(&self) -> &'static str {
        match self {
            AgentId::ClaudeCode => "Claude Code",
            AgentId::Codex => "Codex CLI",
        }
    }

    /// MVP stand-in for a real per-agent icon in the menu bar — a single
    /// letter, since we don't have licensed Anthropic/OpenAI logo assets.
    pub fn tray_letter(&self) -> &'static str {
        match self {
            AgentId::ClaudeCode => "C",
            AgentId::Codex => "X",
        }
    }
}

/// One module per agent. Deliberately not a dynamic plugin registry: adding a
/// new agent (e.g. ChatGPT, once it has a viable data source) means adding one
/// file here plus one match arm in `agent_for`, nothing more.
pub trait UsageAgent: Send + Sync {
    fn id(&self) -> AgentId;

    /// Root directories that may contain this agent's session logs, honoring
    /// the agent's env var override (comma-separated list of base dirs).
    fn log_roots(&self) -> Vec<PathBuf>;

    /// Enumerate candidate log files, bounded to roughly the last `max_age_days`
    /// where the on-disk layout makes that cheap (e.g. Codex's dated folders).
    fn discover_files(&self, max_age_days: i64) -> Vec<PathBuf>;

    /// Parse only the bytes appended since the last call for this path,
    /// tracked via `cursors`. Returns normalized, deduped-by-key usage events.
    fn parse_incremental(&self, path: &Path, cursors: &mut ScanCursors) -> Vec<UsageEvent>;
}

pub fn agent_for(id: AgentId) -> Box<dyn UsageAgent> {
    match id {
        AgentId::ClaudeCode => Box::new(claude_code::ClaudeCodeAgent),
        AgentId::Codex => Box::new(codex::CodexAgent),
    }
}
