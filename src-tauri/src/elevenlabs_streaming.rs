use anyhow::{anyhow, Context, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{http::Request, Message},
    MaybeTlsStream, WebSocketStream,
};
use tokio::net::TcpStream;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Структура для активного WebSocket соединения
struct StreamingConnection {
    write: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    is_transmitting: Arc<AtomicBool>,
    sent_since_open: Arc<AtomicBool>,
    is_alive: Arc<AtomicBool>,
    cancel_token: tokio_util::sync::CancellationToken,
    reader_task: tokio::task::JoinHandle<()>,
    keepalive_task: tokio::task::JoinHandle<()>,
    sample_rate: u32,
    audio_format: String,
}

/// Публичный клиент для gated streaming
#[derive(Clone)]
pub struct ElevenLabsStreamingClient {
    connection: Arc<Mutex<Option<StreamingConnection>>>,
}

#[derive(Serialize)]
struct AudioChunkMessage {
    message_type: String,
    audio_base_64: String,
    sample_rate: u32,
    #[serde(default)]
    commit: bool,
}

#[derive(Deserialize, Debug)]
struct TranscriptMessage {
    message_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    session_id: Option<String>,
}

// Tauri event payloads
#[derive(Serialize, Clone)]
struct SessionStartedEvent {
    session_id: String,
}

#[derive(Serialize, Clone)]
struct TranscriptEvent {
    text: String,
    is_partial: bool,
}

#[derive(Serialize, Clone)]
struct ErrorEvent {
    error: String,
}

#[derive(Serialize, Clone)]
struct ConnectionClosedEvent {
    code: u16,
    reason: String,
}

impl ElevenLabsStreamingClient {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns whether any audio has been sent since the last gate open
    pub async fn has_audio_since_open(&self) -> bool {
        if let Some(conn) = self.connection.lock().await.as_ref() {
            conn.sent_since_open.load(Ordering::Relaxed)
        } else {
            false
        }
    }

    /// Подключиться к ElevenLabs WebSocket и начать gated streaming
    pub async fn connect(
        &self,
        api_key: String,
        sample_rate: u32,
        language_code: String,
        app_handle: AppHandle,
    ) -> Result<()> {
        // Проверяем что нет активного соединения
        let mut conn_guard = self.connection.lock().await;
        
        // Check if existing connection is actually alive
        if let Some(conn) = conn_guard.as_ref() {
            if conn.is_alive.load(Ordering::Relaxed) {
                return Err(anyhow!("Connection already exists. Disconnect first."));
            } else {
                // Cleanup dead connection
                log::info!("[ElevenLabs] Cleaning up dead connection before reconnecting");
                *conn_guard = None;
            }
        }

        // Определяем audio format на основе sample rate
        let audio_format = match sample_rate {
            8000 => "pcm_8000",
            16000 => "pcm_16000",
            22050 => "pcm_22050",
            24000 => "pcm_24000",
            44100 => "pcm_44100",
            48000 => "pcm_48000",
            _ => {
                log::warn!("[ElevenLabs] Unsupported sample rate {}, using pcm_16000", sample_rate);
                "pcm_16000"
            }
        };

        let ws_url = if language_code.is_empty() || language_code == "auto" {
            format!(
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime&audio_format={}&commit_strategy=manual&enable_partials=true",
                audio_format
            )
        } else {
            format!(
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime&language_code={}&audio_format={}&commit_strategy=manual&enable_partials=true",
                language_code, audio_format
            )
        };

        log::info!("[ElevenLabs] Connecting to WebSocket (sample_rate: {}, audio_format: {})", sample_rate, audio_format);

        // Создаем HTTP запрос с заголовком xi-api-key
        let request = Request::builder()
            .uri(ws_url)
            .header("Host", "api.elevenlabs.io")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
            .header("xi-api-key", &api_key)
            .body(())
            .context("Failed to build WebSocket request")?;

        let (ws_stream, response) = connect_async(request)
            .await
            .context("Failed to connect to ElevenLabs WebSocket")?;

        log::info!("[ElevenLabs] WebSocket connected successfully, status: {:?}", response.status());

        let (write, read) = ws_stream.split();
        let write = Arc::new(Mutex::new(write));

        // Флаги для gate control
        let is_transmitting = Arc::new(AtomicBool::new(false));
        let sent_since_open = Arc::new(AtomicBool::new(false));
        
        // Flag for connection liveness
        let is_alive = Arc::new(AtomicBool::new(true));

        // Токен для остановки background tasks
        let cancel_token = tokio_util::sync::CancellationToken::new();

        // Запускаем background task для чтения сообщений
        let reader_task = {
            let app_handle = app_handle.clone();
            let cancel_token = cancel_token.clone();
            let is_alive = is_alive.clone();
            let write = write.clone();
            tokio::spawn(async move {
                message_reader_task(read, write, app_handle, cancel_token, is_alive).await;
            })
        };

        // Запускаем background task для keep-alive
        let keepalive_task = {
            let write = write.clone();
            let cancel_token = cancel_token.clone();
            let is_transmitting = is_transmitting.clone();
            let sample_rate_keep = sample_rate;
            tokio::spawn(async move {
                keepalive_task(write, cancel_token, is_transmitting, sample_rate_keep).await;
            })
        };

        // Сохраняем соединение
        *conn_guard = Some(StreamingConnection {
            write,
            is_transmitting,
            sent_since_open,
            is_alive,
            cancel_token,
            reader_task,
            keepalive_task,
            sample_rate,
            audio_format: audio_format.to_string(),
        });

        log::info!("[ElevenLabs] Gated streaming session started");
        Ok(())
    }

