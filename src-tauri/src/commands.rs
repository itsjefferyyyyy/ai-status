use crate::agents::AgentId;
use crate::poll::{refresh, PollState};
use crate::settings::{self, Settings};
use crate::usage::limits;
use crate::usage::model::UsageSnapshot;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
pub struct PlanTierOption {
    key: String,
    label: String,
}

#[tauri::command]
pub fn list_plan_tiers(agent: AgentId) -> Vec<PlanTierOption> {
    limits::plan_tiers_for(agent)
        .iter()
        .map(|t| PlanTierOption {
            key: t.key.to_string(),
            label: t.label.to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn get_usage(app: AppHandle, state: State<Arc<PollState>>) -> Vec<UsageSnapshot> {
    let current_settings = settings::load(&app);
    refresh(&app, &state, &current_settings)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, new_settings: Settings) -> Result<(), String> {
    settings::save(&app, &new_settings)?;
    let _ = app.emit("settings-changed", &new_settings);
    Ok(())
}

const MAIN_WINDOW_WIDTH: f64 = 340.0;
const MIN_MAIN_WINDOW_HEIGHT: f64 = 90.0;
const MAX_MAIN_WINDOW_HEIGHT: f64 = 640.0;

/// Called by the popover whenever its content height changes (agents/windows
/// toggled, data arrives), so the window tracks its content instead of
/// carrying fixed empty space. Re-anchors to the tray afterwards since
/// resizing shifts a top-left-anchored window away from tray-center.
#[tauri::command]
pub fn resize_main_window(app: AppHandle, height: f64) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let clamped = height.clamp(MIN_MAIN_WINDOW_HEIGHT, MAX_MAIN_WINDOW_HEIGHT);
    window
        .set_size(tauri::LogicalSize::new(MAIN_WINDOW_WIDTH, clamped))
        .map_err(|e| e.to_string())?;

    use tauri_plugin_positioner::{Position, WindowExt};
    let _ = window.move_window(Position::TrayCenter);
    Ok(())
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    // Same SPA entry point as the "main" window — Popover vs. Settings is
    // decided client-side by reading the current window's label, avoiding
    // any dependency on SvelteKit sub-route prerendering/fallback behavior
    // inside the Tauri asset server.
    tauri::WebviewWindowBuilder::new(&app, "settings", tauri::WebviewUrl::App("/".into()))
        .title("AI Status Settings")
        .inner_size(420.0, 480.0)
        .resizable(true)
        .visible(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
