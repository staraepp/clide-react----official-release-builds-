//! Streaming recognition: words arrive while the user is still speaking.
//!
//! The Objective-C recogniser objects are not `Send`, so they live on one
//! dedicated thread and the rest of the app talks to it over a channel. Audio
//! goes in as 16 kHz samples; hypotheses come back as [`LiveEvent`]s.
//!
//! Recognition stays on-device, same as the batch path.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_avf_audio::{AVAudioFormat, AVAudioPCMBuffer};
use objc2_foundation::{NSError, NSLocale, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognizer,
};

use crate::providers::error::ProviderError;

const PROVIDER_ID: &str = "apple";
const SAMPLE_RATE: f64 = 16_000.0;

/// What the recogniser has to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveEvent {
    /// The current best guess for everything said so far. May still change.
    Partial(String),
    /// The settled transcript. Nothing follows it.
    Final(String),
    /// Recognition ended without a result.
    Failed(String),
}

enum Command {
    Audio(Vec<i16>),
    EndAudio,
}

/// A running recognition session. Dropping it cancels recognition.
pub struct LiveRecognizer {
    commands: Sender<Command>,
}

impl LiveRecognizer {
    /// Start recognising. Returns once the recogniser is confirmed available,
    /// so the caller can fall back to batch transcription immediately.
    pub fn start(
        language: Option<String>,
    ) -> Result<(Self, Receiver<LiveEvent>), ProviderError> {
        super::require_speech_access()?;

        let (commands, command_rx) = mpsc::channel::<Command>();
        let (events, event_rx) = mpsc::channel::<LiveEvent>();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);

        std::thread::Builder::new()
            .name("clide-live-speech".into())
            .spawn(move || run(language, command_rx, events, ready_tx))
            .map_err(|error| ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: error.to_string(),
            })?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok((Self { commands }, event_rx)),
            Ok(Err(detail)) => Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail,
            }),
            Err(_) => Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: "Apple Speech did not start in time".into(),
            }),
        }
    }

    pub fn push(&self, samples: &[i16]) {
        let _ = self.commands.send(Command::Audio(samples.to_vec()));
    }

    /// No more audio is coming; the recogniser will settle and send `Final`.
    pub fn end_audio(&self) {
        let _ = self.commands.send(Command::EndAudio);
    }
}

fn run(
    language: Option<String>,
    commands: Receiver<Command>,
    events: Sender<LiveEvent>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    unsafe {
        let recognizer: Option<Retained<SFSpeechRecognizer>> = match language.as_deref() {
            Some(code) => {
                let locale = NSLocale::localeWithLocaleIdentifier(&NSString::from_str(code));
                SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            }
            None => Some(SFSpeechRecognizer::new()),
        };

        let Some(recognizer) = recognizer else {
            let _ = ready.send(Err("macOS has no speech recogniser for this language".into()));
            return;
        };
        if !recognizer.isAvailable() {
            let _ = ready.send(Err("Apple Speech is unavailable right now".into()));
            return;
        }

        let Some(format) = AVAudioFormat::initStandardFormatWithSampleRate_channels(
            AVAudioFormat::alloc(),
            SAMPLE_RATE,
            1,
        ) else {
            let _ = ready.send(Err("could not describe the audio format".into()));
            return;
        };

        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.setShouldReportPartialResults(true);
        request.setRequiresOnDeviceRecognition(true);
        // Punctuation and capitals as the words arrive; typing "hello world"
        // with no sentence casing would be worse than waiting.
        request.setAddsPunctuation(true);

        let handler_events = events.clone();
        let handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                if !error.is_null() {
                    let message = (*error).localizedDescription().to_string();
                    let _ = handler_events.send(LiveEvent::Failed(message));
                    return;
                }
                if result.is_null() {
                    return;
                }
                let result = &*result;
                let text = result.bestTranscription().formattedString().to_string();
                let event = if result.isFinal() {
                    LiveEvent::Final(text)
                } else {
                    LiveEvent::Partial(text)
                };
                let _ = handler_events.send(event);
            },
        );

        let task = recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler);
        let _ = ready.send(Ok(()));

        while let Ok(command) = commands.recv() {
            match command {
                Command::Audio(samples) => append(&request, &format, &samples),
                Command::EndAudio => request.endAudio(),
            }
        }

        // The owner dropped its handle: the dictation is over either way.
        task.cancel();
    }
}

unsafe fn append(
    request: &SFSpeechAudioBufferRecognitionRequest,
    format: &AVAudioFormat,
    samples: &[i16],
) {
    if samples.is_empty() {
        return;
    }
    let frames = samples.len() as u32;
    let Some(buffer) =
        AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(AVAudioPCMBuffer::alloc(), format, frames)
    else {
        return;
    };
    buffer.setFrameLength(frames);

    let channels = buffer.floatChannelData();
    if channels.is_null() {
        return;
    }
    let destination = (*channels).as_ptr();
    for (index, sample) in samples.iter().enumerate() {
        *destination.add(index) = f32::from(*sample) / 32768.0;
    }

    request.appendAudioPCMBuffer(&buffer);
}
