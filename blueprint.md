# Clide v2 Product + Technical Blueprint

## 1. What Clide actually is

**Clide is a macOS-first voice utility combining system-wide dictation with a lightweight transcription workspace.**

The primary daily interaction is extremely short:

```text
Shortcut
   ↓
Speak
   ↓
Transcribe
   ↓
Optionally clean/rewrite
   ↓
Insert into focused app
   ↓
Save transcript to history
```

The secondary workflow is:

```text
Drop audio/video
   ↓
Import job
   ↓
Transcribe
   ↓
Transcript enters normal Clide history
```

Clide is being built for **us first**, but the architecture and UX should be clean enough that it can eventually become a public app.

That means we can expose some power-user functionality without designing ourselves into a corner.

---

# 2. Product principles

Clide should feel:

**Fast.**
Activating Clide should never feel like opening an application. The HUD appears essentially immediately and gets out of the way immediately afterward.

**Predictable.**
No secret provider switching, hidden AI rewrites, unexplained clipboard modifications, or random automatic retries.

**Provider-independent.**
Clide is the product. Groq, Apple Speech, Whisper, Parakeet, ElevenLabs, and future engines are interchangeable transcription backends.

**Local-friendly.**
Cloud transcription should not be structurally privileged over local transcription.

**Beautiful without becoming stupid.**
Shaders, glass, motion, and reactive visuals belong in the presentation layer. They must not infect normal buttons, text fields, tables, and dense information surfaces.

**Private by default.**
Dictation audio is ephemeral. Once transcription has completed, microphone recordings are deleted rather than becoming a giant accidental voice archive.

---

# 3. Platform stack

```text
Clide.app
│
├── React
│   ├── Dashboard
│   ├── History
│   ├── Import queue
│   ├── Provider/model UI
│   ├── Settings
│   ├── Onboarding
│   └── Motion/shaders
│
├── Tauri 2
│
└── Rust Core
    ├── Audio engine
    ├── Shortcut manager
    ├── Transcription engine
    ├── Provider adapters
    ├── Local inference adapters
    ├── Text processing
    ├── Accessibility
    ├── App context
    ├── Keychain
    ├── SQLite
    └── macOS integration
```

### Frontend

**React + TypeScript + Vite**

Tauri currently provides React support and documents Vite-based setups directly.

I would use:

```text
React
TypeScript
Vite
Tailwind
Motion
Radix-style primitives where useful
custom components
WebGL/WebGPU shader layer
```

I would **not** introduce Zustand immediately.

React state + a couple of focused contexts should be enough initially.

When shared state genuinely becomes annoying, then add Zustand.

---

# 4. Rust owns the application

This is important.

React should not become Clide's brain.

React owns:

```text
presentation
navigation
animations
forms
dashboard layout
settings UI
history rendering
model browser
```

Rust owns:

```text
audio capture
global shortcuts
provider requests
local inference
permissions
Accessibility
text insertion
active-app detection
Keychain
persistence
file imports
```

The frontend effectively asks:

```ts
invoke("start_dictation")
invoke("stop_dictation")
invoke("set_provider")
invoke("transcribe_import")
```

and subscribes to application events.

Something conceptually like:

```text
dictation:started
dictation:level
dictation:stopped

transcription:started
transcription:partial
transcription:complete
transcription:failed

insertion:started
insertion:complete
insertion:failed

import:queued
import:progress
import:complete
import:failed
```

That separation will save us enormous amounts of pain later.

---

# 5. Dictation interaction

Clide supports two activation styles.

### Hold

```text
press shortcut
↓
record
↓
release shortcut
↓
transcribe
```

### Toggle

```text
press shortcut
↓
record

press shortcut again
↓
transcribe
```

There is still only **one smart shortcut**.

The user chooses whether that shortcut behaves as Hold or Toggle.

Tauri's current global-shortcut API exposes `Pressed` and `Released` shortcut states, which makes the hold interaction architecturally reasonable rather than something we'd have to hack around.

---

# 6. Recording HUD

The HUD is tiny.

Not:

```text
┌─────────────────────────────────────┐
│ YOU ARE CURRENTLY RECORDING 🎙️      │
│ Provider: Whisper Large V3 Turbo    │
│ Model latency: 72ms                 │
│ 00:00:03                            │
└─────────────────────────────────────┘
```

Absolutely not. 💀

More like:

```text
╭──────────────────╮
│ ▁▃▆▄▂  Listening │
╰──────────────────╯
```

