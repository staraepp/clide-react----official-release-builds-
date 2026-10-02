//! Non-secret application preferences.
//!
//! Stored as individual rows so that one unreadable value falls back to its
//! default instead of resetting everything the user configured.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::context::TechnicalVocabulary;
use crate::dictation::fallback::FallbackPolicy;
use crate::refine::RefineStyle;

use crate::database::kv;
use crate::dictation::machine::DictationBehavior;
use crate::processing::ProcessingMode;
use crate::providers::ProviderRegistry;
use crate::refine::RefinerRegistry;

/// How much decorative rendering Clide is allowed to do.
///
/// This is a user preference, not a performance guess. macOS Reduce Motion is
/// honoured on top of it by the frontend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisualIntensity {
    /// Static background. No ambient animation at all.
    Reduced,
    #[default]
    Normal,
    /// Ambient motion plus reaction to dictation state.
    High,
}

/// The default shortcut. Matches what clide.dev tells people to press, and
/// it is unclaimed by macOS itself.
pub const DEFAULT_SHORTCUT: &str = "Alt+Period";

mod keys {
    pub const SHORTCUT: &str = "shortcut";
    pub const BEHAVIOR: &str = "dictation.behavior";
    pub const MODE: &str = "processing.mode";
    pub const PROVIDER: &str = "provider.selected";
    pub const MODEL: &str = "provider.model";
    pub const LANGUAGE: &str = "dictation.language";
    pub const INTENSITY: &str = "visual.intensity";
    pub const ONBOARDING: &str = "onboarding.complete";
    pub const FALLBACK: &str = "dictation.fallback";
    pub const REFINE_STYLE: &str = "processing.refine_style";
    pub const SPOKEN: &str = "processing.spoken_punctuation";
    pub const REFINE_ENGINES: &str = "processing.refine_engines";
    pub const REFINE_MODEL: &str = "processing.refine_model";
    pub const TECHNICAL_VOCABULARY: &str = "dictation.technical_vocabulary";
    pub const FORMAT_TECHNICAL: &str = "processing.format_technical";
    pub const LIVE_TYPING: &str = "dictation.live_typing";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Tauri accelerator string, e.g. "Alt+Space".
    pub shortcut: String,
    pub behavior: DictationBehavior,
    pub mode: ProcessingMode,
    pub provider_id: String,
    pub model_id: String,
    /// ISO-639-1, or `None` to let the provider detect it.
    pub language: Option<String>,
    pub visual_intensity: VisualIntensity,
    /// Whether Clide may try another on-device engine when the chosen one
    /// cannot run. Whatever it picks is always named in the HUD.
    pub fallback: FallbackPolicy,
    /// How far Rewrite may go. Only consulted in Rewrite mode.
    pub refine_style: RefineStyle,
    /// Turn spoken "comma" / "new line" into punctuation. On by default: it is
    /// instant, offline, and cannot invent words the user did not say.
    pub spoken_punctuation: bool,
    /// Refinement engines the user has explicitly switched on, in the order
    /// they should be tried. Empty means Rewrite falls back to the polished
    /// transcript — never that clide picks an engine on their behalf.
    pub refine_engines: Vec<String>,
    /// The model Rewrite should use on engines that offer a choice (Ollama).
    /// `None` lets the engine pick the best one installed.
    pub refine_model: Option<String>,
    /// Prime the engine with developer vocabulary. Only takes effect on
    /// engines that accept a hint (local Whisper).
    pub technical_vocabulary: TechnicalVocabulary,
    /// Wrap spoken file paths in backticks. Off by default: backticks are
    /// literal text, and harmful in a terminal.
    pub format_technical_terms: bool,
    /// Type words into the focused app as they are spoken, on engines that
    /// can stream. Skips Rewrite and spoken corrections, which need the whole
    /// recording.
    pub live_typing: bool,
    pub onboarding_complete: bool,
}

impl AppSettings {
    /// Defaults for a machine that has never run Clide.
    pub fn defaults(provider_id: &str, model_id: &str) -> Self {
        Self {
            shortcut: DEFAULT_SHORTCUT.to_string(),
            behavior: DictationBehavior::Hold,
            mode: ProcessingMode::Polished,
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            language: None,
            visual_intensity: VisualIntensity::Normal,
            fallback: FallbackPolicy::default(),
            refine_style: RefineStyle::default(),
            spoken_punctuation: true,
            refine_engines: vec!["apple-intelligence".to_string()],
            refine_model: None,
            technical_vocabulary: TechnicalVocabulary::default(),
            format_technical_terms: false,
            live_typing: true,
            onboarding_complete: false,
        }
    }
}

