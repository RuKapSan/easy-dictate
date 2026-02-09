use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex,
};
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
    /// Transcription provider used (e.g., "openai", "groq", "elevenlabs")
    #[serde(default)]
    pub transcription_provider: Option<String>,
    /// LLM provider used for post-processing (e.g., "openai", "groq")
    #[serde(default)]
    pub llm_provider: Option<String>,
    /// Whether custom instructions were applied
    #[serde(default)]
    pub custom_instructions_used: bool,
}

/// Data needed to create a new history entry
pub struct NewHistoryEntry {
    pub original: String,
    pub translated: Option<String>,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub transcription_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub custom_instructions_used: bool,
}

impl HistoryEntry {
    fn from_new(id: u64, data: NewHistoryEntry) -> Self {
        Self {
            id,
            timestamp: Utc::now(),
            original_text: data.original,
            translated_text: data.translated,
            source_language: data.source_language,
            target_language: data.target_language,
            transcription_provider: data.transcription_provider,
            llm_provider: data.llm_provider,
            custom_instructions_used: data.custom_instructions_used,
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
    settings: RwLock<Arc<AppSettings>>,
    recorder: Recorder,
    active_recording: Mutex<Option<RecordingSession>>,
    transcription: TranscriptionService,
    elevenlabs_streaming: ElevenLabsStreamingClient,
    audio_streaming_handle: Mutex<Option<AudioStreamingHandle>>,
    is_transcribing: AtomicBool,
    /// Session ID for which force_translate was requested (0 = none)
    force_translate_session: AtomicU64,
    /// Current recording session counter
    session_counter: AtomicU64,
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
            settings: RwLock::new(Arc::new(initial)),
            recorder,
            active_recording: Mutex::new(None),
            transcription,
            elevenlabs_streaming,
            audio_streaming_handle: Mutex::new(None),
            is_transcribing: AtomicBool::new(false),
            force_translate_session: AtomicU64::new(0),
            session_counter: AtomicU64::new(0),
            tray_status_item: Mutex::new(None),
            history: RwLock::new(Vec::new()),
            history_id_counter: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub async fn current_settings(&self) -> Arc<AppSettings> {
        self.settings.read().await.clone()
    }

    /// Atomically read-modify-write settings under an exclusive lock.
    pub async fn update_settings<F>(&self, f: F) -> AppSettings
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut guard = self.settings.write().await;
        let mut new = (**guard).clone();
        f(&mut new);
        *guard = Arc::new(new.clone());
        new
    }

    pub async fn replace_settings(&self, next: AppSettings) {
        *self.settings.write().await = Arc::new(next);
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

    /// Start a new recording session and return its ID.
    /// If `force_translate` is true, mark this session for forced translation.
    pub fn start_session(&self, force_translate: bool) -> u64 {
        let session_id = self
            .session_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        if force_translate {
            self.force_translate_session
                .store(session_id, std::sync::atomic::Ordering::SeqCst);
        }
        session_id
    }

    /// Get the current session ID (set by the last start_session call).
    pub fn current_session_id(&self) -> u64 {
        self.session_counter
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Check if the given session has force_translate, and consume it.
    pub fn take_force_translate(&self, session_id: u64) -> bool {
        let stored = self
            .force_translate_session
            .load(std::sync::atomic::Ordering::SeqCst);
        if stored == session_id && stored != 0 {
            self.force_translate_session
                .store(0, std::sync::atomic::Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// Add a new entry to the history
    pub async fn add_history_entry(&self, data: NewHistoryEntry) -> HistoryEntry {
        let id = self
            .history_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let entry = HistoryEntry::from_new(id, data);

        let mut history = self.history.write().await;
        history.push(entry.clone());

        // Keep only the last MAX_HISTORY_ENTRIES
        if history.len() > MAX_HISTORY_ENTRIES {
            let drain_count = history.len() - MAX_HISTORY_ENTRIES;
            history.drain(0..drain_count);
        }

        tracing::info!("[History] Added entry {} (total: {})", id, history.len());
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
        tracing::info!("[History] Cleared all history entries");
    }

    /// Delete a specific history entry by ID
    pub async fn delete_history_entry(&self, id: u64) -> bool {
        let mut history = self.history.write().await;
        let initial_len = history.len();
        history.retain(|e| e.id != id);
        let deleted = history.len() < initial_len;
        if deleted {
            tracing::info!("[History] Deleted entry {}", id);
        }
        deleted
    }
}
