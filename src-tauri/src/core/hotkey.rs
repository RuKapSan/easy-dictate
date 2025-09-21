use std::sync::atomic::Ordering;

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcut, ShortcutState};

use crate::{audio::RecordingSession, settings::AppSettings};

use super::{
    events::{emit_error, emit_status, StatusPhase},
    state::AppState,
    transcription,
};

pub fn rebind_hotkey(app: &AppHandle, settings: &AppSettings) -> Result<()> {
    let shortcuts: State<'_, GlobalShortcut<tauri::Wry>> = app.state();
    shortcuts.unregister_all().ok();

    let hotkey = settings.normalized_hotkey();
    let hotkey_clone = hotkey.clone();
    shortcuts
        .on_shortcut(
            hotkey.as_str(),
            move |app_handle, _shortcut, event| match event.state {
                ShortcutState::Pressed => {
                    if let Err(err) = handle_hotkey_pressed(app_handle) {
                        emit_error(app_handle, &err.to_string());
                    }
                }
                ShortcutState::Released => {
                    if let Err(err) = handle_hotkey_released(app_handle) {
                        emit_error(app_handle, &err.to_string());
                    }
                }
            },
        )
        .map_err(|err| anyhow!("Failed to register hotkey {hotkey_clone}: {err}"))?;

    Ok(())
}

pub fn handle_hotkey_pressed(app: &AppHandle) -> Result<()> {
    let state: State<'_, AppState> = app.state();
    if state.is_transcribing().load(Ordering::SeqCst) {
        emit_status(
            app,
            StatusPhase::Transcribing,
            Some("Already transcribing, please wait."),
        );
        return Ok(());
    }

    let mut guard = state
        .active_recording()
        .lock()
        .map_err(|_| anyhow!("Failed to lock active recording state"))?;

    if guard.is_some() {
        return Ok(());
    }

    match state.recorder().start() {
        Ok(active) => {
            emit_status(app, StatusPhase::Recording, Some("Recording..."));
            *guard = Some(active);
        }
        Err(err) => emit_error(app, &err.to_string()),
    }

    Ok(())
}

pub fn handle_hotkey_released(app: &AppHandle) -> Result<()> {
    let state: State<'_, AppState> = app.state();
    let active: Option<RecordingSession> = {
        let mut guard = state
            .active_recording()
            .lock()
            .map_err(|_| anyhow!("Failed to lock active recording state"))?;
        guard.take()
    };

    let Some(active) = active else {
        return Ok(());
    };

    match active.stop() {
        Ok(audio_wav) => {
            if state.is_transcribing().swap(true, Ordering::SeqCst) {
                return Ok(());
            }

            emit_status(app, StatusPhase::Transcribing, Some("Uploading audio..."));
            transcription::spawn_transcription(app, audio_wav);
        }
        Err(err) => emit_error(app, &err.to_string()),
    }

    Ok(())
}
