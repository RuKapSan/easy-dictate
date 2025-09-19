use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    menu::{MenuBuilder, MenuItem, MenuItemBuilder},
    AppHandle, Emitter, Manager, RunEvent, State,
};
#[cfg(not(debug_assertions))]
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_clipboard_manager::ClipboardExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcut, ShortcutState};

mod audio;
mod input;
mod openai;
mod settings;

use audio::{Recorder, RecordingSession};
use input::KeyboardController;
use openai::{OpenAiClient, TranscriptionJob};
use settings::{AppSettings, SettingsStore};

const EVENT_STATUS: &str = "transcription://status";
const EVENT_PARTIAL: &str = "transcription://partial";
const EVENT_COMPLETE: &str = "transcription://complete";

struct AppState {
    settings_store: SettingsStore,
    settings: tokio::sync::RwLock<AppSettings>,
    recorder: Recorder,
    active_recording: Mutex<Option<RecordingSession>>,
    openai: OpenAiClient,
    keyboard: Arc<KeyboardController>,
    is_transcribing: AtomicBool,
    tray_status_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

impl AppState {
    fn new(settings_store: SettingsStore, initial: AppSettings) -> Result<Self> {
        Ok(Self {
            settings_store,
            settings: tokio::sync::RwLock::new(initial),
            recorder: Recorder::new()?,
            active_recording: Mutex::new(None),
            openai: OpenAiClient::new()?,
            keyboard: Arc::new(KeyboardController::new()?),
            is_transcribing: AtomicBool::new(false),
            tray_status_item: Mutex::new(None),
        })
    }

    async fn current_settings(&self) -> AppSettings {
        self.settings.read().await.clone()
    }

    async fn replace_settings(&self, next: AppSettings) {
        *self.settings.write().await = next;
    }

    async fn persist_settings(&self, next: &AppSettings) -> Result<()> {
        self.settings_store.save(next).await
    }
}

#[derive(Clone, Serialize)]
struct StatusPayload<'a> {
    phase: &'a str,
    message: &'a str,
}

#[derive(Clone, Serialize)]
struct TextPayload<'a> {
    text: &'a str,
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    println!("[easy-dictate] get_settings invoked");
    Ok(state.current_settings().await.sanitized())
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    println!(
        "[easy-dictate] save_settings invoked with hotkey={}",
        settings.hotkey
    );
    log::info!(
        "Сохранение настроек с горячей клавишей: {}",
        settings.hotkey
    );

    let sanitized = settings.sanitized();
    log::info!("После sanitized: hotkey={}", sanitized.hotkey);

    state
        .persist_settings(&sanitized)
        .await
        .map_err(|e| e.to_string())?;
    state.replace_settings(sanitized.clone()).await;

    if let Err(err) = apply_autostart(&app, sanitized.auto_start) {
        app.emit(
            EVENT_STATUS,
            StatusPayload {
                phase: "error",
                message: &format!("Автозапуск: {err}"),
            },
        )
        .ok();
    }

    log::info!(
        "Вызов rebind_hotkey с горячей клавишей: {}",
        sanitized.hotkey
    );
    if let Err(err) = rebind_hotkey(&app, &sanitized) {
        log::error!("Ошибка при перепривязке горячей клавиши: {}", err);
        return Err(err.to_string());
    }

    log::info!("Настройки успешно сохранены");
    Ok(())
}

#[tauri::command]
async fn ping() -> Result<&'static str, String> {
    println!("[easy-dictate] ping invoked");
    Ok("pong")
}

#[tauri::command]
async fn frontend_log(level: Option<String>, message: String) -> Result<(), String> {
    let lvl = level.as_deref().unwrap_or("info");
    match lvl {
        "error" => log::error!("[frontend] {}", message),
        "warn" => log::warn!("[frontend] {}", message),
        "debug" => log::debug!("[frontend] {}", message),
        "trace" => log::trace!("[frontend] {}", message),
        _ => log::info!("[frontend] {}", message),
    }
    println!("[frontend] {}", message);
    Ok(())
}

fn apply_autostart(app: &AppHandle, should_enable: bool) -> Result<()> {
    // В режиме разработки автозапуск может не работать корректно
    #[cfg(debug_assertions)]
    {
        let _ = (app, should_enable); // Подавляем предупреждения о неиспользуемых переменных
        log::warn!("Автозапуск в режиме разработки пропущен (не поддерживается)");
        return Ok(());
    }

    #[cfg(not(debug_assertions))]
    {
        if should_enable {
            app.autolaunch().enable()?;
        } else {
            app.autolaunch().disable()?;
        }
        Ok(())
    }
}