pub fn load(connection: &Connection, provider_id: &str, model_id: &str) -> AppSettings {
    let defaults = AppSettings::defaults(provider_id, model_id);

    AppSettings {
        shortcut: kv::get(connection, keys::SHORTCUT)
            .ok()
            .flatten()
            .unwrap_or(defaults.shortcut),
        behavior: kv::get(connection, keys::BEHAVIOR)
            .ok()
            .flatten()
            .unwrap_or(defaults.behavior),
        mode: kv::get(connection, keys::MODE)
            .ok()
            .flatten()
            .unwrap_or(defaults.mode),
        provider_id: kv::get(connection, keys::PROVIDER)
            .ok()
            .flatten()
            .unwrap_or(defaults.provider_id),
        model_id: kv::get(connection, keys::MODEL)
            .ok()
            .flatten()
            .unwrap_or(defaults.model_id),
        // `None` is persisted as JSON `null`, so deserialize the same
        // `Option<String>` shape that `save` writes. Reading it as a bare
        // `String` treats the valid null value as corrupt and warns at launch.
        language: kv::get::<Option<String>>(connection, keys::LANGUAGE)
            .ok()
            .flatten()
            .flatten(),
        fallback: kv::get(connection, keys::FALLBACK)
            .ok()
            .flatten()
            .unwrap_or(defaults.fallback),
        refine_style: kv::get(connection, keys::REFINE_STYLE)
            .ok()
            .flatten()
            .unwrap_or(defaults.refine_style),
        spoken_punctuation: kv::get(connection, keys::SPOKEN)
            .ok()
            .flatten()
            .unwrap_or(defaults.spoken_punctuation),
        refine_engines: kv::get(connection, keys::REFINE_ENGINES)
            .ok()
            .flatten()
            .unwrap_or(defaults.refine_engines),
        refine_model: kv::get::<Option<String>>(connection, keys::REFINE_MODEL)
            .ok()
            .flatten()
            .flatten(),
        technical_vocabulary: kv::get(connection, keys::TECHNICAL_VOCABULARY)
            .ok()
            .flatten()
            .unwrap_or(defaults.technical_vocabulary),
        format_technical_terms: kv::get(connection, keys::FORMAT_TECHNICAL)
            .ok()
            .flatten()
            .unwrap_or(defaults.format_technical_terms),
        live_typing: kv::get(connection, keys::LIVE_TYPING)
            .ok()
            .flatten()
            .unwrap_or(defaults.live_typing),
        visual_intensity: kv::get(connection, keys::INTENSITY)
            .ok()
            .flatten()
            .unwrap_or(defaults.visual_intensity),
        onboarding_complete: kv::get(connection, keys::ONBOARDING)
            .ok()
            .flatten()
            .unwrap_or(defaults.onboarding_complete),
    }
}

/// Repair preferences that name something this build no longer ships.
///
/// Earlier builds offered cloud engines; a database written by one of them can
/// still select `groq` or switch on a cloud rewriter. Left alone, the next
/// dictation would fail with "not available in this build". Returns whether
/// anything changed, so the caller knows to persist it.
pub fn reconcile(
    settings: &mut AppSettings,
    providers: &ProviderRegistry,
    refiners: &RefinerRegistry,
) -> bool {
    let mut changed = false;

    if providers.get(&settings.provider_id).is_none() {
        let fallback = providers.default_provider();
        settings.provider_id = fallback.id().to_string();
        settings.model_id = fallback.default_model().to_string();
        changed = true;
    }

    let before = settings.refine_engines.len();
    settings
        .refine_engines
        .retain(|id| refiners.get(id).is_some());
    changed |= settings.refine_engines.len() != before;

    changed
}

