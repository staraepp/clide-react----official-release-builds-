//! Local transcription with NVIDIA Parakeet, through ONNX Runtime.
//!
//! Parakeet is a transducer rather than an encoder-decoder, and it ships as
//! four artifacts loaded from a directory rather than one weights file. Both
//! differences stop at this module's edge.

use std::path::PathBuf;
use std::time::Instant;

use async_trait::async_trait;

use super::audio::read_wav_as_mono_f32;
use crate::models::{catalog, Engine, ModelStore};
use crate::models::catalog::ParakeetArch;
use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, ModelInfo, Transcription, TranscriptionProvider,
    TranscriptionRequest,
};

const PROVIDER_ID: &str = "local-parakeet";

pub struct LocalParakeetProvider {
    models: ModelStore,
}

impl LocalParakeetProvider {
    pub fn new(models: ModelStore) -> Self {
        Self { models }
    }

    /// Parakeet loads from a *directory*, not a file, and the two
    /// architectures take different code paths — so the entry is resolved
    /// alongside the path.
    fn resolve(&self, model_id: &str) -> Result<(PathBuf, ParakeetArch), ProviderError> {
        let entry = catalog::find(model_id).ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        if !self.models.is_installed(&entry) {
            // Not "unknown" — a real model that simply is not downloaded.
            return Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: format!("{} is not downloaded yet", entry.name),
            });
        }

        let arch = entry.arch.ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        Ok((self.models.directory_for(&entry), arch))
    }
}

#[async_trait]
impl TranscriptionProvider for LocalParakeetProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Parakeet"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: true,
            batch: true,
            // The crate supports streaming; this adapter does not implement it.
            streaming: false,
            timestamps: true,
            word_timestamps: true,
            diarization: false,
            language_detection: false,
            translation: false,
            prompting: false,
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.models
            .installed()
            .into_iter()
            .filter(|status| status.entry.engine == Engine::Parakeet)
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
        "parakeet-tdt-0.6b-v3"
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError> {
        let (directory, arch) = self.resolve(&request.model)?;
        let audio = read_wav_as_mono_f32(request.audio.path())?;
        let sample_rate = request.audio.sample_rate;

        let started = Instant::now();
        let model = request.model.clone();

        // ONNX inference is blocking and CPU-heavy; it must not run on an
        // async worker or it would stall every other task in the runtime.
        let text = tauri::async_runtime::spawn_blocking(move || {
            run_parakeet(&directory, arch, audio, sample_rate)
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

fn run_parakeet(
    directory: &std::path::Path,
    arch: ParakeetArch,
    audio: Vec<f32>,
    sample_rate: u32,
) -> Result<String, ProviderError> {
    use parakeet_rs::{Parakeet, ParakeetTDT, Transcriber};

    let failure = |detail: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail,
    };

    // `None` leaves the crate on its CPU default. Its own notes say CoreML
    // currently runs these graphs *slower*, because their dynamic shapes stop
    // CoreML from planning for the ANE — so opting in would be a regression.
    //
    // Both architectures satisfy the same `Transcriber` trait, so only
    // construction differs; Clide captures mono, hence one channel.
    let result = match arch {
        ParakeetArch::Tdt => {
            let mut model = ParakeetTDT::from_pretrained(directory, None)
                .map_err(|e| failure(format!("the model could not be loaded: {e}")))?;
            model.transcribe_samples(audio, sample_rate, 1, None)
        }
        ParakeetArch::Ctc => {
            let mut model = Parakeet::from_pretrained(directory, None)
                .map_err(|e| failure(format!("the model could not be loaded: {e}")))?;
            model.transcribe_samples(audio, sample_rate, 1, None)
        }
    }
    .map_err(|e| failure(format!("transcription failed: {e}")))?;

    Ok(result.text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(name: &str) -> (LocalParakeetProvider, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-parakeet-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (LocalParakeetProvider::new(ModelStore::new(&dir)), dir)
    }

    #[test]
    fn parakeet_declares_itself_local() {
        let (local, _dir) = provider("local");
        assert!(local.capabilities().local);
    }

    #[test]
    fn its_default_model_is_a_parakeet_one() {
        let (local, _dir) = provider("default");
        let entry = catalog::find(local.default_model()).expect("default is in the catalogue");
        assert_eq!(entry.engine, Engine::Parakeet);
    }

    /// Whisper weights must never be offered by the Parakeet engine.
    #[test]
    fn it_only_offers_parakeet_models() {
        let (local, dir) = provider("filter");
        let whisper = catalog::find("whisper-base").unwrap();
        let store = ModelStore::new(&dir);
        store.prepare_directory(&whisper).unwrap();
        for file in &whisper.files {
            std::fs::write(store.file_path(&whisper, file), vec![0u8; file.bytes as usize])
                .unwrap();
        }

        assert!(store.is_installed(&whisper), "the whisper model was installed");
        assert!(local.models().is_empty(), "Parakeet offered a Whisper model");
    }

    #[tokio::test]
    async fn an_uninstalled_model_says_so() {
        let (local, _dir) = provider("uninstalled");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "parakeet-tdt-0.6b-v3".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(error.to_string().contains("not downloaded"), "got: {error}");
    }
}
