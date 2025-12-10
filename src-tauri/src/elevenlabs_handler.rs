use tauri::{AppHandle, Emitter, Listener, Manager};
use crate::core::state::AppState;
use crate::settings::AppSettings;

/// Настраивает обработчики событий для ElevenLabs streaming
pub fn setup_elevenlabs_event_handlers(app: &AppHandle) {
    let app_clone = app.clone();

    // Обработчик транскрипций
    app.listen("elevenlabs://transcript", move |event| {
        let app = app_clone.clone();

        // Парсим payload
        if let Ok(payload) = serde_json::from_str::<TranscriptEventPayload>(
            event.payload()
        ) {
            // Обрабатываем partial транскрипции - показываем в UI
            if payload.is_partial {
                log::debug!("[ElevenLabs Handler] Partial transcript: {}", payload.text);

                // Отправляем partial событие в UI для отображения в реальном времени
                let _ = app.emit("transcription://partial", serde_json::json!({
                    "text": payload.text
                }));
                append_transcript_log(&app, "partial", &payload.text);
                return;
            }

            log::info!("[ElevenLabs Handler] Processing committed transcript: {}", payload.text);

            // Запускаем обработку в отдельной задаче
            tauri::async_runtime::spawn(async move {
                if let Err(e) = process_transcript(&app, payload.text).await {
                    log::error!("[ElevenLabs Handler] Failed to process transcript: {}", e);
                }
            });
        }
    });

    log::info!("[ElevenLabs Handler] Event handlers registered");
}

fn append_transcript_log(app: &AppHandle, tag: &str, text: &str) {
    let handle = app.clone();
    let tag = tag.to_string();
    let text = text.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let resolver = handle.path();
        if let Ok(dir) = resolver.app_log_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("transcripts.log");
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                use std::io::Write;
                let _ = writeln!(file, "[{}] {}", tag, text);
            }
        }
    });
}

// Also handle ElevenLabs errors to update UI status
pub fn setup_elevenlabs_error_handlers(app: &AppHandle) {
    let app_err = app.clone();
    app.listen("elevenlabs://error", move |event| {
        let app = app_err.clone();
        use crate::core::events::{emit_status, StatusPhase};
        use std::sync::atomic::Ordering;

        log::error!("[ElevenLabs Handler] Error event: {}", event.payload());

        let state = app.state::<AppState>();
        state.is_transcribing().store(false, Ordering::SeqCst);

        emit_status(&app, StatusPhase::Error, Some("Streaming error"));
        // Transition back to Idle after error for UI to recover
        emit_status(&app, StatusPhase::Idle, Some("Ready for next transcription"));
        log::info!("[ElevenLabs Handler] Error handled, state reset to Idle");
    });

    log::info!("[ElevenLabs Handler] Error handlers registered");
}

#[derive(serde::Deserialize)]
struct TranscriptEventPayload {
    text: String,
    is_partial: bool,
}

