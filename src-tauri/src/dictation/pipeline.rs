//! The dictation transaction, end to end.
//!
//! ```text
//! shortcut -> capture -> finalize -> transcribe -> process -> insert -> history
//! ```
//!
//! Every step moves the state machine first and emits second, so the UI can
//! never observe a stage that the machine did not agree to. Nothing here
//! retries by itself and nothing silently changes provider: a failure stops,
//! keeps whatever it has, and waits for the user.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::audio::AudioError;
use crate::database::{now_ms, transcripts};
use crate::dictation::events;
use crate::dictation::machine::{DictationInput, DictationState, FailureStage};
use crate::hud;
use crate::insertion;
use crate::processing;
use crate::providers::{AudioClip, TranscriptionRequest};
use crate::refine::{accepts_refinement, RefineRequest, RefineStyle};
use crate::state::AppState;

/// How often the HUD waveform is refreshed. 30 Hz is smooth to the eye and
/// two orders of magnitude cheaper than emitting per audio callback.
const LEVEL_INTERVAL: Duration = Duration::from_millis(33);

/// How long the Done state stays on screen before the HUD disappears.
const DONE_LINGER: Duration = Duration::from_millis(900);

/// Begin capturing. Safe to call from the shortcut, the tray, or the UI; a
/// call that arrives mid-transaction is ignored rather than treated as an error.
pub async fn start(app: &AppHandle) {
    let state = app.state::<AppState>();

    // Applying first makes the "am I allowed to start" check atomic.
    let Ok(next) = state.session.apply(DictationInput::StartCapture) else {
        tracing::debug!(current = state.session.state().name(), "start ignored");
        return;
    };

    // Captured now, not at insertion time: history should record the app the
    // user was speaking to, even if they switch away while it transcribes.
    let target = insertion::focus::frontmost();
    state.session.begin(target.clone());

    // Lower other apps' audio so the user's voice is the loudest thing in the
    // room. Restored on every path out of capture — see `unduck_audio`.
    state.session.duck_audio();

    if let Err(error) = state.recorder.start() {
        tracing::error!(?error, "microphone capture failed to start");
        fail(app, FailureStage::Capture, capture_message(&error));
        return;
    }

    events::emit_state(app, &next);
    events::emit(
        app,
        events::DICTATION_STARTED,
        events::StartedPayload {
            target_app: target.app_name.clone(),
        },
    );

    hud::show(app);
    spawn_level_ticker(app.clone());
}

/// Stop capturing and run the rest of the pipeline.
pub async fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();

    let Ok(next) = state.session.apply(DictationInput::StopCapture) else {
        tracing::debug!(current = state.session.state().name(), "stop ignored");
        return;
    };
    events::emit_state(app, &next);
    events::emit_bare(app, events::DICTATION_STOPPED);

    // The microphone is closed, so there is nothing left to hear over.
    // Transcription does not need the room quiet.
    state.session.unduck_audio();

    let recorder = app.clone();
    let recorded =
        tauri::async_runtime::spawn_blocking(move || recorder.state::<AppState>().recorder.stop())
            .await;

    let clip = match recorded {
        Ok(Ok(clip)) => clip,
        Ok(Err(error)) => {
            tracing::warn!(?error, "no usable audio");
            state.session.release_audio();
            fail(app, FailureStage::Capture, capture_message(&error));
            return;
        }
        Err(error) => {
            tracing::error!(?error, "audio worker panicked");
            state.session.release_audio();
            fail(
                app,
                FailureStage::Capture,
                "The recording could not be saved.",
            );
            return;
        }
    };

    // The target captured at start is kept: the user may already be looking
    // somewhere else, but they were speaking to that app.
    state.session.attach(clip);

    let Ok(next) = state.session.apply(DictationInput::AudioFinalized) else {
        return;
    };
    events::emit_state(app, &next);

    transcribe_and_deliver(app).await;
}

/// Re-run the still-pending audio through the provider after a failure.
pub async fn retry(app: &AppHandle) {
    let state = app.state::<AppState>();

    if !state.session.can_retry() {
        tracing::debug!("retry requested with no recoverable audio");
        return;
    }

    let Ok(next) = state.session.apply(DictationInput::Retry) else {
        return;
    };
    events::emit_state(app, &next);

    transcribe_and_deliver(app).await;
}

