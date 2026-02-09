use anyhow::anyhow;
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_updater::UpdaterExt;

mod audio;
mod audio_stream;
mod core;
mod elevenlabs;
mod elevenlabs_handler;
mod elevenlabs_streaming;
mod groq;
mod groq_llm;
mod input;
mod openai;
mod settings;

use core::{
    commands,
    events::{emit_error, emit_status, StatusPhase},
    hotkey,
    state::AppState,
    tray,
};
use settings::SettingsStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: Some("logs.log".into()) }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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

            // Check if app should start minimized:
            // 1. Command line args --autostart or --minimized (for autostart plugin)
            // 2. Setting: start_minimized is true
            let has_autostart_arg = std::env::args().any(|arg| arg == "--autostart" || arg == "--minimized");
            let should_start_minimized = has_autostart_arg || initial.start_minimized;

            tracing::info!("[Setup] Start minimized: {} (args: {}, setting: {})",
                       should_start_minimized, has_autostart_arg, initial.start_minimized);

            if let Some(window) = handle.get_webview_window("main") {
                if !should_start_minimized {
                    if let Err(e) = window.show() {
                        tracing::warn!("[Setup] Failed to show main window: {}", e);
                    }
                    if let Err(e) = window.unminimize() {
                        tracing::warn!("[Setup] Failed to unminimize main window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        tracing::warn!("[Setup] Failed to set focus on main window: {}", e);
                    }
                } else {
                    // Keep window hidden when starting minimized
                    if let Err(e) = window.hide() {
                        tracing::warn!("[Setup] Failed to hide main window: {}", e);
                    }
                    tracing::info!("[Setup] Window hidden (start minimized enabled)");
                }
            }

            // Initialize overlay window: keep hidden, set click-through
            // Overlay will be shown and positioned on the correct monitor when recording starts
            if let Some(overlay) = handle.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
                tracing::info!("[Setup] Overlay window initialized (hidden until recording)");
            }

            if let Err(e) = commands::apply_autostart(handle, initial.auto_start) {
                tracing::warn!("[Setup] Failed to apply autostart setting: {}", e);
            }
            // Handle hotkey registration failure gracefully (e.g., when another instance is running)
            // This allows the app to start but won't have hotkey functionality
            if let Err(e) = hotkey::rebind_hotkey(handle, &initial) {
                tracing::warn!("[Setup] Failed to register hotkey (another instance may be running): {}", e);
                // Emit an error to let the user know
                emit_error(handle, &format!("Hotkey registration failed: {}. Close other instances and restart.", e));
            }
            emit_status(handle, StatusPhase::Idle, None);

            let status_item = tray::install_tray(handle)?;
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(mut guard) = state.tray_status_item().lock() {
                    *guard = Some(status_item);
                }
            }

            // Setup ElevenLabs streaming event handlers
            elevenlabs_handler::setup_elevenlabs_event_handlers(handle);
            elevenlabs_handler::setup_elevenlabs_error_handlers(handle);

            // Log where file logs are stored
            if let Ok(log_dir) = resolver.app_log_dir() {
                tracing::info!("[Log] File logging enabled: {}", log_dir.join("logs.log").display());
            }

            // Check for updates on app start (background task) - if enabled in settings
            if initial.auto_update {
                let update_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tracing::info!("[Updater] Checking for updates...");
                    match update_handle.updater_builder().build() {
                    Ok(updater) => match updater.check().await {
                        Ok(Some(update)) => {
                            let new_version = update.version.to_string();
                            tracing::info!("[Updater] Update available: {} -> {}", update.current_version, new_version);

                            // Notify frontend that update is available
                            let _ = update_handle.emit("update://available", &new_version);

                            // Auto-download and install the update
                            match update.download_and_install(|chunk, total| {
                                tracing::debug!("[Updater] Downloaded {} of {} bytes", chunk, total.unwrap_or(0));
                            }, || {
                                tracing::info!("[Updater] Download finished, installing...");
                            }).await {
                                Ok(_) => {
                                    tracing::info!("[Updater] Update installed successfully. Restart required.");
                                    let _ = update_handle.emit("update://installed", &new_version);
                                }
                                Err(e) => tracing::error!("[Updater] Failed to download/install update: {}", e),
                            }
                        }
                        Ok(None) => tracing::info!("[Updater] App is up to date"),
                        Err(e) => tracing::warn!("[Updater] Failed to check for updates: {}", e),
                    },
                    Err(e) => tracing::error!("[Updater] Failed to build updater: {}", e),
                }
                });
            } else {
                tracing::info!("[Updater] Auto-update disabled in settings");
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
                if let Err(e) = window.hide() {
                    tracing::warn!("[Window] Failed to hide window on close request: {}", e);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            core::commands::get_settings,
            core::commands::save_settings,
            core::commands::ping,
            core::commands::get_app_version,
            core::commands::toggle_auto_translate,
            core::commands::frontend_log,
            core::commands::elevenlabs_streaming_connect,
            core::commands::elevenlabs_streaming_disconnect,
            core::commands::elevenlabs_streaming_open_gate,
            core::commands::elevenlabs_streaming_close_gate,
            core::commands::elevenlabs_streaming_send_chunk,
            core::commands::elevenlabs_streaming_is_connected,
            core::commands::show_overlay_no_focus,
            core::commands::check_for_updates,
            // History commands
            core::commands::get_history,
            core::commands::clear_history,
            core::commands::delete_history_entry,
            // Test mode commands
            core::commands::inject_test_audio,
            core::commands::get_test_state,
            core::commands::simulate_hotkey_press,
            core::commands::simulate_hotkey_release,
            core::commands::show_main_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                // keep running until the user explicitly quits from the tray menu
            }
        });
}
