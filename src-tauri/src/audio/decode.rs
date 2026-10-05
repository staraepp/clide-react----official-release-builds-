//! Decoding an uploaded audio file into what the speech engines take.
//!
//! macOS already reads wav, m4a/aac, mp3, flac, caf and aiff, so this asks
//! AVFoundation to do the decoding and only does the last step itself:
//! downmix to mono and bring the rate to 16 kHz. Containers AVFoundation cannot
//! read (browser `MediaRecorder`'s webm/opus, ogg) are refused up front with
//! a message naming what does work.

use std::path::Path;

use objc2::AnyThread;
use objc2_avf_audio::{AVAudioFile, AVAudioPCMBuffer};
use objc2_foundation::{NSString, NSURL};

use super::resample::{MonoDownsampler, TARGET_SAMPLE_RATE};

/// Formats worth naming in an error message.
pub const SUPPORTED_FORMATS: &str = "wav, m4a, mp3, flac, caf, aiff";

/// Longer than this is almost certainly not a dictation, and decoding it would
/// hold a large buffer and the speech engine for a long time.
pub const MAX_SECONDS: usize = 30 * 60;

const CHUNK_FRAMES: u32 = 32_768;

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// A container this build cannot read (webm, ogg, opus).
    Unsupported(String),
    /// A supported-looking file that would not decode.
    Unreadable(String),
    TooLong,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Unsupported(what) => write!(
                f,
                "{what} audio is not supported. Send one of: {SUPPORTED_FORMATS}."
            ),
            DecodeError::Unreadable(detail) => write!(
                f,
                "The audio could not be read ({detail}). Send one of: {SUPPORTED_FORMATS}."
            ),
            DecodeError::TooLong => write!(
                f,
                "The audio is longer than {} minutes.",
                MAX_SECONDS / 60
            ),
        }
    }
}

/// Refuse containers AVFoundation cannot decode, judging by the first bytes
/// and by the file name's extension.
pub fn check_container(head: &[u8], file_name: Option<&str>) -> Result<(), DecodeError> {
    // EBML header: webm and matroska.
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Err(DecodeError::Unsupported("webm".into()));
    }
    if head.starts_with(b"OggS") {
        return Err(DecodeError::Unsupported("ogg/opus".into()));
    }

    let extension = file_name
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("webm") | Some("mkv") => Err(DecodeError::Unsupported("webm".into())),
        Some("ogg") | Some("oga") | Some("opus") => Err(DecodeError::Unsupported("ogg/opus".into())),
        _ => Ok(()),
    }
}

/// Decode `path` to 16 kHz mono 16-bit samples. Blocking.
pub fn decode_to_16k_mono(path: &Path) -> Result<Vec<i16>, DecodeError> {
    let unreadable = |detail: String| DecodeError::Unreadable(detail);

    // SAFETY: every call below is made on this thread, on objects created here
    // and dropped before returning; AVAudioFile documents no thread affinity
    // beyond not being shared.
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let file = AVAudioFile::initForReading_error(AVAudioFile::alloc(), &url)
            .map_err(|error| unreadable(error.localizedDescription().to_string()))?;

        let format = file.processingFormat();
        let rate = format.sampleRate();
        let channels = format.channelCount() as usize;
        let total_frames = file.length().max(0) as u64;
        if channels == 0 || rate <= 0.0 {
            return Err(unreadable("no audio stream".into()));
        }
        if total_frames as f64 / rate > MAX_SECONDS as f64 {
            return Err(DecodeError::TooLong);
        }

        let buffer = AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            CHUNK_FRAMES,
        )
        .ok_or_else(|| unreadable("could not allocate a buffer".into()))?;

        let mut mono: Vec<f32> = Vec::with_capacity(total_frames as usize);
        let mut remaining = total_frames;

        while remaining > 0 {
            let want = remaining.min(u64::from(CHUNK_FRAMES)) as u32;
            file.readIntoBuffer_frameCount_error(&buffer, want)
                .map_err(|error| unreadable(error.localizedDescription().to_string()))?;

            let got = buffer.frameLength() as usize;
            if got == 0 {
                break;
            }
            let planes = buffer.floatChannelData();
            if planes.is_null() {
                return Err(unreadable("unsupported sample format".into()));
            }

            for frame in 0..got {
                let mut sum = 0.0f32;
                for channel in 0..channels {
                    sum += *(*planes.add(channel)).as_ptr().add(frame);
                }
                mono.push(sum / channels as f32);
            }
            remaining = remaining.saturating_sub(got as u64);
        }

        Ok(to_16k(&mono, rate))
    }
}

