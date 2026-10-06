# AGENTS.md

## Clide v2

Clide is a macOS-first voice utility built with React, TypeScript, Tauri 2, and Rust. It combines fast system-wide dictation with a lightweight transcription workspace.

## REQUIRED: Read `blueprint.md` first

Before planning, implementing, refactoring, reviewing, or modifying Clide, **read `blueprint.md` in full**.

`blueprint.md` is the primary product and architecture specification for Clide. It defines:

- the product identity and intended user experience
- the React/Tauri/Rust responsibility split
- dictation behavior
- shortcut behavior
- recording HUD behavior
- provider architecture
- processing modes
- local model handling
- history and import behavior
- permissions and Accessibility behavior
- storage and credential rules
- dashboard direction
- shaders and motion
- release scope and phased implementation order

Do not rely only on this file when making product or architectural decisions.

If `AGENTS.md` and `blueprint.md` appear to conflict, **follow `blueprint.md` unless the user explicitly instructs otherwise**.

If the requested work would materially deviate from `blueprint.md`, point that out before making a large architectural change.

---

# Core product rule

Clide's primary workflow is:

```text
shortcut
→ record speech
→ transcribe
→ optionally process
→ insert into the focused application
→ save transcript text to history
```

The secondary workflow is:

```text
import audio/video
→ create transcription job
→ transcribe
→ add result to unified history
```

Clide should feel nearly invisible during normal dictation and expressive when the main application is opened.

---

# Technical stack

Use:

- Tauri 2
- React
- TypeScript
- Vite
- Rust
- Tailwind CSS
- Motion for UI animation
- SQLite for local structured storage
- macOS Keychain for secrets

Do not introduce a large dependency or framework without a concrete need.

In particular, do not add global state libraries such as Zustand merely because they are convenient. Start with React state and focused contexts. Add another state layer only when shared state complexity actually justifies it.

---

# Architecture boundaries

## React owns presentation

React should handle:

- dashboard rendering
- history UI
- import UI
- onboarding
- settings
- provider/model selection UI
- animations
- shader presentation
- forms
- navigation
- visual state

React should **not** become the native systems layer.

## Rust owns application behavior

Rust should handle:

- microphone capture
- global shortcuts
- transcription orchestration
- provider adapters
- local inference
- text-processing pipeline
- macOS Accessibility integration
- text insertion
- clipboard fallback
- active application/context detection
- permissions
- Keychain integration
- SQLite persistence
- file import processing
- temporary audio lifecycle

Keep the boundary explicit through Tauri commands and events.

Avoid moving native or security-sensitive behavior into frontend JavaScript merely because it is easier in the moment.

---

# Dictation state machine

Treat dictation as an explicit state machine.

Expected high-level states:

```text
Idle
Capturing
FinalizingAudio
Transcribing
Processing
Inserting
Complete
```

Failures should remain distinguishable:

```text
CaptureFailed
TranscriptionFailed
ProcessingFailed
InsertionFailed
```

Do not collapse every failure into a generic "dictation failed" state.

A successful transcription followed by failed insertion is still a successful transcription and should be recoverable by copying the generated text.

---

# Global shortcut

Clide uses **one smart global shortcut**.

The user can configure its behavior as either:

- hold-to-talk
- press-to-toggle

Do not create several mandatory global shortcuts for different modes.

Additional shortcuts may exist later only when there is a clear reason.

---

# Recording HUD

The recording HUD must remain small, fast, non-focus-stealing, and useful.

Primary states:

- Listening
- Processing
- Inserting
- Done
- Error

The listening state should include a compact waveform or microphone-level visualization.

Do not turn the HUD into a miniature dashboard.

Do not steal keyboard focus from the application the user is dictating into.

---

# Text processing modes

Clide has three conceptual modes.

## Verbatim

Preserve the user's wording as closely as practical.

## Polished

Use conservative deterministic/local cleanup for things such as:

- obvious casing fixes
- whitespace cleanup
- duplicate-word cleanup
- filler cleanup where safe
- conservative punctuation normalization

Do not unnecessarily invoke an LLM for normal polished dictation.

## Rewrite

Use an LLM processing stage to convert rough spoken language into more intentionally written text.

The speech-to-text provider and rewrite provider must remain separate architectural concepts.

Changing the STT model must not implicitly change the rewrite model.

---

# Provider architecture

All transcription providers should implement a normalized adapter interface.

The rest of the app should work with capabilities rather than provider-specific conditionals.

A provider adapter should conceptually expose information such as:

```text
id
name
models
capabilities
credential requirements
transcribe
optional streaming
```

Capabilities can include:

- local
- streaming
- batch transcription
- timestamps
- word timestamps
- diarization
- language detection
- translation
- prompting

Avoid architecture like:

```text
if provider == "groq"
if provider == "elevenlabs"
if provider == "apple"
```

Provider-specific behavior belongs inside adapters.

Providers expected by the broader architecture include:

- Apple Speech
- Groq Whisper
- local Whisper
- Parakeet
- ElevenLabs
- Deepgram
- AssemblyAI
- OpenAI
- future compatible backends

Not all providers belong in the first release.

---

# Provider failures

Do **not** silently switch the user's audio to another cloud provider.

If a transcription provider fails:

- clearly report the failure
- let the user retry
- let the user choose another configured provider

Temporary audio may remain available long enough to perform an explicit retry.

Delete it once the transaction succeeds, is cancelled, or expires.

---

# Audio retention

History stores text, not permanent microphone recordings.

Dictation audio is temporary.

Do not create a permanent voice archive by default.

Audio may exist only while necessary for:

- current transcription
- explicit retry/retranscription
- short-lived failure recovery

Then remove it.

---

# Text insertion

Primary insertion strategy:

1. macOS Accessibility APIs
2. clipboard/paste fallback when direct insertion fails or is unsupported

When using the clipboard fallback, preserve and restore the user's previous clipboard contents where practical and safe.

Insertion failure should never discard a successfully generated transcript.

---

# Context awareness

Context access is configurable per application.

Conceptual levels:

```text
0 - no context
1 - active application identity
2 - selected text
3 - nearby text
```

Use restrictive defaults.

Do not silently read broad application content.

Clearly represent what context an app profile is allowed to access.

Per-app profiles may eventually override:

- processing mode
- transcription provider
- model
- language
- context level
- insertion strategy

Use progressive disclosure rather than exposing every control immediately.

---

# Local-only

**Clide is 100% on-device (decided 2026-10-01, overriding blueprint §10–§14).**

There are no cloud STT providers, no cloud rewrite engines, no API keys, and no
credential store. Engines: Apple Speech, local Whisper, local Parakeet.
Rewrite uses on-device Apple Intelligence only.

Do not reintroduce a network leg for transcription or rewriting. The only
network traffic allowed is downloading models the user chose, the
once-a-day release check, and the signed update package the user chose to
install (Settings -> About). Nothing in the app opens a web browser.

Two recorded exceptions (added with the Local API, v2.1.0):

- **Loopback API.** An optional, off-by-default HTTP server (`src-tauri/src/api/`)
  may listen on `127.0.0.1` only. It never binds another interface, never makes
  outbound requests, requires a bearer token on every request, and rejects a
  foreign `Host` or any unlisted `Origin`. It exposes health, models,
  transcription of uploaded files and a read-only event stream. It must not
  start/stop dictation or insert text without a new decision.
- **Bearer token in SQLite.** The API token is stored in the app's KV table
  (`localapi.bearer`), by user decision, not in the Keychain. It must stay out
  of `AppSettings`, `settings:changed` payloads, `get_system_status` and logs;
  only `get_local_api_token` / `regenerate_local_api_token` return it. It is the
  only credential allowed in that table.

# Local models

