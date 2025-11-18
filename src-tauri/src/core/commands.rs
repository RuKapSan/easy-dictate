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

// ============================================================================
// ElevenLabs Gated Streaming Commands
// ============================================================================

#[tauri::command]
pub async fn elevenlabs_streaming_connect(
    app: AppHandle,
    state: State<'_, AppState>,
    api_key: String,
    sample_rate: u32,
    language_code: String,
) -> Result<(), String> {
    // 1. Connect to WebSocket
    state
        .elevenlabs_streaming()
        .connect(api_key, sample_rate, language_code, app.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 2. Spawn dedicated thread for audio streaming (CPAL Stream is !Send)
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let cancel_clone = cancel_token.clone();
    let streaming_client = state.elevenlabs_streaming().clone();

    std::thread::spawn(move || {
        use crate::audio_stream::ContinuousAudioCapture;

        // Create audio capture on this thread
        let mut audio_capture = match ContinuousAudioCapture::new() {
            Ok(capture) => capture,
            Err(e) => {
                log::error!("[AudioStreaming] Failed to create audio capture: {}", e);
                return;
            }
        };

        // Start audio capture
        let audio_rx = match audio_capture.start() {
            Ok(rx) => rx,
            Err(e) => {
                log::error!("[AudioStreaming] Failed to start audio capture: {}", e);
                return;
            }
        };

        let sample_rate = audio_capture.sample_rate();
        log::info!("[AudioStreaming] Audio capture started: {} Hz", sample_rate);

        // Create tokio runtime for async operations on this thread
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("[AudioStreaming] Failed to create runtime: {}", e);
                return;
            }
        };

        // Run streaming task
        rt.block_on(async move {
            audio_streaming_task(audio_rx, audio_capture, streaming_client, cancel_clone).await;
        });
    });

    // Store cancellation token
    let mut cancel_guard = state
        .audio_streaming_cancel()
        .lock()
        .map_err(|_| "Failed to lock cancel token".to_string())?;
    *cancel_guard = Some(cancel_token);

    log::info!("[Commands] ElevenLabs streaming connected and audio pipeline started");

    Ok(())
}

/// Background task that manages audio capture and forwards chunks to ElevenLabs WebSocket
async fn audio_streaming_task(
    mut audio_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    mut audio_capture: crate::audio_stream::ContinuousAudioCapture,
    streaming_client: crate::elevenlabs_streaming::ElevenLabsStreamingClient,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    log::info!("[AudioStreaming] Task started");

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[AudioStreaming] Task cancelled, stopping audio capture");
                let _ = audio_capture.stop();
                break;
            }
            chunk = audio_rx.recv() => {
                match chunk {
                    Some(pcm_data) => {
                        // Send chunk to streaming client (will check gate internally)
                        if let Err(e) = streaming_client.send_audio_chunk(pcm_data).await {
                            log::error!("[AudioStreaming] Failed to send chunk: {}", e);
                            // If connection is dead or other fatal error, stop the loop
                            if e.to_string().contains("Connection is dead") || e.to_string().contains("closed") {
                                break;
                            }
                        }
                    }
                    None => {
                        log::info!("[AudioStreaming] Audio stream ended");
                        break;
                    }
                }
            }
        }
    }

    log::info!("[AudioStreaming] Task finished");
}

#[tauri::command]
pub async fn elevenlabs_streaming_disconnect(
    state: State<'_, AppState>,
) -> Result<(), String> {
    log::info!("[Commands] Disconnecting ElevenLabs streaming...");

    // 1. Cancel audio streaming task (which will stop audio capture)
    if let Ok(mut cancel_guard) = state.audio_streaming_cancel().lock() {
        if let Some(token) = cancel_guard.take() {
            token.cancel();
            log::info!("[Commands] Audio streaming task cancelled");
        }
    }

    // 2. Disconnect WebSocket
    state
        .elevenlabs_streaming()
        .disconnect()
        .await
        .map_err(|e| e.to_string())?;

    log::info!("[Commands] ElevenLabs streaming disconnected");

    Ok(())
}

#[tauri::command]
pub async fn elevenlabs_streaming_open_gate(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .elevenlabs_streaming()
        .open_gate()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn elevenlabs_streaming_close_gate(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .elevenlabs_streaming()
        .close_gate_and_commit()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn elevenlabs_streaming_send_chunk(
    state: State<'_, AppState>,
    pcm_data: Vec<u8>,
) -> Result<(), String> {
    state
        .elevenlabs_streaming()
        .send_audio_chunk(pcm_data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn elevenlabs_streaming_is_connected(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    Ok(state.elevenlabs_streaming().is_connected().await)
}

#[tauri::command]
pub async fn show_overlay_no_focus(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    
    if let Some(window) = app.get_webview_window("overlay") {
        // Ensure click-through is enabled before showing
        // window.set_ignore_cursor_events(true).map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE, SWP_SHOWWINDOW
            };

            if let Ok(hwnd) = window.hwnd() {
                let hwnd = HWND(hwnd.0 as _);
                log::info!("[Commands] Showing overlay without focus (HWND: {:?})", hwnd);
                unsafe {
                    // Show window without activating and ensure it's top most
                    let _ = SetWindowPos(
                        hwnd, 
                        Some(HWND_TOPMOST), 
                        0, 0, 0, 0, 
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW
                    );
                }
            } else {
                log::warn!("[Commands] Failed to get HWND for overlay, falling back to standard show");
                window.show().map_err(|e| e.to_string())?;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            window.show().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
