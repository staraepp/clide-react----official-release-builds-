//! Microphone capture and the lifetime of the audio it produces.

pub mod clip;
pub mod ducking;
pub mod error;
pub mod recorder;
pub mod resample;

pub use clip::{RecordedClip, RECOVERY_WINDOW};
pub use error::AudioError;
pub use recorder::{Recorder, SampleTap};
