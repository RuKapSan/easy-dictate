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
    if let Err(e) = shortcuts.unregister_all() {
        log::warn!("[Hotkey] Failed to unregister shortcuts: {}", e);
    }

    // Register main hotkey (respects auto_translate setting)
    let hotkey = settings.normalized_hotkey();
    let hotkey_clone = hotkey.clone();
    shortcuts
        .on_shortcut(
            hotkey.as_str(),
            move |app_handle, _shortcut, event| match event.state {
                ShortcutState::Pressed => {
                    handle_hotkey_pressed(app_handle, false);
                }
                ShortcutState::Released => {
                    handle_hotkey_released(app_handle);
                }
            },
        )
        .map_err(|err| anyhow!("Failed to register hotkey {hotkey_clone}: {err}"))?;

    // Register translate hotkey (forces translation ON for this session)
    if !settings.translate_hotkey.is_empty() {
        let translate_hotkey = settings.translate_hotkey.trim().to_string();
        let translate_hotkey_clone = translate_hotkey.clone();
        shortcuts
            .on_shortcut(
                translate_hotkey.as_str(),
                move |app_handle, _shortcut, event| match event.state {
                    ShortcutState::Pressed => {
                        handle_hotkey_pressed(app_handle, true);
                    }
                    ShortcutState::Released => {
                        handle_hotkey_released(app_handle);
                    }
                },
            )
            .map_err(|err| anyhow!("Failed to register translate hotkey {translate_hotkey_clone}: {err}"))?;
        log::info!("[Hotkey] Registered translate hotkey: {}", translate_hotkey);
    }

    // Register toggle translate hotkey
    if !settings.toggle_translate_hotkey.is_empty() {
        let toggle_hotkey = settings.toggle_translate_hotkey.trim().to_string();
        let toggle_hotkey_clone = toggle_hotkey.clone();
        shortcuts
            .on_shortcut(
                toggle_hotkey.as_str(),
                move |app_handle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        handle_toggle_translate_hotkey(app_handle);
                    }
                },
            )
            .map_err(|err| anyhow!("Failed to register toggle translate hotkey {toggle_hotkey_clone}: {err}"))?;
        log::info!("[Hotkey] Registered toggle translate hotkey: {}", toggle_hotkey);
    }

    Ok(())
}

/// Handle hotkey press event - spawns async task to avoid blocking the event thread
/// force_translate: if true, translation will be forced ON regardless of settings
pub fn handle_hotkey_pressed(app: &AppHandle, force_translate: bool) {
    let app_clone = app.clone();

    // Spawn async task to handle the press without blocking
    tauri::async_runtime::spawn(async move {
        if let Err(err) = handle_hotkey_pressed_async(&app_clone, force_translate).await {
            emit_error(&app_clone, &err.to_string());
        }
    });
}

/// Handle toggle translate hotkey - toggles auto_translate setting
pub fn handle_toggle_translate_hotkey(app: &AppHandle) {
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let state: State<'_, AppState> = app_clone.state();
        let mut settings = state.current_settings().await;
        settings.auto_translate = !settings.auto_translate;

        // Persist the toggle
        if let Err(e) = state.persist_settings(&settings).await {
            emit_error(&app_clone, &format!("Failed to save settings: {}", e));
            return;
        }
        state.replace_settings(settings.clone()).await;

        log::info!("[Toggle Hotkey] Auto-translate now: {}", settings.auto_translate);

        // Emit status update
        let message = if settings.auto_translate {
            "Translation enabled"
        } else {
            "Translation disabled"
        };
        emit_status(&app_clone, StatusPhase::Idle, Some(message));
    });
}

