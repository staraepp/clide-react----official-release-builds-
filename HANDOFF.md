# Clide v2 — Agent Handoff

**Canonical repository: https://github.com/staraepp/clide_stt** (branch `main`).

Do not push to `staraepp/clide-react----official-release-builds-`; that repo is
for release binaries only.

**Website repository: https://github.com/staraepp/Clide-website**, working copy
at `/Users/brenomenezes/learn/aaa` (Next.js). Deployed at
https://clide.staraep.fun.

**Purpose:** if the session building Clide ends, another agent picks up here.
Read `blueprint.md` (product truth) and `AGENTS.md` (engineering rules) first —
this file only records *state of the build*, never product decisions.

**Last updated:** 2026-09-04, lossless rewrite, insertion, update-check, and signed v0.1.1 release.

> Update this file at every milestone, not at the end of a session. The user
> asked for this explicitly and repeatedly. A milestone is: a decision made, a
> file group rewritten, a build passing or failing, a test run.

---

# LOCAL-ONLY PIVOT (2026-10-01) — Phase 1 of 3 done

User direction: Clide becomes 100% local, Whisperflow-like (voice backtracking
plus a polishing model), and "agent techier" (technical vocabulary and spoken
file paths as code in dev apps). Full plan: `~/.claude/plans/cheeky-tickling-meadow.md`.

**Phase 1 (done): cloud and BYOK removed.** Deleted `providers/{groq,openai,
deepgram,elevenlabs,assemblyai}`, `providers/http.rs`, `openai_compatible.rs`,
`credentials/`, `refine/cloud.rs`, and the frontend key UI. `TranscriptionProvider`
lost `credential_requirement`, `validate_credentials` and the `credential`
argument. `FallbackPolicy::AnyConfigured` is gone. Apple Speech stays (on-device,
the only engine usable on a fresh install) and is now the default provider.
`settings::reconcile` repairs old databases that still select `groq` or a cloud
rewriter. `SystemStatus.providerReady` replaces `providerConfigured` /
`providerNeedsKey`. This deliberately deviates from blueprint §10–§14 on the
user's explicit instruction. Verified: `cargo test` 185 passed / 3 ignored,
clippy `-D warnings` clean, `tsc` + `vite build` clean, and onboarding /
dashboard / Settings / Models rendered in a browser against a mocked backend.
**Not verified:** the packaged app and a real dictation after the change.
`credentials.json` from older installs is left on disk, unused.

**Phase 2 (next): backtrack.** `processing/backtrack.rs` runs first in
`processing::process()` on the full transcript ("scratch that" removes back to
the previous sentence boundary); phrase-level repeat collapse in `polish.rs`;
self-correction instruction added to `RefineStyle`. Not live/streaming — the
pipeline is one-shot by design.

**Phase 3: technical context.** New `context/` module. Vocabulary priming via
`TranscriptionRequest.prompt` (pipeline.rs hardcodes `None` today; only local
Whisper has `prompting`). Code-span formatting of spoken paths is an opt-in
setting, default OFF: backticks typed into a terminal are shell command
substitution.

---

# START HERE

## The app works. Verified end to end, on real hardware.

`shortcut -> capture -> transcribe -> process -> insert -> history` runs, and
has been observed running with **Parakeet CTC entirely on-device** as well as
with Groq (345–388 ms). The user dictates into other apps with it.

```
cargo test    188 passed, 2 ignored     (the ignored tests type into real apps)
cargo clippy  clean with -D warnings
tsc / vite    clean
```

## Current distribution state

Version **0.1.1** is published from app commit `f639176` at
https://github.com/staraepp/clide_stt/releases/tag/v0.1.1. The `.app` and DMG
are signed with the installed Apple Development certificate, both pass
`codesign --verify`, and the DMG passes `hdiutil verify`. They are not Developer
ID signed or notarized, so public Gatekeeper approval remains unavailable on
this Mac. The website is deployed from commit `4297430`.

Every Download button now serves the DMG directly from:

```
https://clide.staraep.fun/downloads/clide-0.1.1-apple-silicon.dmg
```

The release artifact is 13,904,340 bytes. Its SHA-256 is:

```
c16e94d33f59aba76b8e4c1b6b2911f1494d3166934d59cdf7e10d8b7fbdfb22
```

The live website download and the GitHub Release asset both return those exact
bytes. The About panel now embeds `f639176`; `build.rs` watches the resolved Git
branch ref as well as `.git/HEAD`, preventing stale build metadata after commits.

Clide checks GitHub Releases at launch at most once per 24 hours, caches the
latest successful result in SQLite, and exposes a manual Check now button in
About. This is update awareness, not silent installation: Tauri auto-install
requires a separate updater signing key plus a Developer ID/notarized release.

The installed final signed build correctly reports **Accessibility: Not
granted**. The old functional probe was a false positive and is gone. A human
must toggle Clide once in System Settings for the new signed identity before the
real TextEdit/browser insertion smoke tests can be completed. Do not cite the
pre-signing UI probes as proof: macOS discarded their synthetic events.

---

## WHAT THE APP IS NOW

**8 STT engines:** Groq, Apple Speech, OpenAI, Deepgram, ElevenLabs,
AssemblyAI, local Whisper, local Parakeet.

**36 local models:** the complete 33-model canonical whisper.cpp GGML family
(multilingual and English-only, Q5/Q8, Large v1/v2/v3 and Turbo) plus 3
Parakeet models (TDT plus CTC full and quantised). Every Whisper URL and byte
count was re-verified against the live Hugging Face repository — **none are
estimates**.

**Three processing modes**, all live: Verbatim, Polished (deterministic, local)
and Rewrite (Apple Intelligence, on-device).

## MODEL + INSERTION UPGRADE (2026-09-03)

The local catalogue grew from 12 to **36 models** without adding a new runtime:
all 33 GGML weights published by the canonical `ggerganov/whisper.cpp`
repository are now installable, alongside the existing three Parakeet models.
English-only weights correctly declare `multilingual: false`, the Models screen
shows language capability on each card, and All / Multilingual / English-only
filters keep the larger feed usable. All 33 Whisper URLs and exact sizes were
verified live before the catalogue was accepted.

Successful dictation now copies the transcript to the clipboard and deliberately
leaves it there. The fallback no longer restores old clipboard contents after
280 ms, which could race slow web/Electron inputs and report Done before they
consumed the paste. `FocusTarget` captures the original process id; Accessibility
writes prefer the system-wide focused element while verifying its process id,
then fall back to the captured app root. Clipboard fallback waits 50 ms for the
pasteboard and posts paced Cmd+V events to the global HID stream; process-targeted
key events were removed because AppKit/WebKit route paste through the key window.

Verification after this upgrade: frontend TypeScript/Vite build clean; full
Rust suite **188 passed / 2 ignored**; clippy clean with `-D warnings`; all 33
Whisper artifact URLs and byte lengths match the live canonical repository.

### Icon

`assets/icon.svg` is the source of truth. Sizes are rendered by a throwaway
Swift/WebKit script — no build dependency was added just to make an icon — and
assembled with `iconutil`. **Edit the SVG and re-render; never hand-edit the
PNGs.** If a stale icon appears after a change, it is macOS's cache: `touch`
the bundle, `lsregister -f` it, delete
`~/Library/Caches/com.apple.iconservices.store`, `killall Dock`.
 Six providers: Groq, OpenAI, Deepgram,
ElevenLabs, AssemblyAI, local Whisper, local Parakeet. Issue 1 still blocked on
the missing Groq key.

> Update this file at every milestone, not at the end of a session. The user asked
> for this explicitly. A milestone is: a decision made, a file group rewritten, a
> build passing or failing, a test run.

## REVIEW OF THE SHADER PASS (2026-09-03)

Independently re-verified, not taken from the log above: `cargo test` **92
passed / 1 ignored**, `cargo clippy --all-targets -- -D warnings` **clean**, the
packaged app launches with neither the language warning nor a schema error, and
the live dashboard renders correctly with cards and type crisp.

**Capture caveat for the next agent:** `app_screenshot` returns a *blank* window
for this app — the accelerated WebGL layer defeats that capture path. The DOM is
fine; use `screencapture -o -x -l <window_id>` instead. Do not conclude the UI is
broken from a blank `app_screenshot`.

### Accepted: the aurora adaptation

Porting the site's simplex-noise + domain-warped field into the existing
one-pass WebGL1 renderer is the right call, and deliberately dropping the
pointer flowmap, bloom, grain, and grid pass is exactly right for a utility that
idles behind other windows. The oceanic palette staying gated behind
`u_presence` preserves "blue means voice." Keep this.

