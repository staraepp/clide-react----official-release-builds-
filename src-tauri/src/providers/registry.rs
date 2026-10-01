//! The set of transcription backends this build knows about.

use std::sync::Arc;

use crate::models::ModelStore;

use super::apple::AppleSpeechProvider;
use super::local::{LocalParakeetProvider, LocalWhisperProvider};
use super::traits::{ProviderDescriptor, TranscriptionProvider};

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn TranscriptionProvider>>,
}

impl ProviderRegistry {
    pub fn new(models: ModelStore) -> Self {
        Self {
            // Order matters only for `default_provider`; everything else
            // looks providers up by id. Every engine runs on this Mac.
            providers: vec![
                // Ships with macOS: usable on a fresh install with no
                // download, which also makes it the safest fallback.
                Arc::new(AppleSpeechProvider::new()),
                Arc::new(LocalWhisperProvider::new(models.clone())),
                Arc::new(LocalParakeetProvider::new(models)),
            ],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn TranscriptionProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    /// The provider used when nothing has been chosen yet: the one engine that
    /// needs no download.
    pub fn default_provider(&self) -> Arc<dyn TranscriptionProvider> {
        Arc::clone(&self.providers[0])
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .iter()
            .map(|p| ProviderDescriptor::of(p.as_ref()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider that offers models must default to one of them.
    ///
    /// Downloadable engines are exempt from *having* models: `models()`
    /// reports what is installed, and nothing is installed on a fresh machine.
    /// That is the correct answer, so the invariant is conditional on offering
    /// any.
    #[test]
    fn every_provider_that_offers_models_defaults_to_one_of_them() {
        let registry = ProviderRegistry::new(ModelStore::new(&std::env::temp_dir()));

        for descriptor in registry.descriptors() {
            if descriptor.models.is_empty() {
                continue;
            }

            assert!(
                descriptor
                    .models
                    .iter()
                    .any(|m| m.id == descriptor.default_model),
                "{} defaults to a model it does not list",
                descriptor.id
            );
        }
    }

    /// Clide is local-only: no engine may need the network.
    #[test]
    fn every_engine_runs_on_this_mac() {
        let registry = ProviderRegistry::new(ModelStore::new(&std::env::temp_dir()));
        for descriptor in registry.descriptors() {
            assert!(descriptor.capabilities.local, "{} is not local", descriptor.id);
        }
    }

    /// A fresh install has nothing downloaded, so the default must be the
    /// engine that ships with macOS.
    #[test]
    fn the_default_engine_works_on_a_fresh_install() {
        let registry = ProviderRegistry::new(ModelStore::new(
            &std::env::temp_dir().join("clide-registry-fresh"),
        ));
        assert!(!registry.default_provider().models().is_empty());
    }

    #[test]
    fn providers_are_looked_up_by_id_not_by_position() {
        let registry = ProviderRegistry::new(ModelStore::new(&std::env::temp_dir()));
        assert!(registry.get("apple").is_some());
        assert!(registry.get("local-whisper").is_some());
        assert!(registry.get("local-parakeet").is_some());
        assert!(registry.get("not-a-provider").is_none());
    }
}
