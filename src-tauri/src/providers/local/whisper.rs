//! Local transcription with whisper.cpp.
//!
//! Runs whisper.cpp through `whisper-rs`, Metal-accelerated on Apple Silicon.
//! No network, no credential, and the audio never leaves the machine — which is
//! why `Capabilities::local` exists rather than the pipeline special-casing it.
//!
//! The models this offers are exactly the ones actually installed. An engine
//! that advertises weights the user has not downloaded would fail at the worst
//! possible moment, so `models()` reads the disk.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;

use crate::models::{catalog, Engine, ModelStore};
use super::audio::read_wav_as_mono_f32;
use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, ModelInfo, Transcription, TranscriptionProvider,
    TranscriptionRequest,
};

const PROVIDER_ID: &str = "local-whisper";

/// The model currently held in memory, and which weights it came from.
///
/// Loading a Whisper model reads it from disk and compiles its GPU kernels,
/// which took ~10 seconds for a quantised large model. Doing that on every
/// dictation made a two-second job feel broken, so the loaded state is kept
/// and reused until a different model is chosen. The state holds the context
/// alive, so keeping it is enough.
type Loaded = Option<(PathBuf, whisper_rs::WhisperState)>;

pub struct LocalWhisperProvider {
    models: ModelStore,
    loaded: Arc<Mutex<Loaded>>,
}

impl LocalWhisperProvider {
    pub fn new(models: ModelStore) -> Self {
        Self {
            models,
            loaded: Arc::new(Mutex::new(None)),
        }
    }

    fn weights_for(&self, model_id: &str) -> Result<PathBuf, ProviderError> {
        let entry = catalog::find(model_id).ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        if !self.models.is_installed(&entry) {
            // Not "unknown" — the user picked a real model that simply is not
            // downloaded yet, and the message should say so.
            return Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: format!("{} is not downloaded yet", entry.name),
            });
        }

        Ok(self.models.path_for(&entry))
    }
}

#[async_trait]
impl TranscriptionProvider for LocalWhisperProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Local Whisper"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: true,
            batch: true,
            streaming: false,
            timestamps: true,
            word_timestamps: false,
            diarization: false,
            language_detection: true,
            translation: true,
            prompting: true,
        }
    }

    /// Only what is installed. The catalogue of *installable* models is a
    /// separate concept, served by `models::catalog` to the model manager UI.
    fn models(&self) -> Vec<ModelInfo> {
        self.models
            .installed()
            .into_iter()
            .filter(|status| status.entry.engine == Engine::Whisper)
            .map(|status| ModelInfo {
                id: status.entry.id,
                name: status.entry.name,
                description: status.entry.description,
                speed: status.entry.speed,
                quality: status.entry.quality,
                multilingual: status.entry.multilingual,
            })
            .collect()
    }

    fn default_model(&self) -> &'static str {
        "whisper-large-v3-turbo"
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError> {
        let weights = self.weights_for(&request.model)?;
        let audio = read_wav_as_mono_f32(request.audio.path())?;

        let started = Instant::now();
        let model = request.model.clone();
        let language = request.language.clone();
        let prompt = request.prompt.clone();

        // Inference is CPU/GPU-bound and blocking; it must not run on an async
        // worker or it would stall every other task in the runtime.
        let loaded = Arc::clone(&self.loaded);
        let text = tauri::async_runtime::spawn_blocking(move || {
            run_whisper(&loaded, &weights, &audio, language.as_deref(), prompt.as_deref())
        })
        .await
        .map_err(|_| ProviderError::ServiceUnavailable {
            provider: PROVIDER_ID,
            status: 500,
        })??;

        Ok(Transcription {
            text,
            provider: PROVIDER_ID.to_string(),
            model,
            language: request.language,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn run_whisper(
    loaded: &Mutex<Loaded>,
    weights: &std::path::Path,
    audio: &[f32],
    language: Option<&str>,
    prompt: Option<&str>,
) -> Result<String, ProviderError> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let failure = |detail: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail,
    };

    // Held for the whole run: two dictations never overlap, and a second one
    // waiting for the first is correct, not a problem.
    let mut guard = loaded.lock().unwrap_or_else(|e| e.into_inner());

    if !matches!(guard.as_ref(), Some((path, _)) if path == weights) {
        // Drop the old model first so two large models are never resident at
        // once while the new one loads.
        *guard = None;

        let context = WhisperContext::new_with_params(
            weights.to_string_lossy().as_ref(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| failure(format!("the model could not be loaded: {e}")))?;

        let state = context
            .create_state()
            .map_err(|e| failure(format!("the model could not be started: {e}")))?;

        *guard = Some((weights.to_path_buf(), state));
    }

    let Some((_, state)) = guard.as_mut() else {
        return Err(failure("the model could not be loaded".into()));
    };

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // Dictation wants the words that were said, not a creative reading.
    params.set_temperature(0.0);
    params.set_translate(false);
    // Whisper's own silence guards, as a second line behind the recorder's
    // speech check: don't emit blank tokens, and drop segments it believes
    // contain no speech.
    params.set_suppress_blank(true);
    params.set_no_speech_thold(0.6);
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // `None` leaves whisper to detect the language itself.
    params.set_language(language);
    if let Some(prompt) = prompt {
        params.set_initial_prompt(prompt);
    }

    if let Err(error) = state.full(params, audio) {
        // A state that failed mid-run is not trusted again.
        *guard = None;
        return Err(failure(format!("transcription failed: {error}")));
    }

    let Some((_, state)) = guard.as_ref() else {
        return Err(failure("the model could not be loaded".into()));
    };

    // Segments carry borrowed UTF-8 that can be split mid-character on a
    // truncated decode, so read them lossily rather than dropping the segment.
    let mut text = String::new();
    for segment in state.as_iter() {
        if let Ok(chunk) = segment.to_str_lossy() {
            text.push_str(&chunk);
        }
    }

    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(name: &str) -> (LocalWhisperProvider, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-local-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (LocalWhisperProvider::new(ModelStore::new(&dir)), dir)
    }

    #[test]
    fn a_local_provider_declares_itself_local() {
        let (local, _dir) = provider("local");
        assert!(local.capabilities().local);
    }

    #[test]
    fn nothing_is_offered_until_something_is_installed() {
        let (local, _dir) = provider("empty");
        assert!(local.models().is_empty());
    }

    /// A model the user has not downloaded must produce a message about the
    /// download, not "unknown model" — it is a real model, just absent.
    #[tokio::test]
    async fn an_uninstalled_model_says_so() {
        let (local, _dir) = provider("uninstalled");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "whisper-base".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(
            error.to_string().contains("not downloaded"),
            "got: {error}"
        );
    }

    #[tokio::test]
    async fn a_model_outside_the_catalogue_is_unknown() {
        let (local, _dir) = provider("unknown");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "whisper-imaginary".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(matches!(error, ProviderError::UnknownModel { .. }));
    }
}
