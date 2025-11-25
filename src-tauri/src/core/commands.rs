use tauri::{AppHandle, State};

use crate::settings::AppSettings;

use super::{
    events::{emit_error, emit_status, StatusPhase},
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
    // 500 ~= -36dB (conservative)
    // 1000 ~= -30dB
    const NOISE_THRESHOLD: f32 = 500.0;

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

                        // Apply noise gate
                        if rms < NOISE_THRESHOLD {
                            // Silence the chunk
                            pcm_data.fill(0);
                        }

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