### RESOLVED: the idle aperture was removed and replaced

The user confirmed the idle canvas *should* carry something — "the app feels too
empty" — so this was not a straight deletion. What replaced it:

**One field, two intensities.** The resting state is now the same simplex
aurora, rendered in cool blue-grey (`neutralMist` `#E7EFF5`-ish, `neutralDeep`
`#D9E5EE`), weighted toward the empty lower canvas by `lowerBias` and fading
back as `restingScale` drops with presence. As dictation starts it gathers into
the site's oceanic blues. There is no second object competing with the ribbon,
and "blue means voice" still holds because the resting tone is blue-*grey*, not
voice blue.

The warm constants are gone: `porcelain` and `paperShadow` were the only warm
values (R > G > B) in an otherwise entirely cool system, and on screen they read
as a tan smudge.

**Also added:** the HUD chip now runs the same shader at chip scale
(`Hud.tsx`), because it is the only thing visible while dictating into another
app. It is suppressed in the failure state, where blue would wrongly read as
"still working".

### Original finding (kept for context): the idle aperture

The fan of contour shells anchored at uv `(0.79, 0.18)` works against the
approved direction on three counts:

1. **It breaks the palette's temperature.** `paperShadow`
   `vec3(0.700, 0.690, 0.665)` and `porcelain` `vec3(1.0, 0.984, 0.954)` are
   *warm* — R > G > B — inside a system whose every other value is cool
   (paper `#F4F9FD`, ink `#0A2338`, voice `#5B9BC9`). On screen it reads as a
   tan smudge, not as paper relief.
2. **It is decoration, not atmosphere.** blueprint §19 says shaders live
   underneath the UI and lists gradients, glow and grain as removed for good.
   The log shows the first version was rejected as "too faint" and then
   strengthened — a process pushing toward *more* visible ornament, which is the
   opposite of the brief.
3. **It competes with the signature.** The approved design spends its boldness
   in one place: the ribbon, which is functional and shows dictation state. A
   second, decorative focal point in an empty corner dilutes that.

The motivation was legitimate — the idle canvas is nearly flat and the
lower-right is empty. But the answer is either to accept quiet emptiness (which
is what "clide disappears while working" implies) or to put something
*functional* there, not an ornament.

**To remove:** delete the aperture block in `shader.glsl.ts` (the `apertureUv`
through `shells`/`shellShadows` mixes) and the `porcelain` / `paperShadow`
constants. The aurora and voice-presence code are independent and stay.

### RESOLVED: prompt copy ignored dictation behaviour

`Ribbon.tsx` took a `behavior` prop and now says "Hold" or "Press" to match the
card beneath it.

## ACCESSIBILITY + SIGNING CORRECTION (2026-09-04)

The earlier functional permission probe was wrong: any AX error other than
`kAXErrorAPIDisabled` was treated as proof of trust. That produced a visible
"Granted" state while macOS still discarded synthetic events. Clide now uses
Apple's `AXIsProcessTrusted()` result directly and never claims success based on
an unrelated AX error.

The v0.1.1 `.app` and DMG are certificate-signed with `Apple Development: breno
menezes (SZY5666BHV)`; hard-coded ad-hoc signing was removed from
`tauri.conf.json`. `codesign --verify --deep --strict` passes and the app carries
the hardened-runtime flag. This provides a stable development identity, but it
does **not** provide public Gatekeeper trust or notarization. That still requires
an installed Developer ID Application certificate and Apple notarization
credentials.

Signing changed the app identity from the old ad-hoc build, so Accessibility
must be granted once more. The final signed build currently reports Not granted;
the real TextEdit/browser insertion proof remains blocked only on that user
security toggle.

## TWO REAL BUGS (2026-09-03)

### Downloads died partway with "error decoding response body"

`HTTP_TIMEOUT` was 90 s, and in `reqwest` **`timeout` covers the whole request
including reading the body**. So the cap was not "90 s to respond", it was
"90 s to finish downloading" — which every model in the catalogue exceeds. The
stream was aborted mid-body and surfaced as an opaque decode error.

Model downloads now use their **own client with no total timeout**, in
`lib.rs`. A dead connection is still caught, by `connect_timeout` (20 s) and
`read_timeout` (60 s) instead — the host must answer, and once streaming, bytes
must keep arriving. **Do not add `.timeout()` to that client.**

### Apple Speech always failed with "macOS hasn't been asked yet"

`providers::apple::request_authorization()` existed and **nothing ever called
it**, so the status stayed `NotDetermined` forever and every attempt failed.

Speech recognition is now a first-class permission (`permissions/speech.rs`),
part of `PermissionSnapshot`, requested when the user *selects* Apple Speech —
which keeps the prompt attached to the reason for it — and shown in the Setup
card, but only when Apple Speech is the chosen provider, since nothing else
needs it. It is deliberately **not** part of `can_capture`/`can_insert`: a Mac
without it is still ready to dictate with every other engine.

## LOCAL MODEL CATALOGUE — 11 entries (2026-09-03)

Nine Whisper builds and three Parakeet, spanning **74 MB to 2.5 GB**, so there
is something worth trying on any Mac. Every byte count was read from the
Hugging Face API and every URL was verified to return `200` with a matching
`content-length` — none are estimates.

The quantised builds are the interesting additions: `whisper-large-v3-turbo-q5`
gives Turbo's accuracy at 547 MB instead of 1.5 GB, and is the best trade in
the catalogue.

### Parakeet now has two architectures

TDT (transducer) and CTC take **different loaders** and ship different
artifacts, so `CatalogEntry` gained `arch: Option<ParakeetArch>` and
`run_parakeet` matches on it. The catalogue states the architecture rather than
the loader guessing from which files happen to be present. A test asserts every
Parakeet entry declares one and nothing else does.

The CTC repository nests weights under `onnx/` and suffixes quantised builds,
but the loader expects fixed names — so `ModelFile.name` and the remote path
deliberately differ, with a test pinning that.

## PERSONALITY PASS (2026-09-03)

Feedback: "everything feels too generic", "make the idle shader actually move",
easter eggs, GitHub/version links. Plus, mid-pass: "I don't really like the way
everything is like a list — I'd rather it be grids, bento boxes and cards."

### The shader was throttled for a problem that does not exist

**Every card is opaque white.** The field is only ever seen in the gutters and
the open canvas below, so it was never competing with text — it had been tuned
as if it were. Time scale went from 0.12–0.22 to 0.30–0.62, resting weight from
0.20 to 0.52, and a second faster current now crosses the first. One drifting
layer reads as a gradient that happens to change; two moving against each other
read as something flowing.

The FPS caps, DPR cap, Reduce Motion override and blur/visibility pausing in
`ShaderBackground` are unchanged — this is more visible, not more expensive.

### The wordmark is a live level meter

`components/Wordmark.tsx`. The five-bar mark breathes while idle, and while you
are dictating its bars are **driven from the real microphone level**, straight
from the ref in a rAF loop — going through React state would re-render the
title bar sixty times a second. This is the one place clide's identity and its
function are the same object; keep it that way.

### Easter eggs — `app/useEasterEggs.ts`

Deliberately small, and none of them change what the app *does*:

- Poke the wordmark five times and it dances.
- The Konami code pushes the shader to High and wakes the field for nine
  seconds, then it wears off on its own.
- A console greeting pointing at the repository — anyone opening devtools on an
  open-source app is a potential contributor.

The Konami handler ignores keystrokes while an input is focused, so it can
never fire mid-dictation-test.

### About panel — `settings/AboutSection.tsx`

Version, short commit, and build date, all stamped in by `build.rs`
(`CLIDE_COMMIT`, `CLIDE_BUILD_DATE`, with `rerun-if-changed=../.git/HEAD` so it
cannot go stale). The commit is copyable so a bug report can name the exact
build. Links to the repo, the site, the issue tracker and the licence, opened
through `tauri-plugin-opener` — the provider help link used to merely *copy* a
URL, which was weak; it opens now too.

### Lists became grids

Settings was seven full-width cards stacked vertically, which is a list wearing
card clothing. It is now a 12-column bento: most sections are half width,
Transcription and About take the full row. Setup's three rows became tiles (two
columns with the shortcut spanning, because three across truncated
"Accessibility"), the refine engines became cards, and cloud models now use the
same card as local ones instead of a separate row component.

### Bug found while looking at the result

Apple Speech reported **"API key needed"**. `get_system_status` asked the
credential store about every provider regardless of whether it takes a
credential. Now it checks `credential_requirement()` first, and `SystemStatus`
carries `provider_needs_key` so the UI can tell "key stored" apart from "never
wanted one". Regression test in `commands::settings::credential_status_tests`.

