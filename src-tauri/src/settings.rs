use crate::agents::AgentId;
use crate::usage::model::UsageWindowKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitMode {
    /// Show raw token counts only, no percentage.
    None,
    /// Estimate a percentage against the built-in plan-tier table (usage/limits.rs).
    Auto,
    /// Estimate a percentage against a user-entered limit (`custom_limit`).
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub enabled: bool,
    pub limit_mode: LimitMode,
    /// Which built-in tier to use when `limit_mode` is Auto (see usage/limits.rs keys).
    pub plan_tier: Option<String>,
    /// User-entered token limit when `limit_mode` is Custom.
    pub custom_limit: Option<u64>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            limit_mode: LimitMode::None,
            plan_tier: None,
            custom_limit: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub agents: HashMap<AgentId, AgentSettings>,
    pub windows_visible: HashMap<UsageWindowKind, bool>,
    /// Which agent's 5-hour usage is shown directly in the menu bar tray
    /// title (independent of `windows_visible` — the tray always shows the
    /// 5-hour figure regardless of which windows the popover displays).
    pub primary_agent: AgentId,
}

impl Default for Settings {
    fn default() -> Self {
        let agents = AgentId::ALL
            .into_iter()
            .map(|id| (id, AgentSettings::default()))
            .collect();

        let mut windows_visible = HashMap::new();
        windows_visible.insert(UsageWindowKind::FiveHour, true);
        windows_visible.insert(UsageWindowKind::Daily, true);
        windows_visible.insert(UsageWindowKind::Weekly, false);

        Self {
            agents,
            windows_visible,
            primary_agent: AgentId::ClaudeCode,
        }
    }
}

const STORE_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

pub fn load(app: &AppHandle) -> Settings {
    let Ok(store) = app.store(STORE_FILE) else {
        return Settings::default();
    };
    store
        .get(SETTINGS_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    store.set(SETTINGS_KEY, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
