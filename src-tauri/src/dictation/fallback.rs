//! Choosing a backup engine when the selected one cannot run.
//!
//! Clide runs entirely on this Mac, so a substitute can never move a recording
//! anywhere: every candidate is another on-device engine. That removes the
//! privacy question the old cloud-era version of this module had to answer,
//! but not the transparency one — **a fallback is never silent.** Whatever
//! runs is named in the result and surfaced to the HUD, so "why does this
//! transcript look different" always has an answer.

use serde::{Deserialize, Serialize};

use crate::providers::{ProviderRegistry, TranscriptionProvider};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FallbackPolicy {
    /// Never substitute. A failure is reported and the user decides.
    Off,
    /// Try another on-device engine that has a usable model.
    #[default]
    LocalOnly,
}

/// A backend Clide could actually use right now, and why it was chosen.
pub struct Candidate {
    pub provider: Arc<dyn TranscriptionProvider>,
    pub model: String,
}

/// Candidates to try after `failed_provider`, in order.
///
/// The registry's own order decides, so there is no hidden ranking to reason
/// about. An engine with nothing downloaded offers no models and is skipped.
pub fn candidates(
    registry: &ProviderRegistry,
    policy: FallbackPolicy,
    failed_provider: &str,
) -> Vec<Candidate> {
    if policy == FallbackPolicy::Off {
        return Vec::new();
    }

    let mut found = Vec::new();

    for descriptor in registry.descriptors() {
        if descriptor.id == failed_provider {
            continue;
        }
        let Some(provider) = registry.get(&descriptor.id) else {
            continue;
        };

        // Prefer the provider's own default when it offers it, otherwise the
        // first model it actually has.
        let offered = provider.models();
        let model = offered
            .iter()
            .find(|model| model.id == provider.default_model())
            .or_else(|| offered.first())
            .map(|model| model.id.clone());

        let Some(model) = model else { continue };
        found.push(Candidate { provider, model });
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelStore;

    fn registry() -> ProviderRegistry {
        ProviderRegistry::new(ModelStore::new(
            &std::env::temp_dir().join("clide-fallback-none"),
        ))
    }

    #[test]
    fn off_never_substitutes_anything() {
        let found = candidates(&registry(), FallbackPolicy::Off, "local-whisper");
        assert!(found.is_empty());
    }

    #[test]
    fn the_failed_provider_is_never_its_own_fallback() {
        let found = candidates(&registry(), FallbackPolicy::LocalOnly, "apple");
        assert!(found.iter().all(|c| c.provider.id() != "apple"));
    }

    /// A local engine with nothing downloaded cannot serve a transcription, so
    /// it must not be offered as a rescue.
    ///
    /// Apple Speech is the exception and the reason this is worth having: it
    /// ships with macOS, always has a model, and is therefore the one engine
    /// that can rescue a dictation on a machine where nothing was downloaded.
    #[test]
    fn only_engines_with_a_usable_model_are_candidates() {
        let found = candidates(&registry(), FallbackPolicy::LocalOnly, "local-whisper");

        for candidate in &found {
            assert!(
                !candidate.provider.models().is_empty(),
                "{} was offered with no models installed",
                candidate.provider.id()
            );
        }

        assert!(
            found.iter().any(|c| c.provider.id() == "apple"),
            "Apple Speech ships with macOS and should always be able to rescue"
        );
    }
}
