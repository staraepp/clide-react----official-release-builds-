//! Voice corrections: "scratch that" and friends.
//!
//! The pipeline is one-shot — the whole recording is transcribed, then
//! processed — so a correction cannot edit text as it is being spoken. It
//! resolves afterwards instead: each command phrase deletes the clause (or
//! sentence) spoken just before it, then disappears itself, so
//! "put it in models scratch that in providers" arrives as
//! "in providers".
//!
//! Runs on the raw transcript, before spoken punctuation and Polish, because
//! deleting whole spans is simplest while the text is still untouched and
//! because a command phrase is never something the user wanted typed.
//!
//! # Known limit
//!
//! The phrases are matched in running speech, so a sentence that genuinely
//! contains "scratch that" ("we should scratch that off the list") is treated
//! as a command. The phrase list is kept short for that reason.

use super::polish::{core_of, ends_sentence};

/// How far back a command reaches.
#[derive(Clone, Copy)]
enum Scope {
    /// Back to the previous comma or sentence end.
    Clause,
    /// Back to the previous sentence end only.
    Sentence,
}

const COMMANDS: &[(&[&str], Scope)] = &[
    (&["scratch", "the", "last", "sentence"], Scope::Sentence),
    (&["delete", "the", "last", "sentence"], Scope::Sentence),
    (&["undo", "the", "last", "sentence"], Scope::Sentence),
    (&["scratch", "last", "sentence"], Scope::Sentence),
    (&["delete", "last", "sentence"], Scope::Sentence),
    (&["undo", "last", "sentence"], Scope::Sentence),
    // Engines transcribe the command as heard, so tense varies: "scratched
    // that" is what Parakeet wrote for a spoken "scratch that".
    // Announced restarts. Deliberately exact: "I messed up the deploy" is a
    // real sentence, "sorry I messed up" is a correction. The whole sentence in
    // progress is dropped, because an abandoned half-sentence cannot be fixed
    // by cutting at a comma.
    (&["sorry", "i", "messed", "up"], Scope::Sentence),
    (&["let", "me", "start", "over"], Scope::Sentence),
    (&["let", "me", "start", "again"], Scope::Sentence),
    (&["let", "me", "restart"], Scope::Sentence),
    (&["scratch", "that"], Scope::Clause),
    (&["scratched", "that"], Scope::Clause),
    (&["strike", "that"], Scope::Clause),
    (&["struck", "that"], Scope::Clause),
];

/// Words people say just before a correction. Whisper punctuates them as their
/// own sentence ("Wait, scratch that."), which would otherwise be mistaken for
/// the thing being scratched.
const LEAD_INS: &[&str] = &[
    "wait", "no", "oh", "sorry", "actually", "hmm", "um", "uh", "okay", "ok",
];

/// The transcript after corrections, and how many were applied.
#[derive(Debug, PartialEq, Eq)]
pub struct Backtracked {
    pub text: String,
    pub corrections: usize,
}

/// Apply every voice correction in `input`, left to right.
///
/// Text without a command is returned exactly as it came in.
pub fn apply_backtracking(input: &str) -> Backtracked {
    let mut corrections = 0;
    let lines: Vec<String> = input
        .lines()
        .map(|line| {
            let (text, applied) = backtrack_line(line);
            corrections += applied;
            text
        })
        .collect();

    if corrections == 0 {
        return Backtracked {
            text: input.to_string(),
            corrections: 0,
        };
    }

    Backtracked {
        text: lines.join("\n").trim().to_string(),
        corrections,
    }
}

fn backtrack_line(line: &str) -> (String, usize) {
    let mut tokens: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    let started_capitalised = tokens
        .first()
        .and_then(|token| token.chars().find(|c| c.is_alphabetic()))
        .is_some_and(char::is_uppercase);

    let mut applied = 0;
    while let Some((at, length, scope)) = find_command(&tokens) {
        let start = span_start(&tokens, at, scope);
        tokens.drain(start..at + length);
        applied += 1;

        // Deleting the opening of the line leaves the survivor as the new
        // opening, so it inherits the capital the original line started with.
        if start == 0 && started_capitalised {
            if let Some(first) = tokens.first_mut() {
                *first = capitalise_first(first);
            }
        }
    }

    if applied > 0 {
        // "apples, oranges, scratch that." leaves "apples," — a comma with
        // nothing after it is a leftover, not something the user said.
        if let Some(last) = tokens.last_mut() {
            while last.ends_with([',', ';', ':']) {
                last.pop();
            }
        }
    }

    (tokens.join(" "), applied)
}

fn find_command(tokens: &[String]) -> Option<(usize, usize, Scope)> {
    let cores: Vec<String> = tokens.iter().map(|token| core_of(token)).collect();

    for at in 0..cores.len() {
        for (phrase, scope) in COMMANDS {
            let end = at + phrase.len();
            if end <= cores.len() && cores[at..end].iter().zip(*phrase).all(|(a, b)| a == b) {
                // "Wait, scratch that" — the lead-in is part of the command,
                // not part of what is being scratched.
                let mut start = at;
                while start > 0 && LEAD_INS.contains(&cores[start - 1].as_str()) {
                    start -= 1;
                }
                return Some((start, end - start, *scope));
            }
        }
    }
    None
}

/// Where the deleted span begins: just after the previous boundary that is
/// strictly before the last word already spoken.
///
/// The last word is excluded because Whisper-style engines punctuate what they
/// hear — "put it in models. Scratch that." — and that full stop closes the
/// very sentence being scratched, not the one before it.
fn span_start(tokens: &[String], command_at: usize, scope: Scope) -> usize {
    if command_at < 2 {
        return 0;
    }

    let last = command_at - 1;

    // A finished sentence is scratched whole. Without this, "Hi, how are you?
    // Scratch that." would stop at the comma and leave "Hi," behind.
    let scope = if ends_sentence(&tokens[last]) {
        Scope::Sentence
    } else {
        scope
    };

    (1..=last)
        .rev()
        .find(|&index| is_boundary(&tokens[index - 1], scope))
        .unwrap_or(0)
}

