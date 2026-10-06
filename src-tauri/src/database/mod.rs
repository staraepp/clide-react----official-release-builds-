//! Local SQLite storage.

pub mod dictionary;
pub mod kv;
pub mod providers;
pub mod schema;
pub mod transcripts;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

/// Owns the one connection Clide uses.
///
/// A single serialised connection is plenty: writes happen once per dictation
/// and reads are a history list. It keeps transactional behaviour obvious and
/// avoids a pool for a workload that does not have one.
pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let connection = Connection::open(path)?;
        schema::apply(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> rusqlite::Result<Self> {
        let connection = Connection::open_in_memory()?;
        schema::apply(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Borrow the connection. Panics only if a previous holder panicked
    /// mid-query, which would mean the database state is already suspect.
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Unix milliseconds, the one timestamp format used across storage.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_twice_keeps_existing_data() {
        let dir = std::env::temp_dir().join("clide-db-tests");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("clide.sqlite3");

        let db = Database::open(&path).unwrap();
        transcripts::insert(
            &db.lock(),
            transcripts::NewTranscript {
                text: "persisted".into(),
                source: transcripts::TranscriptSource::Dictation,
                source_app: None,
            },
            now_ms(),
        )
        .unwrap();
        drop(db);

        let reopened = Database::open(&path).unwrap();
        assert_eq!(transcripts::count(&reopened.lock()).unwrap(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrations_are_safe_to_run_repeatedly() {
        let db = Database::in_memory().unwrap();
        // Applying again over an existing schema must not error.
        schema::apply(&db.lock()).unwrap();
        schema::apply(&db.lock()).unwrap();
    }
}
