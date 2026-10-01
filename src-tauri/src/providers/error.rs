use thiserror::Error;

/// Failures normalised across every transcription backend.
///
/// Adapters translate their own wire errors into these so the rest of Clide
/// can reason about a failure without knowing which provider produced it.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("{provider} is temporarily unavailable ({status})")]
    ServiceUnavailable { provider: &'static str, status: u16 },

    #[error("{provider} rejected the request: {detail}")]
    BadRequest {
        provider: &'static str,
        detail: String,
    },

    #[error("{provider} does not offer the model \"{model}\"")]
    UnknownModel {
        provider: &'static str,
        model: String,
    },

    #[error("the recording could not be read: {0}")]
    AudioUnreadable(String),
}
