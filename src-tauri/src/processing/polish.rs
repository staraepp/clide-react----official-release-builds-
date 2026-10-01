//! Deterministic cleanup for Polished mode.
//!
//! Every rule here is chosen to be reversible in the user's head: they should
//! be able to look at the inserted text and recognise their own sentence. When
//! a rule is ambiguous it is left out. No language model is involved, so
//! Polished costs nothing in latency.

/// Fillers removed when they stand alone as a whole word.
///
/// Kept deliberately short. "like", "so", "right", and "well" all carry
/// meaning often enough that deleting them changes sentences.
const FILLERS: &[&str] = &["um", "umm", "ummm", "uh", "uhh", "uhhh", "uhm", "erm"];

/// Words English legitimately doubles, protected from duplicate collapsing.
const LEGITIMATE_DOUBLES: &[&str] = &["had", "that", "is", "no", "very", "yes"];

/// Collapse runs of spaces and tabs while keeping line structure intact.
///
/// Shared by both modes: Verbatim's "minimal normalisation" is exactly this.
pub fn normalize_whitespace(input: &str) -> String {
    let lines: Vec<String> = input
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();

    // Drop blank lines at the edges but keep internal paragraph breaks.
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(0, |i| i + 1);

    lines
        .get(start..end)
        .unwrap_or_default()
        .join("\n")
        .trim()
        .to_string()
}

/// Run the full Polished pipeline. Input is assumed whitespace-normalised.
pub fn polish(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let words = tokenize(line);
            let words = strip_fillers(words);
            let words = collapse_stutters(words);
            let words = collapse_phrase_repeats(words);
            let line = words.join(" ");
            let line = tidy_punctuation(&line);
            capitalize_sentences(&line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tokenize(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

/// The alphabetic core of a token, ignoring surrounding punctuation.
pub(super) fn core_of(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Whether dropping this token would also drop punctuation that ends a clause.
pub(super) fn ends_sentence(token: &str) -> bool {
    token.ends_with(['.', '!', '?'])
}

fn strip_fillers(words: Vec<String>) -> Vec<String> {
    let kept: Vec<String> = words
        .iter()
        .filter(|token| {
            let core = core_of(token);
            // A filler that ends a sentence is load-bearing punctuation;
            // leave it rather than joining two sentences together.
            !FILLERS.contains(&core.as_str()) || ends_sentence(token)
        })
        .cloned()
        .collect();

    // Never let cleanup empty a line the user actually spoke.
    if kept.is_empty() {
        words
    } else {
        kept
    }
}

/// Collapse "the the file" into "the file".
///
/// Only immediate repeats of the same word are touched, and only for words
/// English does not naturally double.
fn collapse_stutters(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());

    for token in words {
        let core = core_of(&token);
        let repeats = out
            .last()
            .map(|previous| {
                let previous_core = core_of(previous);
                !core.is_empty()
                    && previous_core == core
                    && !LEGITIMATE_DOUBLES.contains(&core.as_str())
                    // A repeat across a sentence boundary is not a stutter.
                    && !ends_sentence(previous)
            })
            .unwrap_or(false);

        if repeats {
            // Keep the later token: it carries the punctuation that follows.
            out.pop();
        }
        out.push(token);
    }

    out
}

/// Longest phrase treated as a false start that was simply said again.
const MAX_REPEATED_PHRASE: usize = 4;

/// Collapse "I want it I want it to" into "I want it to".
///
/// The same safety bar as `collapse_stutters`, stretched to short phrases: only
/// an *immediate* repeat of two to four words, ignoring case and punctuation,
/// and never across a sentence boundary — "It works. It works." is the user
/// emphasising, not restarting. Anything fuzzier (a half-spoken word, a
/// reworded restart) is left for Rewrite, where a model can judge it.
fn collapse_phrase_repeats(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());

    for token in words {
        out.push(token);

        // A repeat can itself be repeated, so keep collapsing the tail.
        while let Some(first_copy) = repeated_tail_start(&out) {
            let length = (out.len() - first_copy) / 2;
            out.drain(first_copy..first_copy + length);
        }
    }

    out
}

/// Where the first copy of a phrase repeated at the end of `words` begins.
fn repeated_tail_start(words: &[String]) -> Option<usize> {
    (2..=MAX_REPEATED_PHRASE).rev().find_map(|length| {
        let start = words.len().checked_sub(length * 2)?;
        let (first, second) = words[start..].split_at(length);

        let same = first
            .iter()
            .zip(second)
            .all(|(a, b)| !core_of(a).is_empty() && core_of(a) == core_of(b));
        let one_sentence = !first.iter().any(|token| ends_sentence(token));

        (same && one_sentence).then_some(start)
    })
}

/// Remove space before closing punctuation and guarantee one space after it.
///
/// Two passes rather than one: collapsing a duplicate comma changes what the
/// next character is, and the spacing rule has to see the collapsed result.
fn tidy_punctuation(line: &str) -> String {
    let tightened = tighten(line);
    space_after_punctuation(&tightened).trim().to_string()
}

/// " ," -> ",", and ",," -> ",".
fn tighten(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c == ' '
            && matches!(
                chars.get(i + 1),
                Some(',' | '.' | '!' | '?' | ';' | ':' | ')')
            )
        {
            continue;
        }
        // A pause heard twice becomes a doubled comma.
        if c == ',' && out.ends_with(',') {
            continue;
        }
        out.push(c);
    }

    out
}

/// "word,next" -> "word, next", leaving decimals like 1,500 alone.
fn space_after_punctuation(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if matches!(c, ',' | ';' | ':')
            && chars.get(i + 1).is_some_and(|next| next.is_alphabetic())
        {
            out.push(' ');
        }
    }

    out
}

