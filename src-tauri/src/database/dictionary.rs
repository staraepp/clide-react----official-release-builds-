//! The dictionary: words the user added by hand and words learned from their
//! dictations.
//!
//! Only manual words ever change how text is spelled or what the speech engine
//! is primed with. Learned words are a record of what has been said; the user
//! decides which of them matter.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::processing::dictionary::{normalize_key, words, Word};

/// Longest entry accepted. Long enough for a product name or short phrase.
pub const MAX_ENTRY_CHARS: usize = 60;
const DEFAULT_LIMIT: u32 = 200;
const MAX_LIMIT: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WordSource {
    /// Typed in by the user.
    Manual,
    /// Picked up from a dictation.
    Auto,
}

impl WordSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
        }
    }

    fn from_column(value: &str) -> Self {
        if value == "manual" {
            Self::Manual
        } else {
            Self::Auto
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub word: String,
    pub source: WordSource,
    /// Times it has been dictated.
    pub uses: u32,
    pub created_at: i64,
    pub last_used_at: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryQuery {
    pub search: Option<String>,
    pub source: Option<WordSource>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub manual: u32,
    pub learned: u32,
}

#[derive(Debug, Error)]
pub enum DictionaryError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

/// Add every word of a finished dictation, counting repeats. Returns how many
/// words were new.
pub fn learn(connection: &Connection, text: &str, now: i64) -> rusqlite::Result<usize> {
    let found = words(text);
    if found.is_empty() {
        return Ok(0);
    }

    let transaction = connection.unchecked_transaction()?;
    let before = total(&transaction)?;
    {
        let mut upsert = transaction.prepare_cached(
            "INSERT INTO dictionary (key, word, source, uses, created_at, last_used_at)
             VALUES (?1, ?2, 'auto', 1, ?3, ?3)
             ON CONFLICT(key) DO UPDATE SET uses = uses + 1, last_used_at = ?3",
        )?;
        // A word first heard at the start of a sentence is stored lower case;
        // the first time it is heard mid-sentence tells us its real casing.
        let mut recase = transaction.prepare_cached(
            "UPDATE dictionary SET word = ?2 WHERE key = ?1 AND source = 'auto' AND word = key",
        )?;

        for word in &found {
            let key = normalize_key(&word.text);
            let display = display_form(word);
            upsert.execute(params![key, display, now])?;
            if display != key {
                recase.execute(params![key, display])?;
            }
        }
    }
    let after = total(&transaction)?;
    transaction.commit()?;

    Ok((after - before) as usize)
}

/// How a learned word is written: as heard, except that a capital which only
/// marks the start of a sentence is dropped.
fn display_form(word: &Word) -> String {
    let mut chars = word.text.chars();
    let capital_only_by_position = word.starts_sentence
        && chars.next().is_some_and(char::is_uppercase)
        && chars.all(|c| !c.is_uppercase())
        // "I'm", "I'll": the capital belongs to the pronoun.
        && !word.text.starts_with("I'");

    if capital_only_by_position {
        normalize_key(&word.text)
    } else {
        word.text.clone()
    }
}

/// Add a word the user typed, or turn a learned word into one of theirs. The
/// spelling given is kept exactly.
pub fn add_manual(connection: &Connection, raw: &str, now: i64) -> Result<Entry, DictionaryError> {
    let word = clean_entry(raw)?;
    let key = normalize_key(&word);

    connection.execute(
        "INSERT INTO dictionary (key, word, source, uses, created_at, last_used_at)
         VALUES (?1, ?2, 'manual', 0, ?3, ?3)
         ON CONFLICT(key) DO UPDATE SET word = excluded.word, source = 'manual'",
        params![key, word, now],
    )?;

    Ok(connection.query_row(
        "SELECT word, source, uses, created_at, last_used_at FROM dictionary WHERE key = ?1",
        [&key],
        row_to_entry,
    )?)
}

fn clean_entry(raw: &str) -> Result<String, DictionaryError> {
    let word = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    if word.is_empty() {
        return Err(DictionaryError::Invalid("Type a word first."));
    }
    if word.chars().count() > MAX_ENTRY_CHARS {
        return Err(DictionaryError::Invalid(
            "That is too long. Keep it under 60 characters.",
        ));
    }
    if word.chars().any(char::is_control) {
        return Err(DictionaryError::Invalid(
            "That contains characters a word cannot have.",
        ));
    }
    if !word.chars().any(char::is_alphanumeric) {
        return Err(DictionaryError::Invalid(
            "A word needs at least one letter or number.",
        ));
    }
    Ok(word)
}

/// Remove a word, whatever its case. Returns whether it was there.
pub fn remove(connection: &Connection, word: &str) -> rusqlite::Result<bool> {
    let removed = connection.execute(
        "DELETE FROM dictionary WHERE key = ?1",
        [normalize_key(word.trim())],
    )?;
    Ok(removed > 0)
}

/// Forget every learned word, keeping the ones the user added.
pub fn clear_learned(connection: &Connection) -> rusqlite::Result<usize> {
    connection.execute("DELETE FROM dictionary WHERE source = 'auto'", [])
}

/// Yours first, then what is said most often.
pub fn list(connection: &Connection, query: &DictionaryQuery) -> rusqlite::Result<Vec<Entry>> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let pattern = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search| !search.is_empty())
        .map(|search| format!("%{}%", escape_like(&normalize_key(search))));
    let source = query.source.map(WordSource::as_str);

    let mut statement = connection.prepare_cached(
        "SELECT word, source, uses, created_at, last_used_at FROM dictionary
         WHERE (?1 IS NULL OR key LIKE ?1 ESCAPE '\\')
           AND (?2 IS NULL OR source = ?2)
         ORDER BY (source = 'manual') DESC, uses DESC, last_used_at DESC, key ASC
         LIMIT ?3",
    )?;
    let rows = statement.query_map(params![pattern, source, limit], row_to_entry)?;
    rows.collect()
}