## APPLE SPEECH + APPLE INTELLIGENCE (2026-09-03)

### Apple Speech — `providers/apple/`

`SFSpeechRecognizer` through `objc2-speech`. No key, no download, models ship
with macOS — which makes it the one engine usable on a fresh install, and
therefore the **best fallback target**. `requiresOnDeviceRecognition` is forced
on: a provider Clide calls local must actually be local, and without that flag
macOS may route the audio to Apple's servers.

Speech recognition is a **separate permission** from the microphone, even
on-device. `NSSpeechRecognitionUsageDescription` is in `Info.plist` — without
it macOS kills the process rather than denying the request.

This changed an existing fallback test: Apple Speech always has a model, so it
legitimately appears as a rescue candidate even when nothing is downloaded.
That is the desirable behaviour, so the *test* was corrected, not the code.

### Rewrite mode — `refine/`

**A separate module from `providers/`, on purpose.** Blueprint §7 requires the
STT engine and the rewrite engine stay independent so changing one never
silently changes the other. Reusing `TranscriptionProvider` here would collapse
exactly that distinction. It has its own trait, registry, error type, and
setting.

`refine/apple_intelligence.rs` uses the FoundationModels framework via the
`foundation-models` crate — Apple's on-device LLM, macOS 26+, Apple Silicon,
Apple Intelligence switched on. Availability is checked **before every request**
rather than cached, because it can be turned off in System Settings while Clide
is running.

**Two safety properties worth preserving:**

1. *Every* refine failure is recoverable, and the pipeline keeps the
   deterministic transcript when refinement cannot run. A rewrite that fails
   must never cost the user words they already said. There is a test asserting
   every error variant is recoverable.
2. The instructions forbid the model answering the transcript or adding to it.
   A dictation tool that helpfully replies to a question you dictated is a bug.
   Tested for both styles.

`ProcessingMode::Rewrite` now runs the same deterministic polish first, then
hands the result to a refiner — so with no engine available the user still gets
a polished transcript.

### BUILD GOTCHA — read before touching `build.rs`

`foundation-models` compiles a Swift shim that links
`libswift_Concurrency.dylib`. On this SDK that lives **only** in the Xcode
toolchain's back-deployment directory — not `/usr/lib/swift`, not the dyld
cache. Without an rpath the binary links fine and then **aborts at launch**
with a dyld error that names fifty paths and no cause.

`build.rs::link_swift_runtime()` adds it, resolved through `xcode-select` so a
relocated Xcode still works. Do not remove it.

## THE FEEL PASS (2026-09-03)

User feedback: pages felt empty, the window would not drag by its top bar, the
app was "bland" and wanted "dopamine through animations", Settings was
"cluttered and really tightly compacted", and the HUD sat "too much into the
air".

### The window-drag bug — read this before touching the title bar

`-webkit-app-region: drag` **is in the built CSS and this webview ignores it.**
Only the native overlay strip at the very top was draggable, which is why the
window felt stuck. The fix is `data-tauri-drag-region` on the header (and on the
non-interactive children), which is Tauri's own handler. Do not "simplify" it
back to the CSS class.

### Motion

`lib/motion.ts` is the shared vocabulary — one spring, one easing, `PRESS`,
`LIFT`, `enter()`, and a single `LAND` beat. Applied through `Button` and
`Card`, so it is consistent rather than sprinkled.

The governing rule is already in the blueprint: *while working, clide
disappears; when opened, clide comes alive.* The first half was built and the
second was not. **Nothing exceeds ~320 ms**, and anything on the click path
resolves in half that — motion that makes you wait is latency in a costume.
`prefers-reduced-motion` is collapsed globally in `theme.css`, so individual
components do not check it.

### Emptiness

`UsageCard` fills the dead lower half with **real counts** — words, dictations,
distinct apps, and a day streak, all `COUNT`/`SUM` over actual rows
(`database/transcripts::usage`, with tests for the week boundary and for a
streak that stops at the first gap). When there is nothing yet the card says so
rather than showing confident zeroes.

Grid note: Recent (8 wide) previously left a 4-wide hole because Usage (8)
could not sit beside it. Recent now pairs with Setup and Usage runs full width.

### Settings

Each section is now a `Card` with a fixed heading column and the controls
beside it, instead of bare blocks stacked down the page. That was the whole
"cluttered" complaint — there was no grouping and nothing to aim at.

### HUD

`BOTTOM_MARGIN` was 96 px *plus* 16 px of padding inside the window. Now 24 px
and 4 px.

## THE FALLBACK SYSTEM (2026-09-03) — and how it stays inside blueprint §12

The user asked for "is your model not available? use a simple fallback". Taken
literally that is the "automatic cloud roulette" §12 forbids, because silently
sending someone's voice to a vendor they did not pick is a privacy decision
Clide should not make alone. `dictation/fallback.rs` reconciles the two:

1. **Local engines are always safe** to fall back to — the audio never leaves
   the machine, so no boundary is crossed. This is the default
   (`FallbackPolicy::LocalOnly`).
2. **Cloud-to-cloud is opt-in** (`AnyConfigured`), never the default.
3. **No fallback is silent.** `transcription:fell-back` fires and the HUD shows
   "via <provider>", so a transcript that reads differently always has an
   explanation.

A provider is only a candidate when it is genuinely usable: a local engine with
nothing downloaded, or a cloud one with no key, is skipped. Local candidates
are always tried before cloud ones. There are tests for each rule, including
the privacy one — `local_only_never_reaches_for_a_cloud_provider`.

Failure reporting keeps the **original** error, not the last fallback's: that is
the one the user needs to act on.

## THE MODELS PAGE (2026-09-03) — built

One screen for both halves of the same decision: which engine, then which of its
models. `View` gained a `models` entry; `ModelsView.tsx` renders it.

### The ratings are derived, never invented

`AGENTS.md` and blueprint §18 both forbid fake statistics, and a star rating
pulled from nowhere is exactly that. So every number traces to a fact:

- **`models/hardware.rs`** measures this Mac via `sysctl` — chip string, total
  memory, performance cores, and whether it is Apple Silicon. No dependency, no
  permission prompt, read once and cached.
- **`models/rating.rs`** turns that into stars. Accuracy comes from the model's
  declared `QualityClass` and **never varies with hardware** (there is a test
  asserting exactly that). Speed starts from `SpeedClass` and is reduced by
  three real factors: memory pressure from this model's size against usable
  RAM, absence of Metal/ANE on Intel, and Parakeet's dynamic ONNX shapes, which
  the crate's own notes say make CoreML *slower* than CPU here.
- **There is no popularity score.** Clide has no telemetry and could not know
  one. If a rating cannot be derived from a measured or declared fact, it does
  not belong in this file.

`usable_memory_bytes` is two thirds of RAM: the OS, the app being dictated
into, and Clide all need room, so a model rated "runs great" should not cause
swapping.

The Models page states the basis in plain text — "Ratings come from this
hardware, not from a leaderboard" — so the user can judge whether to trust it.

### Feed ordering

`ModelStore::ranked()`: installed first (what you already have is what you can
use now), then by fit, then by overall rating. Tested so a worse-fitting model
can never sort above a better one.

### One round trip

`get_models_page` returns models, providers, hardware, and the current
selection together. Two commands would let the halves disagree mid-render.

## LOCAL MODELS (2026-09-03) — Whisper and Parakeet both wired

**The runtime risk is gone.** Both `whisper-rs 0.16` (with the `metal` feature)
and `parakeet-rs 0.3` were proved to build on this machine in an isolated probe
crate *before* anything was wired into Clide — which is the sequencing the plan
below always called for. Do the same for any future engine.

### What shipped

- **`models/catalog.rs`** — the installable list. Whisper Base / Small /
  Large v3 Turbo, from ggerganov's `whisper.cpp` repo on Hugging Face. Users
  pick from this; they never type a path (blueprint §13).
- **`models/store.rs`** — **the filesystem is the source of truth.** A model is
  installed when its file exists at roughly the expected size, not when a row
  says so, so a hand-deleted model or a killed download resolves correctly on
  the next launch. Tested against the truncated-download case specifically.
- **`models/download.rs`** — streams to a `.partial` file and *renames on
  success*, so an interrupted transfer can never be mistaken for an installed
  model. Emits `model:progress` / `model:complete` / `model:failed`.
  Includes a hand-rolled SHA-256 (verified against published vectors and
  against `shasum -a 256`) so checksum verification adds no dependency.
  Catalogue entries currently carry `sha256: None` — Hugging Face does not
  publish stable digests for these files. Fill them in if a trustworthy source
  appears; the verification path is already wired.