/// Capitalise the first word, anything after sentence-ending punctuation, and
/// the standalone pronoun "i".
fn capitalize_sentences(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut expect_capital = true;

    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if expect_capital && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            expect_capital = false;
            continue;
        }

        if matches!(c, '.' | '!' | '?') {
            expect_capital = true;
        }

        // A lone "i" is always the pronoun in dictated English.
        if c == 'i' && is_standalone_i(&chars, i) {
            out.push('I');
            continue;
        }

        out.push(c);
    }

    out
}

fn is_standalone_i(chars: &[char], index: usize) -> bool {
    let before_is_boundary = index == 0 || !chars[index - 1].is_alphanumeric();
    let after_is_boundary = chars
        .get(index + 1)
        .map_or(true, |c| !c.is_alphanumeric() && *c != '\'');
    before_is_boundary && after_is_boundary
}

#[cfg(test)]
mod phrase_repeat_tests {
    use super::*;

    fn polished(input: &str) -> String {
        polish(&normalize_whitespace(input))
    }

    #[test]
    fn a_restarted_phrase_collapses_to_the_final_version() {
        assert_eq!(
            polished("i want it i want it to be fast"),
            "I want it to be fast"
        );
    }

    #[test]
    fn punctuation_between_the_copies_does_not_hide_the_repeat() {
        assert_eq!(polished("I want it, I want it to be fast."), "I want it to be fast.");
    }

    #[test]
    fn a_phrase_said_three_times_collapses_fully() {
        assert_eq!(
            polished("send the file send the file send the file now"),
            "Send the file now"
        );
    }

    #[test]
    fn repeats_across_a_sentence_boundary_are_emphasis_not_a_restart() {
        assert_eq!(polished("It works. It works."), "It works. It works.");
    }

    #[test]
    fn different_phrases_are_never_collapsed() {
        assert_eq!(
            polished("i want it i would like it to be fast"),
            "I want it I would like it to be fast"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polished(input: &str) -> String {
        polish(&normalize_whitespace(input))
    }

    #[test]
    fn runs_of_whitespace_collapse_but_lines_survive() {
        assert_eq!(normalize_whitespace("  a   b  "), "a b");
        assert_eq!(normalize_whitespace("one\n\ntwo"), "one\n\ntwo");
        assert_eq!(normalize_whitespace("\n\n hello \n\n"), "hello");
    }

    #[test]
    fn standalone_fillers_are_removed() {
        assert_eq!(polished("um can you check the logs"), "Can you check the logs");
        assert_eq!(polished("so uh I need the file"), "So I need the file");
    }

    #[test]
    fn words_that_merely_contain_a_filler_are_left_alone() {
        assert_eq!(polished("the umbrella is uhh blue"), "The umbrella is blue");
        assert_eq!(polished("humming quietly"), "Humming quietly");
    }

    #[test]
    fn stutters_collapse() {
        assert_eq!(polished("open the the file"), "Open the the file".replace("the the", "the"));
        assert_eq!(polished("we we should go"), "We should go");
    }

    #[test]
    fn legitimate_doubles_are_protected() {
        assert_eq!(polished("I had had enough"), "I had had enough");
        assert_eq!(polished("the thing that that broke"), "The thing that that broke");
    }

    #[test]
    fn a_repeat_across_a_sentence_boundary_is_not_a_stutter() {
        assert_eq!(polished("ship it. it works"), "Ship it. It works");
    }

    #[test]
    fn sentences_are_capitalized() {
        assert_eq!(
            polished("this works. that also works? good!"),
            "This works. That also works? Good!"
        );
    }

    #[test]
    fn the_pronoun_i_is_capitalized_without_touching_other_words() {
        assert_eq!(polished("i think i'm right"), "I think i'm right");
        assert_eq!(polished("it is inside"), "It is inside");
    }

    #[test]
    fn punctuation_spacing_is_normalized() {
        assert_eq!(polished("hello , world"), "Hello, world");
        assert_eq!(polished("wait ; then go"), "Wait; then go");
        assert_eq!(polished("yes,,no"), "Yes, no");
    }

    #[test]
    fn decimals_and_ellipses_survive() {
        assert_eq!(polished("it costs 1,500 dollars"), "It costs 1,500 dollars");
        assert_eq!(polished("wait... okay"), "Wait... Okay");
    }

    #[test]
    fn a_line_of_pure_filler_is_returned_rather_than_deleted() {
        assert_eq!(polished("um uh"), "Um uh");
    }

    #[test]
    fn cleanup_is_idempotent() {
        let once = polished("um so i i think that that is fine , really");
        let twice = polish(&normalize_whitespace(&once));
        assert_eq!(once, twice, "polishing twice changed the text again");
    }

    #[test]
    fn a_realistic_dictation_reads_correctly() {
        let raw = "um so i was thinking that we could uh ship the the beta on friday , \
                   and then i can write the the release notes";
        assert_eq!(
            polished(raw),
            "So I was thinking that we could ship the beta on friday, \
             and then I can write the release notes"
        );
    }
}