pub fn counts(connection: &Connection) -> rusqlite::Result<Counts> {
    connection.query_row(
        "SELECT COALESCE(SUM(source = 'manual'), 0), COALESCE(SUM(source = 'auto'), 0)
         FROM dictionary",
        [],
        |row| {
            Ok(Counts {
                manual: row.get(0)?,
                learned: row.get(1)?,
            })
        },
    )
}

/// The words the user added, most used first. These are what respells a
/// transcript and what primes the speech engine.
pub fn manual_words(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare_cached(
        "SELECT word FROM dictionary WHERE source = 'manual'
         ORDER BY uses DESC, last_used_at DESC, key ASC",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

fn total(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("SELECT COUNT(*) FROM dictionary", [], |row| row.get(0))
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        word: row.get(0)?,
        source: WordSource::from_column(&row.get::<_, String>(1)?),
        uses: row.get(2)?,
        created_at: row.get(3)?,
        last_used_at: row.get(4)?,
    })
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn list_all(db: &Database) -> Vec<Entry> {
        list(&db.lock(), &DictionaryQuery::default()).unwrap()
    }

    fn find(db: &Database, word: &str) -> Option<Entry> {
        list_all(db).into_iter().find(|entry| entry.word == word)
    }

    #[test]
    fn every_word_of_a_dictation_is_learned() {
        let db = Database::in_memory().unwrap();

        let added = learn(&db.lock(), "Open the Kubernetes dashboard", 10).unwrap();

        assert_eq!(added, 4);
        assert_eq!(counts(&db.lock()).unwrap(), Counts { manual: 0, learned: 4 });
        assert!(list_all(&db)
            .iter()
            .all(|entry| entry.source == WordSource::Auto && entry.uses == 1));
    }

    #[test]
    fn hearing_a_word_again_counts_it_instead_of_duplicating_it() {
        let db = Database::in_memory().unwrap();

        learn(&db.lock(), "the cat saw the dog", 10).unwrap();
        let added = learn(&db.lock(), "The dog and the cat", 20).unwrap();

        assert_eq!(added, 1, "only 'and' is new");
        assert_eq!(find(&db, "the").unwrap().uses, 4);
        assert_eq!(find(&db, "dog").unwrap().last_used_at, 20);
    }

    #[test]
    fn a_capital_that_only_starts_a_sentence_is_not_kept() {
        let db = Database::in_memory().unwrap();

        learn(&db.lock(), "Paris is big. Hello there.", 10).unwrap();

        assert!(find(&db, "paris").is_some());
        assert!(find(&db, "hello").is_some());
        assert!(find(&db, "Paris").is_none());
    }

    #[test]
    fn hearing_a_name_mid_sentence_gives_it_its_capital() {
        let db = Database::in_memory().unwrap();

        learn(&db.lock(), "Paris is big", 10).unwrap();
        learn(&db.lock(), "we saw Paris", 20).unwrap();

        let paris = find(&db, "Paris").expect("recased");
        assert_eq!(paris.uses, 2);
        assert!(find(&db, "paris").is_none(), "one entry per word");
    }

    #[test]
    fn a_proper_name_is_not_lowercased_by_a_later_lowercase_mention() {
        let db = Database::in_memory().unwrap();

        learn(&db.lock(), "we saw Paris", 10).unwrap();
        learn(&db.lock(), "paris again", 20).unwrap();

        assert!(find(&db, "Paris").is_some());
    }

    #[test]
    fn the_pronoun_i_keeps_its_capital_at_a_sentence_start() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "I'm here", 10).unwrap();
        assert!(find(&db, "I'm").is_some());
    }

    #[test]
    fn learning_never_overrides_a_word_the_user_added() {
        let db = Database::in_memory().unwrap();
        add_manual(&db.lock(), "Kubernetes", 5).unwrap();

        learn(&db.lock(), "kubernetes is big", 10).unwrap();

        let entry = find(&db, "Kubernetes").unwrap();
        assert_eq!(entry.source, WordSource::Manual);
        assert_eq!(entry.uses, 1, "still counted");
    }

    #[test]
    fn adding_a_learned_word_makes_it_the_users_with_their_spelling() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "we use tauri daily", 10).unwrap();

        let entry = add_manual(&db.lock(), "  Tauri ", 20).unwrap();

        assert_eq!(entry.word, "Tauri");
        assert_eq!(entry.source, WordSource::Manual);
        assert_eq!(entry.uses, 1, "its history is kept");
        assert_eq!(counts(&db.lock()).unwrap().manual, 1);
    }

    #[test]
    fn a_phrase_can_be_added_and_its_spacing_is_tidied() {
        let db = Database::in_memory().unwrap();
        let entry = add_manual(&db.lock(), "T3   Code", 1).unwrap();
        assert_eq!(entry.word, "T3 Code");
    }

    #[test]
    fn nonsense_is_refused_with_a_reason() {
        let db = Database::in_memory().unwrap();
        for bad in ["", "   ", "---", "bad\u{0007}word", &"x".repeat(MAX_ENTRY_CHARS + 1)] {
            let error = add_manual(&db.lock(), bad, 1).unwrap_err();
            assert!(matches!(error, DictionaryError::Invalid(_)), "{bad:?}");
        }
        assert!(list_all(&db).is_empty());
    }

    #[test]
    fn removal_ignores_case_and_reports_what_happened() {
        let db = Database::in_memory().unwrap();
        add_manual(&db.lock(), "Kubernetes", 1).unwrap();

        assert!(remove(&db.lock(), "kubernetes").unwrap());
        assert!(!remove(&db.lock(), "kubernetes").unwrap());
        assert!(list_all(&db).is_empty());
    }

    #[test]
    fn clearing_learned_words_keeps_the_ones_the_user_added() {
        let db = Database::in_memory().unwrap();
        add_manual(&db.lock(), "Clide", 1).unwrap();
        learn(&db.lock(), "one two three", 2).unwrap();

        assert_eq!(clear_learned(&db.lock()).unwrap(), 3);

        assert_eq!(counts(&db.lock()).unwrap(), Counts { manual: 1, learned: 0 });
    }

    #[test]
    fn the_list_puts_the_users_words_first_then_the_most_said() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "rare common common common", 1).unwrap();
        add_manual(&db.lock(), "Zebra", 2).unwrap();

        let order: Vec<String> = list_all(&db).into_iter().map(|entry| entry.word).collect();
        assert_eq!(order, ["Zebra", "common", "rare"]);
    }

    #[test]
    fn the_list_can_be_searched_and_filtered() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "kube cluster kubernetes", 1).unwrap();
        add_manual(&db.lock(), "Kubectl", 2).unwrap();

        let search = |search: &str, source| {
            list(
                &db.lock(),
                &DictionaryQuery {
                    search: Some(search.into()),
                    source,
                    limit: None,
                },
            )
            .unwrap()
            .into_iter()
            .map(|entry| entry.word)
            .collect::<Vec<_>>()
        };

        assert_eq!(search("KUBE", None), ["Kubectl", "kube", "kubernetes"]);
        assert_eq!(search("kube", Some(WordSource::Manual)), ["Kubectl"]);
        assert!(search("zzz", None).is_empty());
    }

    #[test]
    fn search_characters_are_not_wildcards() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "alpha beta", 1).unwrap();

        for wildcard in ["%", "_", "al%"] {
            let hits = list(
                &db.lock(),
                &DictionaryQuery {
                    search: Some(wildcard.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(hits.is_empty(), "{wildcard:?} matched {hits:?}");
        }
    }

    #[test]
    fn the_limit_is_honoured_and_capped() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "aa bb cc dd ee", 1).unwrap();

        let few = list(
            &db.lock(),
            &DictionaryQuery {
                limit: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(few.len(), 2);
    }

    #[test]
    fn only_the_users_words_are_offered_for_spelling() {
        let db = Database::in_memory().unwrap();
        learn(&db.lock(), "learned words stay passive", 1).unwrap();
        add_manual(&db.lock(), "Clide", 2).unwrap();
        add_manual(&db.lock(), "T3 Code", 3).unwrap();

        let mut words = manual_words(&db.lock()).unwrap();
        words.sort();
        assert_eq!(words, ["Clide", "T3 Code"]);
    }
}
