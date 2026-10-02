# Clide

System-wide dictation for macOS. Hold a shortcut, speak, and the text lands in
whatever application you were typing in.

```text
shortcut → capture → transcribe → process → insert → history
```

Read [`blueprint.md`](blueprint.md) for what Clide is meant to become, and
[`AGENTS.md`](AGENTS.md) for how to work on it.

## Status: v0.1

The core dictation path is implemented. Rust owns everything native — the
microphone, the global shortcut, on-device transcription, Accessibility
insertion, and SQLite. React owns presentation only.

| | |
|---|---|
| Transcription | 100% on-device: Apple Speech, local Whisper, and local Parakeet. No cloud engines, no API keys |
| Local models | 33 canonical whisper.cpp GGML builds and 3 Parakeet ONNX builds |
| Processing | Verbatim, deterministic local Polished, and on-device Apple Intelligence Rewrite |
| Insertion | Copies every transcript, then targets the original app through Accessibility or Cmd+V |
| History | SQLite with FTS5 full-text search; temporary audio is deleted when the transaction resolves |

Not in this version: file imports, per-app profiles, context reading, streaming
transcription, or a customisable dashboard grid.

## Running it

```bash
npm install
npm run app:build -- --debug --bundles app
open src-tauri/target/debug/bundle/macos/clide.app
```

Use the bundled app rather than `npm run app` (Tauri dev) for anything
involving permissions: macOS grants microphone and Accessibility access to a
bundle identity, and the dev binary does not have a stable one.

On first launch, Clide walks you through microphone access, Accessibility
access, your shortcut, and an engine (Apple Speech works immediately) — then has you run one real dictation
before opening the dashboard.

## Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build   # tsc --noEmit + vite build
```

One test is ignored by default because it types into the real machine:

```bash
# types into whatever app is focused — focus TextEdit first
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored insertion_reaches_the_focused_app
```

## Privacy

Dictation audio is written to a temporary file, transcribed on this Mac, and
deleted as soon as the transaction resolves — with a 120-second window kept
only so a failed transcription can be retried without speaking again. History
stores text, never recordings. Nothing is uploaded; the only network traffic is
downloading models you choose and a once-a-day check for new releases.
