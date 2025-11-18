use anyhow::{anyhow, Context, Result};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tokio_tungstenite::{connect_async, tungstenite::{Message, http::Request}};

#[derive(Clone, Debug)]
pub struct ElevenLabsTranscriptionRequest {
    pub api_key: String,
    pub audio_wav: Vec<u8>,
    #[allow(dead_code)]
    pub language: String,
}

#[derive(Clone)]
pub struct ElevenLabsClient;

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
}

impl ElevenLabsClient {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Отправляет аудио на транскрипцию в ElevenLabs через WebSocket
    pub async fn transcribe(&self, job: ElevenLabsTranscriptionRequest) -> Result<String> {
        if job.api_key.trim().is_empty() {
            return Err(anyhow!("ElevenLabs API key is missing"));
        }

        // Извлекаем аудио из WAV файла и получаем sample rate СРАЗУ
        let (audio_data, sample_rate) = extract_pcm_from_wav(&job.audio_wav)?;

        // Подключаемся к WebSocket API
        // Параметры для WebSocket соединения (без API ключа в URL)
        // Используем scribe_v2_realtime для STT согласно документации
        // ВАЖНО: audio_format должен соответствовать sample_rate аудио!
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

        let ws_url = format!(
            "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id=scribe_v2_realtime&language_code=ru&audio_format={}&commit_strategy=vad",
            audio_format
        );

        log::info!("[ElevenLabs] Connecting to WebSocket (API key: {}..., sample_rate: {}, audio_format: {})",
            &job.api_key.chars().take(8).collect::<String>(),
            sample_rate,
            audio_format);

        // Создаем HTTP запрос с заголовком xi-api-key
        let request = Request::builder()
            .uri(ws_url)
            .header("Host", "api.elevenlabs.io")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
            .header("xi-api-key", &job.api_key)
            .body(())
            .context("Failed to build WebSocket request")?;

        let (ws_stream, response) = match connect_async(request).await {
            Ok(result) => {
                log::info!("[ElevenLabs] WebSocket connected successfully, status: {:?}", result.1.status());
                result
            }
            Err(e) => {
                log::error!("[ElevenLabs] Failed to connect to WebSocket: {:?}", e);
                return Err(anyhow!("Failed to connect to ElevenLabs WebSocket: {}", e));
            }
        };

        log::info!("[ElevenLabs] WebSocket response headers: {:?}", response.headers());

        let (mut write, mut read) = ws_stream.split();

        // Аудио уже извлечено выше (для определения audio_format)
        // Кодируем аудио в base64
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio_data);
        let audio_size = audio_base64.len();

        // Отправляем аудиоблок с commit=true чтобы получить финальную транскрипцию
        let message = AudioChunkMessage {
            message_type: "input_audio_chunk".to_string(),
            audio_base_64: audio_base64,
            sample_rate,
            commit: true,
        };

        let json = serde_json::to_string(&message)
            .context("Failed to serialize audio chunk message")?;

        log::info!("[ElevenLabs] Sending audio chunk ({} bytes of base64, sample_rate: {})", audio_size, sample_rate);

        write
            .send(Message::Text(json))
            .await
            .context("Failed to send audio chunk")?;

        log::info!("[ElevenLabs] Audio chunk sent, waiting for responses...");

        // Читаем результаты
        let mut transcript = String::new();

        while let Some(msg) = read.next().await {
            let msg = msg.context("Error receiving WebSocket message")?;
            log::debug!("[ElevenLabs] Received WebSocket message type: {:?}", msg);

            match msg {
                Message::Text(text) => {
                    log::debug!("[ElevenLabs] Raw message: {}", text);

                    let response: TranscriptMessage = serde_json::from_str(&text)
                        .context("Failed to parse transcript message")?;

                    log::info!("[ElevenLabs] Message type: {}", response.message_type);

                    match response.message_type.as_str() {
                        "session_started" => {
                            log::info!("[ElevenLabs] Session started");
                        }
                        "committed_transcript" | "committed_transcript_with_timestamps" => {
                            log::info!("[ElevenLabs] Committed transcript received (length: {}): '{}'",
                                response.text.len(), response.text);
                            if !response.text.is_empty() {
                                if !transcript.is_empty() {
                                    transcript.push(' ');
                                }
                                transcript.push_str(&response.text);
                            } else {
                                log::warn!("[ElevenLabs] Committed transcript is empty!");
                            }
                            // Получили committed транскрипцию - выходим сразу
                            log::info!("[ElevenLabs] Received committed transcript, closing connection");
                            break;
                        }
                        "partial_transcript" => {
                            // Игнорируем partial для финальной транскрипции
                            log::debug!("[ElevenLabs] Partial transcript: {}", response.text);
                        }
                        "error" | "auth_error" | "quota_exceeded_error" => {
                            log::error!("[ElevenLabs] Error: {:?}", response);
                            return Err(anyhow!("ElevenLabs API error: {:?}", response));
                        }
                        _ => {
                            log::debug!("[ElevenLabs] Unknown message type: {}", response.message_type);
                        }
                    }
                }
                Message::Close(frame) => {
                    log::info!("[ElevenLabs] WebSocket closed: {:?}", frame);
                    break;
                }
                Message::Ping(_) => {
                    log::debug!("[ElevenLabs] Received ping");
                }
                Message::Pong(_) => {
                    log::debug!("[ElevenLabs] Received pong");
                }
                _ => {
                    log::debug!("[ElevenLabs] Received other message type");
                }
            }
        }

        if transcript.is_empty() {
            log::warn!("[ElevenLabs] No transcript received");
        } else {
            log::info!("[ElevenLabs] Final transcript: {}", transcript);
        }

        Ok(transcript.trim().to_string())
    }
}

/// Извлекает PCM аудиоданные из WAV файла и возвращает их вместе с sample rate
fn extract_pcm_from_wav(wav_data: &[u8]) -> Result<(Vec<u8>, u32)> {
    let mut cursor = Cursor::new(wav_data);
    let reader = hound::WavReader::new(&mut cursor)
        .context("Failed to read WAV file")?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    log::info!(
        "[ElevenLabs] WAV spec - sample_rate: {}, channels: {}, bits_per_sample: {}",
        spec.sample_rate,
        spec.channels,
        spec.bits_per_sample
    );

    // Для ElevenLabs нужны данные в формате PCM16 (16-bit signed integers, little-endian)
    let mut samples = Vec::new();
    let reader = hound::WavReader::new(Cursor::new(wav_data))
        .context("Failed to re-create WAV reader")?;

    match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => {
            // PCM16 - просто копируем сэмплы как i16
            for sample in reader.into_samples::<i16>() {
                let s = sample.context("Failed to read sample")?;
                samples.extend_from_slice(&s.to_le_bytes());
            }
        }
        (hound::SampleFormat::Int, 32) => {
            // Конвертируем i32 в i16
            for sample in reader.into_samples::<i32>() {
                let s = sample.context("Failed to read sample")?;
                let s16 = (s >> 16) as i16; // Берем старшие 16 бит
                samples.extend_from_slice(&s16.to_le_bytes());
            }
        }
        (hound::SampleFormat::Float, _) => {
            // Конвертируем float в i16
            for sample in reader.into_samples::<f32>() {
                let s = sample.context("Failed to read sample")?;
                let s16 = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                samples.extend_from_slice(&s16.to_le_bytes());
            }
        }
        _ => {
            return Err(anyhow!(
                "Unsupported WAV format: {:?} with {} bits per sample",
                spec.sample_format,
                spec.bits_per_sample
            ));
        }
    }

    Ok((samples, sample_rate))
}
