//! Live typing: words reach the focused app while the user is still speaking.
//!
//! Streaming recognisers revise their latest words as more audio arrives, but
//! text that has been typed cannot be taken back. So typing is append-only and
//! only ever covers words the recogniser has stopped changing: everything
//! except the last few words of the current hypothesis, plus the lot once the
//! recogniser settles.
//!
//! Live typing writes what was recognised. It does not run the transcript
//! through Rewrite or spoken corrections, which need the finished recording —
//! that is why it is a setting, and why Rewrite mode turns it off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::insertion::clipboard;
use crate::processing::names::apply_known_names;
use crate::providers::apple::live::{LiveEvent, LiveRecognizer};
use crate::state::AppState;

/// Words at the end of a hypothesis that are still likely to change.
const UNSETTLED_TAIL_WORDS: usize = 2;

/// How long to wait for the recogniser to settle after the user stops.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(4);

/// Decides which words are safe to type next.
#[derive(Debug, Default)]
pub struct WordTyper {
    typed_words: usize,
}

impl WordTyper {
    /// The text to append for this hypothesis, or `None` if nothing new is
    /// stable yet. Includes the separating space when words are already typed.
    pub fn next(&mut self, hypothesis: &str, settled: bool) -> Option<String> {
        let words: Vec<&str> = hypothesis.split_whitespace().collect();
        let tail = if settled { 0 } else { UNSETTLED_TAIL_WORDS };
        let stable = words.len().saturating_sub(tail);

        if stable <= self.typed_words {
            return None;
        }

        let fresh = words[self.typed_words..stable].join(" ");
        let text = if self.typed_words > 0 {
            format!(" {fresh}")
        } else {
            fresh
        };
        self.typed_words = stable;
        Some(text)
    }
}

/// What live typing wrote.
pub struct LiveOutcome {
    pub text: String,
}

struct Shared {
    typed: Mutex<String>,
    closed: AtomicBool,
}

/// A running live-typing session. Lives in [`AppState`] between the start and
/// the end of one dictation.
pub struct LiveRun {
    recognizer: Arc<LiveRecognizer>,
    settled: Receiver<()>,
    shared: Arc<Shared>,
}

pub type LiveSlot = Mutex<Option<LiveRun>>;

/// Start live typing for this dictation. Returns `false` — and leaves the
/// ordinary record-then-transcribe path in charge — if it cannot start.
pub fn begin(app: &AppHandle, language: Option<String>) -> bool {
    let state = app.state::<AppState>();

    let (recognizer, events) = match LiveRecognizer::start(language) {
        Ok(started) => started,
        Err(error) => {
            tracing::warn!(%error, "live typing unavailable; using batch transcription");
            return false;
        }
    };
    let recognizer = Arc::new(recognizer);

    let shared = Arc::new(Shared {
        typed: Mutex::new(String::new()),
        closed: AtomicBool::new(false),
    });
    let (settled_tx, settled_rx) = mpsc::channel();

    let consumer_shared = Arc::clone(&shared);
    let consumer_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("clide-live-typing".into())
        .spawn(move || consume(consumer_app, events, consumer_shared, settled_tx));
    if spawned.is_err() {
        return false;
    }

    let feed = Arc::clone(&recognizer);
    state
        .recorder
        .set_tap(Some(Arc::new(move |samples| feed.push(samples))));

    *state.live.lock().unwrap_or_else(|e| e.into_inner()) = Some(LiveRun {
        recognizer,
        settled: settled_rx,
        shared,
    });
    true
}

/// True while a live session is attached to the current dictation.
pub fn is_active(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .live
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
}

/// Abandon live typing. Text already typed stays where it is.
pub fn cancel(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.recorder.set_tap(None);
    let run = state.live.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(run) = run {
        run.shared.closed.store(true, Ordering::Relaxed);
    }
}

/// The recording has stopped: let the recogniser settle, type the last words,
/// and report everything that was typed. Blocking.
pub fn finish(app: &AppHandle) -> Option<LiveOutcome> {
    let state = app.state::<AppState>();
    state.recorder.set_tap(None);

    let run = state
        .live
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()?;

    run.recognizer.end_audio();
    if run.settled.recv_timeout(SETTLE_TIMEOUT).is_err() {
        tracing::warn!("speech recogniser did not settle in time; keeping what was typed");
    }
    run.shared.closed.store(true, Ordering::Relaxed);

    let text = run
        .shared
        .typed
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Some(LiveOutcome { text })
}

fn consume(app: AppHandle, events: Receiver<LiveEvent>, shared: Arc<Shared>, settled: Sender<()>) {
    let mut typer = WordTyper::default();

    while let Ok(event) = events.recv() {
        if shared.closed.load(Ordering::Relaxed) {
            break;
        }

        let (hypothesis, is_final) = match event {
            LiveEvent::Partial(text) => (text, false),
            LiveEvent::Final(text) => (text, true),
            LiveEvent::Failed(message) => {
                tracing::debug!(%message, "live recognition ended without a result");
                (String::new(), true)
            }
        };

        // The full hypothesis, for the local API's `partial` events: what has
        // been recognised so far, before the typer holds back unsettled words.
        if !hypothesis.is_empty() {
            app.state::<AppState>().events.publish_partial(&hypothesis);
        }

        if let Some(delta) = typer.next(&hypothesis, is_final) {
            let delta = apply_known_names(&delta);
            match clipboard::type_text_while_held(&delta) {
                Ok(()) => shared
                    .typed
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push_str(&delta),
                Err(reason) => tracing::warn!(reason, "live typing could not type"),
            }
        }

        if is_final {
            let _ = settled.send(());
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_typed_until_words_have_settled() {
        let mut typer = WordTyper::default();
        assert_eq!(typer.next("Hello", false), None);
        assert_eq!(typer.next("Hello there", false), None);
    }

    #[test]
    fn settled_words_are_typed_once_and_appended() {
        let mut typer = WordTyper::default();
        assert_eq!(typer.next("Hello there my", false), Some("Hello".into()));
        assert_eq!(
            typer.next("Hello there my friend how", false),
            Some(" there my".into())
        );
        // Same hypothesis again: nothing new.
        assert_eq!(typer.next("Hello there my friend how", false), None);
    }

    #[test]
    fn the_final_result_types_the_unsettled_tail() {
        let mut typer = WordTyper::default();
        assert_eq!(typer.next("Hello there my friend", false), Some("Hello there".into()));
        assert_eq!(
            typer.next("Hello there my friend.", true),
            Some(" my friend.".into())
        );
    }

    #[test]
    fn a_hypothesis_that_shrinks_never_types_anything() {
        let mut typer = WordTyper::default();
        typer.next("one two three four five", false);
        assert_eq!(typer.next("one two", false), None);
        assert_eq!(typer.next("one two", true), None);
    }

    #[test]
    fn revised_words_already_typed_are_left_alone() {
        let mut typer = WordTyper::default();
        assert_eq!(typer.next("write a prompt now", false), Some("write a".into()));
        // The recogniser reconsiders an earlier word; only new words are typed.
        assert_eq!(typer.next("right a prompt now please", true), Some(" prompt now please".into()));
    }
}
