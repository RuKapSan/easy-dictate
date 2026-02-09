use serde::ser::SerializeMap;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Settings(#[from] crate::settings::SettingsValidationError),

    #[error("{0}")]
    Hotkey(#[from] anyhow::Error),

    #[error("{0}")]
    Io(String),

    #[error("{0}")]
    Lock(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    #[allow(dead_code)] // Used in release builds (#[cfg(not(debug_assertions))])
    Unavailable(String),
}

impl CommandError {
    fn code(&self) -> &'static str {
        match self {
            Self::Settings(_) => "settings",
            Self::Hotkey(_) => "hotkey",
            Self::Io(_) => "io",
            Self::Lock(_) => "lock",
            Self::NotFound(_) => "not_found",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

// Tauri commands require the error type to implement Serialize.
// We serialize as { code, message } so the frontend can branch on error type.
impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("code", self.code())?;
        map.serialize_entry("message", &self.to_string())?;
        map.end()
    }
}