States:

```text
Listening
Processing
Inserting
Done
Error
```

The waveform responds to real microphone amplitude.

Processing morphs the waveform into a subtle activity animation.

Done gives a quick success transition and disappears.

Error expands slightly because errors actually require information.

The HUD should never steal keyboard focus.

---

# 7. Processing modes

Clide has three conceptual modes.

### Verbatim

Preserve what was said as closely as practical.

Provider formatting can still handle obvious punctuation and capitalization, but Clide should not rewrite the person's sentence.

### Polished

Runs inexpensive deterministic/local cleanup.

Examples include filler cleanup, duplicate words, whitespace normalization, capitalization fixes, and conservative punctuation normalization.

No LLM should normally be required.

### Rewrite

The transcript becomes input to an LLM processing stage.

This mode can restructure rough spoken language into coherent writing.

The architecture should keep **STT provider** and **rewrite provider** completely separate.

For example:

```text
Speech
 ↓
Parakeet
 ↓
raw transcript
 ↓
GPT / Gemini / local LLM
 ↓
rewritten transcript
```

Changing your STT model must never implicitly change the rewrite model.

---

# 8. Per-app profiles

There is one global configuration.

Then individual applications can override it.

Example:

```text
Global
Mode: Polished
Provider: Groq
Context: App identity

Terminal
Mode: Verbatim
Context: None

Discord
Mode: Polished
Context: Nearby text

Google Docs
Mode: Rewrite
Context: Nearby text
```

Possible overrides eventually include:

```text
processing mode
STT provider
model
language
context level
insertion strategy
```

But these should use progressive disclosure.

Normal users should not be staring at twelve dropdowns every time they add an application.

---

# 9. Context system

Context is **configurable per application**.

Clide should model context as explicit capability levels rather than a boolean.

```text
0  No context
1  Active application identity
2  Selected text
3  Nearby text
```

The provider pipeline receives a normalized context object.

For example:

```ts
Context {
  bundleId
  applicationName
  selectedText?
  nearbyText?
}
```

This also lets the UI clearly explain exactly what Clide is reading.

A user should be able to set:

```text
1Password → No context
Terminal → App identity
Discord → Nearby text
Pages → Nearby text
```

without wondering whether Clide is secretly vacuuming their screen.

---

# 10. Provider architecture

We chose **provider adapters**, which is the correct architecture for this app.

Every STT backend implements a common contract.

Conceptually:

```ts
TranscriptionProvider {
    id
    name
    capabilities()
    models()
    validateCredentials()
    transcribe()
    stream?()
}
```

Capabilities might look like:

```text
batch
streaming
local
timestamps
word timestamps
speaker diarization
language detection
translation
prompting
```

Therefore Clide can ask:

```text
Does this model support streaming?
Does it need credentials?
Can it process imports?
Can it return timestamps?
Is it local?
```

instead of littering the application with:

```text
if provider == "groq"
if provider == "elevenlabs"
if provider == "apple"
```

That path leads directly into spaghetti hell.

---

# 11. Initial provider ecosystem

Clide's architecture should anticipate:

```text
Apple Speech
Groq Whisper
Local Whisper
Parakeet
ElevenLabs
Deepgram
AssemblyAI
OpenAI
future providers
```

Not all have to ship immediately.

Current APIs also confirm there is enough capability variation to justify the adapter system.

Groq currently exposes Whisper Large V3 and Whisper Large V3 Turbo through its STT endpoints.

ElevenLabs currently exposes Scribe v2 plus Scribe v2 Realtime, including timestamps and other richer STT information.

Apple's current Speech framework supports both microphone/live audio and prerecorded audio.

So we should normalize capabilities rather than pretending all providers work identically.

---

# 12. Provider failure behavior

No automatic cloud roulette.

If the selected provider fails:

```text
Transcription failed
Groq did not respond.

[Retry]
[Choose another provider]
[Copy audio/transcript recovery info]
```

If the user picks another provider, Clide can retranscribe the still-temporary audio.

Audio must therefore remain alive **until the transaction is resolved**.

After:

```text
success
user cancels
session expires
```

the temporary recording is deleted.

This still honors the "text only history" rule while giving us sane error recovery.

---

# 13. Local model manager

Local transcription gets a proper first-class model manager.

Not:

```text
Choose model path:
/Users/star/Downloads/random-model-v7-final-final.gguf
```

😭

The normal experience:

```text
Local Models

Parakeet TDT
1.2 GB
Very Fast
High accuracy
[Download]

Whisper Large V3 Turbo
1.6 GB
Fast
Very High accuracy
[Download]

Whisper Small
466 MB
Very Fast
Good accuracy
[Installed ✓]
```

Each model entry can expose:

```text
name
engine
architecture
download size
installed size
languages
estimated speed class
quality class
hardware requirements
download status
version
```

Advanced custom model paths can come later if there is actual demand.

---

# 14. Credentials

Cloud providers use **BYOK**.

There is no Clide account requirement.

```text
Settings
→ Providers
→ Groq
→ API Key
```

The secret itself goes into macOS Keychain.

Clide's database only stores something equivalent to:

```text
credential configured = yes
keychain reference = provider.groq.default
```

Never the actual secret.

---

# 15. History

We deliberately selected **minimal storage**.

Each completed transcription needs roughly:

```text
id
text
created_at
source
source_app
```

`source` differentiates:

```text
dictation
import
```

That is enough for the product.

We don't need to transform history into telemetry soup.

Transient diagnostic information can exist during processing without living permanently inside every transcript record.

---

# 16. History search

Minimal records, surprisingly powerful search.

History supports:

```text
full-text transcript search
date filter
source app
source type
provider/model when available
processing mode
```

There is no semantic vector database in v1.

That would be architecture cosplay for a history list.

SQLite FTS is more than enough.

---

# 17. Imports

Imports work in two ways.

### Drop anywhere

Drag an audio/video file onto Clide.

The UI immediately acknowledges it and creates an import job.

### Import queue

A dedicated expanded dashboard surface shows:

```text
interview.mov     Transcribing  63%
voice-note.m4a    Waiting
meeting.mp3       Complete
```

When complete, an import becomes a normal history item.

There is **one transcript system**, not separate "dictation transcripts" and "file transcripts."

Source metadata differentiates them.

---

# 18. Main window

This changed substantially from my original recommendation.

We are **not** making a sidebar utility.

The main Clide window is a customizable **bento dashboard**.

Something conceptually like:

```text
┌────────────────────────────────────────────┐
│ Clide                             ● Ready  │
│                                            │
│ ┌─────────────────────┐ ┌───────────────┐ │
│ │                     │ │ Current       │ │
│ │   Start Dictation   │ │ Groq          │ │
│ │                     │ │ Whisper V3 T  │ │
│ └─────────────────────┘ └───────────────┘ │
│                                            │
│ ┌────────────┐ ┌─────────────────────────┐ │
│ │ Mode       │ │ Recent                  │ │
│ │ Polished   │ │ “Okay so basically…”   │ │
│ └────────────┘ │ “Can you fix…”          │ │
│                └─────────────────────────┘ │
│                                            │
│ ┌──────────────────┐ ┌───────────────────┐ │
│ │ Imports          │ │ Local models      │ │
│ │ 2 processing     │ │ 3 installed       │ │
│ └──────────────────┘ └───────────────────┘ │
└────────────────────────────────────────────┘
```

Widgets can eventually be:

```text
Start Dictation
Recent History
Current Provider
Current Model
Processing Mode
Imports
Local Models
App Profile
Usage
Language
Microphone
Shortcut
System readiness
Provider health
```

Eventually users can resize/rearrange them.

But **not in v0.1**.

For v0.1 the grid can visually resemble a customizable bento while actually using a fixed configuration.

Drag/resizing comes after the product works.

That distinction will save a frightening amount of development time.

---

# 19. Visual system

> **Superseded by the user (2026-10-01): the palette is monochrome and the
> layout is a flat sidebar, not a bento grid. "Blue means voice" below is
> historical. See `agents.md`.**

The design language is:

**modern web application + light, quiet SaaS surfaces + blue + restraint.**

Not Apple Settings.

Not a generic Electron dashboard.

Not a dark "edgy" utility.

Clide's identity is set by the marketing site, and the app follows it rather
than inventing a second one:

```text
paper            #F4F9FD
card             #FFFFFF
ink              #0A2338   (deep navy — no black anywhere)
hairline         #E3EDF5
voice            #5B9BC9
display face     Montserrat 500   (medium, never bold)
body face        DM Sans
utility face     DM Mono          (keycaps, timestamps, micro-labels)
wordmark         lowercase "clide" + the five-bar waveform mark
```

