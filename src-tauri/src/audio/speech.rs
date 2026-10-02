//! Is there actually speech in this recording?
//!
//! Whisper-family models invent text for silence — "Thank you.", "Thanks for
//! watching." — because that is what ended the clips they were trained on. The
//! only dependable defence is to not send them silence, so every recording is
//! measured first.
//!
//! The threshold is relative to the recording's own noise floor rather than a
//! fixed level: a quiet microphone and a loud one both have to work.

use std::time::Duration;

use super::resample::TARGET_SAMPLE_RATE;

/// 20 ms frames.
const FRAME_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize) / 50;

/// A frame counts as voiced when it is this many times louder than the
/// recording's noise floor...
const FLOOR_MULTIPLE: f32 = 3.0;
/// ...and at least this loud in absolute terms (about -56 dBFS), so digital
/// near-silence is never "speech" however quiet the room.
const ABSOLUTE_MINIMUM: f32 = 0.0015;

/// Less voiced audio than this is a click or a cough, not something said.
pub const MIN_SPEECH: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechStats {
    /// Total time with voiced audio in it.
    pub speech: Duration,
}

impl SpeechStats {
    pub fn has_speech(&self) -> bool {
        self.speech >= MIN_SPEECH
    }
}

pub fn analyze(samples: &[i16]) -> SpeechStats {
    let levels: Vec<f32> = samples.chunks(FRAME_SAMPLES).map(frame_level).collect();
    if levels.is_empty() {
        return SpeechStats {
            speech: Duration::ZERO,
        };
    }

    // The quietest fifth of the recording is the room, not the speaker.
    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 5];
    let threshold = (floor * FLOOR_MULTIPLE).max(ABSOLUTE_MINIMUM);

    let voiced = levels.iter().filter(|level| **level > threshold).count();
    SpeechStats {
        speech: Duration::from_millis(voiced as u64 * 20),
    }
}

fn frame_level(frame: &[i16]) -> f32 {
    let sum: f64 = frame
        .iter()
        .map(|sample| {
            let value = f64::from(*sample) / 32768.0;
            value * value
        })
        .sum();
    (sum / frame.len().max(1) as f64).sqrt() as f32
}

/// Phrases speech models produce from silence or noise. Compared against the
/// whole transcript, normalised, so a real sentence that merely contains one is
/// untouched.
const PHANTOM_TRANSCRIPTS: &[&str] = &[
    "thank you",
    "thank you very much",
    "thank you so much",
    "thanks",
    "thanks for watching",
    "thank you for watching",
    "thank you for watching and i ll see you in the next video",
    "please subscribe",
    "like and subscribe",
    "bye",
    "bye bye",
    "goodbye",
    "you",
    "so",
    "okay",
    "oh",
    "uh",
    "um",
    "hmm",
    "subtitles by the amara org community",
    "transcribed by",
];

/// Short recordings with little voiced audio and a transcript that is only one
/// of the phantom phrases: Whisper talking to itself. Deliberately limited to
/// brief speech, so someone who really does say "thank you" for two seconds
/// still gets their words.
pub fn is_phantom_transcript(text: &str, speech: Duration) -> bool {
    if speech > Duration::from_millis(1500) {
        return false;
    }
    let normalised: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    normalised.is_empty() || PHANTOM_TRANSCRIPTS.contains(&normalised.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32, amplitude: i16) -> Vec<i16> {
        (0..(seconds * TARGET_SAMPLE_RATE as f32) as usize)
            .map(|i| {
                let phase = i as f32 * 440.0 * std::f32::consts::TAU / TARGET_SAMPLE_RATE as f32;
                (phase.sin() * f32::from(amplitude)) as i16
            })
            .collect()
    }

    #[test]
    fn silence_has_no_speech() {
        assert!(!analyze(&vec![0; 16_000 * 2]).has_speech());
    }

    #[test]
    fn steady_hiss_has_no_speech() {
        assert!(!analyze(&tone(2.0, 20)).has_speech());
    }

    #[test]
    fn a_quiet_voice_over_a_quiet_room_is_speech() {
        let mut samples = tone(1.0, 6);
        samples.extend(tone(1.0, 300));
        samples.extend(tone(1.0, 6));
        assert!(analyze(&samples).has_speech());
    }

    #[test]
    fn a_click_is_not_speech() {
        let mut samples = vec![0; 16_000 * 2];
        for sample in samples.iter_mut().skip(8_000).take(400) {
            *sample = 9_000;
        }
        assert!(!analyze(&samples).has_speech());
    }

    #[test]
    fn phantom_phrases_are_recognised_only_for_brief_audio() {
        let brief = Duration::from_millis(400);
        assert!(is_phantom_transcript("Thank you.", brief));
        assert!(is_phantom_transcript("  Thanks for watching! ", brief));
        assert!(!is_phantom_transcript("Thank you for the update.", brief));
        assert!(!is_phantom_transcript("Thank you.", Duration::from_secs(3)));
    }
}
