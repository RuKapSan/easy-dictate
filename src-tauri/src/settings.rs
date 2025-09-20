use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs as async_fs;

const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
const CONFIG_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProvider {
    OpenAI,
    Groq,
}

impl Default for TranscriptionProvider {
    fn default() -> Self {
        TranscriptionProvider::OpenAI
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    OpenAI,
    Groq,
}

impl Default for LLMProvider {
    fn default() -> Self {
        LLMProvider::OpenAI
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub provider: TranscriptionProvider,
    pub llm_provider: LLMProvider,
    pub api_key: String,
    pub groq_api_key: String,
    pub model: String,
    pub hotkey: String,
    pub simulate_typing: bool,
    pub copy_to_clipboard: bool,
    pub auto_start: bool,
    pub use_streaming: bool,
    pub auto_translate: bool,
    pub target_language: String,
    pub use_custom_instructions: bool,
    pub custom_instructions: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: TranscriptionProvider::OpenAI,
            llm_provider: LLMProvider::OpenAI,
            api_key: String::new(),
            groq_api_key: String::new(),
            model: "gpt-4o-transcribe".to_string(),
            hotkey: DEFAULT_HOTKEY.to_string(),
            simulate_typing: true,
            copy_to_clipboard: true,
            auto_start: false,
            use_streaming: true,
            auto_translate: false,
            target_language: "русский".to_string(),
            use_custom_instructions: false,
            custom_instructions: String::new(),
        }
    }
}

impl AppSettings {
    pub fn normalized_hotkey(&self) -> String {
        let candidate = self.hotkey.trim();
        if candidate.is_empty() {
            DEFAULT_HOTKEY.to_string()
        } else {
            candidate.replace("  ", " ")
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.api_key = self.api_key.trim().to_string();
        self.groq_api_key = self.groq_api_key.trim().to_string();
        self.model = if self.model.trim().is_empty() {
            "gpt-4o-transcribe".to_string()
        } else {
            self.model.trim().to_string()
        };
        self.hotkey = self.normalized_hotkey();
        self.target_language = if self.target_language.trim().is_empty() {
            "русский".to_string()
        } else {
            self.target_language.trim().to_string()
        };
        self.custom_instructions = self.custom_instructions.trim().to_string();
        if self.use_custom_instructions && self.custom_instructions.is_empty() {
            self.use_custom_instructions = false;
        }
        self
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    root: PathBuf,
}

impl SettingsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn file_path(&self) -> PathBuf {
        self.root.join(CONFIG_FILE)
    }

    pub async fn load(&self) -> Result<AppSettings> {
        let path = self.file_path();
        if !path.exists() {
            return Ok(AppSettings::default());
        }

        let raw = async_fs::read(&path)
            .await
            .with_context(|| format!("Не удалось прочитать {path:?}"))?;
        let mut parsed: AppSettings = serde_json::from_slice(&raw)
            .with_context(|| format!("Не удалось распарсить {path:?}"))?;
        parsed = parsed.sanitized();
        Ok(parsed)
    }

    pub async fn save(&self, settings: &AppSettings) -> Result<()> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root).with_context(|| {
                format!("Не удалось создать каталог {root:?}", root = self.root)
            })?;
        }
        let serialized =
            serde_json::to_vec_pretty(settings).context("Ошибка сериализации настроек")?;
        async_fs::write(self.file_path(), serialized)
            .await
            .context("Ошибка записи настроек")
    }
}
