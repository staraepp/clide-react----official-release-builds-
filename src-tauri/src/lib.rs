//! Clide — system-wide dictation for macOS.
//!
//! Rust owns everything native: the microphone, the global shortcut, provider
//! requests, the Accessibility integration, the Keychain, and persistence.
//! React owns presentation and nothing else.

pub mod audio;
pub mod commands;
pub mod context;
pub mod database;
pub mod dictation;
pub mod hud;
pub mod insertion;
pub mod models;
pub mod permissions;
pub mod processing;
pub mod providers;
pub mod refine;
pub mod settings;
pub mod shortcuts;
pub mod state;
pub mod tray;

use std::time::Duration;

use tauri::{AppHandle, Listener, Manager};

use audio::Recorder;
use database::Database;
use models::ModelStore;
use providers::ProviderRegistry;
use state::AppState;

/// How often expired temporary audio is swept up.
const AUDIO_REAPER_INTERVAL: Duration = Duration::from_secs(30);

/// Network timeout for the update check: short enough that a dead endpoint
/// fails while the user is still paying attention.
const HTTP_TIMEOUT: Duration = Duration::from_secs(90);

/// Model downloads get their own client, and deliberately **no total
/// timeout**.
///
/// `reqwest`'s `timeout` covers the whole request *including reading the
/// body*, so a 90-second cap kills any download longer than 90 seconds — which
/// is every model in the catalogue. The symptom is an opaque
/// "error decoding response body" partway through.
///
/// A stalled connection is still caught, by these two instead: the host has to
/// answer, and once streaming, bytes have to keep arriving.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CLIDE_LOG")
                .unwrap_or_else(|_| "clide=info,warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    shortcuts::on_shortcut(app, event.state());
                })
                .build(),
        )
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::dictation::start_dictation,
            commands::dictation::stop_dictation,
            commands::dictation::cancel_dictation,
            commands::dictation::retry_dictation,
            commands::dictation::dismiss_dictation,
            commands::dictation::get_dictation_state,
            commands::dictation::test_insertion,
            commands::permissions::get_permissions,
            commands::permissions::request_microphone_permission,
            commands::permissions::request_speech_permission,
            commands::permissions::request_accessibility_permission,
            commands::permissions::repair_accessibility_permission,
            commands::permissions::open_accessibility_settings,
            commands::permissions::open_microphone_settings,
            commands::providers::list_providers,
            commands::providers::get_provider_status,
            commands::providers::select_provider,
            commands::history::get_history,
            commands::history::search_history,
            commands::history::get_usage,
            commands::models::list_models,
            commands::models::get_models_page,
            commands::models::download_model,
            commands::models::remove_model,
            commands::history::delete_transcript,
            commands::history::get_source_apps,
            commands::history::copy_text,
            commands::settings::get_settings,
            commands::settings::get_system_status,
            commands::settings::set_shortcut,
            commands::settings::set_dictation_behavior,
            commands::settings::set_processing_mode,
            commands::settings::set_visual_intensity,
            commands::settings::set_fallback_policy,
            commands::settings::list_refiners,
            commands::settings::set_refine_engine_enabled,
            commands::settings::set_spoken_punctuation,
            commands::settings::set_technical_vocabulary,
            commands::settings::set_refine_model,
            commands::settings::set_format_technical_terms,
            commands::settings::set_refine_style,
            commands::settings::get_about,
            commands::settings::set_language,
            commands::settings::complete_onboarding,
            commands::settings::reset_onboarding,
            commands::updates::check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("clide failed to start");
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    let data_dir = handle.path().app_data_dir()?;
    let cache_dir = handle.path().app_cache_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    // Temporary dictation audio lives in the cache directory: it is by
    // definition disposable, and macOS is free to reclaim it.
    let clip_dir = cache_dir.join("dictation-audio");

    let database = Database::open(&data_dir.join("clide.sqlite3"))?;
    let recorder = Recorder::spawn(clip_dir);
    let http = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("Clide/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let downloads = reqwest::Client::builder()
        .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
        .read_timeout(DOWNLOAD_READ_TIMEOUT)
        .user_agent(concat!("Clide/", env!("CARGO_PKG_VERSION")))
        .build()?;

    handle.manage(AppState::new(
        database,
        ModelStore::new(&data_dir),
        http,
        downloads,
        recorder,
        ProviderRegistry::new(ModelStore::new(&data_dir)),
    ));

    // Register the configured shortcut. A failure here is reported through
    // system status rather than being fatal: the app is still usable, and the
    // System card explains that the shortcut is inactive.
    let accelerator = handle.state::<AppState>().settings().shortcut;
    if let Err(error) = shortcuts::register(&handle, &accelerator) {
        tracing::warn!(%error, "the global shortcut is not active");
    }

    if let Err(error) = tray::build(&handle) {
        tracing::warn!(?error, "could not create the menu-bar item");
    }

    mirror_state_to_tray(&handle);
    spawn_audio_reaper(handle.clone());

    Ok(())
}

/// Keep the menu-bar tooltip in step with the dictation state.
fn mirror_state_to_tray(app: &AppHandle) {
    let handle = app.clone();
    app.listen(dictation::events::STATE, move |event| {
        if let Ok(state) = serde_json::from_str::<dictation::DictationState>(event.payload()) {
            tray::update_status(&handle, &state);
        }
    });
}

/// Delete temporary audio whose recovery window has passed.
///
/// The pipeline already deletes audio the moment a transaction resolves; this
/// covers the case where a failure is left on screen and never acknowledged.
fn spawn_audio_reaper(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(AUDIO_REAPER_INTERVAL).await;

            let state = app.state::<AppState>();
            if state.session.expire_stale_audio() {
                tracing::info!("temporary audio expired and was deleted");

                // A failure that offered Retry can no longer honour it.
                if let dictation::DictationState::TranscriptionFailed { message, .. } =
                    state.session.state()
                {
                    let next = state.session.apply(
                        dictation::machine::DictationInput::transcription_failure(message, false),
                    );
                    if let Ok(next) = next {
                        dictation::events::emit_state(&app, &next);
                    }
                }
            }
        }
    });
}
