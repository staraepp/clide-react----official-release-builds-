//! Application state shared across Tauri commands.

use std::sync::Mutex;

use crate::audio::Recorder;
use crate::database::Database;
use crate::models::ModelStore;
use crate::dictation::DictationSession;
use crate::providers::ProviderRegistry;
use crate::refine::RefinerRegistry;
use crate::settings::{self, AppSettings};

pub struct AppState {
    pub db: Database,
    /// Local model weights on this machine.
    pub models: ModelStore,
    /// Separate client for model downloads: no total timeout, because that
    /// would cap how long a download may take. See `lib.rs`.
    pub downloads: reqwest::Client,
    pub recorder: Recorder,
    pub providers: ProviderRegistry,
    /// Text refinement, kept separate from transcription (blueprint §7).
    pub refiners: RefinerRegistry,
    pub session: DictationSession,
    /// The live-typing session attached to the dictation in progress, if any.
    pub live: crate::dictation::live::LiveSlot,

    /// Cached copy of the persisted preferences. The database stays the
    /// source of truth; this exists so the audio and shortcut paths never
    /// touch SQLite on a latency-sensitive code path.
    settings: Mutex<AppSettings>,

    /// The accelerator currently registered with macOS, if registration
    /// succeeded. `None` means the shortcut is configured but not active —
    /// usually because another app already owns it.
    registered_shortcut: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(
        db: Database,
        models: ModelStore,
        downloads: reqwest::Client,
        recorder: Recorder,
        providers: ProviderRegistry,
    ) -> Self {
        let refiners = RefinerRegistry::new();

        let settings = {
            let default_provider = providers.default_provider();
            let connection = db.lock();
            let mut loaded = settings::load(
                &connection,
                default_provider.id(),
                default_provider.default_model(),
            );
            if settings::reconcile(&mut loaded, &providers, &refiners) {
                if let Err(error) = settings::save(&connection, &loaded) {
                    tracing::warn!(%error, "could not persist the repaired preferences");
                }
            }
            loaded
        };

        Self {
            db,
            models,
            downloads,
            recorder,
            providers,
            refiners,
            session: DictationSession::new(),
            live: Mutex::new(None),
            settings: Mutex::new(settings),
            registered_shortcut: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Mutate and persist preferences in one step, so the cache and the
    /// database cannot drift apart.
    pub fn update_settings(
        &self,
        edit: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, String> {
        let mut guard = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        edit(&mut guard);
        settings::save(&self.db.lock(), &guard).map_err(|error| error.to_string())?;
        Ok(guard.clone())
    }

    pub fn registered_shortcut(&self) -> Option<String> {
        self.registered_shortcut
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_registered_shortcut(&self, accelerator: Option<String>) {
        *self
            .registered_shortcut
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = accelerator;
    }
}