- **`providers/local/mod.rs`** — `LocalWhisperProvider`. No credential, no
  network. Inference runs in `spawn_blocking` because it is CPU/GPU-bound and
  would otherwise stall the async runtime.
- **`commands/models.rs`** — `list_models`, `download_model`, `remove_model`.
  `download_model` returns immediately and reports through events; blocking on
  a 1.5 GB transfer would freeze the webview.

### The design decision worth keeping

`LocalWhisperProvider::models()` returns **only what is installed**, read from
disk each call. An engine advertising weights the user has not downloaded would
fail at the worst possible moment. This broke an existing registry test that
assumed every provider ships a fixed catalogue; the invariant was corrected
rather than the behaviour — see `every_provider_that_offers_models_defaults_to_
one_of_them` and its cloud-only counterpart.

### Parakeet — done

`providers/local/parakeet.rs`, via `parakeet-rs 0.3` and ONNX Runtime. Parakeet
TDT 0.6B v3, four artifacts totalling ~2.5 GB, loaded from a directory rather
than a file.

**The multi-file work this forced, and why it was the right shape:**
`CatalogEntry.file_name` became `files: Vec<ModelFile>`, each with its own URL,
size and optional checksum. `is_installed` now requires **every** file — a
regression test covers it, because Parakeet with three of its four artifacts is
useless and must never read as installed. The downloader fetches them in turn,
counts files already present toward progress so a resumed download does not
restart the bar at zero, and still renames each `.partial` only on success.

File sizes in the catalogue were read from the Hugging Face API, not estimated.

**Execution provider:** left on CPU deliberately. `parakeet-rs`'s own notes say
CoreML currently runs these graphs *slower* than CPU, because their dynamic
input shapes stop CoreML planning for the ANE. Revisit if the crate gains
static-shape exports.

`providers/local/` is now `mod.rs` / `whisper.rs` / `parakeet.rs` / `audio.rs`,
with the WAV decode shared rather than duplicated.

## PROVIDERS (2026-09-03) — done

Five cloud backends now ship: **Groq, OpenAI, Deepgram, ElevenLabs,
AssemblyAI**. All are registered in `providers/registry.rs` and appear in the
settings UI automatically, because that screen renders from capabilities rather
than from a hardcoded list. `109 passed, 1 ignored`; clippy clean.

### What was extracted rather than copied

- **`providers/http.rs`** — reading the clip with a size ceiling, `Retry-After`,
  and status-to-`ProviderError` mapping. It also parses the five *different*
  error envelopes these providers use (`{"error":{"message"}}`, `{"err_msg"}`,
  `{"detail":{"message"}}`, `{"detail"}`, `{"error"}`, `{"message"}`) so a
  failure always reaches the user as a sentence instead of raw JSON.
- **`providers/openai_compatible.rs`** — the OpenAI `/audio/transcriptions` wire
  format. Groq deliberately mirrors it, so Groq was refactored onto this shared
  path rather than keeping a private copy. Groq's own tests guarded that move.

### Where each provider actually differs

| | auth | body | notes |
|---|---|---|---|
| Groq / OpenAI | bearer | multipart | identical wire format |
| Deepgram | `Authorization: Token` | raw audio | options are query params |
| ElevenLabs | `xi-api-key` | multipart | `model_id`, `language_code` |
| AssemblyAI | `authorization` | upload → job → **poll** | async; suits imports better than dictation |

AssemblyAI is the odd one: it is asynchronous, so `transcribe` polls every
400 ms with a 120 s ceiling. That is a reason to offer it, not to hide it — but
it will feel slower than the others for live dictation.

### Capabilities are declared honestly

Deepgram streams in reality, but its adapter reports `streaming: false` because
Clide has no streaming path yet. Capabilities describe *what this adapter
implements*, not what the vendor's API can do. Keep it that way — the pipeline
trusts these flags.

## AD ASSETS (2026-09-03)

The user is making an advertisement. Delivered `clide-press-kit.html` — the
approved design system rendered as a screenshot source: wordmark + palette
lockup, the dashboard in a *ready* state with usable sample copy, and all four
HUD states. Kept as HTML rather than PNGs so any region can be captured at any
scale.

A live screenshot is not usable for advertising while the app reports "Setup
needed" and the transcript history reads "This is a simple test." That is
another reason to get the Groq key restored.

**Capture note:** `screencapture -o -x -l <window_id>` is the reliable path;
get the id from Quartz `CGWindowListCopyWindowInfo`.

## CURRENT SESSION — open-issue pass (2026-09-03)

### Milestone: issue 1 verification attempted, blocked on runtime prerequisites

The canonical `main` checkout is active and the existing bundled app was launched
with debug logging. It registered the configured `Cmd+Period` shortcut, but its
own readiness UI currently reports:

- Groq: API key needed
- Microphone: not granted
- Accessibility: not granted

Therefore the required native TextEdit check has **not** run yet. Do not treat
this as evidence for or against the `focusable: false` fix. Issue 1 remains open
until those user-owned prerequisites are restored and a real hold-to-talk run is
performed with focus in TextEdit.

Follow-up during the shader pass: the packaged app now reports both Microphone
and Accessibility as granted. The Groq credential is the only remaining
readiness blocker before the native TextEdit verification can run.

### Milestone: issue 2 fix implemented; first test invocation blocked by PATH

`settings::load` now deserializes `dictation.language` as the same
`Option<String>` shape that `settings::save` persists, so valid JSON `null` is
no longer treated as a corrupt string. A regression test covers automatic
language (`None`) save/reload. The first `cargo fmt --check` / focused-test
invocation did not run because this shell could not resolve `cargo`; verification
is pending a rerun with the installed toolchain's absolute path.

The absolute-path rerun reached Cargo, but repository-wide `cargo fmt --check`
reported pre-existing formatting drift across many unrelated Rust files and the
chained focused test did not start. No formatting rewrite was applied. Verify the
edited file separately and run the focused test as an independent command.

### Milestone: issue 2 focused regression suite passes

`cargo test --manifest-path src-tauri/Cargo.toml settings::` passes all 5
settings tests, including `automatic_language_round_trips_as_none`. The fix is
functionally verified at the unit boundary. Repository-wide rustfmt drift remains
outside this issue; the touched settings module is checked independently.

`rustfmt --edition 2021 --check src-tauri/src/settings/mod.rs` also passes after
normalizing one pre-existing assertion layout in that same module.

### Milestone: issue 2 debug build passes

`cargo build --manifest-path src-tauri/Cargo.toml` completed successfully with
the language deserialization fix. Runtime launch-log verification is next.

### Milestone: issue 2 closed by runtime verification

The rebuilt debug executable launched against the existing database row
`dictation.language = null`, registered `Cmd+Period`, and emitted no unreadable-
setting warning. Issue 2 is fixed at both the unit and live startup boundaries.

### Milestone: issue 3 source-of-truth decision

Code inspection confirmed that every status/read path already asks the
credential file directly, while SQLite's `provider_configs.credential_configured`
is only written as a mirror. The mirror will be removed rather than promoted:
the credential file remains the single source of credential truth, and
`provider_configs` will retain only per-provider model selection. A versioned
SQLite migration will preserve existing model rows while dropping the staleable
flag.

### Milestone: issue 3 storage rewrite implemented

The provider store no longer models or writes `credential_configured`.
Credential save/remove commands now mutate only the authoritative credential
store. Schema version 2 rebuilds `provider_configs` with `provider_id`,
`model_id`, and `updated_at`, copying existing model selections before dropping
the mirrored flag. Provider tests now cover model create/update, and a migration
test covers model preservation, column removal, and `user_version = 2`.

### Milestone: issue 3 focused tests pass

`database::providers` passes 2/2 tests and
`database::schema::v2_migration_preserves_models_and_drops_credential_mirror`
passes. The migration preserves the selected model, removes the credential
column, and advances the database version. A pre-existing rustfmt line-wrap in
the touched schema file was normalized; touched-file formatting is rerun next.

All four touched Rust files pass targeted `rustfmt --check`.

### Milestone: complete Rust verification passes

The full Rust suite passes: **92 passed, 1 ignored** (the intentionally manual
focused-app insertion test). `cargo clippy --all-targets -- -D warnings` is also
clean. No local-model work was started.

### Milestone: issue 3 debug build passes; first backup check fails safely

