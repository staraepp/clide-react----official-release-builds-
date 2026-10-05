//! Schema and migrations.
//!
//! History stays deliberately small: a transcript is text plus enough context
//! to find it again. Nothing about providers, latency, or confidence is
//! recorded per row — that is diagnostics, not history.

use rusqlite::Connection;

/// Bumped whenever `apply` gains a step. `user_version` tracks what a database
/// on disk has already had applied.
const TARGET_VERSION: i64 = 2;

pub fn apply(connection: &Connection) -> rusqlite::Result<()> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 1 {
        connection.execute_batch(V1)?;
    }

    if version < 2 {
        connection.execute_batch(V2)?;
    }

    connection.pragma_update(None, "user_version", TARGET_VERSION)?;
    Ok(())
}

const V1: &str = r#"
BEGIN;

CREATE TABLE IF NOT EXISTS transcripts (
    id          TEXT    PRIMARY KEY NOT NULL,
    text        TEXT    NOT NULL,
    -- Unix milliseconds, UTC.
    created_at  INTEGER NOT NULL,
    -- 'dictation' or 'import'. Imports arrive in a later version but share
    -- this table: there is one transcript system, not two.
    source      TEXT    NOT NULL,
    -- Display name of the app that received the text, e.g. 'TextEdit'.
    source_app  TEXT
);

CREATE INDEX IF NOT EXISTS transcripts_created_at_idx
    ON transcripts (created_at DESC);
CREATE INDEX IF NOT EXISTS transcripts_source_idx
    ON transcripts (source, created_at DESC);
CREATE INDEX IF NOT EXISTS transcripts_source_app_idx
    ON transcripts (source_app);

-- External-content FTS: the index stores no second copy of the text.
CREATE VIRTUAL TABLE IF NOT EXISTS transcripts_fts USING fts5 (
    text,
    content = 'transcripts',
    content_rowid = 'rowid',
    tokenize = "unicode61 remove_diacritics 2"
);

CREATE TRIGGER IF NOT EXISTS transcripts_fts_insert
AFTER INSERT ON transcripts BEGIN
    INSERT INTO transcripts_fts (rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_delete
AFTER DELETE ON transcripts BEGIN
    INSERT INTO transcripts_fts (transcripts_fts, rowid, text)
    VALUES ('delete', old.rowid, old.text);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_update
AFTER UPDATE ON transcripts BEGIN
    INSERT INTO transcripts_fts (transcripts_fts, rowid, text)
    VALUES ('delete', old.rowid, old.text);
    INSERT INTO transcripts_fts (rowid, text) VALUES (new.rowid, new.text);
END;

-- Preferences, JSON-encoded. Nothing secret, except the one recorded exception:
-- the local API's bearer token (see AGENTS.md).
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- Which model a provider is set to, and whether a key has been stored for it.
-- The key itself is never here.
CREATE TABLE IF NOT EXISTS provider_configs (
    provider_id           TEXT PRIMARY KEY NOT NULL,
    model_id              TEXT,
    credential_configured INTEGER NOT NULL DEFAULT 0,
    updated_at            INTEGER NOT NULL
);

COMMIT;
"#;

// Remove the credential-state mirror. The credential store is authoritative;
// keeping a second boolean in SQLite allows the two to drift whenever the file
// is moved, removed, or becomes unreadable.
const V2: &str = r#"
BEGIN;

CREATE TABLE provider_configs_v2 (
    provider_id TEXT PRIMARY KEY NOT NULL,
    model_id    TEXT,
    updated_at  INTEGER NOT NULL
);

INSERT INTO provider_configs_v2 (provider_id, model_id, updated_at)
SELECT provider_id, model_id, updated_at FROM provider_configs;

DROP TABLE provider_configs;
ALTER TABLE provider_configs_v2 RENAME TO provider_configs;

COMMIT;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_migration_preserves_models_and_drops_credential_mirror() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(V1).unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection
            .execute(
                "INSERT INTO provider_configs
                 (provider_id, model_id, credential_configured, updated_at)
                 VALUES ('groq', 'whisper-large-v3-turbo', 1, 42)",
                [],
            )
            .unwrap();

        apply(&connection).unwrap();

        let model: String = connection
            .query_row(
                "SELECT model_id FROM provider_configs WHERE provider_id = 'groq'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(model, "whisper-large-v3-turbo");

        let columns: Vec<String> = connection
            .prepare("PRAGMA table_info(provider_configs)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(columns, ["provider_id", "model_id", "updated_at"]);
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            TARGET_VERSION
        );
    }
}
