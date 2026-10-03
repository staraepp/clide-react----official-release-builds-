//! The contract every transcription backend implements.
//!
//! The rest of Clide asks a provider what it *can do* rather than which
//! provider it *is*. Adding another on-device engine later
//! should mean writing one adapter, not touching the dictation pipeline. Every
//! backend runs on this Mac: there is no network leg and no credential.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::ProviderError;

/// What a backend supports. The UI and pipeline branch on these, never on ids.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Runs on this machine; no network and no credential. True for every
    /// engine in this build.
    pub local: bool,
    /// Can transcribe a finished recording in one request.
    pub batch: bool,
    /// Can return partial transcripts while audio is still arriving.
    pub streaming: bool,
    pub timestamps: bool,
    pub word_timestamps: bool,
    pub diarization: bool,
    pub language_detection: bool,
    pub translation: bool,
    /// Accepts a priming prompt to bias vocabulary.
    pub prompting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeedClass {
    Fast,
    Balanced,
    Thorough,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QualityClass {
    Good,
    High,
    VeryHigh,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub speed: SpeedClass,
    pub quality: QualityClass,
    /// True when the model handles more than English.
    pub multilingual: bool,
}

/// A recording handed to a provider.
///
/// Clide always captures 16 kHz mono WAV, which every current backend accepts
/// directly, so `transcribe` is not expected to transcode.
#[derive(Clone, Debug)]
pub struct AudioClip {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f32,
}

impl AudioClip {
    pub fn wav(path: impl Into<PathBuf>, duration_secs: f32) -> Self {
        Self {
            path: path.into(),
            sample_rate: crate::audio::resample::TARGET_SAMPLE_RATE,
            channels: 1,
            duration_secs,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio.wav".into())
    }
}

/// Everything a transcription needs, independent of backend.
#[derive(Clone, Debug)]
pub struct TranscriptionRequest {
    pub audio: AudioClip,
    pub model: String,
    /// ISO-639-1. `None` asks the provider to detect it.
    pub language: Option<String>,
    /// Vocabulary hint, only sent to providers whose capabilities allow it.
    pub prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub language: Option<String>,
    /// Wall-clock time the provider call took.
    pub latency_ms: u64,
}

#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn models(&self) -> Vec<ModelInfo>;
    fn default_model(&self) -> &'static str;

    /// Load whatever `transcribe` will need, ahead of the first dictation.
    ///
    /// Blocking, and best-effort: a failure here is not an error, because the
    /// real attempt will report it. Engines with nothing to load do nothing.
    fn warm_up(&self, _model: &str) {}

    /// Release anything held between dictations (a loaded model, GPU state).
    ///
    /// Called as the app quits. Engines that keep native state alive must free
    /// it here: letting the process exit with it still resident makes the
    /// runtime's own static cleanup abort, and macOS reports a crash.
    fn unload(&self) {}

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError>;

    /// Whether this provider knows the given model id.
    fn has_model(&self, model: &str) -> bool {
        self.models().iter().any(|m| m.id == model)
    }
}

/// The serialisable shape the frontend renders. Lets the provider UI be
/// written once against capabilities rather than per backend.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub capabilities: Capabilities,
    pub models: Vec<ModelInfo>,
    pub default_model: String,
}

impl ProviderDescriptor {
    pub fn of(provider: &dyn TranscriptionProvider) -> Self {
        Self {
            id: provider.id().to_string(),
            name: provider.name().to_string(),
            capabilities: provider.capabilities(),
            models: provider.models(),
            default_model: provider.default_model().to_string(),
        }
    }
}
