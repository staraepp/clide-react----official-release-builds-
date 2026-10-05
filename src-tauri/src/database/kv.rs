//! Generic JSON key/value storage for non-secret preferences.
//!
//! Nothing secret belongs here, with one recorded exception: the local API's
//! bearer token (see `api::token` and AGENTS.md). This table otherwise holds
//! things like the chosen shortcut and visual intensity.

use rusqlite::{Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn get<T: DeserializeOwned>(connection: &Connection, key: &str) -> rusqlite::Result<Option<T>> {
    let raw: Option<String> = connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?;

    Ok(raw.and_then(|json| match serde_json::from_str(&json) {
        Ok(value) => Some(value),
        Err(error) => {
            // A preference written by an older build should not stop the app
            // from starting; fall back to the default.
            tracing::warn!(key, ?error, "discarding unreadable setting");
            None
        }
    }))
}

pub fn set<T: Serialize>(connection: &Connection, key: &str, value: &T) -> rusqlite::Result<()> {
    let json = serde_json::to_string(value).map_err(|error| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(error))
    })?;
    connection.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::schema;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        schema::apply(&connection).unwrap();
        connection
    }

    #[test]
    fn values_round_trip_and_overwrite() {
        let connection = db();
        assert_eq!(get::<String>(&connection, "shortcut").unwrap(), None);

        set(&connection, "shortcut", &"Alt+Space".to_string()).unwrap();
        assert_eq!(
            get::<String>(&connection, "shortcut").unwrap().as_deref(),
            Some("Alt+Space")
        );

        set(&connection, "shortcut", &"Ctrl+Space".to_string()).unwrap();
        assert_eq!(
            get::<String>(&connection, "shortcut").unwrap().as_deref(),
            Some("Ctrl+Space")
        );
    }

    #[test]
    fn an_unreadable_value_falls_back_instead_of_failing() {
        let connection = db();
        connection
            .execute(
                "INSERT INTO settings (key, value) VALUES ('count', 'not json')",
                [],
            )
            .unwrap();
        assert_eq!(get::<u32>(&connection, "count").unwrap(), None);
    }
}