/// Bring mono samples at `rate` to 16 kHz 16-bit.
fn to_16k(mono: &[f32], rate: f64) -> Vec<i16> {
    let target = f64::from(TARGET_SAMPLE_RATE);

    if rate >= target {
        // Averaging windows doubles as cheap anti-aliasing.
        let mut downsampler = MonoDownsampler::new(rate.round() as u32, 1);
        let mut out = Vec::with_capacity((mono.len() as f64 * target / rate) as usize + 1);
        downsampler.push(mono, &mut out);
        downsampler.flush(&mut out);
        return out;
    }

    // Narrowband audio (8 kHz telephone recordings): linear interpolation up.
    let length = (mono.len() as f64 * target / rate) as usize;
    (0..length)
        .map(|index| {
            let position = index as f64 * rate / target;
            let below = position.floor() as usize;
            let above = (below + 1).min(mono.len().saturating_sub(1));
            let fraction = (position - below as f64) as f32;
            let sample = mono[below.min(mono.len() - 1)] * (1.0 - fraction) + mono[above] * fraction;
            (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
        })
        .collect()
}

/// Write 16 kHz mono 16-bit samples as a WAV the engines can read.
pub fn write_wav(path: &Path, samples: &[i16]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        writer.write_sample(*sample).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_wav(path: &Path, rate: u32, channels: u16, seconds: f32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for index in 0..(rate as f32 * seconds) as usize {
            let value = ((index as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16;
            for _ in 0..channels {
                writer.write_sample(value).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("clide-decode-{}-{name}", std::process::id()))
    }

    #[test]
    fn stereo_44k_becomes_16k_mono() {
        let path = temp("stereo.wav");
        write_test_wav(&path, 44_100, 2, 2.0);
        let samples = decode_to_16k_mono(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let expected = 32_000i64;
        assert!((samples.len() as i64 - expected).abs() < 64, "got {}", samples.len());
        assert!(samples.iter().any(|s| s.abs() > 4000), "the signal was lost");
    }

    #[test]
    fn narrowband_8k_is_brought_up_to_16k() {
        let path = temp("narrow.wav");
        write_test_wav(&path, 8_000, 1, 1.0);
        let samples = decode_to_16k_mono(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!((samples.len() as i64 - 16_000).abs() < 64, "got {}", samples.len());
    }

    #[test]
    fn a_text_file_is_unreadable_not_a_panic() {
        let path = temp("not-audio.m4a");
        std::fs::write(&path, b"this is not audio").unwrap();
        let result = decode_to_16k_mono(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(result, Err(DecodeError::Unreadable(_))));
    }

    #[test]
    fn webm_and_ogg_are_refused_by_magic_bytes_and_by_name() {
        assert!(matches!(
            check_container(&[0x1A, 0x45, 0xDF, 0xA3, 0, 0], Some("clip")),
            Err(DecodeError::Unsupported(_))
        ));
        assert!(matches!(
            check_container(b"OggS....", None),
            Err(DecodeError::Unsupported(_))
        ));
        assert!(check_container(b"RIFF....", Some("take.webm")).is_err());
        assert!(check_container(b"RIFF....", Some("take.WAV")).is_ok());
        assert!(check_container(b"ID3", Some("take.mp3")).is_ok());
    }

    #[test]
    fn the_error_names_the_formats_that_work() {
        let message = DecodeError::Unsupported("webm".into()).to_string();
        assert!(message.contains("wav") && message.contains("m4a"));
    }
}
