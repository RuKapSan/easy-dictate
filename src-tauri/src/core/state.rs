use std::sync::{atomic::AtomicBool, Arc, Mutex};

use anyhow::Result;
use tauri::menu::MenuItem;
use tokio::sync::RwLock;

use crate::{
    audio::{Recorder, RecordingSession},
    groq::GroqClient,
    groq_llm::GroqLLMClient,
    input::KeyboardController,
    openai::OpenAiClient,
    settings::{AppSettings, SettingsStore},
};

use super::transcription::TranscriptionService;

pub struct AppState {
    settings_store: SettingsStore,
    settings: RwLock<AppSettings>,
    recorder: Recorder,
    active_recording: Mutex<Option<RecordingSession>>,
    transcription: TranscriptionService,
    is_transcribing: AtomicBool,
    tray_status_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

impl AppState {
    pub fn new(settings_store: SettingsStore, initial: AppSettings) -> Result<Self> {
        let recorder = Recorder::new()?;
        let keyboard = Arc::new(KeyboardController::new()?);
        let transcription = TranscriptionService::new(
            OpenAiClient::new()?,
            GroqClient::new()?,
            GroqLLMClient::new()?,
            keyboard,
        );

        Ok(Self {
            settings_store,
            settings: RwLock::new(initial),
            recorder,
            active_recording: Mutex::new(None),
            transcription,
            is_transcribing: AtomicBool::new(false),
            tray_status_item: Mutex::new(None),
        })
    }

    pub async fn current_settings(&self) -> AppSettings {
        self.settings.read().await.clone()
    }

    pub async fn replace_settings(&self, next: AppSettings) {
        *self.settings.write().await = next;
    }

    pub async fn persist_settings(&self, next: &AppSettings) -> Result<()> {
        self.settings_store.save(next).await
    }

    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    pub fn active_recording(&self) -> &Mutex<Option<RecordingSession>> {
        &self.active_recording
    }

    pub fn transcription(&self) -> TranscriptionService {
        self.transcription.clone()
    }

    pub fn is_transcribing(&self) -> &AtomicBool {
        &self.is_transcribing
    }

    pub fn tray_status_item(&self) -> &Mutex<Option<MenuItem<tauri::Wry>>> {
        &self.tray_status_item
    }
}