The post-change debug build completed. Before migrating the live database, the
first attempt to create/read a SQLite-consistent temporary backup failed with
SQLite error 14. The rebuilt app was not launched, so the live database remains
at schema version 1 and was not modified by this attempt. Backup creation must
succeed before live migration verification proceeds.

Follow-up inspection confirmed the backup was created successfully at
`/tmp/clide-schema-v2.qw48Wr/clide.sqlite3`; the error came from the CLI's
`-readonly` open mode on this WAL-mode copy. A normal SQLite open reports
`integrity_check = ok`, schema version 1, and all 3 transcripts. The safety gate
is satisfied; the backup is retained through live migration verification.

### Milestone: issue 3 closed by live migration verification

Launching the rebuilt app migrated the existing database from version 1 to 2.
Post-launch `PRAGMA integrity_check` returns `ok`; all 3 transcript rows remain;
and `provider_configs` now contains only `provider_id`, `model_id`, and
`updated_at`. The credential mirror is gone from both code and live storage.

### Milestone: first debug app-bundle build invocation fails on shell PATH

`npm run app:build -- --debug` reached Tauri but could not spawn
`cargo metadata` because Cargo is absent from this shell's PATH. No bundle was
replaced. Rerun after loading the installed Rust environment; this is the same
tool-discovery issue seen during the first focused test invocation.

### Milestone: updated debug bundles pass

After loading `/Users/brenomenezes/.cargo/env`,
`npm run app:build -- --debug` completed successfully. TypeScript type-checking
and the Vite production build passed, Rust compiled, and Tauri produced both
`src-tauri/target/debug/bundle/macos/clide.app` and the arm64 debug DMG.

### Milestone: packaged-app launch passes

The rebuilt bundled `clide.app` launched against the migrated live database,
registered `Cmd+Period`, and emitted neither the old language warning nor a
schema error. Issue 1 is still blocked by the readiness prerequisites recorded
above; no TextEdit result is claimed.

### Milestone: final scope audit

The working tree changes are limited to the issue-2 settings fix/test, the
issue-3 provider-store migration/tests, and this handoff. `git diff --check`
passes. The dictation pipeline was not re-architected, no local-model work was
started, and the packaged app is left open on `C27F398` (the 60 Hz second
display) for the remaining user-owned readiness steps.

## CURRENT SESSION — website shader adaptation (2026-09-03)

### Milestone: inactive-state follow-up implemented

The shader now gives inactive dictation a distinct non-blue visual: an
asymmetric fan of pearlescent contour shells, designed as a listening aperture
pressed into paper. It breathes slowly at Normal/High, stays static in Reduced,
and recedes as `u_presence` rises so the existing blue voice field takes over.
This preserves the blueprint's “blue means voice” rule while making the idle
canvas deliberately visible instead of nearly flat.

### Milestone: inactive-state frontend build passes

`npm run build` passes TypeScript checking and the Vite production build with
the new idle aperture. `git diff --check` is clean. Shader compile/link and
visual comparison of inactive versus active remain pending.

### Milestone: inactive/active shader comparison passes

A disposable two-panel Vite harness imported the production shader and rendered
Inactive/Normal beside Active/High with mic energy. Both WebGL1 programs compiled
and linked. The first idle treatment was rejected as too faint; the final pass
uses paired warm-gray shadow and porcelain highlight ridges, making the
asymmetric fan clearly visible while remaining non-blue. The active panel stays
cyan/blue and contains no idle fan. All disposable preview files were removed.

### Milestone: inactive-state app and DMG bundles pass

`npm run app:build -- --debug` completed with the Rust environment loaded.
TypeScript, Vite, Rust compilation, `clide.app`, and the arm64 debug DMG all
pass with the final idle-state shader. Packaged-app inspection on `C27F398`
remains pending.

### Milestone: first packaged idle layout rejected and corrected

The packaged dashboard was inspected on `C27F398`. The embossed fan rendered,
but its upper-right focal point sat mostly behind the opaque bento cards. The
aperture is now anchored at normalized y `0.18`, placing it in the dashboard's
open lower-right canvas. A replacement bundle and second packaged visual check
are pending; do not treat the first bundle as final.

### Milestone: inactive-state shader passes packaged visual QA

The replacement bundle was launched and moved to `C27F398` (60 Hz). The idle
aperture is now clearly visible in the open lower-right portion of the real
dashboard: warm-gray recessed arcs with porcelain highlights, without blue or
text interference. Cards, controls, and the large empty reading area remain
crisp. The final app and DMG bundles pass, and the packaged app is left open on
the second display. The shader follow-up is complete.

### Milestone: source inspection and rendering decision

The shader source in `staraepp/Clide-website` commit
`6514b1a9300e2845b62a2c4949e09d11c6f49b3d` was inspected directly. Its hero
uses a two-pass WebGL2 pointer flowmap, simplex-noise fluid field, bloom, grain,
and a separate elastic grid.

Clide will adapt the recognizable simplex/domain-warped aurora and oceanic
palette into its existing one-pass fullscreen-triangle renderer. The website's
pointer flowmap, bloom, grain, and grid are deliberately excluded: they add
continuous utility-app GPU cost and would weaken the sharp light UI. The field
will remain pearl-neutral at rest, gather into voice blue only while dictation
is active, react to mic energy only on High, and retain the existing Reduced /
Normal / High caps plus blur/visibility pausing.

### Milestone: adapted shader implemented

`src/shaders/shader.glsl.ts` now uses the website shader's 3D simplex-noise
family and two-stage domain warping to form fluid contour ribbons. It remains a
single WebGL1 pass with no textures or framebuffers. Neutral pearl/mist colors
render at rest; the website's `#E4F3FB`, `#8BD7F2`, and `#2F9CD4` palette mixes
in only through eased dictation presence, with High-mode mic energy widening
and brightening the ribbons. `ShaderBackground`'s existing frame caps, Reduce
Motion override, DPR cap, and blur/visibility pausing are unchanged.

### Milestone: standalone WebGL compile/render QA passes

A disposable Vite page imported the real shader module and rendered 960x720
idle and active frames in Chromium WebGL1. The program compiled and linked;
idle remained pearl-neutral and the active High / energy 0.65 frame produced
the intended fluid cyan/blue ribbons. The only console error was the disposable
page's missing favicon. The preview harness is removed before packaging.

### Milestone: production frontend build passes

`npm run build` passes TypeScript checking and the Vite production build with
the adapted shader bundled into the main entry. `git diff --check` is clean.

### Milestone: updated app and DMG bundles pass

`npm run app:build -- --debug` completed with the Rust toolchain environment
loaded. Tauri produced the updated `clide.app` and arm64 debug DMG with the new
shader bundled.

### Milestone: packaged dashboard visual QA passes on the second display

The rebuilt app launched and was moved to `C27F398` (60 Hz) before visual
inspection. The live dashboard keeps cards, typography, and controls crisp over
the subtle pearl resting field; no clipping, overlap, or blank WebGL canvas was
observed. The stronger voice-blue active state remains covered by the standalone
WebGL render check because starting microphone capture without an explicit test
utterance was avoided.

### Milestone: shader scope audit passes

`git diff --check` passes after removing the disposable Playwright preview logs.
The final shader change is isolated to `src/shaders/shader.glsl.ts`; the existing
React renderer, dictation pipeline, and local-model plan were not changed. The
rebuilt packaged app is left open on `C27F398` for user inspection.

---

## ACTIVE WORK — light retheme (approved 2026-09-03)

The dark blue-black v1 was rejected: *"giving 2010 trying to be edgy."* The user
asked for light, SaaS, low-contrast, harmonious (Notion / Whispr Flow as touchstones).

**The identity is not ours to invent — it already exists.** The marketing site
served at `localhost:3000` (a Next.js project elsewhere on this machine) defines
it. Read from the live page:

| | |
|---|---|
| Ground | `#F4F9FD` |
| Ink | `#0A2338` (deep navy — no black anywhere) |
| Display | **Montserrat 500** (medium, not bold) |
| Body | **DM Sans** |
| Wordmark | lowercase `clide` + the 5-bar waveform mark |
| Shortcut | **⌥ .** — *the site says this; the build defaulted to ⌥ Space* |

Approved mockup: https://claude.ai/code/artifact/2850ebb1-aff7-4cc4-91b1-4d80822c3cee

### The design rules that came out of it

1. **Blue means voice.** `--color-voice` appears only where clide is hearing or
   handling speech: the waveform, the listening state, the caret. Never on
   buttons, links, or headings. The primary button is ink. This is what keeps the
   contrast low — a blue appearing somewhere new is a bug, not a decoration.
