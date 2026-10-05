# Changelog

## 2.1.0 - 2026-10-05

### Added
- Local API: an optional HTTP API on `127.0.0.1`, off by default, with a bearer
  token on every request (Settings -> Local API).
  - `GET /v1/health`, `GET /v1/models`
  - `POST /v1/audio/transcriptions` (wav, m4a, mp3, flac, caf, aiff)
  - `GET /v1/events`, a server-sent event stream: `state`, `level`, `partial`,
    `transcript`, `error`
- Settings: port, token (reveal, copy, regenerate), allowed web origins, and
  separate switches for the transcription and event endpoints.

### Changed
- Spoken-correction, name and Rewrite handling moved into one shared step used
  by both dictation and the API. Dictation behaves as before.

## 2.0.0

Local-only rewrite: Apple Speech, Whisper and Parakeet engines, voice
backtracking, Rewrite with Apple Intelligence, live typing, signed in-app
updates.