    /// Отправить чанк аудио (только если gate открыт)
    pub async fn send_audio_chunk(&self, pcm_data: Vec<u8>) -> Result<()> {
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected. Call connect() first."))?;

        if !conn.is_alive.load(Ordering::Relaxed) {
             return Err(anyhow!("Connection is dead"));
        }

        // Проверяем gate
        if !conn.is_transmitting.load(Ordering::Relaxed) {
            // Gate закрыт - игнорируем аудио
            return Ok(());
        }

        // Gate открыт - отправляем
        conn.sent_since_open.store(true, Ordering::Relaxed);
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&pcm_data);

        let message = AudioChunkMessage {
            message_type: "input_audio_chunk".to_string(),
            audio_base_64: audio_base64,
            sample_rate: conn.sample_rate,
            commit: false,
        };

        let json = serde_json::to_string(&message)?;

        let mut write = conn.write.lock().await;
        write.send(Message::Text(json)).await
            .context("Failed to send audio chunk")?;
        
        Ok(())
    }

    /// Открыть gate - начать передачу (KeyDown)
    pub async fn open_gate(&self) -> Result<()> {
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected"))?;
            
        if !conn.is_alive.load(Ordering::Relaxed) {
             return Err(anyhow!("Connection is dead"));
        }

        conn.sent_since_open.store(false, Ordering::Relaxed);
        conn.is_transmitting.store(true, Ordering::Relaxed);
        log::info!("[ElevenLabs] Gate OPENED - transmitting audio");
        Ok(())
    }

    /// Закрыть gate и отправить commit (KeyUp)
    pub async fn close_gate_and_commit(&self) -> Result<()> {
        // 1. Send commit on CURRENT connection
        {
            let conn_guard = self.connection.lock().await;
            let conn = conn_guard
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?;

            if !conn.is_alive.load(Ordering::Relaxed) {
                 return Err(anyhow!("Connection is dead"));
            }

            conn.is_transmitting.store(false, Ordering::Relaxed);
            log::info!("[ElevenLabs] Gate CLOSED - sending commit");

            // If no audio was sent since gate open, skip commit to avoid input_error
            if !conn.sent_since_open.load(Ordering::Relaxed) {
                log::warn!("[ElevenLabs] No audio since gate opened; skipping commit");
                return Ok(());
            }

            // Send commit with a very small silence to ensure proper segment finalization
            let duration_ms: usize = 50;
            let samples = (conn.sample_rate as usize * duration_ms) / 1000;
            let silence_bytes = vec![0u8; samples * 2];
            let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&silence_bytes);

            let message = AudioChunkMessage {
                message_type: "input_audio_chunk".to_string(),
                audio_base_64: audio_base64,
                sample_rate: conn.sample_rate,
                commit: true,
            };

            let json = serde_json::to_string(&message)?;

            let mut write = conn.write.lock().await;
            write.send(Message::Text(json)).await
                .context("Failed to send commit")?;
        } // Unlock mutex here

        Ok(())
    }

    /// Закрыть gate без коммита (если аудио не было)
    pub async fn close_gate(&self) -> Result<()> {
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected"))?;
        if !conn.is_alive.load(Ordering::Relaxed) {
            return Err(anyhow!("Connection is dead"));
        }
        conn.is_transmitting.store(false, Ordering::Relaxed);
        Ok(())
    }



    /// Отключиться и закрыть WebSocket
    pub async fn disconnect(&self) -> Result<()> {
        let mut conn_guard = self.connection.lock().await;

        if let Some(conn) = conn_guard.take() {
            log::info!("[ElevenLabs] Disconnecting...");
            
            conn.is_alive.store(false, Ordering::Relaxed);

            // Отменяем background tasks
            conn.cancel_token.cancel();

            // Ждем завершения tasks
            let _ = conn.reader_task.await;
            let _ = conn.keepalive_task.await;

            // Закрываем WebSocket
            let mut write = conn.write.lock().await;
            let _ = write.send(Message::Close(None)).await;

            log::info!("[ElevenLabs] Disconnected");
        }

        Ok(())
    }

    /// Проверить подключение
    pub async fn is_connected(&self) -> bool {
        if let Some(conn) = self.connection.lock().await.as_ref() {
            conn.is_alive.load(Ordering::Relaxed)
        } else {
            false
        }
    }
}