2. **The ribbon is the signature.** One element in the hero card that morphs
   prompt -> live meter -> typed transcript -> landed. Everything around it stays
   quiet so it carries the screen.
3. **Removed for good:** gradients, glow, grain, 22px radii, every shadow but one,
   the dark shader. That was the "2010 edge."

### Retheme checklist

- [x] Self-hosted Montserrat + DM Sans + DM Mono as woff2 in `src/styles/fonts/`
      (88 KB total, latin subset). Bundled by Vite; the app stays offline.
- [x] `src/styles/theme.css` rewritten. Tokens are now `paper / card / sunken /
      ink / ink-2 / ink-3 / line / line-2 / voice / voice-deep / voice-tint`
      plus `ok / warn / stop`. Tailwind v4 `@theme` generates the utilities, so
      components use `text-ink` and `border-line`, not the old
      `text-[--color-ice-50]` arbitrary syntax.
- [x] Every component moved off the old tokens (verified by grep — zero
      `ice`/`abyss`/`hairline`/`grain` references remain)
- [x] `shaders/shader.glsl.ts` reskinned. New `u_presence` uniform, eased in the
      render loop, so the wash gathers while dictating and recedes after.
- [x] `DEFAULT_SHORTCUT` -> `Alt+Period`
- [x] `tauri.conf.json` — `productName` and window title are now lowercase
      `clide`, `backgroundColor` `#F4F9FD`
- [x] `blueprint.md` §19 rewritten around the light system and the colour rule
- [x] Rebuilt, launched, screenshotted — the light theme renders correctly:
      Montserrat headings, DM Sans body, `⌥ .` keycaps, pale wash background

## THE MILESTONE IS MET

The user ran real dictations on 2026-09-03 while this session was building.
From the app's own log:

```
microphone open  sample_rate=24000 channels=2 sample_format=F32
transcription complete  provider=groq model=whisper-large-v3-turbo
                        latency_ms=388 characters=23
transcript saved
accessibility insertion declined; pasting
```

Two runs, 388 ms and 345 ms end to end. Text "This is a simple test." — correct,
and Polished capitalised it and closed the sentence. Transcripts landed in
SQLite. **shortcut -> capture -> transcribe -> process -> insert -> history
works.**

## OPEN ISSUES AND RESOLUTIONS, in priority order

### 1. Insertion always falls back to clipboard paste (unverified fix in place)

Both runs logged a fallback, for two different reasons:

- `nothing is focused to type into` — no focused AX element at all. This is
  what a focus-stealing HUD looks like.
- `the focused control does not accept direct text` — expected and correct;
  the target was clide's own WKWebView textarea, and web views refuse direct
  `AXSelectedText` writes.

**Fix applied but NOT yet confirmed:** the HUD window now declares
`"focusable": false` in `tauri.conf.json`. tao overrides `canBecomeKeyWindow`
and `canBecomeMainWindow` from that flag
(`tao-0.35.3/src/platform_impl/macos/window.rs:417-432`), so no `tauri-nspanel`
dependency is needed after all.

**How to verify:** focus TextEdit (a *native* control, not a web view), hold
`⌥ .`, speak. The log should show neither fallback reason and the text should
arrive without the clipboard being touched.

**Watch for a side effect:** a non-key window may not deliver clicks to its
webview. If the HUD's error-state Retry/Copy buttons stop responding, do not
undo `focusable: false` — dictation reliability outranks HUD affordances
(AGENTS.md decision priority). Move recovery to the dashboard and tray instead.

### 2. [FIXED 2026-09-03] `dictation.language` warned on every launch

`settings::load` now reads `Option<String>` consistently with the shape written
by `settings::save`. Unit coverage passes and the rebuilt packaged app launches
against the existing `null` row without the warning.

### 3. [FIXED 2026-09-03] `provider_configs` credential state could drift

Schema v2 removes `credential_configured` from SQLite. The dedicated credential
file is now the only source of credential truth; `provider_configs` stores only
per-provider model selection and its update timestamp.

## DO NOT REPEAT

This session ran `rm -rf ~/Library/Application Support/com.staraep.clide` to get
a clean onboarding run, while the user was actively testing. It destroyed their
history and settings. The Keychain entry survived (different store), which is
why transcription kept working. **Do not wipe app data without asking.**

### New in this pass

- **`src/dashboard/Ribbon.tsx`** — the signature element, extracted so the hero
  card stays readable. One surface that changes what it holds (prompt -> live
  meter -> transcript -> failure reason) rather than four swapped panels.
- **Wordmark is lowercase** across UI copy, tray menu, and `productName`. Note
  this renames the bundle to `clide.app`, which resets TCC — permissions must be
  granted again after this build.
- `lib/format.ts` gained `clockTime()` and punctuation-key glyphs, so `⌥ .`
  renders as a full stop rather than the word "Period".

### Also worth telling the user

The site's providers section says *"the providers layer is a single Swift file."*
The app is Rust/Tauri. The site copy is wrong, not the app.

## Status

| Area | State |
|---|---|
| Scaffold (Vite, Tailwind v4, Tauri 2, icons, two window entries) | done |
| Rust: state machine, audio, providers, processing | done, unit-tested |
| Rust: insertion (AX + clipboard), keychain, permissions | done |
| Rust: SQLite + FTS5, settings, shortcuts, HUD, tray, commands | done |
| `cargo test` | **92 passing, 1 ignored** |
| `cargo clippy --all-targets` | **clean** |
| Frontend: design system, shader, HUD, dashboard, history, settings, onboarding | done |
| `tsc --noEmit` + `vite build` | **clean** |
| Bundled `.app` builds, launches, registers `Cmd+Period`, opens schema v2 | **verified** |
| Live DB: schema, FTS5 match, index follows deletes | **verified by direct query** |
| Native TextEdit AX run without clipboard fallback | **BLOCKED: Groq key required** |

### Why the end-to-end run is still open

Microphone and Accessibility are now granted. The only remaining prerequisite
is a Groq API key in the current credential store.

The user has now authorized Computer Use, but only on the second 60 Hz display:
`C27F398`. The main display is `ASUS VG277Q1A` at 165 Hz and must not be used for
verification. The packaged Clide window is currently back on `C27F398`.

## Next steps, in order

1. Enter the Groq key in the bundled app.
2. Then the blueprint's actual milestone: focus TextEdit on the second, 60 Hz
   monitor, hold `Cmd+Period`,
   speak, confirm text appears **in TextEdit** and a row appears in History.
3. Watch specifically for:
   - **HUD focus stealing.** It is created with `focused: false` and `show()` is
     never paired with `set_focus()`. If macOS still pulls focus off the text
     field, the fix is `tauri-nspanel` (a genuinely non-activating panel) — that
     is the known escape hatch, not more `set_ignore_cursor_events` fiddling.
   - **AX insertion into the target.** `kAXSelectedTextAttribute` works in
     native text controls; web views and canvas editors will fall through to the
     clipboard path. Both are correct outcomes — check the HUD says which.
4. Only once that is reliable: polish, then v0.2 scope from the blueprint.

## Decisions already made (do not re-litigate)

- **Audio format:** 16 kHz mono s16 WAV. Groq accepts WAV directly, so there is
  no transcode step. Box-filter decimation in `audio/resample.rs`.
- **Audio buffering:** accumulated in memory, written to WAV on stop. Dictation
  clips are short; keeps file I/O out of the realtime callback. Hard cap 10 min.
- **`cpal::Stream` is `!Send`** on macOS → it lives on a dedicated worker thread
  (`audio/recorder.rs`), driven by a command channel.
- **Temp audio** is owned by `RecordedClip`, which deletes the file on `Drop`.
  120 s recovery window for explicit retry; orphans swept at startup.
- **Retry semantics:** `retryable` in `TranscriptionFailed` means *the audio is
  still on disk*, not that the error was transient. Clide never auto-retries and
  never silently switches provider.
- **Transcript is persisted to SQLite *before* insertion** so a failed insertion
  can never lose a good transcript.
- **Two Vite entries:** `index.html` (dashboard) + `hud.html` (HUD), so the HUD
  stays cheap.
- **Default shortcut:** `Alt+Period`, behaviour configurable Hold vs Toggle,
  one shortcut only.
- **Rewrite mode** is declared in the enum but returns `ModeUnavailable` — never
  silently falls back to Polished.
- **`NSPasteboard` is not thread-safe.** Its internal type cache corrupts under
  concurrent access and crashes inside AppKit (found via a SIGSEGV in the test
  suite). All pasteboard access goes through `clipboard::access()`, which holds
  a process-wide lock; `insert_via_paste` holds it across the whole
  borrow-paste-restore sequence so a concurrent Copy cannot interleave.
  There is a regression test (`concurrent_access_is_serialised`).
