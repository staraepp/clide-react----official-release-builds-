//! Apple's built-in speech recognition.
//!
//! No key, no network, no download — the models ship with macOS. That makes it
//! the one engine that works on a fresh install before the user has configured
//! anything, which is why it is also the safest fallback target.
//!
//! `requiresOnDeviceRecognition` is forced on. Without it macOS may route audio
//! to Apple's servers, and a provider Clide describes as local must actually be
//! local.

pub mod live;

use std::sync::mpsc;
use std::time::Instant;

use async_trait::async_trait;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_foundation::{NSError, NSLocale, NSString, NSURL};
use objc2_speech::{
    SFSpeechRecognitionResult, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
    SFSpeechURLRecognitionRequest,
};

use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "apple";
const MODEL_ID: &str = "apple-speech";

/// A recognition run should finish in well under this. The ceiling exists so a
/// task that never calls its handler surfaces as a failure instead of hanging
/// the dictation pipeline forever.
const RESULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub struct AppleSpeechProvider;

impl AppleSpeechProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AppleSpeechProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether the user has granted speech recognition.
///
/// Separate from the microphone permission: macOS treats sending audio to the
/// recogniser as its own consent, even when recognition is on-device.
pub fn authorization() -> SFSpeechRecognizerAuthorizationStatus {
    unsafe { SFSpeechRecognizer::authorizationStatus() }
}

/// Ask for speech-recognition access, blocking until macOS answers.
pub fn request_authorization() -> SFSpeechRecognizerAuthorizationStatus {
    let current = authorization();
    if current != SFSpeechRecognizerAuthorizationStatus::NotDetermined {
        return current;
    }

    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = sender.send(status);
    });

    unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };

    receiver
        .recv_timeout(std::time::Duration::from_secs(60))
        .unwrap_or(SFSpeechRecognizerAuthorizationStatus::NotDetermined)
}

/// Fail with a sentence the user can act on when speech recognition has not
/// been granted.
pub(crate) fn require_speech_access() -> Result<(), ProviderError> {
    match authorization() {
        SFSpeechRecognizerAuthorizationStatus::Authorized => Ok(()),
        SFSpeechRecognizerAuthorizationStatus::NotDetermined => Err(ProviderError::BadRequest {
            provider: PROVIDER_ID,
            detail: "macOS hasn't been asked for speech recognition access yet".into(),
        }),
        _ => Err(ProviderError::BadRequest {
            provider: PROVIDER_ID,
            detail: "Speech recognition access is turned off in System Settings".into(),
        }),
    }
}

#[async_trait]
impl TranscriptionProvider for AppleSpeechProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Apple Speech"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: true,
            batch: true,
            streaming: true,
            timestamps: true,
            word_timestamps: true,
            diarization: false,
            language_detection: false,
            translation: false,
            prompting: false,
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: MODEL_ID.into(),
            name: "On-device".into(),
            description: "Built into macOS. No key, no download, nothing leaves this Mac.".into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::High,
            multilingual: true,
        }]
    }

    fn default_model(&self) -> &'static str {
        MODEL_ID
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError> {
        require_speech_access()?;

        let path = request.audio.path().to_path_buf();
        let language = request.language.clone();
        let started = Instant::now();

        // The recognition callbacks arrive on their own queue, and the
        // surrounding API is blocking, so this stays off the async workers.
        let text = tauri::async_runtime::spawn_blocking(move || recognise(&path, language.as_deref()))
            .await
            .map_err(|_| ProviderError::ServiceUnavailable {
                provider: PROVIDER_ID,
                status: 500,
            })??;

        Ok(Transcription {
            text,
            provider: PROVIDER_ID.to_string(),
            model: request.model,
            language: request.language,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn recognise(path: &std::path::Path, language: Option<&str>) -> Result<String, ProviderError> {
    let failure = |detail: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail,
    };

    unsafe {
        // A locale-specific recogniser can legitimately not exist; the default
        // one always does.
        let recognizer: Option<Retained<SFSpeechRecognizer>> = match language {
            Some(code) => {
                let identifier = NSString::from_str(code);
                let locale = NSLocale::localeWithLocaleIdentifier(&identifier);
                SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            }
            None => Some(SFSpeechRecognizer::new()),
        };

        let Some(recognizer) = recognizer else {
            return Err(failure(
                "macOS has no speech recogniser for this language".into(),
            ));
        };

        if !recognizer.isAvailable() {
            return Err(failure(
                "Apple Speech is unavailable right now — try again shortly".into(),
            ));
        }

        let path_string = NSString::from_str(&path.to_string_lossy());
        let url = NSURL::fileURLWithPath(&path_string);
        let speech_request =
            SFSpeechURLRecognitionRequest::initWithURL(SFSpeechURLRecognitionRequest::alloc(), &url);

        // A provider described as local must actually be local: without this
        // macOS is free to send the audio to Apple's servers.
        speech_request.setRequiresOnDeviceRecognition(true);

        let (sender, receiver) = mpsc::channel::<Result<String, String>>();

        // The handler fires repeatedly with partial results. Only the final one
        // is taken; `send` on an already-closed channel is ignored, so a late
        // callback after a timeout is harmless.
        let handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                if !error.is_null() {
                    let message = (*error).localizedDescription().to_string();
                    let _ = sender.send(Err(message));
                    return;
                }
                if result.is_null() {
                    return;
                }
                let result = &*result;
                if result.isFinal() {
                    let text = result.bestTranscription().formattedString().to_string();
                    let _ = sender.send(Ok(text));
                }
            },
        );

        let _task =
            recognizer.recognitionTaskWithRequest_resultHandler(&speech_request, &handler);

        match receiver.recv_timeout(RESULT_TIMEOUT) {
            Ok(Ok(text)) => Ok(text.trim().to_string()),
            Ok(Err(message)) => Err(failure(message)),
            Err(_) => Err(failure(
                "Apple Speech did not return a result in time".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_speech_is_local() {
        let apple = AppleSpeechProvider::new();
        assert!(apple.capabilities().local);
    }

    #[test]
    fn its_default_model_is_one_it_offers() {
        let apple = AppleSpeechProvider::new();
        assert!(apple.has_model(apple.default_model()));
    }

    /// Unlike the local engines, Apple Speech ships with macOS — it must offer
    /// its model whether or not anything has been downloaded.
    #[test]
    fn it_always_offers_a_model() {
        assert!(!AppleSpeechProvider::new().models().is_empty());
    }

    #[test]
    fn reading_the_authorization_status_does_not_panic() {
        let _ = authorization();
    }
}
