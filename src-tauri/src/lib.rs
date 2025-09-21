use anyhow::anyhow;
use tauri::{Manager, RunEvent};

mod audio;
mod core;
mod groq;
mod groq_llm;
mod input;
mod openai;
mod settings;

use core::{
    commands,
    events::{emit_status, StatusPhase},
    hotkey,
    state::AppState,
    tray,
};
use settings::SettingsStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle();

            let resolver = handle.path();
            let config_dir = resolver
                .app_config_dir()
                .map_err(|err| anyhow!("Failed to locate application config directory: {err}"))?;
            let store = SettingsStore::new(config_dir);
            let initial = tauri::async_runtime::block_on(store.load())?;
            let state = AppState::new(store, initial.clone())?;
            app.manage(state);

            if let Some(window) = handle.get_webview_window("main") {
                window.show().ok();
                window.unminimize().ok();
                window.set_focus().ok();
            }

            commands::apply_autostart(handle, initial.auto_start).ok();
            hotkey::rebind_hotkey(handle, &initial)?;
            emit_status(handle, StatusPhase::Idle, None);

            let status_item = tray::install_tray(handle)?;
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(mut guard) = state.tray_status_item().lock() {
                    *guard = Some(status_item);
                }
            }

            handle.on_menu_event(|app_handle, event| match event.id().as_ref() {
                "open" => tray::show_settings_window(app_handle),
                "quit" => app_handle.exit(0),
                _ => {}
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        })
        .invoke_handler(tauri::generate_handler![
            core::commands::get_settings,
            core::commands::save_settings,
            core::commands::ping,
            core::commands::frontend_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                // keep running until the user explicitly quits from the tray menu
            }
        });
}
