//! Settings and the aggregate readiness view the dashboard renders.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::dictation::events;
use crate::dictation::machine::DictationBehavior;
use crate::permissions::{self, PermissionSnapshot};
use crate::processing::ProcessingMode;
use crate::settings::{AppSettings, VisualIntensity};
use crate::shortcuts;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    app.state::<AppState>().settings()
}

/// Everything the dashboard's System and Dictation cards need, in one call.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub permissions: PermissionSnapshot,
    pub settings: AppSettings,
    /// The accelerator macOS actually accepted, if any.
    pub registered_shortcut: Option<String>,
    pub shortcut_registered: bool,
    pub provider_name: String,
    pub model_name: String,
    /// Whether the selected engine can run the selected model right now. False
    /// when the chosen local model has not been downloaded.
    pub provider_ready: bool,
    /// The selected engine accepts a vocabulary hint, so technical vocabulary
    /// can actually take effect.
    pub provider_prompting: bool,
    /// The selected engine can recognise speech as it arrives, so live typing
    /// can take effect.
    pub provider_streaming: bool,
    /// True when this build is ad-hoc signed, which makes macOS drop the
    /// Accessibility grant on every rebuild even though System Settings still
    /// shows the switch on. Lets the UI explain the contradiction.
    pub ad_hoc_build: bool,
    /// True when a dictation would work end to end right now.
    pub ready: bool,
}

#[tauri::command]
pub fn get_system_status(app: AppHandle) -> SystemStatus {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let permissions = permissions::snapshot();
    let registered_shortcut = state.registered_shortcut();

    let (provider_name, model_name) = match state.providers.get(&settings.provider_id) {
        Some(provider) => {
            let model_name = provider
                .models()
                .into_iter()
                .find(|model| model.id == settings.model_id)
                .map(|model| model.name)
                .unwrap_or_else(|| settings.model_id.clone());
            (provider.name().to_string(), model_name)
        }
        None => (settings.provider_id.clone(), settings.model_id.clone()),
    };

    let provider_ready = state
        .providers
        .get(&settings.provider_id)
        .is_some_and(|provider| provider.has_model(&settings.model_id));
    let provider_prompting = state
        .providers
        .get(&settings.provider_id)
        .is_some_and(|provider| provider.capabilities().prompting);
    let provider_streaming = state
        .providers
        .get(&settings.provider_id)
        .is_some_and(|provider| provider.capabilities().streaming);
    let shortcut_registered = registered_shortcut.is_some();

    SystemStatus {
        ready: permissions.can_capture()
            && permissions.can_insert()
            && shortcut_registered
            && provider_ready,
        permissions,
        settings,
        registered_shortcut,
        shortcut_registered,
        provider_name,
        model_name,
        provider_ready,
        provider_prompting,
        provider_streaming,
        ad_hoc_build: crate::permissions::is_ad_hoc(),
    }
}

/// Change the global shortcut.
///
/// Registration is attempted immediately so the user finds out here — not the
/// next time they try to dictate — that another app already owns the keys.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, accelerator: String) -> Result<(), String> {
    if !shortcuts::is_valid(&accelerator) {
        return Err(format!("\"{accelerator}\" is not a valid shortcut."));
    }

    shortcuts::register(&app, &accelerator).map_err(|error| error.to_string())?;

    app.state::<AppState>()
        .update_settings(|settings| settings.shortcut = accelerator.clone())?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_dictation_behavior(
    app: AppHandle,
    behavior: DictationBehavior,
) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.behavior = behavior)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_processing_mode(app: AppHandle, mode: ProcessingMode) -> Result<(), String> {
    if !mode.is_available() {
        return Err("That mode is not available yet.".into());
    }
    app.state::<AppState>()
        .update_settings(|settings| settings.mode = mode)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_visual_intensity(app: AppHandle, intensity: VisualIntensity) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.visual_intensity = intensity)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_language(app: AppHandle, language: Option<String>) -> Result<(), String> {
    let language = language.filter(|value| !value.trim().is_empty());
    app.state::<AppState>()
        .update_settings(|settings| settings.language = language.clone())?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.onboarding_complete = true)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Let the user go back through onboarding from settings.
#[tauri::command]
pub fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.onboarding_complete = false)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Choose what Clide may substitute when the selected engine cannot run.
///
/// `anyConfigured` is the only value that lets a recording reach a cloud vendor
/// the user did not pick, which is why it is opt-in rather than the default.
#[tauri::command]
pub fn set_fallback_policy(
    app: AppHandle,
    fallback: crate::dictation::fallback::FallbackPolicy,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.fallback = fallback)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// The refinement engines this build knows about, with live availability.
///
/// Availability is read fresh: Apple Intelligence can be switched off in
/// System Settings while Clide is running.
#[tauri::command]
pub fn list_refiners(app: AppHandle) -> Vec<crate::refine::RefinerDescriptor> {
    app.state::<AppState>().refiners.descriptors()
}

/// How far Rewrite may go. Only consulted in Rewrite mode.
#[tauri::command]
pub fn set_refine_style(
    app: AppHandle,
    style: crate::refine::RefineStyle,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.refine_style = style)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Who this build is, and where to go with it.
