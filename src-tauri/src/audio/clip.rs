//! Temporary audio on disk.
//!
//! Clide's history stores text, never microphone recordings. A clip exists
//! only long enough to be transcribed — plus a short recovery window so a
//! provider failure can be retried without asking the user to speak again.
//! `RecordedClip` deletes its file on drop, which makes losing track of one
//! difficult by construction.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a clip survives after its transaction fails, so the user has time
/// to press Retry. After this it is deleted and the failure stops offering one.
pub const RECOVERY_WINDOW: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub struct RecordedClip {
    path: PathBuf,
    duration: Duration,
    /// How much of the clip had a voice in it.
    speech: Duration,
    created: Instant,
}

impl RecordedClip {
    pub fn new(path: PathBuf, duration: Duration) -> Self {
        Self {
            path,
            duration,
            speech: duration,
            created: Instant::now(),
        }
    }

    pub fn with_speech(mut self, speech: Duration) -> Self {
        self.speech = speech;
        self
    }

    pub fn speech(&self) -> Duration {
        self.speech
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Whether this clip is still inside its retry window.
    pub fn is_recoverable(&self) -> bool {
        self.created.elapsed() < RECOVERY_WINDOW
    }

    /// Remove the file now rather than waiting for the drop.
    pub fn discard(self) {
        drop(self);
    }
}

impl Drop for RecordedClip {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(?error, "failed to delete temporary audio");
            }
        }
    }
}

/// Delete anything left in the clip directory by a previous run.
///
/// A crash mid-transaction is the one path that can leak audio past the
/// recovery window, so startup sweeps the directory clean.
pub fn sweep_orphans(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "wav") {
            if let Err(error) = std::fs::remove_file(&path) {
                tracing::warn!(?error, ?path, "failed to sweep orphaned audio");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("clide-clip-tests-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dropping_a_clip_deletes_the_recording() {
        let dir = temp_dir("drop");
        let path = dir.join("take.wav");
        std::fs::write(&path, b"RIFF").unwrap();

        let clip = RecordedClip::new(path.clone(), Duration::from_secs(1));
        assert!(path.exists());
        clip.discard();
        assert!(!path.exists(), "temporary audio outlived its transaction");
    }

    #[test]
    fn a_fresh_clip_is_still_retryable() {
        let dir = temp_dir("window");
        let path = dir.join("take.wav");
        std::fs::write(&path, b"RIFF").unwrap();
        let clip = RecordedClip::new(path, Duration::from_secs(1));
        assert!(clip.is_recoverable());
    }

    #[test]
    fn sweeping_removes_audio_left_by_a_previous_run() {
        let dir = temp_dir("sweep");
        std::fs::write(dir.join("a.wav"), b"RIFF").unwrap();
        std::fs::write(dir.join("b.wav"), b"RIFF").unwrap();
        std::fs::write(dir.join("keep.txt"), b"not audio").unwrap();

        sweep_orphans(&dir);

        assert!(!dir.join("a.wav").exists());
        assert!(!dir.join("b.wav").exists());
        assert!(dir.join("keep.txt").exists());
    }
}
