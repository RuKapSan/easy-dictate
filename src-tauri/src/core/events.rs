use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::state::AppState;

pub const EVENT_STATUS: &str = "transcription://status";
pub const EVENT_PARTIAL: &str = "transcription://partial";
pub const EVENT_COMPLETE: &str = "transcription://complete";
pub const EVENT_SETTINGS_CHANGED: &str = "settings://changed";

#[derive(Clone, Copy, Debug)]
pub enum StatusPhase {
    Idle,
    Recording,
    Transcribing,
    Success,
    Error,
}

impl StatusPhase {
    pub fn key(self) -> &'static str {
        match self {
            StatusPhase::Idle => "idle",
            StatusPhase::Recording => "recording",
            StatusPhase::Transcribing => "transcribing",
            StatusPhase::Success => "success",
            StatusPhase::Error => "error",
        }
    }

    pub fn default_message(self) -> &'static str {
        match self {
            StatusPhase::Idle => "Ready. Use the global hotkey to start a recording.",
            StatusPhase::Recording => "Listening... release the hotkey to stop.",
            StatusPhase::Transcribing => "Transcribing audio...",
            StatusPhase::Success => "Transcription complete.",
            StatusPhase::Error => "Something went wrong.",
        }
    }

    pub fn tray_label(self) -> &'static str {
        match self {
            StatusPhase::Idle => "Status: Idle",
            StatusPhase::Recording => "Status: Recording",
            StatusPhase::Transcribing => "Status: Transcribing",
            StatusPhase::Success => "Status: Complete",
            StatusPhase::Error => "Status: Error",
        }
    }
}

#[derive(Clone, Serialize)]
struct StatusPayload<'a> {
    phase: &'static str,
    message: &'a str,
}

#[derive(Clone, Serialize)]
struct TextPayload<'a> {
    text: &'a str,
}

pub fn emit_status(app: &AppHandle, phase: StatusPhase, message: Option<&str>) {
    let text = message.unwrap_or_else(|| phase.default_message());
    if let Err(e) = app.emit(
        EVENT_STATUS,
        StatusPayload {
            phase: phase.key(),
            message: text,
        },
    ) {
        log::error!("[Events] Failed to emit status event: {}", e);
    }

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(guard) = state.tray_status_item().lock() {
            if let Some(item) = guard.as_ref() {
                if let Err(e) = item.set_text(phase.tray_label()) {
                    log::warn!("[Events] Failed to update tray status: {}", e);
                }
            }
        }
    }
}

pub fn emit_partial(app: &AppHandle, text: &str) {
    if let Err(e) = app.emit(EVENT_PARTIAL, TextPayload { text }) {
        log::error!("[Events] Failed to emit partial event: {}", e);
    }
}

pub fn emit_complete(app: &AppHandle, text: &str) {
    if let Err(e) = app.emit(EVENT_COMPLETE, TextPayload { text }) {
        log::error!("[Events] Failed to emit complete event: {}", e);
    }
}

pub fn emit_error(app: &AppHandle, message: &str) {
    emit_status(app, StatusPhase::Error, Some(message));
}

#[derive(Clone, Serialize)]
pub struct SettingsChangedPayload {
    pub auto_translate: bool,
    pub target_language: String,
}

pub fn emit_settings_changed(app: &AppHandle, auto_translate: bool, target_language: &str) {
    if let Err(e) = app.emit(
        EVENT_SETTINGS_CHANGED,
        SettingsChangedPayload {
            auto_translate,
            target_language: target_language.to_string(),
        },
    ) {
        log::error!("[Events] Failed to emit settings-changed event: {}", e);
    }
}
