use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs as async_fs;

const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
const CONFIG_FILE: &str = "settings.json";
const DEFAULT_MODEL: &str = "gpt-4o-transcribe";
const DEFAULT_TARGET_LANGUAGE: &str = "English";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProvider {
    #[default]
    OpenAI,
    Groq,
    ElevenLabs,
    /// Mock provider for E2E testing without API keys
    /// Returns a hardcoded response after a short delay
    #[serde(rename = "mock")]
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    #[default]
    OpenAI,
    Groq,
}

impl TranscriptionProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            TranscriptionProvider::OpenAI => "OpenAI",
            TranscriptionProvider::Groq => "Groq",
            TranscriptionProvider::ElevenLabs => "ElevenLabs",
            TranscriptionProvider::Mock => "Mock (Testing)",
        }
    }

    pub fn is_mock(&self) -> bool {
        matches!(self, TranscriptionProvider::Mock)
    }
}

impl LLMProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            LLMProvider::OpenAI => "OpenAI",
            LLMProvider::Groq => "Groq",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub provider: TranscriptionProvider,
    pub llm_provider: LLMProvider,
    pub api_key: String,
    pub groq_api_key: String,
    pub elevenlabs_api_key: String,
    pub model: String,
    pub hotkey: String,
    pub translate_hotkey: String,
    pub toggle_translate_hotkey: String,
    pub simulate_typing: bool,
    pub copy_to_clipboard: bool,
    pub auto_start: bool,
    pub auto_update: bool,
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
            elevenlabs_api_key: String::new(),
            model: DEFAULT_MODEL.to_string(),
            hotkey: DEFAULT_HOTKEY.to_string(),
            translate_hotkey: String::new(),
            toggle_translate_hotkey: String::new(),
            simulate_typing: true,
            copy_to_clipboard: true,
            auto_start: false,
            auto_update: true,
            use_streaming: true,
            auto_translate: false,
            target_language: DEFAULT_TARGET_LANGUAGE.to_string(),
            use_custom_instructions: false,
            custom_instructions: String::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsValidationError {
    #[error("A global hotkey must be set.")]
    MissingHotkey,
    #[error("Global hotkey '{0}' is not valid.")]
    InvalidHotkey(String),
    #[error("{0} API key is required.")]
    MissingApiKey(&'static str),
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

    pub fn normalized(mut self) -> Self {
        self.api_key = self.api_key.trim().to_string();
        self.groq_api_key = self.groq_api_key.trim().to_string();
        self.model = if self.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            self.model.trim().to_string()
        };
        self.hotkey = self.normalized_hotkey();
        self.translate_hotkey = self.translate_hotkey.trim().to_string();
        self.toggle_translate_hotkey = self.toggle_translate_hotkey.trim().to_string();
        self.target_language = if self.target_language.trim().is_empty() {
            DEFAULT_TARGET_LANGUAGE.to_string()
        } else {
            self.target_language.trim().to_string()
        };
        self.custom_instructions = self.custom_instructions.trim().to_string();
        if !self.use_custom_instructions || self.custom_instructions.is_empty() {
            self.use_custom_instructions = false;
        }
        self
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

        let main_key = parts.last().copied().unwrap_or("");
        let modifiers = &parts[..parts.len() - 1];

        let mut valid_keys: HashSet<String> = [
            "Space",
            "Escape",
            "Enter",
            "Tab",
            "Backspace",
            "Delete",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "CapsLock",
            "PageUp",
            "PageDown",
            "Home",
            "End",
            "Insert",
            "Pause",
            "PrintScreen",
            "ScrollLock",
            "ContextMenu",
            "Backquote",
            "Minus",
            "Equal",
            "BracketLeft",
            "BracketRight",
            "Backslash",
            "Semicolon",
            "Quote",
            "Comma",
            "Period",
            "Slash",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        valid_keys.extend((1..=24).map(|i| format!("F{i}")));
        valid_keys.extend((0..=9).map(|i| i.to_string()));
        valid_keys.extend((b'A'..=b'Z').map(|c| (c as char).to_string()));

        if !valid_keys.contains(main_key) {
            return false;
        }

        let valid_modifiers = ["Ctrl", "Shift", "Alt", "Win"];
        for modifier in modifiers {
            if !valid_modifiers.contains(modifier) {
                return false;
            }
        }

        if modifiers.is_empty() && !main_key.starts_with('F') {
            return false;
        }

        true
    }

    pub fn requires_llm(&self) -> bool {
        self.auto_translate
            || (self.use_custom_instructions && !self.custom_instructions.trim().is_empty())
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        let hotkey = self.normalized_hotkey();
        if hotkey.is_empty() {
            return Err(SettingsValidationError::MissingHotkey);
        }
        if !self.is_valid_hotkey() {
            return Err(SettingsValidationError::InvalidHotkey(hotkey));
        }

        // Note: We don't validate API keys here during save_settings.
        // API keys are validated when actually needed (before transcription).
        // This allows users to save other settings (hotkey, simulate_typing, etc.)
        // without being blocked by missing API keys.

        Ok(())
    }

    /// Validate that required API keys are present for the current configuration
    /// This should be called before performing transcription, not during settings save
    pub fn validate_for_transcription(&self) -> Result<(), SettingsValidationError> {
        match self.provider {
            TranscriptionProvider::Mock => {} // Mock doesn't need API key
            TranscriptionProvider::OpenAI if self.api_key.trim().is_empty() => {
                return Err(SettingsValidationError::MissingApiKey("OpenAI"));
            }
            TranscriptionProvider::Groq if self.groq_api_key.trim().is_empty() => {
                return Err(SettingsValidationError::MissingApiKey("Groq"));
            }
            TranscriptionProvider::ElevenLabs if self.elevenlabs_api_key.trim().is_empty() => {
                return Err(SettingsValidationError::MissingApiKey("ElevenLabs"));
            }
            _ => {} // API key is present
        }

        if self.requires_llm() {
            match self.llm_provider {
                LLMProvider::OpenAI if self.api_key.trim().is_empty() => {
                    return Err(SettingsValidationError::MissingApiKey("OpenAI"));
                }
                LLMProvider::Groq if self.groq_api_key.trim().is_empty() => {
                    return Err(SettingsValidationError::MissingApiKey("Groq"));
                }
                _ => {} // API key is present
            }
        }

        Ok(())
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
            .with_context(|| format!("Failed to read {path:?}"))?;
        let parsed: AppSettings =
            serde_json::from_slice(&raw).with_context(|| format!("Failed to parse {path:?}"))?;
        Ok(parsed.normalized())
    }

    pub async fn save(&self, settings: &AppSettings) -> Result<()> {
        if !self.root.exists() {
            async_fs::create_dir_all(&self.root)
                .await
                .with_context(|| {
                    format!(
                        "Failed to create config directory {root:?}",
                        root = self.root
                    )
                })?;
        }

        let normalized = settings.clone().normalized();
        let serialized = serde_json::to_vec_pretty(&normalized)
            .context("Failed to serialize settings to JSON")?;

        async_fs::write(self.file_path(), serialized)
            .await
            .context("Failed to write settings file")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert_eq!(settings.provider, TranscriptionProvider::OpenAI);
        assert_eq!(settings.hotkey, "Ctrl+Shift+Space");
        assert_eq!(settings.model, "gpt-4o-transcribe");
        assert!(settings.simulate_typing);
        assert!(settings.copy_to_clipboard);
        assert!(!settings.auto_start);
        assert!(settings.auto_update);
    }

    #[test]
    fn test_transcription_provider_is_mock() {
        assert!(TranscriptionProvider::Mock.is_mock());
        assert!(!TranscriptionProvider::OpenAI.is_mock());
        assert!(!TranscriptionProvider::Groq.is_mock());
        assert!(!TranscriptionProvider::ElevenLabs.is_mock());
    }

    #[test]
    fn test_normalized_replaces_empty_hotkey_with_default() {
        let mut settings = AppSettings::default();
        settings.hotkey = "".to_string();

        let normalized = settings.normalized();
        assert_eq!(normalized.hotkey, "Ctrl+Shift+Space");
    }

    #[test]
    fn test_validate_invalid_hotkey_no_modifiers() {
        let mut settings = AppSettings::default();
        settings.hotkey = "A".to_string(); // No modifiers

        let result = settings.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_hotkey() {
        let mut settings = AppSettings::default();
        settings.hotkey = "Ctrl+Shift+A".to_string();

        let result = settings.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_allows_save_without_api_key() {
        let mut settings = AppSettings::default();
        settings.api_key = "".to_string(); // Empty API key
        settings.hotkey = "Ctrl+Shift+Space".to_string();

        // Should succeed - API key not required for saving
        let result = settings.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_for_transcription_requires_api_key() {
        let mut settings = AppSettings::default();
        settings.provider = TranscriptionProvider::OpenAI;
        settings.api_key = "".to_string();

        let result = settings.validate_for_transcription();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SettingsValidationError::MissingApiKey("OpenAI")));
    }

    #[test]
    fn test_validate_for_transcription_mock_no_api_key() {
        let mut settings = AppSettings::default();
        settings.provider = TranscriptionProvider::Mock;
        settings.api_key = "".to_string();

        // Mock provider doesn't need API key
        let result = settings.validate_for_transcription();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_for_transcription_with_api_key() {
        let mut settings = AppSettings::default();
        settings.provider = TranscriptionProvider::OpenAI;
        settings.api_key = "sk-test123".to_string();

        let result = settings.validate_for_transcription();
        assert!(result.is_ok());
    }

    #[test]
    fn test_normalized_trims_whitespace() {
        let mut settings = AppSettings::default();
        settings.api_key = "  sk-test123  ".to_string();
        settings.model = "  gpt-4  ".to_string();
        settings.hotkey = "  Ctrl+Shift+A  ".to_string();

        let normalized = settings.normalized();
        assert_eq!(normalized.api_key, "sk-test123");
        assert_eq!(normalized.model, "gpt-4");
        assert_eq!(normalized.hotkey, "Ctrl+Shift+A");
    }

    #[test]
    fn test_normalized_uses_defaults_for_empty() {
        let mut settings = AppSettings::default();
        settings.model = "".to_string();
        settings.hotkey = "".to_string();
        settings.target_language = "".to_string();

        let normalized = settings.normalized();
        assert_eq!(normalized.model, "gpt-4o-transcribe");
        assert_eq!(normalized.hotkey, "Ctrl+Shift+Space");
        assert_eq!(normalized.target_language, "English");
    }

    #[test]
    fn test_requires_llm_when_auto_translate() {
        let mut settings = AppSettings::default();
        settings.auto_translate = true;
        assert!(settings.requires_llm());
    }

    #[test]
    fn test_requires_llm_when_custom_instructions() {
        let mut settings = AppSettings::default();
        settings.use_custom_instructions = true;
        settings.custom_instructions = "Custom prompt".to_string();
        assert!(settings.requires_llm());
    }

    #[test]
    fn test_does_not_require_llm_by_default() {
        let settings = AppSettings::default();
        assert!(!settings.requires_llm());
    }

    #[test]
    fn test_is_valid_hotkey_with_function_keys_no_modifiers() {
        let mut settings = AppSettings::default();

        // Function keys can be used without modifiers
        for i in 1..=24 {
            settings.hotkey = format!("F{}", i);
            assert!(settings.is_valid_hotkey(), "F{} should be valid", i);
        }
    }

    #[test]
    fn test_is_valid_hotkey_with_modifiers() {
        let mut settings = AppSettings::default();

        let valid_combinations = vec![
            "Ctrl+A",
            "Shift+B",
            "Alt+C",
            "Win+D",
            "Ctrl+Shift+E",
            "Ctrl+Alt+F",
            "Ctrl+Shift+Alt+G",
        ];

        for hotkey in valid_combinations {
            settings.hotkey = hotkey.to_string();
            assert!(settings.is_valid_hotkey(), "{} should be valid", hotkey);
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = AppSettings::default();
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original.provider, deserialized.provider);
        assert_eq!(original.hotkey, deserialized.hotkey);
        assert_eq!(original.model, deserialized.model);
    }
}
