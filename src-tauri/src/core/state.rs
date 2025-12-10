use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::menu::MenuItem;
use tokio::sync::RwLock;

use crate::{
    audio::{Recorder, RecordingSession},
    elevenlabs::ElevenLabsClient,
    elevenlabs_streaming::ElevenLabsStreamingClient,
    groq::GroqClient,
    groq_llm::GroqLLMClient,
    input::KeyboardController,
    openai::OpenAiClient,
    settings::{AppSettings, SettingsStore},
};

use super::transcription::TranscriptionService;

/// Entry in the transcription history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub original_text: String,
    pub translated_text: Option<String>,
    /// Detected source language (e.g., "Russian", "English")
    #[serde(default)]
    pub source_language: Option<String>,
    /// Target language if translated
    #[serde(default)]
    pub target_language: Option<String>,
}

impl HistoryEntry {
    pub fn new(
        id: u64,
        original: String,
        translated: Option<String>,
        source_language: Option<String>,
        target_language: Option<String>,
    ) -> Self {
        Self {
            id,
            timestamp: Utc::now(),
            original_text: original,
            translated_text: translated,
            source_language,
            target_language,
        }
    }
}

/// Handle for managing an audio streaming thread
pub struct AudioStreamingHandle {
    pub cancel_token: tokio_util::sync::CancellationToken,
    pub join_handle: JoinHandle<()>,
}

/// Maximum number of history entries to keep
const MAX_HISTORY_ENTRIES: usize = 100;

pub struct AppState {
    settings_store: SettingsStore,
    settings: RwLock<AppSettings>,
    recorder: Recorder,
    active_recording: Mutex<Option<RecordingSession>>,
    transcription: TranscriptionService,
    elevenlabs_streaming: ElevenLabsStreamingClient,
    audio_streaming_handle: Mutex<Option<AudioStreamingHandle>>,
    is_transcribing: AtomicBool,
    force_translate: AtomicBool,
    tray_status_item: Mutex<Option<MenuItem<tauri::Wry>>>,
    /// Transcription history
    history: RwLock<Vec<HistoryEntry>>,
    /// Counter for generating unique history entry IDs
    history_id_counter: std::sync::atomic::AtomicU64,
}

impl AppState {
    pub fn new(settings_store: SettingsStore, initial: AppSettings) -> Result<Self> {
        let recorder = Recorder::new()?;
        let keyboard = Arc::new(KeyboardController::new()?);
        let transcription = TranscriptionService::new(
            OpenAiClient::new()?,
            GroqClient::new()?,
            GroqLLMClient::new()?,
            ElevenLabsClient::new()?,
            keyboard,
        );

        let elevenlabs_streaming = ElevenLabsStreamingClient::new();

        Ok(Self {
            settings_store,
            settings: RwLock::new(initial),
            recorder,
            active_recording: Mutex::new(None),
            transcription,
            elevenlabs_streaming,
            audio_streaming_handle: Mutex::new(None),
            is_transcribing: AtomicBool::new(false),
            force_translate: AtomicBool::new(false),
            tray_status_item: Mutex::new(None),
            history: RwLock::new(Vec::new()),
            history_id_counter: std::sync::atomic::AtomicU64::new(1),
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

    pub fn elevenlabs_streaming(&self) -> &ElevenLabsStreamingClient {
        &self.elevenlabs_streaming
    }

    pub fn audio_streaming_handle(&self) -> &Mutex<Option<AudioStreamingHandle>> {
        &self.audio_streaming_handle
    }

    pub fn set_force_translate(&self, force: bool) {
        self.force_translate.store(force, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get_force_translate(&self) -> bool {
        self.force_translate.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn clear_force_translate(&self) {
        self.force_translate.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Add a new entry to the history
    pub async fn add_history_entry(
        &self,
        original: String,
        translated: Option<String>,
        source_language: Option<String>,
        target_language: Option<String>,
    ) -> HistoryEntry {
        let id = self.history_id_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let entry = HistoryEntry::new(id, original, translated, source_language, target_language);

        let mut history = self.history.write().await;
        history.push(entry.clone());

        // Keep only the last MAX_HISTORY_ENTRIES
        if history.len() > MAX_HISTORY_ENTRIES {
            let drain_count = history.len() - MAX_HISTORY_ENTRIES;
            history.drain(0..drain_count);
        }

        log::info!("[History] Added entry {} (total: {})", id, history.len());
        entry
    }

    /// Get all history entries (newest first)
    pub async fn get_history(&self) -> Vec<HistoryEntry> {
        let history = self.history.read().await;
        let mut result = history.clone();
        result.reverse(); // Newest first
        result
    }

    /// Clear all history entries
    pub async fn clear_history(&self) {
        let mut history = self.history.write().await;
        history.clear();
        log::info!("[History] Cleared all history entries");
    }

    /// Delete a specific history entry by ID
    pub async fn delete_history_entry(&self, id: u64) -> bool {
        let mut history = self.history.write().await;
        let initial_len = history.len();
        history.retain(|e| e.id != id);
        let deleted = history.len() < initial_len;
        if deleted {
            log::info!("[History] Deleted entry {}", id);
        }
        deleted
    }
}
