use crate::core::state::{AppState, NewHistoryEntry};
use tauri::{AppHandle, Emitter, Listener, Manager};

/// Настраивает обработчики событий для ElevenLabs streaming
pub fn setup_elevenlabs_event_handlers(app: &AppHandle) {
    let app_clone = app.clone();

    // Обработчик транскрипций
    app.listen("elevenlabs://transcript", move |event| {
        let app = app_clone.clone();

        // Парсим payload
        if let Ok(payload) = serde_json::from_str::<TranscriptEventPayload>(event.payload()) {
            // Обрабатываем partial транскрипции - показываем в UI
            if payload.is_partial {
                tracing::debug!("[ElevenLabs Handler] Partial transcript: {}", payload.text);

                // Отправляем partial событие в UI для отображения в реальном времени
                let _ = app.emit(
                    "transcription://partial",
                    serde_json::json!({
                        "text": payload.text
                    }),
                );
                append_transcript_log(&app, "partial", &payload.text);
                return;
            }

            tracing::info!(
                "[ElevenLabs Handler] Processing committed transcript: {}",
                payload.text
            );

            // Запускаем обработку в отдельной задаче
            tauri::async_runtime::spawn(async move {
                if let Err(e) = process_transcript(&app, payload.text).await {
                    tracing::error!("[ElevenLabs Handler] Failed to process transcript: {}", e);
                }
            });
        }
    });

    tracing::info!("[ElevenLabs Handler] Event handlers registered");
}

fn append_transcript_log(app: &AppHandle, tag: &str, text: &str) {
    let handle = app.clone();
    let tag = tag.to_string();
    let text = text.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let resolver = handle.path();
        let dir = match resolver.app_log_dir() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("[Transcript] Failed to get log directory: {}", e);
                return;
            }
        };

        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(
                "[Transcript] Failed to create log directory {:?}: {}",
                dir,
                e
            );
            return;
        }

        let path = dir.join("transcripts.log");
        let mut file = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("[Transcript] Failed to open {:?}: {}", path, e);
                return;
            }
        };

        use std::io::Write;
        if let Err(e) = writeln!(file, "[{}] {}", tag, text) {
            tracing::warn!("[Transcript] Failed to write to {:?}: {}", path, e);
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

        tracing::error!("[ElevenLabs Handler] Error event: {}", event.payload());

        let state = app.state::<AppState>();
        state.is_transcribing().store(false, Ordering::SeqCst);

        emit_status(&app, StatusPhase::Error, Some("Streaming error"));
        // Transition back to Idle after error for UI to recover
        emit_status(
            &app,
            StatusPhase::Idle,
            Some("Ready for next transcription"),
        );
        tracing::info!("[ElevenLabs Handler] Error handled, state reset to Idle");
    });

    tracing::info!("[ElevenLabs Handler] Error handlers registered");
}

#[derive(serde::Deserialize)]
struct TranscriptEventPayload {
    text: String,
    is_partial: bool,
}

/// Обрабатывает полученную транскрипцию и выводит текст
async fn process_transcript(app: &AppHandle, text: String) -> anyhow::Result<()> {
    use crate::core::events::{emit_complete, emit_status, StatusPhase};
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    use tauri_plugin_clipboard_manager::ClipboardExt;

    let state = app.state::<AppState>();
    let mut settings = (*state.current_settings().await).clone();

    // Check if force_translate was requested for the current session
    let session_id = state.current_session_id();
    if state.take_force_translate(session_id) {
        settings.auto_translate = true;
        tracing::info!(
            "[ElevenLabs Handler] Force translate enabled for session {}",
            session_id
        );
    }

    // Store original text before LLM processing for history
    let original_text = text.clone();

    // Применяем LLM обработку если нужно (reusing clients from AppState)
    let final_text = if settings.requires_llm() {
        tracing::info!("[ElevenLabs Handler] Applying LLM processing...");
        emit_status(app, StatusPhase::Transcribing, Some("Applying LLM..."));

        let service = state.transcription();
        match service.refine(&settings, original_text.clone()).await {
            Ok(refined) => refined,
            Err(e) => {
                tracing::error!("[ElevenLabs Handler] LLM processing failed: {}", e);
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
            tracing::error!("[ElevenLabs Handler] Failed to copy to clipboard: {}", e);
        } else {
            tracing::info!("[ElevenLabs Handler] Text copied to clipboard");
        }
    }

    // Выводим текст через эмуляцию ввода если включено
    if settings.simulate_typing && !trimmed.is_empty() {
        tracing::info!("[ElevenLabs Handler] Typing text character by character");
        let keyboard = state.transcription().keyboard();
        let text_clone = trimmed.clone();

        if let Err(e) =
            tauri::async_runtime::spawn_blocking(move || keyboard.type_text(&text_clone))
                .await
                .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?
        {
            tracing::error!("[ElevenLabs Handler] Failed to type text: {}", e);
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
        let custom_instructions_used =
            settings.use_custom_instructions && !settings.custom_instructions.trim().is_empty();

        state
            .add_history_entry(NewHistoryEntry {
                original: if translated_text.is_some() {
                    original_text
                } else {
                    trimmed.clone()
                },
                translated: translated_text,
                source_language: None, // TODO: detect from transcription
                target_language: if settings.auto_translate {
                    Some(settings.target_language.clone())
                } else {
                    None
                },
                transcription_provider: Some("elevenlabs".to_string()),
                llm_provider: llm_provider_used,
                custom_instructions_used,
            })
            .await;
        tracing::info!("[ElevenLabs Handler] Added to history");
    }

    // Сбрасываем флаг транскрипции
    state.is_transcribing().store(false, Ordering::SeqCst);

    // Emit success status BEFORE complete (for overlay to show final text)
    emit_status(app, StatusPhase::Success, None);
    emit_complete(app, &trimmed);
    emit_status(app, StatusPhase::Idle, Some("Ready for next transcription"));

    Ok(())
}