- **Mic level reaches the UI as a ref, not React state** (`useMicLevel`), and
  `Waveform` reads it in its own rAF loop. At ~30 updates/s, state would
  re-render the dashboard on every audio frame.
- **Shader throttling** is in `ShaderBackground`: fps capped per intensity
  (reduced = one static frame, normal = 30, high = 60), stopped entirely on
  blur/hidden, and system Reduce Motion overrides the app setting.
- **`DictationState`'s JSON shape is a contract** with the UI, pinned by the
  `wire_format` test in `dictation/machine.rs`.

## Environment / gotchas

- Repo path contains a space and an unbalanced `(`. Cargo verified fine; if
  `.app` bundling or codesign chokes, relocate the repo rather than fighting it.
- macOS 26.6.2, arm64, Rust 1.94, Node 26, Xcode at `/Applications/Xcode.app`.
- Ad-hoc-signed dev builds can reset TCC identity → macOS may re-prompt for
  microphone and Accessibility. Credentials no longer use the Keychain; see the
  explicit deviation below.
- **Testing display:** use `C27F398` (60 Hz), the user's second monitor, for all
  on-screen verification. `ASUS VG277Q1A` is the 165 Hz main display; do not
  screenshot or interact with it.
- Groq API key is the user's BYOK credential. The agent does not have one; the
  Groq leg of the pipeline can only be validated after the user enters it.

## Where things live

```text
src-tauri/src/
  dictation/machine.rs   the state machine — every state change goes through it
  dictation/pipeline.rs  the transaction, end to end. Start reading here.
  audio/recorder.rs      cpal on a worker thread; 16 kHz mono WAV
  insertion/mod.rs       AX first, clipboard paste second, clipboard-only last
  providers/traits.rs    the adapter contract new backends implement
  providers/groq/        the only backend in this build
src/
  lib/commands.ts        every IPC call, in one file — the contract with Rust
  lib/types.ts           mirrors the serde shapes; DictationState is a union
  dictation/labels.ts    state → words + colour, shared by HUD/dashboard/tray
  shaders/               the ambient background and its throttling rules
```

## Commands

```bash
npm install                     # frontend deps
cargo test --manifest-path src-tauri/Cargo.toml
npm run app                     # tauri dev (fast loop, weak TCC identity)
npm run app:build -- --debug    # bundled .app — use this for permission testing
```


---

# END-OF-SESSION STATE (2026-09-03)

## Shipped this session

1. **Light retheme**, matched to the marketing site. Built, launched, verified
   on screen. See "ACTIVE WORK" above for the tokens and the colour rule.
2. **Credentials moved off the macOS Keychain** — see below.
3. **HUD `focusable: false`** — applied, still unverified against a native app.

## Credentials: why this deviates from the blueprint

`blueprint.md` §14 and `AGENTS.md` both say API keys go in the macOS Keychain.
**They no longer do.** The user instructed this explicitly, and AGENTS.md allows
a blueprint deviation on an explicit instruction.

**Their reason was real.** An ad-hoc-signed build gets a new code identity on
every rebuild, so macOS sees a different application asking for the same
Keychain item and re-prompts for authorisation on every launch. That made the
app unusable during development.

**What replaced it:** `src-tauri/src/credentials/mod.rs` — a JSON file at
`~/Library/Application Support/com.staraep.clide/credentials.json`, mode `0600`
inside a `0700` directory. `security-framework` was dropped from `Cargo.toml`.

**What this costs, stated plainly:** the key is now **plaintext on disk**,
protected only by file permissions. Any process running as this user can read
it. That is strictly weaker than the Keychain.

**What is unchanged:** the key still never reaches SQLite, frontend state, logs,
or any error message. The UI still only learns `configured: true`. There is a
test asserting the file mode is `0600` and tests asserting no error variant can
carry a secret.

**How to undo it properly:** ship a build signed with a stable Developer ID
certificate. The identity stops changing between builds, the repeated prompts
disappear, and `credentials/mod.rs` can go back to `security-framework` generic
passwords behind the exact same public interface (`store` / `read` / `delete` /
`is_configured`). Nothing else in the app would change.

**Migration note:** any key the user stored in the *old* Keychain build is still
in their login Keychain and is no longer read. They need to paste the key once
more in Settings -> Transcription. There is no automatic migration, deliberately
— reading the old item would have triggered exactly the prompt this change
removes.

## ACTIVE WORK — local models (Parakeet, local Whisper)

### 2026-09-03 milestone: runtime proof started outside Clide

The user explicitly requested local models, so the earlier “do not start unless
asked” gate is cleared. The first candidate is Parakeet TDT 0.6B v3 int8 ONNX
(~670 MB of model artifacts) through `screenpipe/audiopipe` commit
`b04ec0d7fee7e3163d76fdcff24fb758dec7c729`. This is a Rust-only path with no
Python runtime. A disposable `/tmp` checkout compiled the library, but its
example failed because upstream renamed the crate to `audiopipe` without
updating four `stt::` references. Those references are patched only in `/tmp`;
no Clide dependency or source has been changed. The real transcription proof is
being rerun against whisper.cpp's 16 kHz mono JFK sample.

The architecture is already ready for it. `TranscriptionProvider`
(`providers/traits.rs`) has `local` in its `Capabilities`, and the pipeline
branches on capabilities rather than provider ids, so a local engine plugs in
without the dictation code changing.

**Implementation plan, in order:**

1. **Pick the runtime first, and prove it outside the app.** For Parakeet on
   Apple Silicon the realistic options are an ONNX Runtime build with CoreML
   execution, or `candle` with a converted checkpoint. Write a throwaway binary
   that transcribes a fixed 16 kHz mono WAV and prints text. Do not touch clide
   until that binary works — this step is where the real risk is, and it is
   entirely independent of the app.
2. **Add `providers/local/mod.rs`** implementing `TranscriptionProvider` with
   `capabilities().local = true` and `CredentialRequirement::None`. The audio
   clip handed to it is already 16 kHz mono WAV, which is what these models
   want, so no conversion is needed.
3. **Model storage.** `~/Library/Application Support/com.staraep.clide/models/`,
   one directory per model id. Add a `local_models` table (the blueprint's §26
   list already names it) recording id, version, size, installed-at.
4. **Download manager.** A `models/` module in Rust: fetch with progress events
   (`model:progress`, `model:complete`, `model:failed` — mirror the existing
   event naming), verify a checksum, unpack, record in SQLite. The UI for it is
   a list, not a browser — blueprint §13 is explicit that users must never be
   asked for a file path.
5. **Only then** surface it in the provider UI. `ProviderSettings.tsx` already
   renders from capabilities, so a local provider will appear with a "Local"
   chip and no API-key field automatically.

Do not start at step 5.

## Known issues / resolutions from this pass

1. **HUD focus fix unverified.** Focus TextEdit — a *native* control, not a web
   view — hold the shortcut, speak. The log should show no
   "accessibility insertion declined" line at all. Watch for the HUD's
   error-state buttons going unclickable as a side effect; if they do, move
   recovery to the dashboard rather than reverting `focusable: false`.
2. **RESOLVED: `dictation.language` launch warning.** The load/save shapes now
   match; unit and packaged-launch verification pass.
3. **RESOLVED: `provider_configs` credential drift.** Schema v2 removed the
   mirrored flag and live migration preserved all transcript rows.

## Test status at end of session

```
cargo test    91 passed, 1 ignored     (the ignored one types into the focused app)
tsc --noEmit  clean
vite build    clean
cargo clippy  clean as of the retheme commit
```

The user changed their shortcut to `Cmd+Period` at runtime; the default in code
is `Alt+Period`, matching the marketing site.


---

# TRAPS — things that already cost a session

Every one of these was hit, diagnosed, and fixed. They all look like something
else, so read before debugging any of them again.

### `-webkit-app-region: drag` is ignored by this webview

It is present in the built CSS and does nothing; only the native overlay strip
was draggable, so the window felt stuck. The title bar uses
**`data-tauri-drag-region`** instead. Do not "simplify" it back.

### Never infer Accessibility trust from a generic AX call

The old probe treated any error other than `kAXErrorAPIDisabled` as proof of
trust. That is false: a different AX failure can occur while macOS still drops
synthetic input. Use `AXIsProcessTrusted()` directly and relaunch after changing
the permission if macOS does not refresh it immediately.

