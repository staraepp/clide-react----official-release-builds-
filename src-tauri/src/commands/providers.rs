//! Choosing which on-device engine and model transcribe dictation.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::database::{now_ms, providers as provider_store};
use crate::providers::ProviderDescriptor;
use crate::state::AppState;

/// What the UI needs to know about each engine.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    /// Whether the engine has at least one model it can run right now.
    pub ready: bool,
    pub model_id: String,
    pub model_name: String,
    pub selected: bool,
}

#[tauri::command]
pub fn list_providers(app: AppHandle) -> Vec<ProviderDescriptor> {
    app.state::<AppState>().providers.descriptors()
}

#[tauri::command]
pub fn get_provider_status(app: AppHandle) -> Result<Vec<ProviderStatus>, String> {
    let state = app.state::<AppState>();
    let settings = state.settings();

    Ok(state
        .providers
        .descriptors()
        .into_iter()
        .map(|descriptor| {
            let stored = provider_store::get(&state.db.lock(), &descriptor.id)
                .ok()
                .flatten();

            let model_id = stored
                .as_ref()
                .and_then(|config| config.model_id.clone())
                .unwrap_or_else(|| descriptor.default_model.clone());

            let model_name = descriptor
                .models
                .iter()
                .find(|model| model.id == model_id)
                .map(|model| model.name.clone())
                .unwrap_or_else(|| model_id.clone());

            ProviderStatus {
                ready: !descriptor.models.is_empty(),
                selected: settings.provider_id == descriptor.id,
                id: descriptor.id,
                name: descriptor.name,
                model_id,
                model_name,
            }
        })
        .collect())
}

/// Choose the provider and model used for dictation.
#[tauri::command]
pub fn select_provider(
    app: AppHandle,
    provider_id: String,
    model_id: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| format!("Unknown provider \"{provider_id}\"."))?;

    let model_id = model_id.unwrap_or_else(|| provider.default_model().to_string());
    if !provider.has_model(&model_id) {
        return Err(format!(
            "{} does not offer the model \"{model_id}\".",
            provider.name()
        ));
    }

    provider_store::set_model(&state.db.lock(), &provider_id, &model_id, now_ms())
        .map_err(|error| error.to_string())?;

    // Apple Speech needs macOS's speech-recognition consent, which is separate
    // from the microphone even though recognition is on-device. Asking here —
    // at the moment the user chooses it — keeps the prompt tied to the reason
    // for it, and stops the provider failing every attempt with
    // "macOS hasn't been asked yet".
    if provider_id == "apple" {
        let granted = tauri::async_runtime::spawn_blocking(
            crate::permissions::request_speech_access,
        );
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            match granted.await {
                Ok(status) => {
                    tracing::info!(?status, "speech recognition permission resolved");
                    // The Setup card reads permissions fresh; nudge it.
                    crate::dictation::events::emit_bare(
                        &app_handle,
                        crate::dictation::events::SETTINGS_CHANGED,
                    );
                }
                Err(error) => {
                    tracing::warn!(?error, "speech permission request did not complete")
                }
            }
        });
    }

    state.update_settings(|settings| {
        settings.provider_id = provider_id.clone();
        settings.model_id = model_id.clone();
    })?;

    Ok(())
}
