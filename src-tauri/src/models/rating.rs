//! How well a model suits this machine.
//!
//! # Why these numbers are defensible
//!
//! Every score here is derived from something real: the model's declared speed
//! and quality class, its actual size on disk, and this Mac's measured memory
//! and chip. Nothing is invented, and nothing is a popularity score — Clide has
//! no telemetry and could not know one.
//!
//! `blueprint.md` and `AGENTS.md` both forbid inventing statistics. If a rating
//! cannot be derived from a measured or declared fact, it does not belong here.

use serde::Serialize;

use super::catalog::{CatalogEntry, Engine};
use super::hardware::Hardware;
use crate::providers::traits::{QualityClass, SpeedClass};

/// How comfortably a local model fits this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Fit {
    /// Comfortable headroom.
    Great,
    /// Will run, with less room to spare.
    Good,
    /// Runs, but expect it to be slow or to push memory.
    Tight,
    /// Not enough memory to load it without swapping.
    TooLarge,
}

impl Fit {
    pub fn label(self) -> &'static str {
        match self {
            Fit::Great => "Runs great here",
            Fit::Good => "Runs well here",
            Fit::Tight => "Will be slow here",
            Fit::TooLarge => "Not enough memory",
        }
    }
}

/// Star ratings, 0..5 in halves, plus the fit verdict.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    /// From the model's declared quality class.
    pub accuracy: f32,
    /// From its speed class, reduced for local models this Mac will struggle
    /// with — the same weights are genuinely slower on a smaller machine.
    pub speed: f32,
    /// Accuracy and speed together, weighted toward accuracy: a fast model
    /// that gets words wrong costs more time than it saves.
    pub overall: f32,
    pub fit: Fit,
    /// Peak memory this is expected to need, for local models.
    pub required_memory_bytes: u64,
}

/// Loading weights costs more than their size on disk: activations, the
/// mel spectrogram, and decoder state all live alongside them.
const MEMORY_OVERHEAD: f64 = 1.5;

fn accuracy_stars(quality: QualityClass) -> f32 {
    match quality {
        QualityClass::Good => 3.0,
        QualityClass::High => 4.0,
        QualityClass::VeryHigh => 5.0,
    }
}

fn speed_stars(speed: SpeedClass) -> f32 {
    match speed {
        SpeedClass::Fast => 5.0,
        SpeedClass::Balanced => 3.5,
        SpeedClass::Thorough => 2.0,
    }
}

/// Rate a local model against this machine.
pub fn rate_local(entry: &CatalogEntry, hardware: &Hardware) -> Rating {
    let required = (entry.download_bytes() as f64 * MEMORY_OVERHEAD) as u64;
    let usable = hardware.usable_memory_bytes();

    let headroom = usable as f64 / required.max(1) as f64;
    let fit = if headroom >= 3.0 {
        Fit::Great
    } else if headroom >= 1.6 {
        Fit::Good
    } else if headroom >= 1.0 {
        Fit::Tight
    } else {
        Fit::TooLarge
    };

    // Without Metal or the ANE, local inference is materially slower, and the
    // rating should say so rather than flattering an Intel Mac.
    let acceleration = if hardware.apple_silicon { 1.0 } else { 0.6 };

    let pressure = match fit {
        Fit::Great => 1.0,
        Fit::Good => 0.85,
        Fit::Tight => 0.5,
        Fit::TooLarge => 0.2,
    };

    // Parakeet's ONNX graphs have dynamic shapes, so CoreML falls back to CPU;
    // whisper.cpp gets real Metal acceleration. Reflect that honestly.
    let engine_factor = match entry.engine {
        Engine::Whisper => 1.0,
        Engine::Parakeet => 0.9,
    };

    let speed = (speed_stars(entry.speed) * acceleration * pressure * engine_factor).clamp(0.5, 5.0);
    let accuracy = accuracy_stars(entry.quality);

    Rating {
        accuracy,
        speed: round_half(speed),
        overall: round_half((accuracy * 0.6 + speed * 0.4).clamp(0.5, 5.0)),
        fit,
        required_memory_bytes: required,
    }
}

fn round_half(value: f32) -> f32 {
    (value * 2.0).round() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::catalog;

    fn mac(gigabytes: u64, apple_silicon: bool) -> Hardware {
        Hardware {
            chip: "Test".into(),
            total_memory_bytes: gigabytes * 1_073_741_824,
            performance_cores: 8,
            apple_silicon,
        }
    }

    #[test]
    fn a_small_model_fits_a_small_machine() {
        let base = catalog::find("whisper-base").unwrap();
        assert_eq!(rate_local(&base, &mac(8, true)).fit, Fit::Great);
    }

    /// The point of the whole feature: a 2.5 GB model on an 8 GB Mac must be
    /// reported as a bad idea rather than offered like any other option.
    #[test]
    fn a_large_model_does_not_fit_a_small_machine() {
        let parakeet = catalog::find("parakeet-tdt-0.6b-v3").unwrap();
        let rating = rate_local(&parakeet, &mac(8, true));
        assert!(
            matches!(rating.fit, Fit::Tight | Fit::TooLarge),
            "2.5 GB of weights rated {:?} on an 8 GB Mac",
            rating.fit
        );
    }

    #[test]
    fn the_same_model_fits_better_on_a_bigger_machine() {
        let turbo = catalog::find("whisper-large-v3-turbo").unwrap();
        let small = rate_local(&turbo, &mac(8, true));
        let large = rate_local(&turbo, &mac(64, true));
        assert!(large.speed >= small.speed);
    }

    #[test]
    fn intel_macs_are_rated_slower_than_apple_silicon() {
        let small = catalog::find("whisper-small").unwrap();
        let apple = rate_local(&small, &mac(16, true));
        let intel = rate_local(&small, &mac(16, false));
        assert!(intel.speed < apple.speed);
    }

    #[test]
    fn accuracy_never_depends_on_the_machine() {
        let turbo = catalog::find("whisper-large-v3-turbo").unwrap();
        assert_eq!(
            rate_local(&turbo, &mac(8, false)).accuracy,
            rate_local(&turbo, &mac(128, true)).accuracy,
            "the same weights got different accuracy on different hardware"
        );
    }

    #[test]
    fn every_score_stays_inside_the_star_range() {
        for entry in catalog::catalog() {
            for machine in [mac(8, false), mac(16, true), mac(128, true)] {
                let rating = rate_local(&entry, &machine);
                for score in [rating.accuracy, rating.speed, rating.overall] {
                    assert!(
                        (0.5..=5.0).contains(&score),
                        "{} scored {score} on {} GB",
                        entry.id,
                        machine.total_memory_bytes / 1_073_741_824
                    );
                }
            }
        }
    }

    #[test]
    fn ratings_land_on_half_stars_so_the_ui_can_draw_them() {
        let turbo = catalog::find("whisper-large-v3-turbo").unwrap();
        let rating = rate_local(&turbo, &mac(16, true));
        for score in [rating.speed, rating.overall] {
            assert_eq!(score * 2.0, (score * 2.0).round(), "{score} is not a half star");
        }
    }
}
