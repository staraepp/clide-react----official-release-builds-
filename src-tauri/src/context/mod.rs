//! What Clide knows about where the user is dictating.
//!
//! This is blueprint context **level 1** — which application is frontmost, by
//! bundle identity — and nothing more. It never reads a window title, a
//! document, or selected text. The only decision it feeds is whether to prime
//! the speech engine with developer vocabulary.

use serde::{Deserialize, Serialize};

use crate::insertion::focus::FocusTarget;

/// Whether Clide primes the engine with technical vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TechnicalVocabulary {
    /// Only when a developer tool or a browser is frontmost.
    #[default]
    Auto,
    /// In every app.
    Always,
    Off,
}

/// Bundle identifiers of apps where developer vocabulary is likely.
///
/// Browsers are included on purpose: most coding agents and docs now live in a
/// browser tab, and Clide cannot see which tab without reading more than it
/// should.
const TECHNICAL_BUNDLES: &[&str] = &[
    // Editors and IDEs
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.vscodium",
    "com.todesktop.230313mzl4w4u92", // Cursor
    "com.exafunction.windsurf",
    "dev.zed.Zed",
    "com.apple.dt.Xcode",
    "com.sublimetext.4",
    "com.panic.Nova",
    // Terminals
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
    // Agent apps
    "com.anthropic.claudefordesktop",
    "com.openai.chat",
    // Browsers
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "org.chromium.Chromium",
    "company.thebrowser.Browser", // Arc
    "com.apple.Safari",
    "org.mozilla.firefox",
    "com.brave.Browser",
    "com.microsoft.edgemac",
    "com.vivaldi.Vivaldi",
];

/// Prefixes that cover a whole family of bundles.
const TECHNICAL_BUNDLE_PREFIXES: &[&str] = &["com.jetbrains."];

/// Names for apps whose bundle id Clide cannot rely on (wrappers, forks).
const TECHNICAL_APP_NAMES: &[&str] = &["t3 code", "claude code", "cursor", "codex"];

/// A glossary in the shape Whisper's initial prompt responds to best: plain
/// text that reads like something a developer would say.
pub const TECHNICAL_VOCABULARY: &str = "Glossary: TypeScript, JavaScript, React, Next.js, \
Node.js, npm, pnpm, Git, GitHub, pull request, commit, branch, merge, rebase, localhost, \
API, JSON, YAML, Docker, kubectl, regex, async, await, stdout, CLI, MCP, Claude Code, Tauri, \
Rust, Cargo, Vite, Tailwind, Vitest, Playwright, tsconfig, package.json, README.";

pub fn is_technical_app(target: &FocusTarget) -> bool {
    if let Some(bundle) = target.bundle_id.as_deref() {
        if TECHNICAL_BUNDLES.contains(&bundle)
            || TECHNICAL_BUNDLE_PREFIXES
                .iter()
                .any(|prefix| bundle.starts_with(prefix))
        {
            return true;
        }
    }

    target
        .app_name
        .as_deref()
        .map(str::to_lowercase)
        .is_some_and(|name| TECHNICAL_APP_NAMES.iter().any(|known| name == *known))
}

/// The vocabulary hint for this dictation, if one applies.
///
/// `engine_accepts_prompts` comes from the engine's capabilities: asking an
/// engine that cannot take a hint is a silent no-op, so it is decided here
/// rather than relying on the engine to ignore it.
pub fn vocabulary_prompt(
    setting: TechnicalVocabulary,
    target: &FocusTarget,
    engine_accepts_prompts: bool,
) -> Option<&'static str> {
    if !engine_accepts_prompts || target.is_clide() {
        return None;
    }

    match setting {
        TechnicalVocabulary::Off => None,
        TechnicalVocabulary::Always => Some(TECHNICAL_VOCABULARY),
        TechnicalVocabulary::Auto => is_technical_app(target).then_some(TECHNICAL_VOCABULARY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, bundle: &str) -> FocusTarget {
        FocusTarget {
            app_name: Some(name.into()),
            bundle_id: Some(bundle.into()),
            pid: None,
        }
    }

    #[test]
    fn editors_terminals_and_browsers_are_technical() {
        for (name, bundle) in [
            ("Code", "com.microsoft.VSCode"),
            ("Terminal", "com.apple.Terminal"),
            ("iTerm2", "com.googlecode.iterm2"),
            ("Google Chrome", "com.google.Chrome"),
            ("WebStorm", "com.jetbrains.WebStorm"),
        ] {
            assert!(is_technical_app(&app(name, bundle)), "{name} should be technical");
        }
    }

    #[test]
    fn a_wrapper_app_is_recognised_by_name() {
        assert!(is_technical_app(&app("T3 Code", "com.example.unknown")));
    }

    #[test]
    fn ordinary_apps_are_not_technical() {
        for (name, bundle) in [
            ("Notes", "com.apple.Notes"),
            ("TextEdit", "com.apple.TextEdit"),
            ("Messages", "com.apple.MobileSMS"),
        ] {
            assert!(!is_technical_app(&app(name, bundle)), "{name} should not be technical");
        }
        assert!(!is_technical_app(&FocusTarget::default()));
    }

    #[test]
    fn auto_only_primes_technical_apps() {
        let terminal = app("Terminal", "com.apple.Terminal");
        let notes = app("Notes", "com.apple.Notes");

        assert!(vocabulary_prompt(TechnicalVocabulary::Auto, &terminal, true).is_some());
        assert!(vocabulary_prompt(TechnicalVocabulary::Auto, &notes, true).is_none());
    }

    #[test]
    fn always_primes_every_app_and_off_primes_none() {
        let notes = app("Notes", "com.apple.Notes");
        let terminal = app("Terminal", "com.apple.Terminal");

        assert!(vocabulary_prompt(TechnicalVocabulary::Always, &notes, true).is_some());
        assert!(vocabulary_prompt(TechnicalVocabulary::Off, &terminal, true).is_none());
    }

    #[test]
    fn an_engine_that_cannot_take_a_hint_is_never_given_one() {
        let terminal = app("Terminal", "com.apple.Terminal");
        assert!(vocabulary_prompt(TechnicalVocabulary::Always, &terminal, false).is_none());
    }

    #[test]
    fn clide_itself_is_never_primed() {
        let clide = app("clide", "com.staraep.clide");
        assert!(vocabulary_prompt(TechnicalVocabulary::Always, &clide, true).is_none());
    }
}