pub fn save(connection: &Connection, settings: &AppSettings) -> rusqlite::Result<()> {
    kv::set(connection, keys::SHORTCUT, &settings.shortcut)?;
    kv::set(connection, keys::BEHAVIOR, &settings.behavior)?;
    kv::set(connection, keys::MODE, &settings.mode)?;
    kv::set(connection, keys::PROVIDER, &settings.provider_id)?;
    kv::set(connection, keys::MODEL, &settings.model_id)?;
    kv::set(connection, keys::LANGUAGE, &settings.language)?;
    kv::set(connection, keys::INTENSITY, &settings.visual_intensity)?;
    kv::set(connection, keys::FALLBACK, &settings.fallback)?;
    kv::set(connection, keys::REFINE_STYLE, &settings.refine_style)?;
    kv::set(connection, keys::SPOKEN, &settings.spoken_punctuation)?;
    kv::set(connection, keys::REFINE_ENGINES, &settings.refine_engines)?;
    kv::set(connection, keys::REFINE_MODEL, &settings.refine_model)?;
    kv::set(
        connection,
        keys::TECHNICAL_VOCABULARY,
        &settings.technical_vocabulary,
    )?;
    kv::set(
        connection,
        keys::FORMAT_TECHNICAL,
        &settings.format_technical_terms,
    )?;
    kv::set(connection, keys::LIVE_TYPING, &settings.live_typing)?;
    kv::set(connection, keys::ONBOARDING, &settings.onboarding_complete)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn a_fresh_install_gets_working_defaults() {
        let db = Database::in_memory().unwrap();
        let settings = load(&db.lock(), "apple", "apple-speech");

        assert_eq!(settings.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(settings.behavior, DictationBehavior::Hold);
        assert!(!settings.onboarding_complete);
        // Rewrite must never be the default: it is not implemented.
        assert!(settings.mode.is_available());
    }

    #[test]
    fn settings_round_trip() {
        let db = Database::in_memory().unwrap();
        let mut settings = load(&db.lock(), "apple", "apple-speech");
        settings.shortcut = "Ctrl+Shift+D".into();
        settings.behavior = DictationBehavior::Toggle;
        settings.mode = ProcessingMode::Verbatim;
        settings.visual_intensity = VisualIntensity::High;
        settings.language = Some("en".into());
        settings.onboarding_complete = true;
        save(&db.lock(), &settings).unwrap();

        let reloaded = load(&db.lock(), "apple", "apple-speech");
        assert_eq!(reloaded.shortcut, "Ctrl+Shift+D");
        assert_eq!(reloaded.behavior, DictationBehavior::Toggle);
        assert_eq!(reloaded.mode, ProcessingMode::Verbatim);
        assert_eq!(reloaded.visual_intensity, VisualIntensity::High);
        assert_eq!(reloaded.language.as_deref(), Some("en"));
        assert!(reloaded.onboarding_complete);
    }

    #[test]
    fn automatic_language_round_trips_as_none() {
        let db = Database::in_memory().unwrap();
        let settings = load(&db.lock(), "apple", "apple-speech");
        assert_eq!(settings.language, None);

        save(&db.lock(), &settings).unwrap();

        let reloaded = load(&db.lock(), "apple", "apple-speech");
        assert_eq!(reloaded.language, None);
    }

    #[test]
    fn one_corrupt_value_does_not_reset_the_others() {
        let db = Database::in_memory().unwrap();
        let mut settings = load(&db.lock(), "apple", "apple-speech");
        settings.shortcut = "Ctrl+Shift+D".into();
        save(&db.lock(), &settings).unwrap();

        db.lock()
            .execute(
                "UPDATE settings SET value = 'garbage' WHERE key = 'processing.mode'",
                [],
            )
            .unwrap();

        let reloaded = load(&db.lock(), "apple", "apple-speech");
        assert_eq!(reloaded.shortcut, "Ctrl+Shift+D", "good values were lost");
        assert_eq!(
            reloaded.mode,
            ProcessingMode::Polished,
            "no fallback applied"
        );
    }

    /// Code formatting inserts literal backticks, so it must never switch
    /// itself on.
    #[test]
    fn technical_formatting_is_off_by_default_and_persists() {
        let db = Database::in_memory().unwrap();
        let mut settings = load(&db.lock(), "apple", "apple-speech");
        assert!(!settings.format_technical_terms);
        assert_eq!(settings.technical_vocabulary, TechnicalVocabulary::Auto);

        settings.format_technical_terms = true;
        settings.technical_vocabulary = TechnicalVocabulary::Always;
        save(&db.lock(), &settings).unwrap();

        let reloaded = load(&db.lock(), "apple", "apple-speech");
        assert!(reloaded.format_technical_terms);
        assert_eq!(reloaded.technical_vocabulary, TechnicalVocabulary::Always);
    }

    #[test]
    fn a_retired_cloud_engine_is_replaced_by_the_default() {
        let providers =
            ProviderRegistry::new(crate::models::ModelStore::new(&std::env::temp_dir()));
        let refiners = RefinerRegistry::new();

        let mut settings = AppSettings::defaults("groq", "whisper-large-v3-turbo");
        settings.refine_engines = vec!["groq-rewrite".into(), "apple-intelligence".into()];

        assert!(reconcile(&mut settings, &providers, &refiners));
        assert_eq!(settings.provider_id, "apple");
        assert_eq!(settings.model_id, "apple-speech");
        assert_eq!(settings.refine_engines, vec!["apple-intelligence".to_string()]);
    }

    #[test]
    fn reconcile_leaves_a_valid_selection_alone() {
        let providers =
            ProviderRegistry::new(crate::models::ModelStore::new(&std::env::temp_dir()));
        let refiners = RefinerRegistry::new();

        let mut settings = AppSettings::defaults("local-whisper", "whisper-base");
        assert!(!reconcile(&mut settings, &providers, &refiners));
        assert_eq!(settings.provider_id, "local-whisper");
        assert_eq!(settings.model_id, "whisper-base");
    }

    #[test]
    fn no_setting_key_could_hold_a_credential() {
        // The settings table is explicitly not a place for secrets; this
        // asserts the key list stays that way.
        for key in [
            keys::SHORTCUT,
            keys::BEHAVIOR,
            keys::MODE,
            keys::PROVIDER,
            keys::MODEL,
            keys::LANGUAGE,
            keys::INTENSITY,
            keys::ONBOARDING,
        ] {
            assert!(!key.contains("key"), "{key} looks like a credential slot");
            assert!(!key.contains("secret"));
            assert!(!key.contains("token"));
        }
    }
}
