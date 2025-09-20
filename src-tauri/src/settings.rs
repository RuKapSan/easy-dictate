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

    pub fn is_valid_hotkey(&self) -> bool {
        let hotkey = self.normalized_hotkey();
        if hotkey.is_empty() {
            return false;
        }

        let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return false;
        }

        // Check if last part is a valid main key (not a modifier)
        let main_key = parts.last().unwrap();
        let modifiers = &parts[..parts.len()-1];

        // Valid main keys
        let valid_main_keys = [
            "Space", "Escape", "Enter", "Tab", "Backspace", "Delete",
            "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "CapsLock",
            "PageUp", "PageDown", "Home", "End", "Insert", "Pause",
            "PrintScreen", "ScrollLock", "ContextMenu", "Backquote",
            "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash",
            "Semicolon", "Quote", "Comma", "Period", "Slash"
        ];

        let valid_function_keys = (1..=24).map(|i| format!("F{i}")).collect::<Vec<_>>();
        let valid_digit_keys = (0..=9).map(|i| i.to_string()).collect::<Vec<_>>();
        let valid_letter_keys = (b'A'..=b'Z').map(|c| (c as char).to_string()).collect::<Vec<_>>();

        let all_valid_keys = [
            valid_main_keys.iter().map(|s| *s).collect::<Vec<_>>(),
            valid_function_keys,
            valid_digit_keys,
            valid_letter_keys,
        ].concat();

        if !all_valid_keys.contains(&main_key) {
            return false;
        }

        // Check modifiers are valid
        let valid_modifiers = ["Ctrl", "Shift", "Alt", "Win"];
        for modifier in modifiers {
            if !valid_modifiers.contains(modifier) {
                return false;
            }
        }

        // Must have at least one modifier or be a function key
        if modifiers.is_empty() && !main_key.starts_with('F') {
            return false;
        }

        true
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

        // Validate API keys for selected providers
        if self.provider == TranscriptionProvider::OpenAI && self.api_key.is_empty() {
            return AppSettings::default();
        }
        if self.provider == TranscriptionProvider::Groq && self.groq_api_key.is_empty() {
            return AppSettings::default();
        }

        // Validate LLM provider API keys if needed
        let needs_llm = self.auto_translate || (self.use_custom_instructions && !self.custom_instructions.is_empty());
        if needs_llm {
            if self.llm_provider == LLMProvider::OpenAI && self.api_key.is_empty() {
                return AppSettings::default();
            }
            if self.llm_provider == LLMProvider::Groq && self.groq_api_key.is_empty() {
                return AppSettings::default();
            }
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
            async_fs::create_dir_all(&self.root).await.with_context(|| {
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