/// Обрабатывает полученную транскрипцию и выводит текст
async fn process_transcript(app: &AppHandle, text: String) -> anyhow::Result<()> {
    use tauri::Manager;
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use crate::core::events::{emit_complete, emit_status, StatusPhase};
    use std::sync::atomic::Ordering;

    let state = app.state::<AppState>();
    let mut settings = state.current_settings().await;

    // Check if force_translate is set for this session
    if state.get_force_translate() {
        settings.auto_translate = true;
        log::info!("[ElevenLabs Handler] Force translate enabled for this session");
        state.clear_force_translate();
    }

    // Store original text before LLM processing for history
    let original_text = text.clone();

    // Применяем LLM обработку если нужно
    let final_text = if settings.requires_llm() {
        log::info!("[ElevenLabs Handler] Applying LLM processing...");
        emit_status(app, StatusPhase::Transcribing, Some("Applying LLM..."));

        match apply_llm_refinement(&settings, &original_text).await {
            Ok(refined) => refined,
            Err(e) => {
                log::error!("[ElevenLabs Handler] LLM processing failed: {}", e);
                original_text.clone() // Используем оригинальный текст
            }
        }
    } else {
        original_text.clone()
    };

    let trimmed = final_text.trim().to_string();

    // Copy to clipboard if enabled (ALWAYS, not just when simulate_typing is off)
    if settings.copy_to_clipboard && !trimmed.is_empty() {
        if let Err(e) = app.clipboard().write_text(&trimmed) {
            log::error!("[ElevenLabs Handler] Failed to copy to clipboard: {}", e);
        } else {
            log::info!("[ElevenLabs Handler] Text copied to clipboard");
        }
    }

    // Выводим текст через эмуляцию ввода если включено
    if settings.simulate_typing && !trimmed.is_empty() {
        log::info!("[ElevenLabs Handler] Typing text character by character");
        let keyboard = state.transcription().keyboard();
        let text_clone = trimmed.clone();

        if let Err(e) = tauri::async_runtime::spawn_blocking(move || {
            keyboard.type_text(&text_clone)
        }).await.map_err(|e| anyhow::anyhow!("Task join error: {}", e))? {
            log::error!("[ElevenLabs Handler] Failed to type text: {}", e);
        }
    }

    append_transcript_log(app, "committed", &trimmed);

    // Save to history (only non-empty results)
    if !trimmed.is_empty() {
        let translated_text = if settings.auto_translate && trimmed != original_text {
            Some(trimmed.clone())
        } else {
            None
        };

        // Determine LLM provider if LLM was used
        let llm_provider_used = if settings.requires_llm() {
            Some(format!("{:?}", settings.llm_provider).to_lowercase())
        } else {
            None
        };

        // Check if custom instructions were used
        let custom_instructions_used = settings.use_custom_instructions
            && !settings.custom_instructions.trim().is_empty();

        let _ = state.add_history_entry(
            if translated_text.is_some() { original_text } else { trimmed.clone() },
            translated_text,
            None, // source_language - TODO: detect from transcription
            if settings.auto_translate { Some(settings.target_language.clone()) } else { None },
            Some("elevenlabs".to_string()), // transcription_provider
            llm_provider_used,
            custom_instructions_used,
        ).await;
        log::info!("[ElevenLabs Handler] Added to history");
    }

    // Сбрасываем флаг транскрипции
    state.is_transcribing().store(false, Ordering::SeqCst);

    // Emit success status BEFORE complete (for overlay to show final text)
    emit_status(app, StatusPhase::Success, None);
    emit_complete(app, &trimmed);
    emit_status(app, StatusPhase::Idle, Some("Ready for next transcription"));

    Ok(())
}

/// Применяет LLM обработку к тексту
async fn apply_llm_refinement(settings: &AppSettings, text: &str) -> anyhow::Result<String> {
    use crate::settings::LLMProvider;
    use crate::groq_llm::GroqLLMClient;
    use crate::openai::{OpenAiClient, RefinementRequest};

    let llm_key = match settings.llm_provider {
        LLMProvider::OpenAI => settings.api_key.trim().to_string(),
        LLMProvider::Groq => settings.groq_api_key.trim().to_string(),
    };

    if llm_key.is_empty() {
        let provider_name = match settings.llm_provider {
            LLMProvider::OpenAI => "OpenAI",
            LLMProvider::Groq => "Groq",
        };
        return Err(anyhow::anyhow!(
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

    let request = RefinementRequest {
        api_key: llm_key,
        model: settings.model.clone(),
        auto_translate: settings.auto_translate,
        target_language: settings.target_language.clone(),
        custom_instructions,
        vocabulary,
    };

    match settings.llm_provider {
        LLMProvider::OpenAI => {
            let client = OpenAiClient::new()?;
            client.refine_transcript(text.to_string(), &request).await
        }
        LLMProvider::Groq => {
            let client = GroqLLMClient::new()?;
            client.refine_transcript(text.to_string(), &request).await
        }
    }
}

