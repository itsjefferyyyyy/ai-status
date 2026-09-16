pub mod agents;
mod commands;
mod poll;
pub mod scan;
mod settings;
pub mod usage;

use poll::PollState;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_usage,
            commands::get_settings,
            commands::update_settings,
            commands::open_settings_window,
            commands::list_plan_tiers,
            commands::resize_main_window,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let state = Arc::new(PollState::new());
            app.manage(state.clone());

            let settings_item =
                MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        let _ = commands::open_settings_window(app.clone());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let Some(window) = tray.app_handle().get_webview_window("main") else {
                            return;
                        };
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            use tauri_plugin_positioner::{Position, WindowExt};
                            let _ = window.move_window(Position::TrayCenter);
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            if let Some(main_window) = app.get_webview_window("main") {
                let win = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = win.hide();
                    }
                });
            }

            // Populate the tray title immediately on launch instead of
            // waiting up to 60s for the first background tick.
            let initial_settings = settings::load(app.handle());
            poll::refresh(app.handle(), &state, &initial_settings);

            poll::spawn_poll_loop(app.handle().clone(), state);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