/// Abandon the transaction. Audio is deleted immediately.
pub async fn cancel(app: &AppHandle) {
    let state = app.state::<AppState>();

    if state.session.state().is_capturing() {
        state.recorder.abort();
    }
    state.session.unduck_audio();

    let applied = state.session.apply(DictationInput::Cancel);
    state.session.release_audio();
    hud::hide(app);

    if let Ok(next) = applied {
        events::emit_state(app, &next);
    }
}

/// Acknowledge a settled state and return to Idle.
pub fn dismiss(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.session.release_audio();
    hud::hide(app);
    if let Ok(next) = state.session.apply(DictationInput::Dismiss) {
        events::emit_state(app, &next);
    }
}

// --- pipeline stages -------------------------------------------------------

async fn transcribe_and_deliver(app: &AppHandle) {
    let Some(raw) = transcribe(app).await else {
        return;
    };
    let Some(processed) = process(app, raw).await else {
        return;
    };
    persist(app, &processed);
    deliver(app, processed).await;
}

/// Try each usable substitute in turn, announcing the one that works.
///
/// Returns `None` when the policy allows nothing, or nothing succeeds — in
/// which case the caller reports the *original* failure, because that is the
/// one the user needs to act on.
async fn try_fallback(
    app: &AppHandle,
    state: &tauri::State<'_, AppState>,
    policy: crate::dictation::fallback::FallbackPolicy,
    failed_provider: &str,
    request: &TranscriptionRequest,
) -> Option<crate::providers::Transcription> {
    let candidates =
        crate::dictation::fallback::candidates(&state.providers, policy, failed_provider);

    for candidate in candidates {
        let mut attempt = request.clone();
        attempt.model = candidate.model.clone();

        match candidate.provider.transcribe(attempt).await
        {
            Ok(result) => {
                tracing::info!(
                    failed = %failed_provider,
                    rescued_by = %result.provider,
                    model = %result.model,
                    "fell back to another engine"
                );
                // Never silent: the HUD names what actually ran.
                events::emit(
                    app,
                    events::TRANSCRIPTION_FELL_BACK,
                    events::FallbackPayload {
                        failed_provider: failed_provider.to_string(),
                        used_provider: candidate.provider.name().to_string(),
                        used_model: result.model.clone(),
                    },
                );
                return Some(result);
            }
            Err(error) => {
                tracing::debug!(
                    candidate = %candidate.provider.id(),
                    %error,
                    "fallback candidate also failed"
                );
            }
        }
    }

    None
}

async fn transcribe(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    events::emit_bare(app, events::TRANSCRIPTION_STARTED);

    let Some(pending) = state.session.pending_snapshot() else {
        fail(
            app,
            FailureStage::Transcription,
            "The recording is no longer available.",
        );
        return None;
    };

    let (provider_id, model_id, language, policy, vocabulary) = {
        let settings = state.settings();
        (
            settings.provider_id,
            settings.model_id,
            settings.language,
            settings.fallback,
            settings.technical_vocabulary,
        )
    };

    let Some(provider) = state.providers.get(&provider_id) else {
        fail(
            app,
            FailureStage::Transcription,
            format!("The provider \"{provider_id}\" is not available in this build."),
        );
        return None;
    };

    let target = state.session.target();
    let prompt = crate::context::vocabulary_prompt(
        vocabulary,
        &target,
        provider.capabilities().prompting,
    );
    if prompt.is_some() {
        tracing::info!(app = target.label(), "technical vocabulary applied");
    }

    let request = TranscriptionRequest {
        audio: AudioClip::wav(pending.path, pending.duration_secs),
        model: model_id,
        language,
        prompt: prompt.map(str::to_string),
    };

    let first_attempt = provider.transcribe(request.clone()).await;

    // Only reach for a substitute once the chosen engine has actually failed,
    // and never silently: whatever runs is named in the HUD. See
    // `dictation::fallback`.
    let outcome = match first_attempt {
        Ok(result) => Ok(result),
        Err(original) => match try_fallback(app, &state, policy, &provider_id, &request).await {
            Some(result) => Ok(result),
            None => Err(original),
        },
    };

    match outcome {
        Ok(result) => {
            tracing::info!(
                provider = %result.provider,
                model = %result.model,
                latency_ms = result.latency_ms,
                characters = result.text.len(),
                "transcription complete"
            );
            events::emit(
                app,
                events::TRANSCRIPTION_COMPLETE,
                events::TextPayload {
                    text: result.text.clone(),
                },
            );
            Some(result.text)
        }
        Err(error) => {
            tracing::warn!(%error, "transcription failed");
            // Retry is offered while the audio is still on disk — not based on
            // the error being transient. A wrong API key is worth retrying
            // once the user has fixed it.
            let retryable = state.session.can_retry();
            let message = error.to_string();

            let next = state
                .session
                .apply(DictationInput::transcription_failure(&message, retryable));
            if let Ok(next) = next {
                events::emit_state(app, &next);
            }
            events::emit(
                app,
                events::TRANSCRIPTION_FAILED,
                events::FailurePayload { message, retryable },
            );
            hud::show(app);
            None
        }
    }
}

