//! The event contract between Rust and the UI.
//!
//! Two layers, on purpose:
//!
//! * `STATE` carries the whole `DictationState` and is what the UI actually
//!   renders from. One authoritative value means the interface cannot show a
//!   contradictory combination of flags.
//! * The lifecycle events mirror the pipeline stages for anything that needs
//!   to react to a specific moment (the HUD's success chime, a future menu-bar
//!   animation) without diffing state.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::machine::DictationState;
use crate::state::AppState;

pub const STATE: &str = "dictation:state";
pub const LEVEL: &str = "dictation:level";

pub const DICTATION_STARTED: &str = "dictation:started";
pub const DICTATION_STOPPED: &str = "dictation:stopped";

pub const TRANSCRIPTION_STARTED: &str = "transcription:started";
pub const TRANSCRIPTION_COMPLETE: &str = "transcription:complete";
pub const TRANSCRIPTION_FAILED: &str = "transcription:failed";
/// Emitted when a substitute engine served the transcription. A fallback is
/// never silent — see `dictation::fallback`.
pub const TRANSCRIPTION_FELL_BACK: &str = "transcription:fell-back";

/// Emitted when spoken corrections ("scratch that") changed the transcript.
pub const CORRECTION_APPLIED: &str = "dictation:correction-applied";

pub const PROCESSING_STARTED: &str = "processing:started";
pub const PROCESSING_COMPLETE: &str = "processing:complete";

pub const INSERTION_STARTED: &str = "insertion:started";
pub const INSERTION_COMPLETE: &str = "insertion:complete";
pub const INSERTION_FAILED: &str = "insertion:failed";

/// A new row landed in history; the dashboard and history view refresh.
pub const HISTORY_CHANGED: &str = "history:changed";

/// Settings changed in the backend (e.g. a shortcut re-registration).
pub const SETTINGS_CHANGED: &str = "settings:changed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelPayload {
    /// 0.0..=1.0 microphone RMS.
    pub level: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPayload {
    pub text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailurePayload {
    pub message: String,
    /// Whether the UI should offer Retry.
    pub retryable: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedPayload {
    /// The app that will receive the text, for the HUD's context line.
    pub target_app: Option<String>,
}

pub fn emit_state(app: &AppHandle, state: &DictationState) {
    if let Err(error) = app.emit(STATE, state) {
        tracing::warn!(?error, "could not emit dictation state");
    }

    // The same change, for the local API's event stream.
    if let Some(app_state) = app.try_state::<AppState>() {
        let settings = app_state.settings();
        let mode = serde_json::to_value(settings.mode)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        app_state
            .events
            .publish_state(state, &mode, &settings.model_id);
    }
}

pub fn emit<T: Serialize + Clone>(app: &AppHandle, name: &str, payload: T) {
    if let Err(error) = app.emit(name, payload) {
        tracing::warn!(?error, name, "could not emit event");
    }
}

pub fn emit_bare(app: &AppHandle, name: &str) {
    emit(app, name, ());
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionPayload {
    /// How many "scratch that" style commands were applied.
    pub count: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackPayload {
    pub failed_provider: String,
    /// The display name of whatever actually ran, for the HUD.
    pub used_provider: String,
    pub used_model: String,
}