Local models are first-class providers, not hacks bolted onto cloud logic.

The eventual model manager should support:

- browsing supported models
- downloading
- removing
- installation state
- model version
- file size
- language support
- quality/speed classes
- hardware compatibility

Normal users should not be forced to manage arbitrary model paths.

Custom paths can be an advanced feature later if needed.

---

# History

Keep history storage intentionally small.

A transcript record should remain close to:

```text
id
text
created_at
source
source_app
```

`source` should distinguish at least:

- dictation
- import

Do not turn every transcript into a telemetry document.

History should support:

- full-text search
- date filtering
- source application filtering
- source type filtering

Provider/model/mode filters may be supported when that metadata is available without bloating the core record model.

Do not add semantic/vector search unless there is a demonstrated need.

SQLite full-text search is preferred for normal history search.

---

# Dictionary

The dictionary (`database/dictionary.rs`, table `dictionary`) holds words, never
transcripts. By user decision (2026-10-06) every word of a finished dictation is
added automatically (`dictionaryAutoLearn`, on by default, dictation only, not
API uploads), and the user can add words by hand.

- Only **manual** words change behaviour: they respell matching text
  (`processing/dictionary.rs`, case-insensitive, runs in `finish_text`) and are
  appended to the Whisper prompt (`context::with_dictionary`). Learned words are
  a passive record until the user keeps one.
- It stays local, can be cleared, and must not store anything but words.

---

# Imports

Imported audio/video and live dictation share one transcript/history system.

Imports should support:

- drag-and-drop onto the main app
- a visible import queue
- progress
- failure state
- completed transcription

When complete, an import becomes a normal history item with source metadata.

Do not create an entirely separate transcript architecture for imports.

---

# Dashboard

> **Superseded (2026-10-01, user direction).** The app is now a flat sidebar
> (Home / History / Models / Settings) with a single-column Home: dictation,
> engine + style, recent transcripts, usage. No cards, no bento grid, and no
> dashboard customization is planned. The widget list below is kept as the pool
> of things Home may surface, not as a layout.

The main application previously used a **bento-style dashboard**.

The intended long-term dashboard is customizable and can contain widgets such as:

- Start Dictation
- Recent History
- Current Provider
- Current Model
- Processing Mode
- Imports
- Local Models
- App Profile
- Language
- Microphone
- Shortcut
- System Readiness
- Provider Health

However:

**Do not build drag/reorder/resize customization in the first release.**

Early versions should use a fixed bento layout that visually represents the intended product.

Real dashboard customization comes later.

---

# Visual direction

> **Superseded (2026-10-01, user direction).** The palette is **monochrome**:
> ink on paper, light or dark following macOS. "Blue means voice" no longer
> applies; `--color-voice` is the ink colour. Flat surfaces separated by
> hairlines, DM Sans, no card chrome. The shader is a faint grayscale wash.
> The bullets below are the original direction.

Clide is not intended to look like macOS Settings.

The visual direction was:

- modern custom web-app UI
- soft experimental surfaces
- blue as the primary visual anchor
- translucent surfaces where useful
- subtle grain
- layered blur
- custom shaders
- strong typography
- crisp controls
- expressive but controlled motion

Shaders are atmosphere, not interface chrome.

Keep buttons, text, tables, settings, and other information surfaces readable and sharp.

---

# Shader and motion performance

Visual intensity is user-selectable.

Expected modes:

- Reduced
- Normal
- High

Respect macOS Reduce Motion.

Throttle or stop expensive visual effects when appropriate, including when:

- the window is unfocused
- the window is obscured
- the system is in a constrained power state
- reduced-motion preferences are enabled

Do not allow an idle utility app to consume significant GPU resources for decorative rendering.

Performance regressions caused by shaders are bugs.

---

# Menu bar

The menu-bar item is primarily:

- status
- Open Clide
- Start Dictation
- Settings
- Quit

Keep it compact.

