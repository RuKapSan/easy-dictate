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
            // Попробуем быстро переподключиться к стримингу и сразу открыть gate
            log::warn!("[Hotkey] ElevenLabs selected but streaming NOT connected. Attempting quick reconnect...");

            let api_key = settings.elevenlabs_api_key.trim().to_string();
            if api_key.is_empty() {
                log::warn!("[Hotkey] ElevenLabs API key is empty; falling back to standard recording.");
            } else {
                // Используем встроенную команду подключения (определит реальный sample rate)
                let state_clone = state.clone();
                let connect_res = tauri::async_runtime::block_on(
                    crate::core::commands::elevenlabs_streaming_connect(
                        app.clone(),
                        state_clone,
                        api_key,
                        48_000,
                        "auto".to_string(),
                    )
                );

                match connect_res {
                    Ok(()) => {
                        // Проверяем состояние и открываем gate
                        let reconnected = tauri::async_runtime::block_on(
                            state.elevenlabs_streaming().is_connected()
                        );
                        if reconnected {
                            log::info!("[Hotkey] Reconnected. Opening gate...");
                            if let Err(e) = tauri::async_runtime::block_on(
                                state.elevenlabs_streaming().open_gate()
                            ) {
                                emit_error(app, &format!("Failed to open gate after reconnect: {}", e));
                            } else {
                                emit_status(app, StatusPhase::Recording, Some("Streaming..."));
                                return Ok(());
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("[Hotkey] Failed to reconnect ElevenLabs streaming: {}", err);
                    }
                }
            }

            log::warn!("[Hotkey] Falling back to standard recording.");
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
            // Если не было аудио, не отправляем commit и не переходим в Processing
            let had_audio = tauri::async_runtime::block_on(
                state.elevenlabs_streaming().has_audio_since_open()
            );

            if !had_audio {
                log::info!("[Hotkey] No audio since gate opened; closing gate without commit");
                let _ = tauri::async_runtime::block_on(
                    state.elevenlabs_streaming().close_gate()
                );
                emit_status(app, StatusPhase::Idle, Some("Ready for next transcription"));
            } else {
                // Gated streaming режим - закрываем gate и отправляем commit
                log::info!("[Hotkey] ElevenLabs gated streaming - closing gate and committing");
                if let Err(e) = tauri::async_runtime::block_on(
                    state.elevenlabs_streaming().close_gate_and_commit()
                ) {
                    emit_error(app, &format!("Failed to close gate: {}", e));
                } else {
                    emit_status(app, StatusPhase::Transcribing, Some("Processing..."));
                }
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
