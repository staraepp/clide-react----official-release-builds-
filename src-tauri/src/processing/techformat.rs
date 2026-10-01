//! Spoken file paths, written as code.
//!
//! In a chat tool that renders Markdown, "open src slash app slash page dot
//! tsx" should land as `src/app/page.tsx`. Two steps, both deterministic:
//!
//! 1. **Spoken form to symbols.** "slash" between two words becomes `/`, and
//!    "dot" before a known extension becomes `.`. Only then — "dot" and
//!    "slash" stay ordinary words everywhere else.
//! 2. **Wrap path-like tokens in backticks.** A token is wrapped when it ends
//!    in a known extension or is unmistakably a path; surrounding punctuation
//!    stays outside the span.
//!
//! This is opt-in because backticks are literal characters. Typed into a
//! terminal they are shell command substitution, and in a code editor they are
//! stray source text. Only the user can know which apps render them.

const EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "rs", "go", "rb", "java", "kt", "swift", "cpp",
    "hpp", "cs", "php", "sh", "zsh", "json", "yaml", "yml", "toml", "md", "mdx", "txt", "css",
    "scss", "html", "sql", "lock", "env", "xml", "plist",
];

/// Product names that look like files but are not.
const NOT_FILES: &[&str] = &[
    "node.js", "next.js", "vue.js", "three.js", "react.js", "express.js", "nuxt.js", "d3.js",
    "chart.js", "nest.js", "p5.js",
];

/// Words that are never a path segment, so "slash the budget" stays a verb.
const NOT_SEGMENTS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "to", "of", "in", "on", "it", "my", "your", "our",
    "their", "this", "that", "i", "we", "you", "he", "she", "they", "is", "are", "was", "be",
    "with", "for", "at", "by", "from",
];

/// Characters a file path is made of. Anything else means it is prose.
fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '~' | '@')
}

/// Convert spoken paths and wrap path-like tokens in backticks.
pub fn format_technical(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let tokens: Vec<String> = line.split_whitespace().map(str::to_string).collect();
            join_spoken_paths(tokens)
                .into_iter()
                .map(|token| wrap_if_path(&token))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn core(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn ends_with_punctuation(token: &str) -> bool {
    token.ends_with(['.', ',', ';', ':', '!', '?'])
}

/// Whether a spoken word could be one piece of a path.
fn is_segment(token: &str) -> bool {
    let word = core(token);
    !word.is_empty() && !NOT_SEGMENTS.contains(&word.as_str())
}

fn is_extension(token: &str) -> bool {
    EXTENSIONS.contains(&core(token).as_str())
}

/// "src slash app dot tsx" -> "src/app.tsx", "dot slash src" -> "./src".
fn join_spoken_paths(tokens: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        let word = core(&tokens[i]);
        let next = tokens.get(i + 1).map(|t| core(t));
        let after = tokens.get(i + 2).map(|t| core(t));

        // "dot dot slash x" -> "../x"
        if word == "dot" && next.as_deref() == Some("dot") && after.as_deref() == Some("slash") {
            if let Some(target) = tokens.get(i + 3) {
                out.push(format!("../{target}"));
                i += 4;
                continue;
            }
        }

        // "dot slash x" -> "./x", "tilde slash x" -> "~/x"
        if (word == "dot" || word == "tilde") && next.as_deref() == Some("slash") {
            if let Some(target) = tokens.get(i + 2) {
                let prefix = if word == "dot" { "." } else { "~" };
                out.push(format!("{prefix}/{target}"));
                i += 3;
                continue;
            }
        }

        let joins_previous = (word == "slash" || word == "dot")
            && out
                .last()
                .is_some_and(|previous| !ends_with_punctuation(previous) && is_segment(previous))
            && tokens.get(i + 1).is_some_and(|following| {
                if word == "dot" {
                    is_extension(following)
                } else {
                    is_segment(following)
                }
            });

        if joins_previous {
            let symbol = if word == "slash" { '/' } else { '.' };
            let previous = out.pop().unwrap_or_default();
            out.push(format!("{previous}{symbol}{}", tokens[i + 1]));
            i += 2;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }

    out
}

/// Wrap `token` in backticks when it is a path, keeping punctuation outside.
fn wrap_if_path(token: &str) -> String {
    if token.contains('`') {
        return token.to_string();
    }

    let body = token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'']);
    let trailing = &token[body.len()..];
    let body_trimmed = body.trim_start_matches(['(', '"', '\'']);
    let leading = &body[..body.len() - body_trimmed.len()];

    if is_path_like(body_trimmed) {
        format!("{leading}`{body_trimmed}`{trailing}")
    } else {
        token.to_string()
    }
}