async fn process(app: &AppHandle, raw: String) -> Option<String> {
    let state = app.state::<AppState>();

    let Ok(next) = state.session.apply(DictationInput::TranscriptReceived) else {
        return None;
    };
    events::emit_state(app, &next);
    events::emit_bare(app, events::PROCESSING_STARTED);

    let (mode, style, engines, spoken, format_technical) = {
        let settings = state.settings();
        (
            settings.mode,
            settings.refine_style,
            settings.refine_engines,
            settings.spoken_punctuation,
            settings.format_technical_terms,
        )
    };

    // Spoken corrections come first: a command phrase is never meant to be
    // typed, and everything after this point should see the text the user
    // actually meant.
    let corrected = processing::backtrack::apply_backtracking(&raw);
    if corrected.corrections > 0 {
        tracing::info!(count = corrected.corrections, "spoken corrections applied");
        events::emit(
            app,
            events::CORRECTION_APPLIED,
            events::CorrectionPayload {
                count: corrected.corrections,
            },
        );
    }

    // The user scratched everything they said. There is nothing to insert, and
    // `processing::process` would helpfully resurrect the raw text, so stop
    // here and treat it as a cancelled dictation.
    if corrected.text.trim().is_empty() && corrected.corrections > 0 {
        if let Ok(next) = state.session.apply(DictationInput::Cancel) {
            events::emit_state(app, &next);
        }
        state.session.release_audio();
        hud::hide(app);
        return None;
    }

    // Opt-in: spoken paths become code spans before Polish, which knows to
    // leave them alone, and before Rewrite, which is told to copy them.
    let raw = if format_technical {
        processing::techformat::format_technical(&corrected.text)
    } else {
        corrected.text
    };

    match processing::process(mode, &raw, spoken) {
        Ok(text) => {
            let text = if mode == processing::ProcessingMode::Rewrite {
                refine_text(app, text, style, &engines).await
            } else {
                text
            };

            events::emit(
                app,
                events::PROCESSING_COMPLETE,
                events::TextPayload { text: text.clone() },
            );
            Some(text)
        }
        Err(error) => {
            // The transcript survives: the failure state carries it so the UI
            // can still offer Copy.
            let next = state.session.apply(DictationInput::Failed {
                stage: FailureStage::Processing,
                message: error.to_string(),
                retryable: false,
                transcript: Some(raw),
                on_clipboard: false,
            });
            if let Ok(next) = next {
                events::emit_state(app, &next);
            }
            hud::show(app);
            None
        }
    }
}

