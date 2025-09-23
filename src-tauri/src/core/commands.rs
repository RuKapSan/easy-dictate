use tauri::{AppHandle, State};

use crate::settings::AppSettings;

use super::{
    events::{emit_error, emit_status, StatusPhase},
    hotkey,
    state::AppState,
};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.current_settings().await.normalized())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let normalized = settings.normalized();
    normalized.validate().map_err(|err| err.to_string())?;

    state
        .persist_settings(&normalized)
        .await
        .map_err(|err| err.to_string())?;
    state.replace_settings(normalized.clone()).await;

    if let Err(err) = apply_autostart(&app, normalized.auto_start) {
        emit_error(&app, &format!("Autostart update failed: {err}"));
    }

    hotkey::rebind_hotkey(&app, &normalized).map_err(|err| err.to_string())?;

    emit_status(
        &app,
        StatusPhase::Idle,
        Some("Settings saved. Ready for the next transcription."),
    );

    Ok(())
}

#[tauri::command]
pub async fn ping() -> Result<&'static str, String> {
    Ok("pong")
}

#[tauri::command]
pub async fn frontend_log(level: Option<String>, message: String) -> Result<(), String> {
    let lvl = level.as_deref().unwrap_or("info");
    match lvl {
        "error" => log::error!("[frontend] {}", message),
        "warn" => log::warn!("[frontend] {}", message),
        "debug" => log::debug!("[frontend] {}", message),
        "trace" => log::trace!("[frontend] {}", message),
        _ => log::info!("[frontend] {}", message),
    }
    Ok(())
}

pub(crate) fn apply_autostart(app: &AppHandle, should_enable: bool) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        let _ = (app, should_enable);
        log::debug!("Skipping autostart toggle in debug builds");
        Ok(())
    }

    #[cfg(not(debug_assertions))]
    {
        use tauri_plugin_autostart::ManagerExt;

        let manager = app.autolaunch();

        if should_enable {
            // Set custom args for autostart
            if let Ok(_exe_path) = std::env::current_exe() {
                std::env::set_var("TAURI_AUTOSTART_ARGS", "--autostart");
            }
            manager.enable().map_err(|err| err.to_string())?;
        } else {
            manager.disable().map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}