fn is_path_like(candidate: &str) -> bool {
    if candidate.is_empty()
        || !candidate.chars().all(is_path_char)
        || candidate.contains("://")
        || NOT_FILES.contains(&candidate.to_lowercase().as_str())
    {
        return false;
    }

    let has_extension = candidate
        .rsplit_once('.')
        .is_some_and(|(stem, extension)| {
            !stem.is_empty()
                && !stem.ends_with(['.', '/'])
                && EXTENSIONS.contains(&extension.to_lowercase().as_str())
        });
    if has_extension {
        return true;
    }

    if !candidate.contains('/') {
        return false;
    }

    let segments = candidate.split('/').filter(|s| !s.is_empty()).count();
    let anchored = candidate.starts_with("./")
        || candidate.starts_with("../")
        || candidate.starts_with("~/")
        || candidate.starts_with('@')
        || (candidate.starts_with('/') && segments >= 2);

    anchored
        || segments >= 3
        || (segments >= 2 && candidate.split('/').any(|s| s.contains('_')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spoken_path_becomes_a_code_span() {
        assert_eq!(
            format_technical("open src slash app slash page dot tsx now"),
            "open `src/app/page.tsx` now"
        );
    }

    #[test]
    fn a_typed_filename_is_wrapped_and_punctuation_stays_outside() {
        assert_eq!(
            format_technical("Update app.tsx, then commands.ts."),
            "Update `app.tsx`, then `commands.ts`."
        );
    }

    #[test]
    fn relative_and_home_paths_are_understood() {
        assert_eq!(
            format_technical("run dot slash scripts slash build"),
            "run `./scripts/build`"
        );
        assert_eq!(format_technical("see tilde slash notes slash todo"), "see `~/notes/todo`");
        assert_eq!(
            format_technical("import dot dot slash lib slash util"),
            "import `../lib/util`"
        );
    }

    #[test]
    fn plain_prose_is_untouched() {
        for text in [
            "this and/or that",
            "we met at 3/4 of the way",
            "dot the i and slash the budget",
            "see you at the client/server boundary",
            "I use Node.js and Next.js daily",
            "visit https://example.com/docs/page.html today",
            "e.g. this works",
        ] {
            assert_eq!(format_technical(text), text, "changed: {text}");
        }
    }

    #[test]
    fn dot_only_joins_when_a_known_extension_follows() {
        assert_eq!(format_technical("dot com"), "dot com");
        assert_eq!(format_technical("config dot json"), "`config.json`");
    }

    #[test]
    fn existing_code_spans_are_left_alone() {
        assert_eq!(format_technical("edit `app.tsx` please"), "edit `app.tsx` please");
    }

    #[test]
    fn scoped_packages_and_module_folders_are_paths() {
        assert_eq!(format_technical("install @types/node"), "install `@types/node`");
        assert_eq!(
            format_technical("delete node_modules/react"),
            "delete `node_modules/react`"
        );
    }

    /// The whole text pipeline, in the order the dictation pipeline runs it.
    #[test]
    fn code_spans_survive_polish_without_being_recapitalised() {
        use crate::processing::{process, ProcessingMode};

        let formatted = format_technical("src slash app dot tsx is broken period");
        let out = process(ProcessingMode::Polished, &formatted, true).unwrap();
        assert_eq!(out, "`src/app.tsx` is broken.");
    }
}