### `reqwest`'s `timeout` covers the response body

A 90-second client timeout is not "90 s to respond", it is "90 s to finish
downloading", so every model download died mid-stream with an opaque
"error decoding response body". Downloads use a **separate client with no total
timeout**, bounded by `connect_timeout` and `read_timeout` instead. **Never add
`.timeout()` to that client.**

### THE INSERTION BUG — Accessibility lied about succeeding (FIXED, VERIFIED)

**Confirmed working 2026-09-04** against the Claude app, the input that had
never accepted a transcript:

```
inserting  target_app="Claude"  frontmost="Claude"
accessibility insertion declined; typing
  reason="the control reported success but its text did not change"
```

The verification caught the false success, the chain fell through, and typing
delivered the text. ~280-340 ms end to end.


Three fixes missed because the diagnosis was wrong. What the log finally showed:

```
inserting  target_app="Claude"  target_pid=33170  frontmost="Claude"
<nothing further>
```

The right app was frontmost, and `insert_via_accessibility` returned `Ok` — so
it returned early and **the typing and paste fallbacks were never reached**.
The Accessibility write was reporting success and doing nothing.

Confirmed against the live app with System Events:

```
role=AXTextArea   selSettable=true   AXValue exists
```

Electron/Chromium text areas report `AXSelectedText` as settable, accept the
write, return `kAXErrorSuccess`, and change nothing. The old guard —
"ask `is_settable` first to avoid a silent no-op" — cannot work, because the
app lies.

**The fix: verify, do not trust.** `insert_via_accessibility` reads `AXValue`
before and after and treats "unchanged" as a refusal, which lets the fallback
chain actually run.

Deliberate limit: verification only happens when the control exposes
`AXValue`. Without it there is no evidence either way, and claiming failure
would insert the transcript **twice** — once by Accessibility and again by
typing. Keep that asymmetry in mind before "tightening" this.

**Debugging lesson:** the log line naming target vs frontmost app is what
solved it. Do not remove it.

### Insertion: paste is now the LAST resort, not the first fallback

The Claude app took neither the Accessibility write nor a synthetic paste. The
order is now:

1. Accessibility write (native fields)
2. **Unicode typing** — `clipboard::type_text`
3. Clipboard paste (last resort)

**Why typing works where paste does not.** Cmd+V is a *command*: the target app
must recognise the chord, route it through its menu system, and choose to read
the pasteboard. Electron/Chromium decide that on their own terms and reject
synthetic chords that do not match what they expect.
`CGEventKeyboardSetUnicodeString` attaches the text to a keystroke with no
keycode and no modifiers, so the app receives ordinary text input — there is no
chord left to refuse.

Chunked at 20 **characters** (never bytes — a byte split would corrupt
multi-byte text; there is a test).

`insert()` also now logs the intended target app alongside the actual frontmost
app, because when insertion silently misses, the first question is always
whether the right app was still in front.

### Earlier fix, kept: paste reached some inputs and not others

Reported as: pastes into Firefox's search field, does nothing in a
Chromium-based text input. Two causes, both in `send_paste_keystroke`:

1. **It synthesized the Command *key*** (0x37) as its own down/up pair around
   the V. Chromium tracks `flagsChanged` on a separate timeline and often had
   not applied the modifier by the time the V landed, so it saw a bare "v".
   AppKit fields read the flag straight off the event and never noticed — which
   is exactly why it looked app-specific. Now only V is posted, with Command as
   a flag.
2. **The event source was `CombinedSessionState`**, which unions in the *real*
   hardware modifier state. With a hold-to-talk shortcut like `Cmd+.` the user
   is often still holding keys when the transcript lands, so the app received
   `Cmd+Opt+V` or similar and ignored it. The source is now `Private`, and
   `wait_for_modifiers_to_clear()` gives the keyboard up to 600 ms to settle —
   bounded, so a genuinely stuck modifier degrades to a late paste, never none.

**Testing note for whoever changes this next:** a `cargo test` binary is not
the signed app, so it has **no Accessibility grant** — it cannot post events or
read focus, and `paste_lands_in_the_focused_control` will fail with "nothing is
focused" no matter what the code does. That is why the chord's *shape* is
extracted into `paste_chord()` and unit-tested instead. Verify behaviour by
running the bundled app and dictating.

### RESOLVED — the DMG does *not* need Xcode (I was wrong)

I raised this as a shipping blocker. **It is not one.** Verified by downloading
the published `v0.1.2` DMG and inspecting the binary inside it:

```
otool -L  ->  /usr/lib/swift/libswift_Concurrency.dylib   (absolute)
otool -l  ->  LC_RPATH: /Applications/Xcode.app/...       (present, unused)
```

The dependency resolves **absolutely**, from the dyld shared cache, which
exists on every macOS 26 machine. The Xcode rpath is baked in but never
consulted. A plain `cargo build --release` binary *does* emit `@rpath/...` —
that is what misled me — but the **bundled** app does not.

Do not spend time "fixing" this. If it is ever revisited, verify against the
bundled app in the DMG, never a bare cargo build.

### Historical note — the rpath is still required at build time

**Measured, not assumed.** The release binary links
`@rpath/libswift_Concurrency.dylib`, and the only rpath is
`/Applications/Xcode.app/.../swift-5.5/macosx`. On a Mac without Xcode at that
exact path, **the app aborts at launch** — so the DMG cannot be handed to users
as-is.

Verified along the way:
- `FoundationModels.framework` *is* weakly linked, so macOS 26 is not required
  just to start. That part is fine.
- Gating the rpath to debug builds does **not** work: a release build without
  it still requests `@rpath/...` and has nowhere to resolve it. This was tried
  and reverted — do not try it again.
- `MACOSX_DEPLOYMENT_TARGET=26.0` does **not** stop the shim back-deploying.
  Also tried and reverted; it silently raises the OS minimum for no benefit.

**The fix** is to bundle the runtime: copy
`libswift_Concurrency.dylib` into `clide.app/Contents/Frameworks/` and add
`@loader_path/../Frameworks` as an rpath, then re-sign. Do this before cutting
a release.

### `foundation-models` needs a Swift rpath

It compiles a Swift shim linking `libswift_Concurrency.dylib`, which on this
SDK exists only in Xcode's back-deployment directory. Without the rpath the
binary links fine and **aborts at launch** with a dyld error naming fifty paths
and no cause. `build.rs::link_swift_runtime()` adds it. Do not remove it.

### `DmgConfig` lives under `bundle.macOS.dmg`

Not `bundle.dmg`. The error message does not say so.

### The build is development-signed, not Developer ID-notarized

macOS keys the grant to code identity. v0.1.1 is signed with the available Apple
Development certificate, which is stable enough for local permission testing.
It still fails `spctl` public assessment because only Developer ID signing plus
notarization removes the public Gatekeeper warning. Do not call the current DMG
notarized or generally trusted.

### `app_screenshot` returns a blank window for this app

The accelerated WebGL layer defeats that capture path. Use
`screencapture -o -x -l <window_id>`, with the id from Quartz
`CGWindowListCopyWindowInfo`. **A blank `app_screenshot` is not a broken UI.**

---

# DESIGN — the rules, and why

The user rejected two earlier directions ("2010 trying to be edgy", then "too
generic"). What survived:

1. **The identity is not ours to invent.** It comes from the marketing site:
   paper `#F4F9FD`, navy ink `#0A2338` (no black anywhere), Montserrat 500,
   DM Sans, lowercase `clide`, the five-bar mark. Fonts are bundled as woff2
   because the app is offline and its CSP admits no remote font host.
2. **Blue means voice.** `--color-voice` appears only where clide is hearing or
   handling speech. Buttons, links and headings are ink. A blue anywhere else
   is a bug, not a decoration.
3. **The ribbon is the signature**, and the wordmark is a **live level meter**
   while dictating. That is the one place clide's identity and its function are
   the same object.
4. **Grids, not lists.** Settings is a 12-column bento, not a stack of
   full-width cards; Setup, refine engines and cloud models are all card grids.
   The user asked for this specifically.
5. **The shader is only ever seen in the gutters** — every card is opaque
   white. It was tuned for a readability problem that does not exist; do not
   re-throttle it.
6. **No invented statistics.** The usage card counts real rows. Model star
   ratings derive from the model's declared class and this Mac's measured chip
   and memory — there is no popularity score, because clide has no telemetry
   and could not know one. The page says so on screen.

# NOT BUILT

Imports (blueprint §17), per-app profiles (§8), the context system (§9),
streaming transcription, and history/models UI polish. `blueprint.md` remains
the product truth for all of it.