Do not cram the entire model/provider/settings interface into the menu bar.

---

# Onboarding

Use guided essentials.

The first-run flow should cover:

1. Welcome
2. Microphone permission
3. Accessibility permission
4. Shortcut setup
5. Default provider/model
6. API key if required
7. Test dictation
8. Dashboard

Request permissions only when their relevant onboarding step is active.

Verify that permission was actually granted before claiming success.

The test dictation should validate the complete path:

```text
shortcut
→ record
→ transcribe
→ process
→ insert
```

---

# Scope discipline

The first usable release is **core dictation first**.

## v0.1 priorities

Build:

- React/Tauri shell
- microphone capture
- one smart global shortcut
- hold and toggle behavior
- tiny recording HUD
- one reliable cloud STT provider
- optionally a second provider if inexpensive to add
- Verbatim mode
- basic Polished mode
- Accessibility insertion
- clipboard fallback
- minimal history
- basic text search
- Keychain credentials
- guided onboarding
- menu-bar status
- base blue visual system
- lightweight ambient shader

## Explicitly not v0.1

Do not let these block the first working release:

- customizable bento resizing/reordering
- local model downloads
- full Parakeet integration
- large provider catalog
- audio/video imports
- per-app context reading
- LLM Rewrite mode
- semantic history search
- cloud sync
- user accounts
- analytics dashboards
- plugin systems
- heavy shader effects

The app must become a reliable dictation tool before those features are allowed to dominate development.

---

# First engineering milestone

Before building the polished dashboard, prove this exact path:

```text
launch Clide
→ register global shortcut
→ press shortcut
→ capture microphone
→ stop recording
→ transcribe
→ receive text
→ insert into TextEdit/Notes
```

If this path is not reliable, work on it before expanding product scope.

This is Clide's spinal cord.

---

# Code organization

Prefer focused modules.

A reasonable structure is:

```text
src/
  app/
  components/
  dashboard/
  dictation/
  history/
  imports/
  models/
  onboarding/
  providers/
  settings/
  shaders/
  hooks/
  lib/
  styles/

src-tauri/src/
  audio/
  context/
  database/
  dictation/
  imports/
  insertion/
  keychain/
  models/
  permissions/
  processing/
  providers/
  shortcuts/
  state/
```

Avoid generic dumping-ground files such as:

- `Utils.ts`
- `AppManager.tsx`
- `TranscriptionManager.rs`

If a file begins accumulating unrelated responsibilities, split it by domain.

---

# Development behavior

When implementing a feature:

1. Read `blueprint.md`.
2. Identify which product phase the feature belongs to.
3. Check whether it violates an architecture boundary.
4. Implement the smallest reliable version.
5. Keep native behavior in Rust where appropriate.
6. Keep UI behavior in React.
7. Preserve explicit failure states.
8. Avoid speculative abstractions.
9. Test the real macOS interaction, not only isolated functions.
10. Do not silently widen product scope.

When fixing bugs:

- reproduce the actual failure
- determine which layer owns it
- avoid frontend workarounds for backend/native defects
- preserve successful intermediate results where possible
- add regression coverage when practical

When refactoring:

- do not change product behavior accidentally
- do not rewrite stable native code merely for style
- keep provider adapters isolated
- keep secrets out of logs
- maintain compatibility with the current blueprint

---

# Decision priority

When unsure, prioritize in this order:

1. Reliable dictation
2. Low interaction latency
3. Correct native macOS behavior
4. Predictable failure recovery
5. User privacy
6. Maintainable provider architecture
7. UI clarity
8. Visual polish
9. Additional features

A beautiful Clide that occasionally loses dictation is a bad Clide.

A simple Clide that reliably captures, transcribes, and inserts speech is a foundation worth building on.

---

# Final reminder

**Read `blueprint.md` before doing meaningful work on this repository.**

This file tells you how to work on Clide.

`blueprint.md` tells you what Clide is supposed to become.