Fonts are bundled with the app as woff2. Clide works offline and its CSP admits
no remote font host, so linking Google Fonts is not an option.

## The one colour rule

**Blue means voice.**

`--color-voice` and its relatives appear only where Clide is hearing or handling
speech: the waveform, the listening state, the caret, the ambient wash while a
dictation is in flight.

Buttons are ink. Links are ink. Headings are ink. Semantic state (good, warning,
critical) has its own muted family and is used only at small sizes.

A blue that turns up anywhere else is a bug, not a decoration. This rule is what
keeps the interface low-contrast and calm while still letting the one thing that
matters — that Clide is listening — read instantly.

## What the surfaces do

```text
cards            white on paper, 1px hairline, 16px radius, no shadow
the hero card    the single exception: one soft shadow
shaders          a pale wash beneath everything, gathering only during dictation
motion           short, few, and tied to state changes
```

Explicitly not part of the language: gradients, glow, grain, glass on every
component, large radii, and decorative colour.

Shaders live underneath the UI. The interface itself stays sharp.

# 20. Shader intensity system

We selected selectable intensity.

So:

```text
Visual effects
○ Reduced
○ Normal
○ High
```

### Reduced

Static or nearly static shader.
Minimal blur movement.
Reduced Motion honored.
No microphone-reactive effects.

### Normal

Slow ambient shader.
Subtle state transitions.
Minor microphone response.

### High

Reactive shader field.
Mic amplitude can affect displacement/intensity.
Provider changes can subtly shift the scene.
Recording state can animate the environment.

But even High should never interfere with text readability.

Shader FPS should also throttle when:

```text
window unfocused
window obscured
Low Power Mode
battery constraints
Reduce Motion
```

A utility that spends 12% GPU while sitting behind Firefox deserves jail.

---

# 21. Menu bar

The menu-bar item is intentionally boring.

It shows:

```text
Clide
● Ready

Open Clide
Start Dictation
Settings
Quit
```

Potentially the status icon changes for:

```text
ready
recording
processing
error
```

Provider switching and model configuration do **not** need to infest this menu.

Its purpose is status + access.

---

# 22. Onboarding

Guided essentials.

### Welcome

Short explanation of Clide.

### Microphone

Explain why.
Request permission.
Verify.

### Accessibility

Explain that it allows Clide to insert dictated text into other applications.
Request.
Verify.

### Shortcut

Choose:

```text
hold
toggle
```

and record the desired shortcut.

### Provider

Recommended choices are shown.

Something like:

```text
Apple Speech
No API key

Groq
API key required

Local
Download required
```

### Test dictation

A real field appears:

```text
Hold ⌥ Space and say something.
```

Clide records, transcribes, processes, and inserts into its own test field.

That validates the entire pipeline.

### Done

Open dashboard.

---

# 23. Permissions philosophy

Do not request five permissions on launch.

The onboarding screen requesting microphone should be the first time macOS asks for microphone access.

Same for Accessibility.

If a user skips something optional, Clide explains which features remain unavailable.

Permissions also get a health/status area later:

```text
Microphone       ✓
Accessibility    ✓
Local Models     ✓
Groq             ✓
```

---

# 24. Insertion engine

Primary strategy:

**macOS Accessibility APIs.**

Fallback:

**clipboard-based paste.**

The clipboard fallback should preserve the previous clipboard when reasonably possible:

```text
capture clipboard
place transcript
paste
restore clipboard
```

It needs careful timing because applications behave differently.

Eventually the insertion engine can maintain compatibility rules for weird applications.

---

# 25. Processing pipeline

The entire dictation transaction should behave as a state machine.

```text
Idle
 ↓
Capturing
 ↓
FinalizingAudio
 ↓
Transcribing
 ↓
Processing
 ↓
Inserting
 ↓
Complete
```

Failure states can occur independently:

```text
CaptureFailed
TranscriptionFailed
ProcessingFailed
InsertionFailed
```

That gives us sensible recovery.

For example, insertion failing should **not** mean transcription failed.

The HUD can offer Copy if insertion fails.

---

# 26. Data storage

Use SQLite.

Conceptually:

```text
transcripts
settings
app_profiles
provider_configs
local_models
import_jobs
dashboard_layout
```

Keys stay in Keychain.

Audio lives only in a temporary cache while processing.

Shader settings, UI preferences, selected models, and similar non-sensitive preferences live locally.

No account.

No server.

No sync in early versions.

---

# 27. v0.1