/// Async implementation of hotkey press handling
async fn handle_hotkey_pressed_async(app: &AppHandle, force_translate: bool) -> Result<()> {
    let state: State<'_, AppState> = app.state();

    // Store force_translate flag for this transcription session
    state.set_force_translate(force_translate);

    // Get settings once at the beginning
    let settings = state.current_settings().await;
    let is_streaming_connected = state.elevenlabs_streaming().is_connected().await;

    log::info!(
        "[Hotkey] Pressed. Provider: {:?}, Streaming connected: {}",
        settings.provider,
        is_streaming_connected
    );

    if settings.provider == TranscriptionProvider::ElevenLabs {
        let is_committing = state.elevenlabs_streaming().is_committing().await;

        if !is_streaming_connected || is_committing {
            log::info!(
                "[Hotkey] Preparing clean session (connected: {}, committing: {})",
                is_streaming_connected,
                is_committing
            );

            let mut connected = false;

            // Try to reconnect using last config (including audio stream restart)
            if let Some((api_key, sample_rate, language_code)) =
                state.elevenlabs_streaming().get_last_config().await
            {
                log::info!(
                    "[Hotkey] Reconnecting with last config: rate={}, lang={}",
                    sample_rate,
                    language_code
                );
                connected = crate::core::commands::elevenlabs_streaming_connect(
                    app.clone(),
                    state.clone(),
                    api_key,
                    sample_rate,
                    language_code,
                )
                .await
                .is_ok();
            }

            // Fallback to settings if no last config or reconnection failed
            if !connected {
                let api_key = settings.elevenlabs_api_key.trim().to_string();
                if api_key.is_empty() {
                    log::warn!(
                        "[Hotkey] ElevenLabs API key is empty; falling back to standard recording."
                    );
                } else {
                    connected = crate::core::commands::elevenlabs_streaming_connect(
                        app.clone(),
                        state.clone(),
                        api_key,
                        48_000,
                        "auto".to_string(),
                    )
                    .await
                    .is_ok();
                }
            }

            if connected {
                log::info!("[Hotkey] Clean session ready. Opening gate...");
                if let Err(e) = state.elevenlabs_streaming().open_gate().await {
                    emit_error(app, &format!("Failed to open gate: {}", e));
                } else {
                    emit_status(app, StatusPhase::Recording, Some("Streaming..."));
                }
                return Ok(());
            }
            // else fall through to legacy recording
        } else {
            // Already connected and not committing: open gate
            log::info!("[Hotkey] ElevenLabs gated streaming - opening gate");
            if let Err(e) = state.elevenlabs_streaming().open_gate().await {
                emit_error(app, &format!("Failed to open gate: {}", e));
            } else {
                emit_status(app, StatusPhase::Recording, Some("Streaming..."));
            }
            return Ok(());
        }
    }

    // Legacy recording mode

    // For Mock provider in test mode, skip real recording
    // Tests use inject_test_audio() to provide audio data directly
    if settings.provider == TranscriptionProvider::Mock {
        log::info!("[Hotkey] Mock provider - skipping real microphone recording");
        emit_status(app, StatusPhase::Recording, Some("Mock recording (test mode)..."));
        return Ok(());
    }

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

/// Handle hotkey release event - spawns async task for streaming, sync for legacy recording
pub fn handle_hotkey_released(app: &AppHandle) {
    let state: State<'_, AppState> = app.state();

    // For legacy recording mode, we need to stop the recording synchronously
    // to capture the audio data before it's lost
    let active: Option<RecordingSession> = {
        if let Ok(mut guard) = state.active_recording().lock() {
            guard.take()
        } else {
            None
        }
    };

    if let Some(active) = active {
        // Handle legacy recording stop synchronously
        match active.stop() {
            Ok(audio_wav) => {
                if state.is_transcribing().swap(true, Ordering::SeqCst) {
                    return;
                }
                emit_status(app, StatusPhase::Transcribing, Some("Uploading audio..."));
                transcription::spawn_transcription(app, audio_wav);
            }
            Err(err) => emit_error(app, &err.to_string()),
        }
        return;
    }

    // For ElevenLabs streaming, spawn async task
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = handle_hotkey_released_async(&app_clone).await {
            emit_error(&app_clone, &err.to_string());
        }
    });
}

/// Async implementation of hotkey release handling for streaming mode
async fn handle_hotkey_released_async(app: &AppHandle) -> Result<()> {
    let state: State<'_, AppState> = app.state();
    let settings = state.current_settings().await;

    // Check if using gated streaming for ElevenLabs
    if settings.provider == TranscriptionProvider::ElevenLabs {
        let is_streaming_connected = state.elevenlabs_streaming().is_connected().await;

        if is_streaming_connected {
            // If no audio was captured, don't send commit
            let had_audio = state.elevenlabs_streaming().has_audio_since_open().await;

            if !had_audio {
                log::info!("[Hotkey] No audio since gate opened; closing gate without commit");
                let _ = state.elevenlabs_streaming().close_gate().await;
                emit_status(app, StatusPhase::Idle, Some("Ready for next transcription"));
            } else {
                // Gated streaming mode - close gate and send commit
                log::info!("[Hotkey] ElevenLabs gated streaming - closing gate and committing");

                // Emit processing status BEFORE waiting for commit
                emit_status(app, StatusPhase::Transcribing, Some("Processing..."));

                if let Err(e) = state.elevenlabs_streaming().close_gate_and_commit().await {
                    emit_error(app, &format!("Failed to close gate: {}", e));
                    emit_status(app, StatusPhase::Idle, Some("Ready for next transcription"));
                }
            }
        }
    }

    Ok(())
}
