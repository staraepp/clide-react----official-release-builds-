//! Transcription backends behind one normalised adapter interface.

pub mod apple;
pub mod error;
pub mod local;
pub mod registry;
pub mod traits;

pub use error::ProviderError;
pub use registry::ProviderRegistry;
pub use traits::{
    AudioClip, Capabilities, ModelInfo, ProviderDescriptor, Transcription, TranscriptionProvider,
    TranscriptionRequest,
};