This is where I want to be ruthless.

**v0.1 is the first version we personally use every day.**

It contains only:

1. React/Tauri application shell
2. Basic dashboard
3. Microphone capture
4. One smart global shortcut
5. Hold + toggle configurations
6. Tiny floating HUD
7. One reliable cloud transcription provider
8. One second transcription provider if easy
9. Verbatim + basic polished mode
10. Accessibility insertion
11. Clipboard fallback
12. Minimal history
13. Search
14. Keychain credentials
15. Guided onboarding
16. Menu-bar status
17. Basic blue visual system
18. Low-cost ambient shader

The first provider I would probably implement is **Groq** because its current STT API is relatively straightforward and supports Whisper models.

Then Apple Speech.

---

# 28. Explicitly NOT v0.1

This is important enough to name.

```text
No customizable bento grid
No local model downloads
No Parakeet
No ElevenLabs
No ten-provider catalog
No file imports
No per-app context reading
No LLM rewrite mode
No semantic search
No sync
No account
No analytics dashboard
No plugin system
No full shader spectacle
```

Otherwise we'll spend three weeks building a beautiful model browser for an application that cannot successfully type "hello" into Notes.

---

# 29. v0.2

Once dictation is reliable:

```text
Import queue
drag-and-drop media
unified import history
more STT adapters
processing mode UI
rewrite mode
LLM adapters
per-app profiles
context system
dashboard widget infrastructure
provider configuration UI
advanced settings
```

This is where Clide starts becoming more than Dictation.exe.

---

# 30. v0.3

Local-first release.

```text
Whisper local engine
Parakeet engine
model catalog
download manager
storage management
model benchmarks/profiles
offline mode
automatic compatibility detection
```

At this point local/cloud parity becomes a major product feature.

---

# 31. v0.4

This is where we unleash the designers from containment.

```text
customizable bento dashboard
drag/resizing
widget picker
dashboard presets
high shader mode
mic-reactive shader effects
richer transitions
advanced import viewer
provider capability surfaces
visual themes
```

The app already works before this happens.

That's the crucial bit.

---

# 32. v1

Clide v1 means:

```text
Reliable system-wide dictation
Hold + toggle
Multiple cloud providers
Local transcription
Whisper + Parakeet
Verbatim / Polished / Rewrite
Per-app profiles
Configurable context
Accessibility insertion
Import audio/video
Unified searchable history
Model manager
BYOK Keychain
Guided onboarding
Customizable bento dashboard
Selectable shader intensity
Polished macOS integration
```

At that point it is an actual product, not a prototype.

---

# 33. Repository structure

I would start approximately here:

```text
clide/
│
├── src/
│   ├── app/
│   ├── components/
│   ├── dashboard/
│   ├── dictation/
│   ├── history/
│   ├── imports/
│   ├── models/
│   ├── onboarding/
│   ├── providers/
│   ├── settings/
│   ├── shaders/
│   ├── hooks/
│   ├── lib/
│   └── styles/
│
├── src-tauri/
│   └── src/
│       ├── audio/
│       ├── context/
│       ├── database/
│       ├── dictation/
│       ├── imports/
│       ├── insertion/
│       ├── keychain/
│       ├── models/
│       ├── permissions/
│       ├── processing/
│       ├── providers/
│       │   ├── apple/
│       │   ├── groq/
│       │   └── traits.rs
│       ├── shortcuts/
│       └── state/
│
└── tests/
```

Do not create giant files called:

```text
TranscriptionManager.rs
AppManager.tsx
Utils.ts
```

that gradually become the graveyard of every architectural decision we regret.

---

# 34. The first engineering milestone

Before dashboard shaders.

Before history.

Before onboarding.

Before settings.

Before model browsers.

We need this:

```text
launch Clide
↓
register shortcut
↓
press shortcut
↓
capture microphone
↓
release
↓
send audio to provider
↓
receive transcript
↓
insert into TextEdit / Notes
```

When that works reliably, we officially have Clide.

Everything else is construction around that spinal cord.

---

# Final product definition

**Clide is a customizable macOS voice workspace built around fast system-wide dictation, flexible transcription engines, local and cloud models, intelligent text processing, and a searchable transcript history.**

Its everyday interaction remains almost invisible.

Its main application can be expressive, customizable, blue, shader-heavy, and information-dense.

Those two sides should coexist:

```text
While working:
Clide disappears.

When opened:
Clide comes alive.
```

That is the product.