/// Rewrite the transcript, keeping the deterministic result if that fails.
///
/// Refinement is a nicety layered on words the user has already said. A model
/// that is switched off, still downloading, or simply unhappy must never cost
/// them the transcript — so every failure here logs and returns the input.
async fn refine_text(
    app: &AppHandle,
    text: String,
    style: RefineStyle,
    engines: &[String],
) -> String {
    let state = app.state::<AppState>();

    let Some(refiner) = state.refiners.first_enabled(engines) else {
        tracing::debug!("rewrite requested but no enabled refinement engine can run");
        return text;
    };

    match refiner
        .refine(RefineRequest {
            text: text.clone(),
            style,
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

/// Save before inserting.
///
/// Insertion is the step most likely to fail (a read-only control, a hostile
/// app, revoked Accessibility access). Writing history first means a transcript
/// that reached this point cannot be lost by anything that happens next.
fn persist(app: &AppHandle, text: &str) {
    let state = app.state::<AppState>();
    let source_app = state.session.target().app_name;

    let record = transcripts::NewTranscript {
        text: text.to_string(),
        source: transcripts::TranscriptSource::Dictation,
        source_app,
    };

    let saved = transcripts::insert(&state.db.lock(), record, now_ms());
    match saved {
        Ok(saved) => {
            tracing::debug!(id = %saved.id, "transcript saved");
            events::emit_bare(app, events::HISTORY_CHANGED);
        }
        Err(error) => {
            // History is important but not worth cancelling an insertion over;
            // the user still gets their text.
            tracing::error!(?error, "could not save transcript to history");
        }
    }
}

async fn deliver(app: &AppHandle, text: String) {
    let state = app.state::<AppState>();

    let Ok(next) = state.session.apply(DictationInput::Processed) else {
        return;
    };
    events::emit_state(app, &next);
    events::emit_bare(app, events::INSERTION_STARTED);

    let payload = text.clone();
    let target = state.session.target();
    let outcome =
        tauri::async_runtime::spawn_blocking(move || insertion::insert(&payload, &target)).await;

    // The transcript is delivered (or on the clipboard); the audio has done
    // its job either way.
    state.session.release_audio();

    match outcome {
        Ok(Ok(method)) => {
            let next = state.session.apply(DictationInput::Inserted {
                transcript: text,
                method,
            });
            if let Ok(next) = next {
                events::emit_state(app, &next);
            }
            events::emit_bare(app, events::INSERTION_COMPLETE);
            schedule_hud_dismiss(app.clone());
        }
        Ok(Err(failure)) => {
            tracing::warn!(message = %failure.message, "insertion failed");
            let next = state.session.apply(DictationInput::insertion_failure(
                &failure.message,
                &text,
                failure.on_clipboard,
            ));
            if let Ok(next) = next {
                events::emit_state(app, &next);
            }
            events::emit(
                app,
                events::INSERTION_FAILED,
                events::FailurePayload {
                    message: failure.message,
                    retryable: false,
                },
            );
            hud::show(app);
        }
        Err(error) => {
            tracing::error!(?error, "insertion task panicked");
            let next = state.session.apply(DictationInput::insertion_failure(
                "Clide could not reach the focused application.",
                &text,
                false,
            ));
            if let Ok(next) = next {
                events::emit_state(app, &next);
            }
            hud::show(app);
        }
    }
}

// --- helpers ---------------------------------------------------------------

fn fail(app: &AppHandle, stage: FailureStage, message: impl Into<String>) {
    // Whatever went wrong, the user's volume is not part of it.
    app.state::<AppState>().session.unduck_audio();

    let state = app.state::<AppState>();
    if let Ok(next) = state.session.apply(DictationInput::failure(stage, message)) {
        events::emit_state(app, &next);
    }
    hud::show(app);
}

fn capture_message(error: &AudioError) -> String {
    match error {
        AudioError::NoInputDevice => "No microphone was found. Connect one and try again.".into(),
        AudioError::PermissionDenied => {
            "clide needs microphone access. Enable it in System Settings.".into()
        }
        AudioError::Empty => {
            "Nothing was recorded — check that the right microphone is selected.".into()
        }
        other => other.to_string(),
    }
}

/// Push microphone level to the HUD while capturing.
fn spawn_level_ticker(app: AppHandle) {
    let epoch = app.state::<AppState>().session.epoch();

    tauri::async_runtime::spawn(async move {
        loop {
            {
                let state = app.state::<AppState>();
                // Stop as soon as this transaction is over, or a newer one
                // has taken its place.
                if state.session.epoch() != epoch || !state.session.state().is_capturing() {
                    break;
                }
                events::emit(
                    &app,
                    events::LEVEL,
                    events::LevelPayload {
                        level: state.recorder.level(),
                    },
                );
            }
            tokio::time::sleep(LEVEL_INTERVAL).await;
        }

        events::emit(&app, events::LEVEL, events::LevelPayload { level: 0.0 });
    });
}

/// Let the Done state breathe, then get out of the way.
fn schedule_hud_dismiss(app: AppHandle) {
    let epoch = app.state::<AppState>().session.epoch();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DONE_LINGER).await;

        let state = app.state::<AppState>();
        // A new dictation may have started while Done was showing.
        if state.session.epoch() != epoch {
            return;
        }
        if matches!(state.session.state(), DictationState::Complete { .. }) {
            dismiss(&app);
        }
    });
}
