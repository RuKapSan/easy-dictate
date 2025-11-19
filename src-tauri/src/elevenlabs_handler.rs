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
    use crate::core::events::{emit_complete, emit_status, StatusPhase};
    use std::sync::atomic::Ordering;

    let state = app.state::<AppState>();
    let settings = state.current_settings().await;

    // Применяем LLM обработку если нужно
    let final_text = if settings.requires_llm() {
        log::info!("[ElevenLabs Handler] Applying LLM processing...");
        emit_status(app, StatusPhase::Transcribing, Some("Applying LLM..."));

        match apply_llm_refinement(&settings, &text).await {
            Ok(refined) => refined,
            Err(e) => {
                log::error!("[ElevenLabs Handler] LLM processing failed: {}", e);
                text // Используем оригинальный текст
            }
        }
    } else {
        text
    };

    // Выводим текст
    output_text(app, &settings, &final_text).await?;

    // Сбрасываем флаг транскрипции
    state.is_transcribing().store(false, Ordering::SeqCst);

    emit_complete(app, &final_text);
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
        return Err(anyhow::anyhow!("LLM API key is empty"));
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

    let request = RefinementRequest {
        api_key: llm_key,
        model: settings.model.clone(),
        auto_translate: settings.auto_translate,
        target_language: settings.target_language.clone(),
        custom_instructions,
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

/// Выводит текст через Enigo (simulate_typing) или Arboard (paste)
async fn output_text(
    app: &AppHandle,
    settings: &AppSettings,
    text: &str,
) -> anyhow::Result<()> {
    use tauri::Manager;
    use tauri_plugin_clipboard_manager::ClipboardExt;

    let state = app.state::<AppState>();

    if settings.simulate_typing {
        // Посимвольная печать через Enigo
        log::info!("[ElevenLabs Handler] Typing text character by character");
        let keyboard = state.transcription().keyboard();
        let text_clone = text.to_string();

        // Run in blocking task to avoid blocking async runtime
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = keyboard.type_text(&text_clone) {
                log::error!("[ElevenLabs Handler] Failed to type text: {}", e);
            }
        });
    } else {
        // Вставка через буфер обмена
        log::info!("[ElevenLabs Handler] Pasting text via clipboard");
        app.clipboard().write_text(text)?;

        // Эмулируем Ctrl+V
        let keyboard = state.transcription().keyboard();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = keyboard.paste() {
                log::error!("[ElevenLabs Handler] Failed to paste: {}", e);
            }
        });
    }

    Ok(())
}