///
/// The links live in Rust rather than hardcoded in the UI so the About panel
/// and any future menu item cannot drift apart.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct About {
    pub version: String,
    pub commit: &'static str,
    pub build_date: &'static str,
    pub repository: &'static str,
    pub website: &'static str,
    pub issues: &'static str,
    pub license: &'static str,
    pub tauri_version: &'static str,
}

#[tauri::command]
pub fn get_about() -> About {
    About {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: env!("CLIDE_COMMIT"),
        build_date: env!("CLIDE_BUILD_DATE"),
        repository: "https://github.com/staraepp/clide_stt",
        website: "https://clide.staraep.fun",
        issues: "https://github.com/staraepp/clide_stt/issues",
        license: "MIT",
        tauri_version: "2",
    }
}

#[cfg(test)]
mod readiness_tests {
    use crate::models::ModelStore;
    use crate::providers::ProviderRegistry;

    /// A fresh install has no downloaded models, yet must be able to dictate:
    /// the default engine has to count as ready on its own.
    #[test]
    fn the_default_engine_is_ready_on_a_fresh_install() {
        let registry = ProviderRegistry::new(ModelStore::new(
            &std::env::temp_dir().join("clide-readiness-fresh"),
        ));
        let provider = registry.default_provider();
        assert!(provider.has_model(provider.default_model()));
    }
}

/// Choose the model Rewrite uses on engines that offer a choice. `None` lets
/// the engine pick the best installed one.
#[tauri::command]
pub fn set_refine_model(app: AppHandle, model: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.refine_model = model)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Choose when developer vocabulary primes the speech engine.
#[tauri::command]
pub fn set_technical_vocabulary(
    app: AppHandle,
    setting: crate::context::TechnicalVocabulary,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.technical_vocabulary = setting)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Turn code formatting of spoken file paths on or off.
///
/// Off by default: the backticks are literal characters, and typed into a
/// terminal they are shell command substitution.
#[tauri::command]
pub fn set_format_technical_terms(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.format_technical_terms = enabled)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Type words as they are spoken, on engines that can stream.
#[tauri::command]
pub fn set_live_typing(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.live_typing = enabled)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

// --- local API -------------------------------------------------------------

/// Switch the loopback API on or off. Off means the port is closed at once.
#[tauri::command]
pub fn set_local_api_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.local_api_enabled = enabled)?;
    crate::api::apply(&app);
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_local_api_port(app: AppHandle, port: u32) -> Result<(), String> {
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port >= crate::settings::MIN_API_PORT)
        .ok_or_else(|| {
            format!(
                "Choose a port between {} and 65535.",
                crate::settings::MIN_API_PORT
            )
        })?;
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.local_api_port = port)?;
    crate::api::apply(&app);
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Web origins allowed to call the API from a browser. Empty by default.
#[tauri::command]
pub fn set_local_api_origins(app: AppHandle, origins: Vec<String>) -> Result<(), String> {
    let mut cleaned: Vec<String> = Vec::new();
    for origin in origins {
        let origin = origin.trim().trim_end_matches('/').to_string();
        if origin.is_empty() {
            continue;
        }
        if !(origin.starts_with("http://") || origin.starts_with("https://")) || origin.contains(' ')
        {
            return Err(format!(
                "\"{origin}\" is not an origin. Use the form http://localhost:3000."
            ));
        }
        if !cleaned.contains(&origin) {
            cleaned.push(origin);
        }
    }
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.local_api_allowed_origins = cleaned)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_local_api_endpoints(
    app: AppHandle,
    transcription: bool,
    events_enabled: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| {
        settings.local_api_transcription = transcription;
        settings.local_api_events = events_enabled;
    })?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn get_local_api_status(app: AppHandle) -> crate::api::ApiStatus {
    crate::api::status(&app)
}

/// The bearer token, created the first time it is asked for.
#[tauri::command]
pub fn get_local_api_token(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let token = crate::api::token::load_or_create(&state.db.lock());
    token.map_err(|error| error.to_string())
}

/// Replace the token. Clients using the old one are refused from now on, and
/// open event streams end.
#[tauri::command]
pub fn regenerate_local_api_token(app: AppHandle) -> Result<String, String> {
    let token = {
        let state = app.state::<AppState>();
        let token = crate::api::token::regenerate(&state.db.lock());
        token.map_err(|error| error.to_string())?
    };
    crate::api::restart(&app);
    Ok(token)
}

/// Switch a refinement engine on or off.
///
/// Explicit rather than automatic: Rewrite only runs an engine the user has
/// switched on.
#[tauri::command]
pub fn set_refine_engine_enabled(
    app: AppHandle,
    engine_id: String,
    enabled: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();

    state.update_settings(|settings| {
        settings.refine_engines.retain(|id| id != &engine_id);
        if enabled {
            settings.refine_engines.push(engine_id.clone());
        }
    })?;

    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Turn spoken punctuation on or off.
///
/// Applies in every mode and costs no model call, which is why it is separate
/// from the Rewrite engines rather than one of them.
#[tauri::command]
pub fn set_spoken_punctuation(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.spoken_punctuation = enabled)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}
