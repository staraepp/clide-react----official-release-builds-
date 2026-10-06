//! Turning a raw transcript into the text the user gets.
//!
//! Shared by dictation and the local API, which is why it knows nothing about
//! the dictation session, the HUD or events: it takes settings and the app's
//! shared state and returns text.

use crate::processing;
use crate::refine::{accepts_refinement, RefineRequest};
use crate::settings::AppSettings;
use crate::state::AppState;

pub struct Finished {
    pub text: String,
    /// How many spoken corrections ("scratch that") were applied.
    pub corrections: usize,
    /// The user scratched everything they said. `text` is empty, and the
    /// caller should treat the dictation as cancelled rather than inserting
    /// the raw words back.
    pub scratched_all: bool,
}

/// Cleanup failed. The transcript is carried so the caller can still offer it.
pub struct FinishFailure {
    pub message: String,
    pub transcript: String,
}

/// Backtracking, known names, optional code formatting, the chosen mode, and
/// (in Rewrite) the refinement engine, in that order.
///
/// `app_name` is the only context a refiner ever sees: the app being written
/// into, or `None` when there is no such app (an API upload).
pub async fn finish_text(
    state: &AppState,
    raw: &str,
    settings: &AppSettings,
    app_name: Option<String>,
) -> Result<Finished, FinishFailure> {
    // Spoken corrections come first: a command phrase is never meant to be
    // typed, and everything after this point should see the text the user
    // actually meant.
    let corrected = processing::backtrack::apply_backtracking(raw);
    if corrected.corrections > 0 {
        tracing::info!(count = corrected.corrections, "spoken corrections applied");
    }

    // `processing::process` would helpfully resurrect the raw text, so a fully
    // scratched dictation is reported instead of passed on.
    if corrected.text.trim().is_empty() && corrected.corrections > 0 {
        return Ok(Finished {
            text: String::new(),
            corrections: corrected.corrections,
            scratched_all: true,
        });
    }

    // Spelling of the names Clide is asked to write most. Always on: it needs
    // no model and is the same in every mode.
    let corrected_text = processing::names::apply_known_names(&corrected.text);

    // The user's own spellings. Read before the first await: the database
    // lock must not be held across one.
    let user_words = crate::database::dictionary::manual_words(&state.db.lock())
        .unwrap_or_default();
    let corrected_text = processing::dictionary::apply_dictionary(&corrected_text, &user_words);

    // Opt-in: spoken paths become code spans before Polish, which knows to
    // leave them alone, and before Rewrite, which is told to copy them.
    let prepared = if settings.format_technical_terms {
        processing::techformat::format_technical(&corrected_text)
    } else {
        corrected_text
    };

    match processing::process(settings.mode, &prepared, settings.spoken_punctuation) {
        Ok(text) => {
            let text = if settings.mode == processing::ProcessingMode::Rewrite {
                refine_text(state, text, settings, app_name).await
            } else {
                text
            };
            Ok(Finished {
                text,
                corrections: corrected.corrections,
                scratched_all: false,
            })
        }
        Err(error) => Err(FinishFailure {
            message: error.to_string(),
            transcript: prepared,
        }),
    }
}

/// Rewrite the transcript, keeping the deterministic result if that fails.
///
/// Refinement is a nicety layered on words the user has already said. A model
/// that is switched off, still downloading, or simply unhappy must never cost
/// them the transcript — so every failure here logs and returns the input.
async fn refine_text(
    state: &AppState,
    text: String,
    settings: &AppSettings,
    app_name: Option<String>,
) -> String {
    let Some(refiner) = state.refiners.first_enabled(&settings.refine_engines) else {
        tracing::debug!("rewrite requested but no enabled refinement engine can run");
        return text;
    };

    match refiner
        .refine(RefineRequest {
            text: text.clone(),
            style: settings.refine_style,
            model: settings.refine_model.clone(),
            app: app_name,
        })
        .await
    {
        Ok(refined) if accepts_refinement(&text, &refined) => {
            tracing::info!(engine = refiner.id(), "transcript refined");
            refined
        }
        Ok(refined) => {
            tracing::warn!(
                engine = refiner.id(),
                original_words = text.split_whitespace().count(),
                refined_words = refined.split_whitespace().count(),
                "refinement looked lossy or wrapped; keeping the transcript"
            );
            text
        }
        Err(error) => {
            tracing::warn!(engine = refiner.id(), %error, "refinement failed; keeping the transcript");
            text
        }
    }
}
