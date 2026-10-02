//! The live dictation transaction: current state, and the audio it owns.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::audio::RecordedClip;
use crate::insertion::FocusTarget;

use super::machine::{transition, DictationInput, DictationState, IllegalTransition};

/// The audio side of a transaction.
///
/// The target is known as soon as recording starts; the clip only exists once
/// recording stops. Holding the `RecordedClip` here is what keeps the
/// recording alive across a provider failure — dropping it deletes the file.
struct Pending {
    clip: Option<RecordedClip>,
    /// Captured when recording started, so history records where the user was
    /// speaking even if they switch apps before the transcript arrives.
    target: FocusTarget,
}

/// Metadata copied out of a pending clip so async work can proceed without
/// taking ownership of (and therefore risking dropping) the recording.
#[derive(Clone, Debug)]
pub struct PendingSnapshot {
    pub path: PathBuf,
    pub duration_secs: f32,
    /// Time with a voice in it.
    pub speech: std::time::Duration,
    pub target: FocusTarget,
}

pub struct DictationSession {
    state: Mutex<DictationState>,
    pending: Mutex<Option<Pending>>,
    /// Held while the microphone is open, so other apps' audio is quieter than
    /// the user's voice. Dropping it restores their volume — which is why it
    /// lives here rather than in a local variable that an early return could
    /// skip past.
    ducked: Mutex<Option<crate::audio::ducking::Ducked>>,
    /// Incremented on every new transaction. Deferred work (auto-hiding the
    /// HUD, the level ticker) carries the epoch it was started for and stops
    /// as soon as a newer transaction begins.
    epoch: AtomicU64,
}

impl Default for DictationSession {
    fn default() -> Self {
        Self::new()
    }
}

impl DictationSession {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DictationState::Idle),
            pending: Mutex::new(None),
            ducked: Mutex::new(None),
            epoch: AtomicU64::new(0),
        }
    }

    pub fn state(&self) -> DictationState {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// Apply an input through the state machine.
    ///
    /// Every state change in Clide goes through here, so an illegal move is
    /// impossible to make by accident from the pipeline.
    pub fn apply(&self, input: DictationInput) -> Result<DictationState, IllegalTransition> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let next = transition(&guard, input)?;

        // A fresh capture invalidates any work still running for the previous
        // transaction.
        if next.is_capturing() && !guard.is_capturing() {
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }

        *guard = next.clone();
        Ok(next)
    }

    /// Record which application the user is dictating into. Called when
    /// capture starts, before any audio exists.
    pub fn begin(&self, target: FocusTarget) {
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending {
            clip: None,
            target,
        });
    }

    /// The application this transaction is speaking to.
    pub fn target(&self) -> FocusTarget {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|pending| pending.target.clone())
            .unwrap_or_default()
    }

    /// Attach freshly recorded audio to the transaction.
    pub fn attach(&self, clip: RecordedClip) {
        let mut guard = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(pending) => pending.clip = Some(clip),
            None => {
                *guard = Some(Pending {
                    clip: Some(clip),
                    target: FocusTarget::default(),
                })
            }
        }
    }

    /// Read the pending clip's metadata without giving up ownership of it, so
    /// a failed transcription still has audio to retry.
    pub fn pending_snapshot(&self) -> Option<PendingSnapshot> {
        let guard = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let pending = guard.as_ref()?;
        let clip = pending.clip.as_ref()?;
        Some(PendingSnapshot {
            path: clip.path().to_path_buf(),
            duration_secs: clip.duration().as_secs_f32(),
            speech: clip.speech(),
            target: pending.target.clone(),
        })
    }

    /// Whether audio is still on disk and inside its recovery window.
    pub fn can_retry(&self) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|pending| pending.clip.as_ref())
            .is_some_and(|clip| clip.is_recoverable())
    }

    /// Delete the pending audio. Called once the transaction is resolved:
    /// on success, on cancellation, or when the recovery window expires.
    pub fn release_audio(&self) {
        let taken = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        // Dropping the clip removes the file.
        drop(taken);
    }

    /// Drop audio whose recovery window has passed. Run periodically.
    pub fn expire_stale_audio(&self) -> bool {
        let mut guard = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let stale = guard
            .as_ref()
            .and_then(|pending| pending.clip.as_ref())
            .is_some_and(|clip| !clip.is_recoverable());
        if stale {
            guard.take();
        }
        stale
    }
}