/// Background task для чтения сообщений из WebSocket
async fn message_reader_task(
    mut read: futures_util::stream::SplitStream<WsStream>,
    _write: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    app_handle: AppHandle,
    cancel_token: tokio_util::sync::CancellationToken,
    is_alive: Arc<AtomicBool>,
) {
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[ElevenLabs] Reader task cancelled");
                break;
            }
            msg_result = read.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        // Process message events; keep session open across commits to preserve context
                        handle_text_message(&text, &app_handle);
                    }
                    Some(Ok(Message::Close(frame))) => {
                        log::info!("[ElevenLabs] WebSocket closed: {:?}", frame);
                        let (code, reason) = if let Some(f) = frame {
                            (u16::from(f.code), f.reason.to_string())
                        } else {
                            (1005, "".to_string()) // 1005 = No Status Received
                        };
                        
                        let _ = app_handle.emit("elevenlabs://connection-closed", ConnectionClosedEvent {
                            code,
                            reason,
                        });
                        break;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        log::debug!("[ElevenLabs] Received pong");
                    }
                    Some(Err(e)) => {
                        log::error!("[ElevenLabs] WebSocket error: {:?}", e);
                        let _ = app_handle.emit("elevenlabs://error", ErrorEvent {
                            error: e.to_string(),
                        });
                        break;
                    }
                    None => {
                        log::info!("[ElevenLabs] WebSocket stream ended");
                        let _ = app_handle.emit("elevenlabs://connection-closed", ConnectionClosedEvent {
                            code: 1006, // Abnormal Closure
                            reason: "Stream ended".to_string(),
                        });
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    is_alive.store(false, Ordering::Relaxed);
    cancel_token.cancel(); // Stop keepalive task
    log::info!("[ElevenLabs] Reader task finished, connection marked dead");
}

/// Обработка текстовых сообщений от ElevenLabs
/// Returns true if connection should be closed (committed transcript received)
fn handle_text_message(text: &str, app_handle: &AppHandle) -> bool {
    log::debug!("[ElevenLabs] Raw message: {}", text);

    if let Ok(msg) = serde_json::from_str::<TranscriptMessage>(text) {
        log::info!("[ElevenLabs] Message type: {}", msg.message_type);

        match msg.message_type.as_str() {
            "session_started" => {
                if let Some(session_id) = msg.session_id {
                    log::info!("[ElevenLabs] Session started: {}", session_id);
                    let _ = app_handle.emit("elevenlabs://session-started", SessionStartedEvent {
                        session_id,
                    });
                }
                false
            }
            "partial_transcript" => {
                log::info!("[ElevenLabs] Partial: {}", msg.text);
                let _ = app_handle.emit("elevenlabs://transcript", TranscriptEvent {
                    text: msg.text,
                    is_partial: true,
                });
                false
            }
            "committed_transcript" | "committed_transcript_with_timestamps" => {
                log::info!("[ElevenLabs] Committed: {}", msg.text);
                let _ = app_handle.emit("elevenlabs://transcript", TranscriptEvent {
                    text: msg.text,
                    is_partial: false,
                });
                false
            }
            "error" | "auth_error" | "quota_exceeded_error" | "input_error" => {
                log::error!("[ElevenLabs] Error received: {:?}", msg);
                let _ = app_handle.emit("elevenlabs://error", ErrorEvent {
                    error: format!("{:?}", msg),
                });
                false
            }
            _ => {
                log::debug!("[ElevenLabs] Unknown message type: {}", msg.message_type);
                false
            }
        }
    } else {
        false
    }
}

/// Background task для keep-alive ping
async fn keepalive_task(
    write: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    cancel_token: tokio_util::sync::CancellationToken,
    is_transmitting: Arc<AtomicBool>,
    sample_rate: u32,
) {
    // Send a tiny silent audio keep-alive every few seconds when gate is closed
    let mut interval = interval(Duration::from_secs(4));

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[ElevenLabs] Keep-alive task cancelled");
                break;
            }
            _ = interval.tick() => {
                if is_transmitting.load(Ordering::Relaxed) {
                    continue;
                }

                // 20ms of silence in PCM16
                let duration_ms: usize = 20;
                let samples = (sample_rate as usize * duration_ms) / 1000;
                let silence_bytes = vec![0u8; samples * 2];
                let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&silence_bytes);

                let message = AudioChunkMessage {
                    message_type: "input_audio_chunk".to_string(),
                    audio_base_64: audio_base64,
                    sample_rate,
                    commit: false,
                };

                if let Ok(json) = serde_json::to_string(&message) {
                    let mut guard = write.lock().await;
                    if let Err(e) = guard.send(Message::Text(json)).await {
                        log::error!("[ElevenLabs] Failed to send keep-alive audio: {}", e);
                        break;
                    } else {
                        log::trace!("[ElevenLabs] Sent silent keep-alive audio");
                    }
                }
            }
        }
    }
}
