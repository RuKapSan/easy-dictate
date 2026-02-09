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
    state::{AppState, NewHistoryEntry},
};

/// Result of transcription containing both original and processed text
pub struct TranscriptionResult {
    /// Original transcription before LLM processing
    pub original: String,
    /// Final text after LLM processing (translation/custom instructions)
    pub processed: String,
    /// Whether LLM processing was applied
    pub llm_applied: bool,
}

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

    /// Apply LLM refinement (translation, custom instructions, vocabulary) to text.
    /// Reuses existing HTTP clients to avoid creating new ones per call.
    pub async fn refine(&self, settings: &AppSettings, text: String) -> Result<String> {
        let refinements_key = match settings.llm_provider {
            LLMProvider::OpenAI => settings.api_key.trim().to_string(),
            LLMProvider::Groq => settings.groq_api_key.trim().to_string(),
        };

        if refinements_key.is_empty() {
            let provider_name = settings.llm_provider.display_name();
            return Err(anyhow!(
                "{} API key is required for translation, custom instructions, or vocabulary correction",
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

        let vocabulary = if settings.use_vocabulary {
            settings.custom_vocabulary.clone()
        } else {
            Vec::new()
        };

        let refinement = RefinementRequest {
            api_key: refinements_key,
            model: settings.llm_model.clone(),
            auto_translate: settings.auto_translate,
            target_language: settings.target_language.clone(),
            custom_instructions,
            vocabulary,
        };

        match settings.llm_provider {
            LLMProvider::OpenAI => self.openai.refine_transcript(text, &refinement).await,
            LLMProvider::Groq => self.groq_llm.refine_transcript(text, &refinement).await,
        }
    }

    pub async fn perform(
        &self,
        settings: &AppSettings,
        audio_wav: Vec<u8>,
    ) -> Result<TranscriptionResult> {
        // Handle Mock provider for E2E testing
        if settings.provider.is_mock() {
            tracing::info!("[Transcription] Using Mock provider for testing");
            // Simulate processing delay
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let mock_text = "Mock transcription result for E2E testing".to_string();
            return Ok(TranscriptionResult {
                original: mock_text.clone(),
                processed: mock_text,
                llm_applied: false,
            });
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

        let original_text = match settings.provider {
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

        let mut processed_text = original_text.clone();
        let mut llm_applied = false;

        if !original_text.trim().is_empty() && settings.requires_llm() {
            processed_text = self.refine(settings, original_text.clone()).await?;
            llm_applied = true;
        }

        Ok(TranscriptionResult {
            original: original_text,
            processed: processed_text,
            llm_applied,
        })
    }
}

pub fn spawn_transcription(app: &AppHandle, audio_wav: Vec<u8>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state: State<'_, AppState> = app_handle.state();
        let mut settings = (*state.current_settings().await).clone();

        // Check if force_translate was requested for the current session
        let session_id = state.current_session_id();
        if state.take_force_translate(session_id) {
            settings.auto_translate = true;
            tracing::info!(
                "[Transcription] Force translate enabled for session {}",
                session_id
            );
        }

        let service = state.transcription();
        let keyboard = service.keyboard();

        let outcome = service.perform(&settings, audio_wav).await;

        match outcome {
            Ok(result) => {
                let trimmed = result.processed.trim().to_string();
                let original_trimmed = result.original.trim().to_string();

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
                            tracing::error!("[Typing] Failed to simulate typing: {}", err);
                        }
                    });
                }

                // Save to history (only non-empty results)
                if !trimmed.is_empty() {
                    // Determine providers used
                    let transcription_provider =
                        Some(format!("{:?}", settings.provider).to_lowercase());
                    let llm_provider_used = if result.llm_applied {
                        Some(format!("{:?}", settings.llm_provider).to_lowercase())
                    } else {
                        None
                    };

                    // Check if custom instructions were used
                    let custom_instructions_used = settings.use_custom_instructions
                        && !settings.custom_instructions.trim().is_empty();

                    // If LLM was applied, save original and processed separately
                    let (original_text, translated_text) =
                        if result.llm_applied && original_trimmed != trimmed {
                            (original_trimmed, Some(trimmed.clone()))
                        } else {
                            (trimmed.clone(), None)
                        };

                    state
                        .add_history_entry(NewHistoryEntry {
                            original: original_text,
                            translated: translated_text,
                            source_language: None, // TODO: detect from transcription
                            target_language: if settings.auto_translate {
                                Some(settings.target_language.clone())
                            } else {
                                None
                            },
                            transcription_provider,
                            llm_provider: llm_provider_used,
                            custom_instructions_used,
                        })
                        .await;
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
