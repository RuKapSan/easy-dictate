use tauri::{AppHandle, State};

use crate::settings::AppSettings;

use super::{
    events::{emit_error, emit_settings_changed, emit_status, StatusPhase},
    hotkey,
    state::{AppState, AudioStreamingHandle},
};
use cpal::traits::{DeviceTrait, HostTrait};

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
pub async fn toggle_auto_translate(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut settings = state.current_settings().await;
    settings.auto_translate = !settings.auto_translate;

    // Persist the toggle
    state
        .persist_settings(&settings)
        .await
        .map_err(|err| err.to_string())?;
    state.replace_settings(settings.clone()).await;

    log::info!("[Toggle] Auto-translate now: {} (target: {})", settings.auto_translate, settings.target_language);

    // Emit settings changed event for UI sync
    emit_settings_changed(&app, settings.auto_translate, &settings.target_language);

    // Emit status update with target language info
    let message = if settings.auto_translate {
        format!("Перевод ВКЛ → {}", settings.target_language)
    } else {
        "Перевод ВЫКЛ".to_string()
    };
    emit_status(&app, StatusPhase::Idle, Some(&message));

    Ok(settings.auto_translate)
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
            // Note: TAURI_AUTOSTART_ARGS is read at compile time by tauri-plugin-autostart,
            // so setting it at runtime has no effect. The --autostart flag is configured
            // in tauri.conf.json or via the plugin's builder instead.
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
    // Determine actual input device sample rate to avoid mismatches with server format
    let actual_sample_rate = {
        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            match device.default_input_config() {
                Ok(cfg) => cfg.sample_rate().0,
                Err(_) => sample_rate,
            }
        } else {
            sample_rate
        }
    };

    if actual_sample_rate != sample_rate {
        log::info!(
            "[Commands] Overriding requested sample rate {} Hz with device rate {} Hz",
            sample_rate,
            actual_sample_rate
        );
    }

    // 1. Connect to WebSocket using the actual device sample rate
    state
        .elevenlabs_streaming()
        .connect(api_key, actual_sample_rate, language_code, app.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 2. Stop and wait for any existing audio streaming task to prevent concurrent access
    {
        let mut handle_guard = state.audio_streaming_handle().lock()
            .map_err(|_| "Failed to lock audio streaming handle".to_string())?;

        if let Some(handle) = handle_guard.take() {
            log::info!("[Commands] Stopping existing audio streaming task and waiting for it to finish");
            handle.cancel_token.cancel();
            // Wait for the thread to finish (with timeout to prevent hanging)
            // The thread should exit quickly after cancel_token is cancelled
            match handle.join_handle.join() {
                Ok(()) => log::info!("[Commands] Previous audio streaming task finished cleanly"),
                Err(_) => log::warn!("[Commands] Previous audio streaming task panicked"),
            }
        }
    }

    // 3. Spawn dedicated thread for audio streaming (CPAL Stream is !Send)
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let cancel_clone = cancel_token.clone();
    let streaming_client = state.elevenlabs_streaming().clone();

    let join_handle = std::thread::spawn(move || {
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

    // Store handle for proper cleanup later
    let mut handle_guard = state.audio_streaming_handle().lock()
        .map_err(|_| "Failed to lock audio streaming handle".to_string())?;
    *handle_guard = Some(AudioStreamingHandle {
        cancel_token,
        join_handle,
    });

    log::info!("[Commands] ElevenLabs streaming connected and audio pipeline started");

    Ok(())
}

/// Background task that manages audio capture and forwards chunks to ElevenLabs WebSocket
async fn audio_streaming_task(
    mut audio_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    mut audio_capture: crate::audio_stream::ContinuousAudioCapture,
    streaming_client: crate::elevenlabs_streaming::ElevenLabsStreamingClient,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    log::info!("[AudioStreaming] Task started");

    // Noise gate threshold (RMS amplitude)
    // PCM16 max is 32767.
    // 100 ~= -50dB (very quiet threshold to not filter out speech)
    // 500 ~= -36dB (too aggressive, filters out normal speech)
    const NOISE_THRESHOLD: f32 = 100.0;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[AudioStreaming] Task cancelled, stopping audio capture");
                let _ = audio_capture.stop();
                break;
            }
            chunk = audio_rx.recv() => {
                match chunk {
                    Some(mut pcm_data) => {
                        // Calculate RMS to check for silence/noise
                        let mut sum_squares = 0.0;
                        let mut sample_count = 0;
                        
                        for chunk in pcm_data.chunks_exact(2) {
                            let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32;
                            sum_squares += sample * sample;
                            sample_count += 1;
                        }

                        let rms = if sample_count > 0 {
                            (sum_squares / sample_count as f32).sqrt()
                        } else {
                            0.0
                        };

                        // Log RMS periodically (every ~1 second = 10 chunks of 100ms)
                        static CHUNK_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                        let count = CHUNK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count % 10 == 0 {
                            log::debug!("[AudioStreaming] RMS level: {:.0}", rms);
                        }

                        // Noise gate temporarily disabled for debugging
                        // TODO: Re-enable after fixing the issue
                        // if rms < NOISE_THRESHOLD {
                        //     // Silence the chunk
                        //     pcm_data.fill(0);
                        // }
                        log::info!("[AudioStreaming] RMS level: {:.0}", rms);

                        // Send chunk to streaming client (will check gate internally)
                        if let Err(e) = streaming_client.send_audio_chunk(pcm_data).await {
                            log::error!("[AudioStreaming] Failed to send chunk: {}", e);
                            // If connection is dead or other fatal error, stop the loop
                            let err_str = e.to_string();
                            if err_str.contains("Connection is dead") || err_str.contains("closed") || err_str.contains("Not connected") {
                                log::info!("[AudioStreaming] Connection closed, stopping audio task");
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

    // 1. Stop audio streaming task and wait for it to finish
    {
        let mut handle_guard = state.audio_streaming_handle().lock()
            .map_err(|_| "Failed to lock audio streaming handle".to_string())?;

        if let Some(handle) = handle_guard.take() {
            log::info!("[Commands] Stopping audio streaming task...");
            handle.cancel_token.cancel();
            // Wait for the thread to finish
            match handle.join_handle.join() {
                Ok(()) => log::info!("[Commands] Audio streaming task stopped cleanly"),
                Err(_) => log::warn!("[Commands] Audio streaming task panicked"),
            }
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

// ============================================================================
// History Commands
// ============================================================================

use super::state::HistoryEntry;

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    Ok(state.get_history().await)
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.clear_history().await;
    Ok(())
}

#[tauri::command]
pub async fn delete_history_entry(
    state: State<'_, AppState>,
    id: u64,
) -> Result<bool, String> {
    Ok(state.delete_history_entry(id).await)
}

// ============================================================================
// Test Mode Commands (for E2E testing without microphone)
// ============================================================================

/// Inject test audio directly into the transcription pipeline
/// This bypasses the microphone completely for reliable E2E testing
/// Accepts raw WAV bytes to avoid Tauri FS scope restrictions
#[tauri::command]
pub async fn inject_test_audio(
    app: AppHandle,
    state: State<'_, AppState>,
    audio_data: Vec<u8>,
) -> Result<String, String> {
    log::info!("[TestMode] Injecting test audio: {} bytes", audio_data.len());

    if audio_data.is_empty() {
        return Err("Audio data is empty".to_string());
    }

    // Validate WAV header (RIFF....WAVE)
    if audio_data.len() < 12 || &audio_data[0..4] != b"RIFF" || &audio_data[8..12] != b"WAVE" {
        return Err("Invalid WAV format: missing RIFF/WAVE header".to_string());
    }

    let audio_wav = audio_data;

    // Get settings and perform transcription directly
    let settings = state.current_settings().await;
    let service = state.transcription();

    // Emit status to UI
    super::events::emit_status(&app, super::events::StatusPhase::Transcribing, Some("Processing test audio..."));

    match service.perform(&settings, audio_wav).await {
        Ok(result) => {
            let trimmed = result.processed.trim().to_string();
            log::info!("[TestMode] Transcription result: {}", trimmed);

            super::events::emit_status(&app, super::events::StatusPhase::Success, None);
            super::events::emit_complete(&app, &trimmed);

            Ok(trimmed)
        }
        Err(err) => {
            log::error!("[TestMode] Transcription failed: {}", err);
            super::events::emit_error(&app, &err.to_string());
            Err(err.to_string())
        }
    }
}

/// Get current app state for testing
#[tauri::command]
pub async fn get_test_state(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use std::sync::atomic::Ordering;

    let is_recording = state.active_recording()
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false);
    let is_transcribing = state.is_transcribing().load(Ordering::SeqCst);
    let settings = state.current_settings().await;

    Ok(serde_json::json!({
        "is_recording": is_recording,
        "is_transcribing": is_transcribing,
        "provider": format!("{:?}", settings.provider),
        "has_api_key": !settings.api_key.is_empty(),
        "hotkey": settings.hotkey
    }))
}

/// Simulate hotkey press for testing (starts recording)
#[tauri::command]
pub async fn simulate_hotkey_press(app: AppHandle) -> Result<(), String> {
    log::info!("[TestMode] Simulating hotkey press");
    super::hotkey::handle_hotkey_pressed(&app, false);
    Ok(())
}

/// Simulate hotkey release for testing (stops recording and triggers transcription)
#[tauri::command]
pub async fn simulate_hotkey_release(app: AppHandle) -> Result<(), String> {
    log::info!("[TestMode] Simulating hotkey release");
    super::hotkey::handle_hotkey_released(&app);
    Ok(())
}

/// Show main window for E2E testing
/// Used when tauri-driver launches the app and the window starts hidden
#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;

    log::info!("[TestMode] Showing main window");
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.unminimize().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

#[tauri::command]
pub async fn show_overlay_no_focus(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("overlay") {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::{HWND, POINT, RECT};
            use windows::Win32::Graphics::Gdi::{MonitorFromPoint, GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTONEAREST};
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, GetCursorPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW
            };

            const OVERLAY_WIDTH: i32 = 600;
            const OVERLAY_HEIGHT: i32 = 150;
            const BOTTOM_MARGIN: i32 = 60;

            if let Ok(hwnd) = window.hwnd() {
                let hwnd = HWND(hwnd.0 as _);

                // Get cursor position and find monitor
                let mut cursor_pos = POINT::default();
                let (x, y) = unsafe {
                    let _ = GetCursorPos(&mut cursor_pos);
                    let monitor = MonitorFromPoint(cursor_pos, MONITOR_DEFAULTTONEAREST);

                    let mut monitor_info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };

                    if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
                        let work_area: RECT = monitor_info.rcWork;
                        let monitor_width = work_area.right - work_area.left;

                        // Center horizontally, position at bottom with margin
                        let x = work_area.left + (monitor_width - OVERLAY_WIDTH) / 2;
                        let y = work_area.bottom - OVERLAY_HEIGHT - BOTTOM_MARGIN;
                        (x, y)
                    } else {
                        // Fallback to primary monitor center-bottom
                        (100, 800)
                    }
                };

                log::info!("[Commands] Positioning overlay at ({}, {}) on monitor with cursor", x, y);

                // SAFETY: SetWindowPos with valid HWND. We're setting position and showing window.
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        x, y,
                        OVERLAY_WIDTH, OVERLAY_HEIGHT,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW
                    );
                }

                window.set_ignore_cursor_events(true).map_err(|e| e.to_string())?;
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
