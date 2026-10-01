//! Transcript processing — the stage between the provider and the keyboard.
//!
//! Processing is deliberately its own step in the pipeline
//! (`audio -> STT -> transcript -> processing -> insertion`) so that adding
//! Rewrite later means adding a mode here, not threading an LLM through the
//! dictation code.

pub mod backtrack;
pub mod polish;
pub mod spoken;
pub mod techformat;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessingMode {
    /// What was said, tidied only where it cannot change meaning.
    #[default]
    Verbatim,
    /// Deterministic local cleanup. No model, no network, no latency.
    Polished,
    /// Reserved: an LLM stage that rewrites speech into written prose.
    Rewrite,
}

impl ProcessingMode {
    /// Every mode this build can run.
    ///
    /// Rewrite is now among them: the deterministic pass happens here and a
    /// refiner finishes the job in `dictation::pipeline`. Whether a refinement
    /// *engine* is available is a separate question, answered by
    /// `refine::RefinerRegistry` — and if none is, the deterministic result is
    /// used rather than the dictation failing.
    pub fn is_available(self) -> bool {
        true
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verbatim => "verbatim",
            Self::Polished => "polished",
            Self::Rewrite => "rewrite",
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("Rewrite needs a refinement engine that is available on this Mac")]
    NoRefiner,
}

/// Run a raw transcript through the selected mode.
///
/// Processing never returns empty text for non-empty input: a cleanup step
/// that deletes someone's sentence is worse than one that leaves it untouched.
pub fn process(
    mode: ProcessingMode,
    raw: &str,
    spoken_punctuation: bool,
) -> Result<String, ProcessingError> {
    // Applied first and in every mode: punctuation the user said aloud is part
    // of what they said, not a stylistic rewrite of it. It costs nothing and
    // never needs a model.
    let raw = if spoken_punctuation {
        spoken::apply_spoken_punctuation(raw)
    } else {
        raw.to_string()
    };
    let raw = raw.as_str();

    let normalized = polish::normalize_whitespace(raw);

    let processed = match mode {
        ProcessingMode::Verbatim => normalized,
        ProcessingMode::Polished => polish::polish(&normalized),
        // Rewrite is finished asynchronously by `refine`; the deterministic
        // pass still runs so the model receives tidy input, and so there is
        // something sane to fall back to if refinement cannot run.
        ProcessingMode::Rewrite => polish::polish(&normalized),
    };

    Ok(if processed.trim().is_empty() && !raw.trim().is_empty() {
        polish::normalize_whitespace(raw)
    } else {
        processed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_keeps_the_words_and_only_fixes_spacing() {
        let raw = "  um   I  think   we should ship it  ";
        assert_eq!(
            process(ProcessingMode::Verbatim, raw, false).unwrap(),
            "um I think we should ship it"
        );
    }

    #[test]
    fn polished_cleans_the_same_sentence_up() {
        let raw = "  um   i  think   we should ship it  ";
        assert_eq!(
            process(ProcessingMode::Polished, raw, false).unwrap(),
            "I think we should ship it"
        );
    }

    /// Rewrite's deterministic half must behave exactly like Polished. The
    /// model runs afterwards, in the pipeline — so if refinement cannot run,
    /// what the user gets is a polished transcript, never an empty one.
    /// Punctuation the user spoke aloud is part of what they said, so it must
    /// survive every mode — including Verbatim, which changes nothing else.
    #[test]
    fn spoken_punctuation_applies_in_every_mode() {
        let spoken = "hello comma this works question mark";

        for mode in [
            ProcessingMode::Verbatim,
            ProcessingMode::Polished,
            ProcessingMode::Rewrite,
        ] {
            let out = process(mode, spoken, true).unwrap();
            assert!(out.contains(','), "{mode:?} dropped the comma: {out}");
            assert!(out.contains('?'), "{mode:?} dropped the question mark: {out}");
            assert!(!out.contains("comma"), "{mode:?} left the word in: {out}");
        }
    }

    /// And is genuinely off when switched off.
    #[test]
    fn spoken_punctuation_can_be_disabled() {
        let out = process(ProcessingMode::Verbatim, "hello comma world", false).unwrap();
        assert!(out.contains("comma"), "the words were replaced anyway: {out}");
    }

    #[test]
    fn rewrite_falls_back_to_the_polished_result() {
        let raw = "um so  i think we should ship it";
        assert_eq!(
            process(ProcessingMode::Rewrite, raw, false).unwrap(),
            process(ProcessingMode::Polished, raw, false).unwrap()
        );
        assert!(ProcessingMode::Rewrite.is_available());
    }

    #[test]
    fn processing_never_swallows_a_transcript() {
        // Nothing but filler: cleanup would empty it, so the raw text wins.
        let raw = "um uh um";
        let out = process(ProcessingMode::Polished, raw, false).unwrap();
        assert!(!out.trim().is_empty(), "processing deleted the transcript");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(process(ProcessingMode::Polished, "   ", false).unwrap(), "");
    }
}