fn is_boundary(token: &str, scope: Scope) -> bool {
    match scope {
        Scope::Sentence => ends_sentence(token),
        Scope::Clause => ends_sentence(token) || token.ends_with([',', ';', ':']),
    }
}

fn capitalise_first(token: &str) -> String {
    let mut chars = token.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        apply_backtracking(input).text
    }

    #[test]
    fn a_punctuated_correction_removes_the_sentence_before_it() {
        assert_eq!(
            run("Put it in the models folder. Scratch that. In the providers folder."),
            "In the providers folder."
        );
    }

    /// Parakeet and some Apple Speech output carry no punctuation at all.
    #[test]
    fn an_unpunctuated_correction_removes_back_to_the_start() {
        assert_eq!(
            run("put it in the models folder scratch that in the providers folder"),
            "in the providers folder"
        );
    }

    #[test]
    fn a_clause_correction_stops_at_the_previous_comma() {
        assert_eq!(
            run("I want pizza, and I want pasta, scratch that, make it salad"),
            "I want pizza, make it salad"
        );
    }

    #[test]
    fn the_sentence_variant_ignores_commas() {
        assert_eq!(
            run("I like pizza. I like pasta, scratch the last sentence"),
            "I like pizza."
        );
        assert_eq!(
            run("I like pizza. Pasta is fine, though, undo last sentence."),
            "I like pizza."
        );
    }

    #[test]
    fn several_corrections_in_one_recording_all_apply() {
        let result = apply_backtracking("Send it Monday scratch that Tuesday scratch that Wednesday");
        assert_eq!(result.text, "Wednesday");
        assert_eq!(result.corrections, 2);
    }

    #[test]
    fn a_command_with_nothing_before_it_just_disappears() {
        let result = apply_backtracking("scratch that hello there");
        assert_eq!(result.text, "hello there");
        assert_eq!(result.corrections, 1);
    }

    #[test]
    fn the_survivor_inherits_a_leading_capital() {
        assert_eq!(
            run("Meet at three scratch that let's meet at five"),
            "Let's meet at five"
        );
    }

    #[test]
    fn a_trailing_comma_left_behind_is_removed() {
        assert_eq!(run("apples, oranges, scratch that."), "apples");
    }

    #[test]
    fn scratching_everything_leaves_nothing() {
        let result = apply_backtracking("Hello there. Scratch that.");
        assert_eq!(result.text, "");
        assert_eq!(result.corrections, 1);
    }

    #[test]
    fn text_without_a_command_is_returned_untouched() {
        let input = "  we scratch the surface of that,   truly  ";
        let result = apply_backtracking(input);
        assert_eq!(result.text, input);
        assert_eq!(result.corrections, 0);
    }

    /// Real Parakeet output: no punctuation, and the command in the past tense.
    #[test]
    fn a_real_unpunctuated_dictation_with_a_past_tense_command() {
        let result = apply_backtracking(
            "hi can we schedule the meeting to next wait no scratched that hi can we \
             reschedule the meeting to tomorrow and plan on how we will sign it",
        );
        assert_eq!(
            result.text,
            "hi can we reschedule the meeting to tomorrow and plan on how we will sign it"
        );
        assert_eq!(result.corrections, 1);

        let polished = crate::processing::process(
            crate::processing::ProcessingMode::Polished,
            &result.text,
            true,
        )
        .unwrap();
        assert!(polished.starts_with("Hi can we reschedule"), "got: {polished}");
    }

    /// Real Whisper output: punctuated, with a "Wait," lead-in.
    #[test]
    fn a_punctuated_wait_scratch_that_removes_the_whole_previous_sentence() {
        let result = apply_backtracking(
            "Hi, how about we move the next meeting to Thursday? Wait, scratch that. \
             Hi, how about we schedule the next meeting to Thursday?",
        );
        assert_eq!(
            result.text,
            "Hi, how about we schedule the next meeting to Thursday?"
        );
        assert_eq!(result.corrections, 1);
    }

    #[test]
    fn lead_in_words_are_removed_with_the_command() {
        assert_eq!(
            run("meet at three oh sorry scratch that meet at five"),
            "meet at five"
        );
    }

    /// The dictation that prompted this: a restart announced mid-sentence.
    #[test]
    fn an_announced_restart_drops_the_abandoned_sentence() {
        let result = apply_backtracking(
            "Hey, I think we should move our meeting to tomorrow. And not only that, we should \
             also talk about- wait, sorry, I messed up. Starting after the meeting schedule.",
        );
        assert_eq!(
            result.text,
            "Hey, I think we should move our meeting to tomorrow. Starting after the meeting schedule."
        );
        assert_eq!(result.corrections, 1);
    }

    #[test]
    fn let_me_start_over_drops_the_sentence_so_far() {
        assert_eq!(
            run("We ship on Friday. We should probably let me start over. We ship on Monday."),
            "We ship on Friday. We ship on Monday."
        );
    }

    #[test]
    fn i_messed_up_without_the_apology_is_ordinary_speech() {
        let input = "I messed up the deploy yesterday.";
        assert_eq!(run(input), input);
    }

    #[test]
    fn each_line_is_corrected_on_its_own() {
        assert_eq!(
            run("First line is fine\nSecond line wrong scratch that second line right"),
            "First line is fine\nSecond line right"
        );
    }
}
