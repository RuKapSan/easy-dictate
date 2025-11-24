use anyhow::anyhow;
use tauri::{Manager, RunEvent};
use tauri_plugin_log::{Target, TargetKind};

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
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: Some("logs.log".into()) }),
                ])
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

            // Check if app is starting with the system (autostart)
            let is_autostart = std::env::args().any(|arg| arg == "--autostart" || arg == "--minimized");

            if let Some(window) = handle.get_webview_window("main") {
                if !is_autostart {
                    if let Err(e) = window.show() {
                        log::warn!("[Setup] Failed to show main window: {}", e);
                    }
                    if let Err(e) = window.unminimize() {
                        log::warn!("[Setup] Failed to unminimize main window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::warn!("[Setup] Failed to set focus on main window: {}", e);
                    }
                } else {
                    // Keep window hidden when starting with the system
                    if let Err(e) = window.hide() {
                        log::warn!("[Setup] Failed to hide main window: {}", e);
                    }
                }
            }
            
            // Initialize overlay window: visible but transparent, click-through, no focus
            if let Some(overlay) = handle.get_webview_window("overlay") {
                // 1. Enable click-through
                let _ = overlay.set_ignore_cursor_events(true);

                // 2. Show without focus (Windows)
                #[cfg(target_os = "windows")]
                {
                    use windows::Win32::Foundation::HWND;
                    use windows::Win32::UI::WindowsAndMessaging::{
                        SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE, SWP_SHOWWINDOW
                    };

                    if let Ok(hwnd) = overlay.hwnd() {
                        let hwnd = HWND(hwnd.0 as _);
                        // SAFETY: SetWindowPos is called with a valid HWND obtained from Tauri's
                        // window handle. The hwnd is guaranteed valid as long as the window exists.
                        // We're only modifying window position flags (topmost, show without activate),
                        // which is safe and doesn't affect memory or cause undefined behavior.
                        // The SWP_NOMOVE | SWP_NOSIZE flags ensure position/size aren't changed.
                        unsafe {
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOPMOST),
                                0, 0, 0, 0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW
                            );
                        }
                    }
                }
                
                // Non-windows fallback
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = overlay.show();
                }
            }

            if let Err(e) = commands::apply_autostart(handle, initial.auto_start) {
                log::warn!("[Setup] Failed to apply autostart setting: {}", e);
            }
            hotkey::rebind_hotkey(handle, &initial)?;
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
                log::info!("[Log] File logging enabled: {}", log_dir.join("logs.log").display());
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
                    log::warn!("[Window] Failed to hide window on close request: {}", e);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            core::commands::get_settings,
            core::commands::save_settings,
            core::commands::ping,
            core::commands::frontend_log,
            core::commands::elevenlabs_streaming_connect,
            core::commands::elevenlabs_streaming_disconnect,
            core::commands::elevenlabs_streaming_open_gate,
            core::commands::elevenlabs_streaming_close_gate,
            core::commands::elevenlabs_streaming_send_chunk,
            core::commands::elevenlabs_streaming_is_connected,
            core::commands::show_overlay_no_focus,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                // keep running until the user explicitly quits from the tray menu
            }
        });
}
