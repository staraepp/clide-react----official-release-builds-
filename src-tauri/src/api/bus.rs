//! The stream of what Clide is doing, for the local API's `/v1/events`.
//!
//! Dictation code publishes here as it already does to the window; nothing is
//! kept or queued unless a client is actually connected. Events never carry the
//! frontmost app or window title, and the sequence for one dictation is
//! `state`, `level`…, `state`, `transcript`, `state`.

use std::sync::Mutex;

use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::database::now_ms;
use crate::dictation::machine::DictationState;

/// Plenty for a few seconds of 30 Hz levels; a client that falls further
/// behind skips ahead rather than being disconnected.
const CAPACITY: usize = 256;

#[derive(Clone, Debug)]
pub struct ApiEvent {
    pub name: &'static str,
    pub data: Value,
}

pub struct EventBus {
    sender: broadcast::Sender<ApiEvent>,
    state: Mutex<&'static str>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            sender: broadcast::channel(CAPACITY).0,
            state: Mutex::new("idle"),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ApiEvent> {
        self.sender.subscribe()
    }

    /// The state as the API names it. Tracked even with nobody listening, so
    /// `/v1/health` and a newly connected stream can report it.
    pub fn current_state(&self) -> &'static str {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send(&self, event: ApiEvent) {
        // An error only means nobody is listening.
        let _ = self.sender.send(event);
    }

    /// A dictation state changed. `mode` and `engine` describe the transcript
    /// if this state carries one.
    pub fn publish_state(&self, state: &DictationState, mode: &str, engine: &str) {
        let mut current = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let events = map_state(state, mode, engine, &mut current);
        drop(current);

        if self.sender.receiver_count() > 0 {
            for event in events {
                self.send(event);
            }
        }
    }

    pub fn publish_level(&self, level: f32) {
        if self.sender.receiver_count() > 0 {
            self.send(ApiEvent {
                name: "level",
                data: json!({ "level": level.clamp(0.0, 1.0), "ts": now_ms() }),
            });
        }
    }

    /// The recogniser's current guess for everything said so far.
    pub fn publish_partial(&self, text: &str) {
        if self.sender.receiver_count() > 0 {
            self.send(ApiEvent {
                name: "partial",
                data: json!({ "text": text }),
            });
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// The API's name for a dictation state.
pub fn api_state(state: &DictationState) -> &'static str {
    match state {
        DictationState::Idle => "idle",
        DictationState::Capturing => "recording",
        DictationState::FinalizingAudio
        | DictationState::Transcribing { .. }
        | DictationState::Processing => "transcribing",
        DictationState::Inserting => "inserting",
        // The text has been delivered; from the outside Clide is at rest.
        DictationState::Complete { .. } => "idle",
        DictationState::CaptureFailed { .. }
        | DictationState::TranscriptionFailed { .. }
        | DictationState::ProcessingFailed { .. }
        | DictationState::InsertionFailed { .. } => "error",
    }
}

/// Events for one state change, updating `last` to the state now in effect.
///
/// A state equal to `last` produces no `state` event: finishing a dictation
/// goes Complete -> (linger) -> Idle, and both mean "idle".
pub fn map_state(
    state: &DictationState,
    mode: &str,
    engine: &str,
    last: &mut &'static str,
) -> Vec<ApiEvent> {
    let ts = now_ms();
    let mut events = Vec::new();

    // A transcript goes out before the state that follows it, so a listener
    // that stops on `idle` has already seen the words.
    let final_text = match state {
        DictationState::Complete { transcript, .. }
        | DictationState::InsertionFailed { transcript, .. } => Some(transcript),
        _ => None,
    };
    if let Some(text) = final_text {
        events.push(ApiEvent {
            name: "transcript",
            data: json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "text": text,
                "mode": mode,
                "engine": engine,
            }),
        });
    }

    let next = api_state(state);
    if next != *last {
        *last = next;
        events.push(ApiEvent {
            name: "state",
            data: json!({ "state": next, "ts": ts }),
        });
    }

    let failure = match state {
        DictationState::CaptureFailed { message } => Some(("capture_failed", message)),
        DictationState::TranscriptionFailed { message, .. } => Some(("transcription_failed", message)),
        DictationState::ProcessingFailed { message, .. } => Some(("processing_failed", message)),
        DictationState::InsertionFailed { message, .. } => Some(("insertion_failed", message)),
        _ => None,
    };
    if let Some((code, message)) = failure {
        events.push(ApiEvent {
            name: "error",
            data: json!({ "code": code, "message": message }),
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictation::machine::InsertionMethod;

    fn names(events: &[ApiEvent]) -> Vec<&'static str> {
        events.iter().map(|event| event.name).collect()
    }

    #[test]
    fn one_dictation_reads_state_then_transcript_then_idle() {
        let mut last = "idle";
        let mut seen = Vec::new();

        for state in [
            DictationState::Capturing,
            DictationState::FinalizingAudio,
            DictationState::Transcribing { attempt: 1 },
            DictationState::Processing,
            DictationState::Inserting,
            DictationState::Complete {
                transcript: "hello world".into(),
                method: InsertionMethod::Accessibility,
            },
            // The HUD's linger ends: Idle again, which must not repeat.
            DictationState::Idle,
        ] {
            seen.extend(map_state(&state, "polished", "engine-x", &mut last));
        }

        let states: Vec<_> = seen
            .iter()
            .filter(|event| event.name == "state")
            .map(|event| event.data["state"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(states, ["recording", "transcribing", "inserting", "idle"]);

        let order = names(&seen);
        let transcript = order.iter().position(|name| *name == "transcript").unwrap();
        let last_state = order.iter().rposition(|name| *name == "state").unwrap();
        assert!(transcript < last_state, "the words must precede the final idle");

        let event = &seen[transcript];
        assert_eq!(event.data["text"], "hello world");
        assert_eq!(event.data["mode"], "polished");
        assert_eq!(event.data["engine"], "engine-x");
        assert!(event.data["id"].as_str().unwrap().len() >= 32);
    }

    #[test]
    fn failures_become_an_error_state_and_an_error_event() {
        let mut last = "idle";
        let events = map_state(
            &DictationState::TranscriptionFailed {
                message: "the model is missing".into(),
                retryable: true,
            },
            "polished",
            "engine-x",
            &mut last,
        );

        assert_eq!(names(&events), ["state", "error"]);
        assert_eq!(events[0].data["state"], "error");
        assert_eq!(events[1].data["code"], "transcription_failed");
        assert_eq!(events[1].data["message"], "the model is missing");
    }

    #[test]
    fn a_transcript_that_could_not_be_inserted_is_still_delivered() {
        let mut last = "inserting";
        let events = map_state(
            &DictationState::InsertionFailed {
                message: "no Accessibility".into(),
                transcript: "kept words".into(),
                on_clipboard: true,
            },
            "verbatim",
            "e",
            &mut last,
        );
        assert_eq!(names(&events), ["transcript", "state", "error"]);
    }

    #[test]
    fn no_event_carries_an_app_name_or_window_title() {
        let mut last = "idle";
        let events = map_state(
            &DictationState::Complete {
                transcript: "x".into(),
                method: InsertionMethod::Typed,
            },
            "polished",
            "e",
            &mut last,
        );
        for event in events {
            let text = event.data.to_string().to_lowercase();
            assert!(!text.contains("app") && !text.contains("window"));
        }
    }

    #[test]
    fn publishing_with_no_listener_still_tracks_the_state() {
        let bus = EventBus::new();
        bus.publish_state(&DictationState::Capturing, "polished", "e");
        assert_eq!(bus.current_state(), "recording");
        bus.publish_level(0.5);
        bus.publish_partial("hi");
    }

    #[tokio::test]
    async fn a_subscriber_receives_levels_and_partials() {
        let bus = EventBus::new();
        let mut receiver = bus.subscribe();
        bus.publish_level(2.0);
        bus.publish_partial("hello");

        let level = receiver.recv().await.unwrap();
        assert_eq!(level.name, "level");
        assert_eq!(level.data["level"], 1.0);
        assert_eq!(receiver.recv().await.unwrap().data["text"], "hello");
    }
}
