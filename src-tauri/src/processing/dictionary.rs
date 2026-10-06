//! The user's dictionary: finding words in a transcript, and spelling the
//! ones the user has chosen their own way.
//!
//! Matching ignores case and only ever changes the spelling the user typed into
//! the dictionary, so "kubernetes" becomes "Kubernetes" but nothing is invented.
//! Adding a very common word ("Will") respells every occurrence; that is the
//! user's choice, and the same trade `names` makes for "Clide".

use super::names::{leading_punctuation, trailing_punctuation};

const MIN_WORD_CHARS: usize = 2;
const MAX_WORD_CHARS: usize = 40;

/// A word found in a transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    /// Apostrophes are always the straight `'`.
    pub text: String,
    /// First word of a sentence, where a capital letter says nothing about
    /// the word itself.
    pub starts_sentence: bool,
}

/// The lookup form of a word: lower case, straight apostrophes.
pub fn normalize_key(word: &str) -> String {
    word.replace('\u{2019}', "'").to_lowercase()
}

/// The words of a transcript, in order.
///
/// Only things worth remembering: numbers, single letters and text inside
/// code spans are skipped.
pub fn words(text: &str) -> Vec<Word> {
    let mut found = Vec::new();
    let mut token = String::new();
    let mut starts_sentence = true;
    let mut token_starts_sentence = true;
    let mut in_code = false;

    let finish = |token: &mut String, starts: bool, found: &mut Vec<Word>| {
        let trimmed = token.trim_matches(|c: char| matches!(c, '\'' | '-' | '_'));
        let length = trimmed.chars().count();
        if (MIN_WORD_CHARS..=MAX_WORD_CHARS).contains(&length)
            && trimmed.chars().any(char::is_alphabetic)
        {
            found.push(Word {
                text: trimmed.to_string(),
                starts_sentence: starts,
            });
        }
        token.clear();
    };

    for c in text.chars() {
        if c == '`' {
            finish(&mut token, token_starts_sentence, &mut found);
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }

        if c.is_alphanumeric() || matches!(c, '\'' | '\u{2019}' | '-' | '_') {
            if token.is_empty() {
                token_starts_sentence = starts_sentence;
                starts_sentence = false;
            }
            token.push(if c == '\u{2019}' { '\'' } else { c });
        } else {
            finish(&mut token, token_starts_sentence, &mut found);
            if matches!(c, '.' | '!' | '?' | '\n') {
                starts_sentence = true;
            }
        }
    }
    finish(&mut token, token_starts_sentence, &mut found);
    found
}

fn core(token: &str) -> String {
    normalize_key(token.trim_matches(|c: char| !c.is_alphanumeric()))
}

/// Respell words and phrases the user added, keeping the punctuation around
/// them. Longer phrases win over their own first word.
pub fn apply_dictionary(input: &str, entries: &[String]) -> String {
    // An entry with punctuation at its edges ("C++") cannot be matched on its
    // alphanumeric core without matching the bare letter, so it is only used
    // as a hint to the engine, never to rewrite text.
    let mut rules: Vec<(Vec<String>, &str)> = entries
        .iter()
        .filter_map(|entry| {
            let tokens: Vec<&str> = entry.split_whitespace().collect();
            let cores: Vec<String> = tokens.iter().map(|token| core(token)).collect();
            let plain = !tokens.is_empty()
                && tokens
                    .iter()
                    .zip(&cores)
                    .all(|(token, core)| !core.is_empty() && normalize_key(token) == *core);
            plain.then_some((cores, entry.as_str()))
        })
        .collect();
    if rules.is_empty() {
        return input.to_string();
    }
    rules.sort_by_key(|(cores, _)| std::cmp::Reverse(cores.len()));

    input
        .lines()
        .map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let mut out: Vec<String> = Vec::with_capacity(tokens.len());
            let mut i = 0;

            while i < tokens.len() {
                let matched = rules.iter().find(|(cores, _)| {
                    i + cores.len() <= tokens.len()
                        && cores
                            .iter()
                            .enumerate()
                            .all(|(offset, word)| core(tokens[i + offset]) == *word)
                });

                match matched {
                    Some((cores, canonical)) => {
                        let last = tokens[i + cores.len() - 1];
                        out.push(format!(
                            "{}{canonical}{}",
                            leading_punctuation(tokens[i]),
                            trailing_punctuation(last)
                        ));
                        i += cores.len();
                    }
                    None => {
                        out.push(tokens[i].to_string());
                        i += 1;
                    }
                }
            }

            out.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<String> {
        words(input).into_iter().map(|word| word.text).collect()
    }

    #[test]
    fn words_are_found_without_punctuation() {
        assert_eq!(
            texts("Hello, world! It's a state-of-the-art test."),
            ["Hello", "world", "It's", "state-of-the-art", "test"]
        );
    }

    #[test]
    fn numbers_single_letters_and_code_are_not_words() {
        assert_eq!(texts("I paid 42 dollars for a b 3.5 `src/app.tsx` today"), [
            "paid", "dollars", "for", "today"
        ]);
    }

    #[test]
    fn curly_apostrophes_become_straight_ones() {
        assert_eq!(texts("don\u{2019}t"), ["don't"]);
        assert_eq!(normalize_key("Don\u{2019}T"), "don't");
    }

    #[test]
    fn sentence_starts_are_reported() {
        let found = words("Paris is big. We saw Paris");
        let starts: Vec<(&str, bool)> = found
            .iter()
            .map(|word| (word.text.as_str(), word.starts_sentence))
            .collect();
        assert_eq!(
            starts,
            [
                ("Paris", true),
                ("is", false),
                ("big", false),
                ("We", true),
                ("saw", false),
                ("Paris", false)
            ]
        );
    }

    #[test]
    fn an_absurdly_long_token_is_ignored() {
        assert!(words(&"a".repeat(300)).is_empty());
    }

    fn apply(input: &str, entries: &[&str]) -> String {
        let entries: Vec<String> = entries.iter().map(|entry| entry.to_string()).collect();
        apply_dictionary(input, &entries)
    }

    #[test]
    fn a_word_takes_the_spelling_the_user_chose() {
        assert_eq!(
            apply("deploy to kubernetes, then (tauri) works", &["Kubernetes", "Tauri"]),
            "deploy to Kubernetes, then (Tauri) works"
        );
    }

    #[test]
    fn a_phrase_is_matched_as_a_whole() {
        assert_eq!(apply("open t3 code now", &["T3 Code"]), "open T3 Code now");
        assert_eq!(apply("ask t3 code.", &["T3 Code"]), "ask T3 Code.");
    }

    #[test]
    fn the_longest_phrase_wins() {
        assert_eq!(
            apply("use next js today", &["Next", "Next JS"]),
            "use Next JS today"
        );
    }

    #[test]
    fn unrelated_words_and_substrings_are_untouched() {
        let text = "the kubernetesish thing and a taurine drink";
        assert_eq!(apply(text, &["Kubernetes", "Tauri"]), text);
    }

    #[test]
    fn an_entry_with_edge_punctuation_never_rewrites_text() {
        let text = "i like c and c plus plus";
        assert_eq!(apply(text, &["C++"]), text);
    }

    #[test]
    fn no_entries_means_no_change_at_all() {
        let text = "  spaced   out \n text ";
        assert_eq!(apply_dictionary(text, &[]), text);
    }

    #[test]
    fn lines_are_handled_independently() {
        assert_eq!(apply("tauri\nrust", &["Tauri", "Rust"]), "Tauri\nRust");
    }
}
