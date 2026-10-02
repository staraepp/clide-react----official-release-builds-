//! Microphone capture.
//!
//! `cpal::Stream` is neither `Send` nor `Sync` on macOS, so the stream lives on
//! a dedicated worker thread and the rest of the app talks to it over a
//! channel. That keeps `Recorder` cheaply shareable inside Tauri state without
//! smuggling a CoreAudio handle across threads.
//!
//! Audio is accumulated in memory as 16 kHz mono PCM and written to a WAV file
//! only when capture stops: dictation clips are short, and keeping file I/O out
//! of the realtime callback avoids glitches.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use super::clip::RecordedClip;
use super::error::AudioError;
use super::resample::{rms_level, MonoDownsampler, TARGET_SAMPLE_RATE};

/// Hard ceiling on a single dictation. Well under Groq's 25 MB upload limit at
/// 16 kHz mono (~19 MB), and long past the point where a dictation was a
/// mistake the user wants to abandon rather than transcribe.
const MAX_CAPTURE: Duration = Duration::from_secs(600);

/// Receives each new batch of 16 kHz mono samples as capture produces them.
///
/// Runs on the audio callback thread, so it must only hand the samples off.
pub type SampleTap = Arc<dyn Fn(&[i16]) + Send + Sync>;

type TapSlot = Arc<Mutex<Option<SampleTap>>>;

/// Where the capture callback writes; drained by the worker thread on stop.
struct CaptureSink {
    downsampler: MonoDownsampler,
    samples: Vec<i16>,
    level: Arc<AtomicU32>,
    max_samples: usize,
    reached_limit: bool,
    tap: TapSlot,
}

impl CaptureSink {
    fn accept(&mut self, interleaved: &[f32]) {
        self.level
            .store(rms_level(interleaved).to_bits(), Ordering::Relaxed);

        if self.samples.len() >= self.max_samples {
            self.reached_limit = true;
            return;
        }
        let before = self.samples.len();
        self.downsampler.push(interleaved, &mut self.samples);

        if self.samples.len() > before {
            if let Ok(tap) = self.tap.lock() {
                if let Some(tap) = tap.as_ref() {
                    tap(&self.samples[before..]);
                }
            }
        }
    }
}

enum Command {
    Start(SyncSender<Result<(), AudioError>>),
    Stop(SyncSender<Result<RecordedClip, AudioError>>),
    Abort,
}

/// Handle to the capture engine. Cloneable, `Send + Sync`, safe to keep in
/// Tauri managed state.
pub struct Recorder {
    commands: SyncSender<Command>,
    level: Arc<AtomicU32>,
    tap: TapSlot,
}

impl Recorder {
    /// Spawn the capture worker. `clip_dir` holds temporary WAV files and is
    /// created if missing.
    pub fn spawn(clip_dir: PathBuf) -> Self {
        let (tx, rx) = sync_channel::<Command>(4);
        let level = Arc::new(AtomicU32::new(0));
        let worker_level = Arc::clone(&level);
        let tap: TapSlot = Arc::new(Mutex::new(None));
        let worker_tap = Arc::clone(&tap);

        std::thread::Builder::new()
            .name("clide-audio".into())
            .spawn(move || worker_loop(rx, clip_dir, worker_level, worker_tap))
            .expect("failed to spawn the audio worker thread");

        Self { commands: tx, level, tap }
    }

    /// Current microphone level in 0.0..=1.0, sampled by the HUD.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// Mirror captured audio to `tap` until it is cleared with `None`. Used by
    /// live typing; the clip written on stop is unaffected.
    pub fn set_tap(&self, tap: Option<SampleTap>) {
        if let Ok(mut slot) = self.tap.lock() {
            *slot = tap;
        }
    }

    pub fn start(&self) -> Result<(), AudioError> {
        let (reply, answer) = sync_channel(1);
        self.commands
            .send(Command::Start(reply))
            .map_err(|_| AudioError::EngineGone)?;
        answer.recv().map_err(|_| AudioError::EngineGone)?
    }

    /// Stop capture and flush the recording to a temporary WAV file.
    pub fn stop(&self) -> Result<RecordedClip, AudioError> {
        let (reply, answer) = sync_channel(1);
        self.commands
            .send(Command::Stop(reply))
            .map_err(|_| AudioError::EngineGone)?;
        answer.recv().map_err(|_| AudioError::EngineGone)?
    }

    /// Stop capture and throw the audio away. Used when the user cancels.
    pub fn abort(&self) {
        let _ = self.commands.send(Command::Abort);
        self.level.store(0, Ordering::Relaxed);
    }
}

struct Active {
    stream: cpal::Stream,
    sink: Arc<Mutex<CaptureSink>>,
    stream_error: Arc<Mutex<Option<String>>>,
    started: Instant,
}

