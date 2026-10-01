//! The refinement backends this build knows about.

use std::sync::Arc;

use super::apple_intelligence::AppleIntelligenceRefiner;
use super::traits::{Refiner, RefinerDescriptor};

pub struct RefinerRegistry {
    refiners: Vec<Arc<dyn Refiner>>,
}

impl RefinerRegistry {
    pub fn new() -> Self {
        Self {
            // Order is the fallback order. Spoken punctuation is *not* here —
            // it is a pre-pass, applied before any of these, so enabling it
            // can never stop a rewrite from happening.
            refiners: vec![Arc::new(AppleIntelligenceRefiner::new())],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Refiner>> {
        self.refiners.iter().find(|r| r.id() == id).cloned()
    }

    /// The first backend that is both switched on and able to run.
    ///
    /// `enabled` is the user's explicit list. A refiner absent from it is
    /// never used, however available it happens to be: Rewrite only ever runs
    /// an engine the user switched on.
    pub fn first_enabled(&self, enabled: &[String]) -> Option<Arc<dyn Refiner>> {
        self.refiners
            .iter()
            .find(|refiner| {
                enabled.iter().any(|id| id == refiner.id()) && refiner.availability().is_ok()
            })
            .cloned()
    }

    pub fn descriptors(&self) -> Vec<RefinerDescriptor> {
        self.refiners.iter().map(|r| r.descriptor()).collect()
    }
}

impl Default for RefinerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refiner_has_a_unique_id() {
        let registry = RefinerRegistry::new();
        let mut ids: Vec<_> = registry.descriptors().into_iter().map(|d| d.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(count, ids.len());
    }

    #[test]
    fn a_refiner_can_be_looked_up_by_id() {
        let registry = RefinerRegistry::new();
        assert!(registry.get("apple-intelligence").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    /// An engine the user has not switched on is never used, no matter how
    /// available it is.
    #[test]
    fn a_refiner_that_is_not_enabled_is_never_chosen() {
        let registry = RefinerRegistry::new();
        assert!(
            registry.first_enabled(&[]).is_none(),
            "a refiner ran with nothing enabled"
        );
        assert!(registry
            .first_enabled(&["groq-rewrite".to_string()])
            .is_none());
    }

    #[test]
    fn every_refiner_runs_on_this_mac() {
        for descriptor in RefinerRegistry::new().descriptors() {
            assert!(descriptor.local, "{} is not local", descriptor.id);
        }
    }
}
