//! The API's bearer token.
//!
//! Stored in the settings table — a recorded exception to "no secrets in
//! SQLite" (AGENTS.md), chosen over the Keychain. It is kept out of
//! `AppSettings` entirely and only leaves through two commands.

use rusqlite::Connection;

use crate::database::kv;

const KEY: &str = "localapi.bearer";

/// 32 random bytes from the operating system, as 64 hex characters.
pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random source is unavailable");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The stored token, created on first use.
pub fn load_or_create(connection: &Connection) -> rusqlite::Result<String> {
    if let Some(existing) = kv::get::<String>(connection, KEY)? {
        if existing.len() == 64 {
            return Ok(existing);
        }
    }
    regenerate(connection)
}

/// Replace the token. Anything holding the old one stops working.
pub fn regenerate(connection: &Connection) -> rusqlite::Result<String> {
    let token = generate();
    kv::set(connection, KEY, &token)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_64_hex_characters_and_never_repeats() {
        let first = generate();
        let second = generate();
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