fn worker_loop(rx: Receiver<Command>, clip_dir: PathBuf, level: Arc<AtomicU32>, tap: TapSlot) {
    let _ = std::fs::create_dir_all(&clip_dir);
    super::clip::sweep_orphans(&clip_dir);

    let mut active: Option<Active> = None;

    while let Ok(command) = rx.recv() {
        match command {
            Command::Start(reply) => {
                let result = if active.is_some() {
                    Err(AudioError::AlreadyRecording)
                } else {
                    match open_stream(Arc::clone(&level), Arc::clone(&tap)) {
                        Ok(started) => {
                            active = Some(started);
                            Ok(())
                        }
                        Err(error) => Err(error),
                    }
                };
                let _ = reply.send(result);
            }

            Command::Stop(reply) => {
                let result = match active.take() {
                    None => Err(AudioError::NotRecording),
                    Some(session) => finish(session, &clip_dir),
                };
                level.store(0, Ordering::Relaxed);
                let _ = reply.send(result);
            }

            Command::Abort => {
                if let Some(session) = active.take() {
                    drop(session.stream);
                }
                level.store(0, Ordering::Relaxed);
            }
        }
    }
}

fn open_stream(level: Arc<AtomicU32>, tap: TapSlot) -> Result<Active, AudioError> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(AudioError::NoInputDevice)?;
    let config = device
        .default_input_config()
        .map_err(|e| AudioError::DeviceUnavailable(e.to_string()))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    let sink = Arc::new(Mutex::new(CaptureSink {
        downsampler: MonoDownsampler::new(sample_rate, channels),
        samples: Vec::with_capacity(TARGET_SAMPLE_RATE as usize * 8),
        level,
        max_samples: TARGET_SAMPLE_RATE as usize * MAX_CAPTURE.as_secs() as usize,
        reached_limit: false,
        tap,
    }));

    let stream_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let error_slot = Arc::clone(&stream_error);
    let on_error = move |error: cpal::StreamError| {
        tracing::error!(?error, "microphone stream error");
        *error_slot.lock().unwrap() = Some(error.to_string());
    };

    // One closure per sample format; each converts into f32 and hands the
    // buffer to the same sink so the conversion path stays in one place.
    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let sink = Arc::clone(&sink);
            let mut scratch: Vec<f32> = Vec::new();
            device.build_input_stream(
                &stream_config,
                move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().copied().map($convert));
                    if let Ok(mut sink) = sink.lock() {
                        sink.accept(&scratch);
                    }
                },
                on_error,
                None,
            )
        }};
    }

    let stream = match sample_format {
        SampleFormat::F32 => {
            let sink = Arc::clone(&sink);
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut sink) = sink.lock() {
                        sink.accept(data);
                    }
                },
                on_error,
                None,
            )
        }
        SampleFormat::I16 => build!(i16, |s| s as f32 / i16::MAX as f32),
        SampleFormat::U16 => build!(u16, |s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0),
        SampleFormat::I32 => build!(i32, |s| s as f32 / i32::MAX as f32),
        SampleFormat::I8 => build!(i8, |s| s as f32 / i8::MAX as f32),
        other => {
            return Err(AudioError::DeviceUnavailable(format!(
                "unsupported sample format {other:?}"
            )))
        }
    }
    .map_err(|e| AudioError::DeviceUnavailable(e.to_string()))?;

    stream
        .play()
        .map_err(|e| AudioError::DeviceUnavailable(e.to_string()))?;

    tracing::info!(sample_rate, channels, ?sample_format, "microphone open");

    Ok(Active {
        stream,
        sink,
        stream_error,
        started: Instant::now(),
    })
}

fn finish(session: Active, clip_dir: &std::path::Path) -> Result<RecordedClip, AudioError> {
    // Dropping the stream stops the callback before we touch the sink.
    drop(session.stream);

    if let Some(message) = session.stream_error.lock().unwrap().take() {
        return Err(AudioError::DeviceUnavailable(message));
    }

    let mut samples = {
        let mut sink = session
            .sink
            .lock()
            .map_err(|_| AudioError::Write("capture buffer was poisoned".into()))?;
        let mut samples = std::mem::take(&mut sink.samples);
        sink.downsampler.flush(&mut samples);
        if sink.reached_limit {
            tracing::warn!("dictation hit the maximum capture length");
        }
        samples
    };

    // A device that is present but muted at the hardware level yields perfect
    // silence; sending that to a provider wastes a request and returns junk.
    if samples.is_empty() || samples.iter().all(|s| *s == 0) {
        return Err(AudioError::Empty);
    }

    // Trim a trailing DC tail if the device padded the buffer.
    while samples.last() == Some(&0) && samples.len() > 1 {
        samples.pop();
    }

    let path = clip_dir.join(format!("{}.wav", uuid::Uuid::new_v4()));
    write_wav(&path, &samples)?;

    Ok(RecordedClip::new(path, session.started.elapsed()))
}

fn write_wav(path: &std::path::Path, samples: &[i16]) -> Result<(), AudioError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| AudioError::Write(e.to_string()))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|e| AudioError::Write(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| AudioError::Write(e.to_string()))
}
