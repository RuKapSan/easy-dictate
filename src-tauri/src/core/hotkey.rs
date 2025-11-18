use std::sync::atomic::Ordering;

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcut, ShortcutState};

use crate::{audio::RecordingSession, settings::{AppSettings, TranscriptionProvider}};

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

    // Получаем настройки для проверки провайдера
    let settings = tauri::async_runtime::block_on(state.current_settings());

    // Проверяем - используем ли gated streaming для ElevenLabs
    log::info!("[Hotkey] Pressed. Provider: {:?}, Streaming connected: {}", settings.provider, tauri::async_runtime::block_on(state.elevenlabs_streaming().is_connected()));

    if settings.provider == TranscriptionProvider::ElevenLabs {
        let is_streaming_connected = tauri::async_runtime::block_on(
            state.elevenlabs_streaming().is_connected()
        );

        if is_streaming_connected {
            // Gated streaming режим - просто открываем gate
            log::info!("[Hotkey] ElevenLabs gated streaming - opening gate");
            if let Err(e) = tauri::async_runtime::block_on(
                state.elevenlabs_streaming().open_gate()
            ) {
                emit_error(app, &format!("Failed to open gate: {}", e));
            } else {
                emit_status(app, StatusPhase::Recording, Some("Streaming..."));
            }
            return Ok(());
        } else {
            log::warn!("[Hotkey] ElevenLabs provider selected but streaming NOT connected. Falling back to standard recording.");
        }
    }

    // Старый режим - recording
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

    // Получаем настройки для проверки провайдера
    let settings = tauri::async_runtime::block_on(state.current_settings());

    // Проверяем - используем ли gated streaming для ElevenLabs
    if settings.provider == TranscriptionProvider::ElevenLabs {
        let is_streaming_connected = tauri::async_runtime::block_on(
            state.elevenlabs_streaming().is_connected()
        );

        if is_streaming_connected {
            // Gated streaming режим - закрываем gate и отправляем commit
            log::info!("[Hotkey] ElevenLabs gated streaming - closing gate and committing");
            if let Err(e) = tauri::async_runtime::block_on(
                state.elevenlabs_streaming().close_gate_and_commit()
            ) {
                emit_error(app, &format!("Failed to close gate: {}", e));
            } else {
                emit_status(app, StatusPhase::Transcribing, Some("Processing..."));
            }
            return Ok(());
        }
    }

    // Старый режим - recording
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
