use std::sync::{atomic::Ordering, Arc};

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt as _;

use crate::{
    elevenlabs::{ElevenLabsClient, ElevenLabsTranscriptionRequest},
    groq::GroqClient,
    groq_llm::GroqLLMClient,
    input::KeyboardController,
    openai::{OpenAiClient, RefinementRequest, TranscriptionRequest},
    settings::{AppSettings, LLMProvider, TranscriptionProvider},
};

use super::{
    events::{emit_complete, emit_error, emit_partial, emit_status, StatusPhase},
    state::AppState,
};

#[derive(Clone)]
pub struct TranscriptionService {
    openai: OpenAiClient,
    groq: GroqClient,
    groq_llm: GroqLLMClient,
    elevenlabs: ElevenLabsClient,
    keyboard: Arc<KeyboardController>,
}

impl TranscriptionService {
    pub fn new(
        openai: OpenAiClient,
        groq: GroqClient,
        groq_llm: GroqLLMClient,
        elevenlabs: ElevenLabsClient,
        keyboard: Arc<KeyboardController>,
    ) -> Self {
        Self {
            openai,
            groq,
            groq_llm,
            elevenlabs,
            keyboard,
        }
    }

    pub fn keyboard(&self) -> Arc<KeyboardController> {
        Arc::clone(&self.keyboard)
    }

    pub async fn perform(&self, settings: &AppSettings, audio_wav: Vec<u8>) -> Result<String> {
        // Handle Mock provider for E2E testing
        if settings.provider.is_mock() {
            log::info!("[Transcription] Using Mock provider for testing");
            // Simulate processing delay
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            return Ok("Mock transcription result for E2E testing".to_string());
        }

        let transcription_api_key = match settings.provider {
            TranscriptionProvider::OpenAI => settings.api_key.trim().to_string(),
            TranscriptionProvider::Groq => settings.groq_api_key.trim().to_string(),
            TranscriptionProvider::ElevenLabs => settings.elevenlabs_api_key.trim().to_string(),
            TranscriptionProvider::Mock => String::new(), // Already handled above
        };

        if transcription_api_key.is_empty() {
            let provider_name = settings.provider.display_name();
            return Err(anyhow!(
                "{} API key is required before starting a transcription",
                provider_name
            ));
        }

        let mut text = match settings.provider {
            TranscriptionProvider::OpenAI | TranscriptionProvider::Groq => {
                let request = TranscriptionRequest {
                    api_key: transcription_api_key,
                    model: settings.model.clone(),
                    audio_wav,
                };

                match settings.provider {
                    TranscriptionProvider::OpenAI => self.openai.transcribe(request).await?,
                    TranscriptionProvider::Groq => self.groq.transcribe(request).await?,
                    _ => unreachable!(),
                }
            }
            TranscriptionProvider::ElevenLabs => {
                let el_request = ElevenLabsTranscriptionRequest {
                    api_key: transcription_api_key,
                    audio_wav,
                    language: String::new(),
                };
                self.elevenlabs.transcribe(el_request).await?
            }
            TranscriptionProvider::Mock => {
                // Should never reach here - Mock is handled above
                unreachable!("Mock provider should be handled earlier")
            }
        };

        if !text.trim().is_empty() && settings.requires_llm() {
            let refinements_key = match settings.llm_provider {
                LLMProvider::OpenAI => settings.api_key.trim().to_string(),
                LLMProvider::Groq => settings.groq_api_key.trim().to_string(),
            };

            if refinements_key.is_empty() {
                let provider_name = settings.llm_provider.display_name();
                return Err(anyhow!(
                    "{} API key is required for translation or custom instructions",
                    provider_name
                ));
            }

            let custom_instructions = if settings.use_custom_instructions {
                let trimmed = settings.custom_instructions.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            } else {
                None
            };

            let refinement = RefinementRequest {
                api_key: refinements_key,
                model: settings.model.clone(),
                auto_translate: settings.auto_translate,
                target_language: settings.target_language.clone(),
                custom_instructions,
            };

            text = match settings.llm_provider {
                LLMProvider::OpenAI => self.openai.refine_transcript(text, &refinement).await?,
                LLMProvider::Groq => self.groq_llm.refine_transcript(text, &refinement).await?,
            };
        }

        Ok(text)
    }
}

pub fn spawn_transcription(app: &AppHandle, audio_wav: Vec<u8>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state: State<'_, AppState> = app_handle.state();
        let mut settings = state.current_settings().await;

        // Check if force_translate is set for this session
        if state.get_force_translate() {
            settings.auto_translate = true;
            log::info!("[Transcription] Force translate enabled for this session");
            // Clear the flag after using it
            state.clear_force_translate();
        }

        let service = state.transcription();
        let keyboard = service.keyboard();

        let outcome = service.perform(&settings, audio_wav).await;

        match outcome {
            Ok(text) => {
                let trimmed = text.trim().to_string();

                if settings.use_streaming && !trimmed.is_empty() {
                    emit_partial(&app_handle, &trimmed);
                }

                if settings.copy_to_clipboard {
                    if let Err(err) = app_handle.clipboard().write_text(trimmed.clone()) {
                        emit_error(&app_handle, &format!("Failed to copy to clipboard: {err}"));
                    }
                }

                if settings.simulate_typing && !trimmed.is_empty() {
                    let keyboard_clone = keyboard.clone();
                    let text_clone = trimmed.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Err(err) = keyboard_clone.type_text(&text_clone) {
                            eprintln!("[easy-dictate] typing simulation failed: {err}");
                        }
                    });
                }

                emit_status(&app_handle, StatusPhase::Success, None);
                emit_complete(&app_handle, &trimmed);
            }
            Err(err) => {
                emit_error(&app_handle, &err.to_string());
            }
        }

        state.is_transcribing().store(false, Ordering::SeqCst);
        emit_status(&app_handle, StatusPhase::Idle, None);
    });
}
