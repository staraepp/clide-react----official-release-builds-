//! The dictionary the Settings page edits.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::database::dictionary::{self, DictionaryQuery, Entry};
use crate::database::now_ms;
use crate::dictation::events;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryPage {
    pub entries: Vec<Entry>,
    /// Over the whole dictionary, not just the entries returned.
    pub counts: dictionary::Counts,
}

#[tauri::command]
pub fn get_dictionary(
    app: AppHandle,
    query: Option<DictionaryQuery>,
) -> Result<DictionaryPage, String> {
    let state = app.state::<AppState>();
    let connection = state.db.lock();
    let entries = dictionary::list(&connection, &query.unwrap_or_default())
        .map_err(|error| error.to_string())?;
    let counts = dictionary::counts(&connection).map_err(|error| error.to_string())?;
    Ok(DictionaryPage { entries, counts })
}

/// Add a word, or turn a learned one into the user's own.
#[tauri::command]
pub fn add_dictionary_word(app: AppHandle, word: String) -> Result<Entry, String> {
    let state = app.state::<AppState>();
    let entry = dictionary::add_manual(&state.db.lock(), &word, now_ms())
        .map_err(|error| error.to_string())?;
    events::emit_bare(&app, events::DICTIONARY_CHANGED);
    Ok(entry)
}

#[tauri::command]
pub fn remove_dictionary_word(app: AppHandle, word: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let removed =
        dictionary::remove(&state.db.lock(), &word).map_err(|error| error.to_string())?;
    if removed {
        events::emit_bare(&app, events::DICTIONARY_CHANGED);
    }
    Ok(removed)
}

/// Forget every learned word. The words the user added stay.
#[tauri::command]
pub fn clear_learned_words(app: AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let removed =
        dictionary::clear_learned(&state.db.lock()).map_err(|error| error.to_string())?;
    if removed > 0 {
        events::emit_bare(&app, events::DICTIONARY_CHANGED);
    }
    Ok(removed)
}