fn rebind_hotkey(app: &AppHandle, settings: &AppSettings) -> Result<()> {
    let shortcuts: State<'_, GlobalShortcut<tauri::Wry>> = app.state();
    log::info!("Rebinding: unregister_all");
    shortcuts.unregister_all().ok();

    let hotkey = settings.normalized_hotkey();
    log::info!("Пере-регистрация горячей клавиши: {hotkey}");
    println!("[easy-dictate] Hotkey bind -> {hotkey}");
    log::info!("Rebinding: registering {}", hotkey);
    shortcuts
        .on_shortcut(
            hotkey.as_str(),
            move |app_handle, _shortcut, event| match event.state {
                ShortcutState::Pressed => {
                    println!("[easy-dictate] Hotkey pressed");
                    log::info!("Горячая клавиша нажата");
                    if let Err(err) = handle_hotkey_pressed(app_handle) {
                        emit_error(app_handle, &err.to_string());
                    }
                }
                ShortcutState::Released => {
                    println!("[easy-dictate] Hotkey released");
                    log::info!("Горячая клавиша отпущена");
                    if let Err(err) = handle_hotkey_released(app_handle) {
                        emit_error(app_handle, &err.to_string());
                    }
                }
            },
        )
        .with_context(|| format!("Не удалось зарегистрировать горячую клавишу {hotkey}"))?;

    log::info!("Rebinding: registered {}", hotkey);

    Ok(())
}

fn handle_hotkey_pressed(app: &AppHandle) -> Result<()> {
    let state: State<'_, AppState> = app.state();

    if state.is_transcribing.load(Ordering::SeqCst) {
        emit_status(
            app,
            "transcribing",
            "Пауза: идёт обработка предыдущей записи",
        );
        return Ok(());
    }

    let mut guard = state
        .active_recording
        .lock()
        .map_err(|_| anyhow!("Ошибка доступа к записи"))?;

    if guard.is_some() {
        return Ok(());
    }

    match state.recorder.start() {
        Ok(active) => {
            emit_status(app, "recording", "Идёт запись...");
            *guard = Some(active);
        }
        Err(err) => emit_error(app, &err.to_string()),
    }
    Ok(())
}

fn handle_hotkey_released(app: &AppHandle) -> Result<()> {
    let state: State<'_, AppState> = app.state();
    let active = {
        let mut guard = state
            .active_recording
            .lock()
            .map_err(|_| anyhow!("Ошибка доступа к записи"))?;
        guard.take()
    };

    let Some(active) = active else {
        return Ok(());
    };

    match active.stop() {
        Ok(audio_wav) => {
            if state.is_transcribing.swap(true, Ordering::SeqCst) {
                return Ok(());
            }

            emit_status(app, "transcribing", "Отправляем аудио в OpenAI...");
            spawn_transcription(app, audio_wav);
        }
        Err(err) => {
            emit_error(app, &err.to_string());
        }
    }

    Ok(())
}

