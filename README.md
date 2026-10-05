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

## Local API

Other apps on your Mac can use Clide through an optional HTTP API. It is **off
by default**; turn it on in Settings -> Local API. It listens on `127.0.0.1`
only (never your network), and every request needs the bearer token shown in
Settings (Copy / Regenerate there).

| Endpoint | What it does |
| --- | --- |
| `GET /v1/health` | `{ ok, version, state, engine, mode }` |
| `GET /v1/models` | Installed speech models, usable as `model` below |
| `POST /v1/audio/transcriptions` | Multipart upload: `file` (wav, m4a, mp3, flac, caf, aiff; up to 25 MB, 30 min), optional `model`, `language`, `response_format` (`json` or `text`), `mode` (`verbatim`, `polished`, `rewrite`). Returns `{ "text": "..." }`. It never types anything. |
| `GET /v1/events` | Server-sent events: `state`, `level` (about 30 per second), `partial` (Apple live typing only), `transcript`, `error` |

```bash
TOKEN=...   # from Settings -> Local API
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:47815/v1/health

curl -H "Authorization: Bearer $TOKEN" \
  -F file=@meeting.m4a -F response_format=text \
  http://127.0.0.1:47815/v1/audio/transcriptions

curl -N -H "Authorization: Bearer $TOKEN" http://127.0.0.1:47815/v1/events
```

Errors are OpenAI-shaped: `{ "error": { "message", "type", "code" } }`. webm and
ogg uploads get `415` naming the supported formats; a dictation in progress
gets `409`; no installed model gets `503`.

Security: the `Host` header must be `127.0.0.1` or `localhost` with the port
(blocks DNS rebinding); any `Origin` header is refused unless you list it under
"Allowed web origins" (no CORS otherwise); requests are rate limited; the
token, uploaded audio, and transcripts are never logged; turning the API off
closes the port immediately. The token lives in Clide's local settings
database (not the Keychain) and is only ever returned by the Settings screen.

## Privacy

Dictation audio is written to a temporary file, transcribed on this Mac, and
deleted as soon as the transaction resolves — with a 120-second window kept
only so a failed transcription can be retried without speaking again. History
stores text, never recordings. Nothing is uploaded; the only network traffic is
downloading models you choose and a once-a-day check for new releases.
The optional Local API (off by default) only listens on this Mac's loopback
interface; it makes no outbound connections.
