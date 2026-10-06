//! Spelling the names Clide is most often asked to write.
//!
//! Speech engines spell by ear. "Clide" arrives as "Clyde", and "Claude Code"
//! as "cladcode" or "clawed code". A fixed table fixes the ones that matter
//! without a model, in every mode and every engine.
//!
//! The trade-off is deliberate: a dictated "Clyde" is rewritten to "Clide",
//! including when the person meant someone called Clyde. That name is far
//! more likely to be this app's own.

/// Spoken variants, lower-case, and the spelling they become. Longest variants
/// first so "clawed code" is matched before any single-word rule.
const NAMES: &[(&[&str], &str)] = &[
    (&["clawed", "code"], "Claude Code"),
    (&["claud", "code"], "Claude Code"),
    (&["clad", "code"], "Claude Code"),
    (&["claude", "code"], "Claude Code"),
    (&["cladcode"], "Claude Code"),
    (&["claudecode"], "Claude Code"),
    (&["claude"], "Claude"),
    (&["clyde"], "Clide"),
    (&["clide"], "Clide"),
];

fn core(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Rewrite known names to their canonical spelling, keeping the punctuation
/// that surrounded them.
pub fn apply_known_names(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let mut out: Vec<String> = Vec::with_capacity(tokens.len());
            let mut i = 0;

            while i < tokens.len() {
                let matched = NAMES.iter().find(|(variant, _)| {
                    i + variant.len() <= tokens.len()
                        && variant
                            .iter()
                            .enumerate()
                            .all(|(offset, word)| core(tokens[i + offset]) == *word)
                });

                match matched {
                    Some((variant, canonical)) => {
                        let first = tokens[i];
                        let last = tokens[i + variant.len() - 1];
                        out.push(format!(
                            "{}{canonical}{}",
                            leading_punctuation(first),
                            trailing_punctuation(last)
                        ));
                        i += variant.len();
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

pub(super) fn leading_punctuation(token: &str) -> &str {
    let end = token
        .find(|c: char| c.is_alphanumeric())
        .unwrap_or(token.len());
    &token[..end]
}

pub(super) fn trailing_punctuation(token: &str) -> &str {
    let start = token
        .rfind(|c: char| c.is_alphanumeric())
        .map_or(token.len(), |index| {
            index + token[index..].chars().next().map_or(0, char::len_utf8)
        });
    &token[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_name_is_spelled_correctly() {
        assert_eq!(apply_known_names("open clyde now"), "open Clide now");
        assert_eq!(apply_known_names("Clyde is fast."), "Clide is fast.");
        assert_eq!(apply_known_names("is it clide?"), "is it Clide?");
    }

    #[test]
    fn claude_code_variants_are_unified() {
        assert_eq!(
            apply_known_names("I found a new agent called cladcode and"),
            "I found a new agent called Claude Code and"
        );
        assert_eq!(apply_known_names("try clawed code, please"), "try Claude Code, please");
        assert_eq!(apply_known_names("claude code"), "Claude Code");
        assert_eq!(apply_known_names("ask claude"), "ask Claude");
    }

    #[test]
    fn surrounding_punctuation_is_kept() {
        assert_eq!(apply_known_names("(clyde),"), "(Clide),");
        assert_eq!(apply_known_names("\"clyde\""), "\"Clide\"");
    }

    #[test]
    fn other_words_are_untouched() {
        let text = "the clyde-sdale horse and a cloud service";
        assert_eq!(apply_known_names(text), text);
    }

    #[test]
    fn each_line_is_handled_on_its_own() {
        assert_eq!(apply_known_names("clyde\nclide"), "Clide\nClide");
    }
}
