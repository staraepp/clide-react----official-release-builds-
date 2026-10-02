//! Dictation commands. Thin wrappers: the pipeline owns the behaviour.

use tauri::{AppHandle, Manager};

use crate::dictation::machine::DictationState;
use crate::dictation::pipeline;
use crate::insertion;
use crate::state::AppState;

const INSERTION_TEST_TEXT: &str =
    "Clide insertion test: every word reached this field exactly.";

#[tauri::command]
pub async fn start_dictation(app: AppHandle) {
    pipeline::start(&app).await;
}

#[tauri::command]
pub async fn stop_dictation(app: AppHandle) {
    pipeline::stop(&app).await;
}

#[tauri::command]
pub async fn cancel_dictation(app: AppHandle) {
    pipeline::cancel(&app).await;
}

/// Re-send the still-pending audio to the provider after a failure.
#[tauri::command]
pub async fn retry_dictation(app: AppHandle) {
    pipeline::retry(&app).await;
}

/// Acknowledge a finished or failed transaction and clear the HUD.
#[tauri::command]
pub fn dismiss_dictation(app: AppHandle) {
    pipeline::dismiss(&app);
}

/// The user pressed on the draggable transcript: keep the dashboard out of the
/// way of wherever they are about to drop it.
#[tauri::command]
pub fn begin_transcript_drag(app: AppHandle) {
    crate::hud::keep_main_window_behind(&app);
}

/// The authoritative state, for a window that just opened and missed the
/// events it would otherwise have been driven by.
#[tauri::command]
pub fn get_dictation_state(app: AppHandle) -> DictationState {
    app.state::<AppState>().session.state()
}

/// Let Settings prove the native delivery path without recording or spending
/// provider credit. The delay gives the user time to focus any editable field.
#[tauri::command]
pub async fn test_insertion() -> Result<String, String> {
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let target = insertion::focus::frontmost();
    let label = target.label().to_string();
    let method = tauri::async_runtime::spawn_blocking(move || {
        insertion::insert(INSERTION_TEST_TEXT, &target)
    })
        .await
        .map_err(|error| format!("The insertion test could not run: {error}"))?
        .map_err(|failure| failure.message)?;
    let method = match method {
        crate::dictation::machine::InsertionMethod::Accessibility => "Accessibility",
        crate::dictation::machine::InsertionMethod::Typed => "typing",
        crate::dictation::machine::InsertionMethod::ClipboardPaste => "clipboard paste",
    };
    Ok(format!("{label} via {method}"))
}