fn spawn_transcription(app: &AppHandle, audio_wav: Vec<u8>) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state: State<'_, AppState> = app_handle.state();
        let settings = state.current_settings().await;
        let client = state.openai.clone();
        let keyboard = state.keyboard.clone();

        if settings.api_key.trim().is_empty() {
            emit_error(&app_handle, "В настройках не указан API ключ OpenAI");
            state.is_transcribing.store(false, Ordering::SeqCst);
            return;
        }

        let job = TranscriptionJob {
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            audio_wav,
            auto_translate: settings.auto_translate,
            target_language: settings.target_language.clone(),
            use_custom_instructions: settings.use_custom_instructions,
            custom_instructions: settings.custom_instructions.clone(),
        };

        let job_clone = job.clone();
        let result = client.transcribe(job).await;

        match result {
            Ok(mut text) => {
                let has_instructions = job_clone.use_custom_instructions
                    && !job_clone.custom_instructions.trim().is_empty();
                if (job_clone.auto_translate || has_instructions) && !text.is_empty() {
                    let processing_message = match (job_clone.auto_translate, has_instructions) {
                        (true, true) => "Перевожу и применяю инструкции...",
                        (true, false) => "Перевожу текст...",
                        (false, true) => "Применяю инструкции...",
                        (false, false) => "",
                    };
                    if !processing_message.is_empty() {
                        emit_status(&app_handle, "transcribing", processing_message);
                    }

                    match client.refine_transcript(text.clone(), &job_clone).await {
                        Ok(refined) => {
                            text = refined;
                        }
                        Err(err) => {
                            emit_error(&app_handle, &format!("Ошибка постобработки текста: {err}"));
                        }
                    }
                }

                if settings.use_streaming && !text.is_empty() {
                    emit_partial(&app_handle, &text);
                }

                if settings.copy_to_clipboard {
                    if let Err(err) = app_handle.clipboard().write_text(text.clone()) {
                        emit_error(&app_handle, &format!("Буфер обмена: {err}"));
                    }
                }

                if settings.simulate_typing {
                    let app_clone = app_handle.clone();
                    let keyboard_clone = keyboard.clone();
                    let text_clone = text.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Err(err) = keyboard_clone.type_text(&text_clone) {
                            emit_error(&app_clone, &err.to_string());
                        }
                    });
                }

                emit_status(&app_handle, "success", "Готово");
                emit_complete(&app_handle, &text);
            }
            Err(err) => {
                emit_error(&app_handle, &err.to_string());
            }
        }

        state.is_transcribing.store(false, Ordering::SeqCst);
        emit_status(&app_handle, "idle", "Готово к записи по горячей клавише");
    });
}

fn emit_status(app: &AppHandle, phase: &str, message: &str) {
    app.emit(EVENT_STATUS, StatusPayload { phase, message })
        .ok();

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(guard) = state.tray_status_item.lock() {
            if let Some(item) = guard.as_ref() {
                let label = match phase {
                    "recording" => "Статус: запись",
                    "transcribing" => "Статус: обработка",
                    "success" => "Статус: готово",
                    "error" => "Статус: ошибка",
                    _ => "Статус: ожидает",
                };
                item.set_text(label).ok();
            }
        }
    }
}

fn emit_partial(app: &AppHandle, text: &str) {
    app.emit(EVENT_PARTIAL, TextPayload { text }).ok();
}

fn emit_complete(app: &AppHandle, text: &str) {
    app.emit(EVENT_COMPLETE, TextPayload { text }).ok();
}

fn emit_error(app: &AppHandle, message: &str) {
    emit_status(app, "error", message);
}

fn install_tray(app: &AppHandle) -> Result<MenuItem<tauri::Wry>> {
    let open_item = MenuItemBuilder::with_id("open", "Открыть настройки").build(app)?;
    let status_item = MenuItemBuilder::with_id("status", "Статус: готово")
        .enabled(false)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Выход").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open_item)
        .separator()
        .item(&status_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let handle = app.clone();
    TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("Easy Dictate")
        .on_tray_icon_event(move |_tray, event| match event {
            TrayIconEvent::Click {
                button,
                button_state,
                ..
            } => {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    show_settings_window(&handle);
                }
            }
            TrayIconEvent::DoubleClick { .. } => {
                show_settings_window(&handle);
            }
            _ => {}
        })
        .build(app)?;

    Ok(status_item)
}

fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle();

            let config_dir = handle
                .path()
                .app_config_dir()
                .context("Не удалось найти каталог настроек")?;
            log::info!("Каталог настроек: {:?}", config_dir);
            println!("[easy-dictate] Config dir -> {:?}", config_dir);
            let store = SettingsStore::new(config_dir);
            let initial = tauri::async_runtime::block_on(store.load())?;
            let state = AppState::new(store.clone(), initial.clone())?;
            app.manage(state);

            if let Some(window) = handle.get_webview_window("main") {
                window.show().ok();
                window.unminimize().ok();
                window.set_focus().ok();
            }

            apply_autostart(&handle, initial.auto_start).ok();
            rebind_hotkey(&handle, &initial)?;
            emit_status(&handle, "idle", "Готово к записи по горячей клавише");

            let status_item = install_tray(&handle)?;
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(mut guard) = state.tray_status_item.lock() {
                    *guard = Some(status_item);
                }
            }

            handle.on_menu_event(|app_handle, event| {
                if event.id() == "open" {
                    show_settings_window(app_handle);
                } else if event.id() == "quit" {
                    app_handle.exit(0);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            ping,
            frontend_log
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                // keep running until explicit quit
            }
        });
}