impl DictationSession {
    /// Lower other apps' audio for the length of the recording.
    ///
    /// Safe to call twice; the second call replaces the first, restoring the
    /// original level before ducking again.
    pub fn duck_audio(&self) {
        *self.ducked.lock().unwrap_or_else(|e| e.into_inner()) =
            crate::audio::ducking::Ducked::engage();
    }

    /// Put the user's volume back.
    ///
    /// Called from every path that ends a recording — stop, cancel, and every
    /// capture failure — because a transaction that ends any other way must
    /// still not leave their Mac quiet.
    pub fn unduck_audio(&self) {
        // Dropping the guard is what restores it.
        self.ducked.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictation::machine::InsertionMethod;
    use std::time::Duration;

    fn clip(name: &str) -> RecordedClip {
        let dir = std::env::temp_dir().join("clide-session-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.wav"));
        std::fs::write(&path, b"RIFF").unwrap();
        RecordedClip::new(path, Duration::from_secs(2))
    }

    #[test]
    fn a_new_session_starts_idle_with_no_audio() {
        let session = DictationSession::new();
        assert_eq!(session.state(), DictationState::Idle);
        assert!(session.pending_snapshot().is_none());
        assert!(!session.can_retry());
    }

    #[test]
    fn illegal_inputs_leave_the_state_untouched() {
        let session = DictationSession::new();
        assert!(session.apply(DictationInput::StopCapture).is_err());
        assert_eq!(session.state(), DictationState::Idle);
    }

    #[test]
    fn each_new_capture_bumps_the_epoch() {
        let session = DictationSession::new();
        let first = session.epoch();

        session.apply(DictationInput::StartCapture).unwrap();
        let second = session.epoch();
        assert!(second > first);

        // Moving through the same transaction must not bump it again.
        session.apply(DictationInput::StopCapture).unwrap();
        assert_eq!(session.epoch(), second);

        session.apply(DictationInput::Cancel).unwrap();
        session.apply(DictationInput::StartCapture).unwrap();
        assert!(session.epoch() > second);
    }

    #[test]
    fn reading_pending_audio_does_not_consume_it() {
        let session = DictationSession::new();
        let path = {
            let clip = clip("retryable");
            let path = clip.path().to_path_buf();
            session.begin(FocusTarget::default());
            session.attach(clip);
            path
        };

        // Two reads in a row, as a failure followed by a retry would do.
        assert!(session.pending_snapshot().is_some());
        assert!(session.pending_snapshot().is_some());
        assert!(path.exists(), "the recording was deleted before the retry");
        assert!(session.can_retry());

        session.release_audio();
        assert!(!path.exists(), "resolved audio was not deleted");
        assert!(!session.can_retry());
    }

    #[test]
    fn a_successful_transaction_ends_with_no_audio_on_disk() {
        let session = DictationSession::new();
        let clip = clip("success");
        let path = clip.path().to_path_buf();
        session.begin(FocusTarget::default());
        session.attach(clip);

        session.apply(DictationInput::StartCapture).unwrap();
        session.apply(DictationInput::StopCapture).unwrap();
        session.apply(DictationInput::AudioFinalized).unwrap();
        session.apply(DictationInput::TranscriptReceived).unwrap();
        session.apply(DictationInput::Processed).unwrap();
        session
            .apply(DictationInput::Inserted {
                transcript: "done".into(),
                method: InsertionMethod::Accessibility,
            })
            .unwrap();
        session.release_audio();

        assert!(!path.exists());
    }
}
