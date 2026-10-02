//! Clipboard access and the synthetic paste used as the insertion fallback.
//!
//! Every completed transcript is copied to the clipboard and left there. This
//! gives the user a reliable recovery path and prevents a slow target app from
//! missing a paste because Clide restored the previous contents too early.
//!
//! **All pasteboard access is serialised.** `NSPasteboard` maintains an
//! internal type cache that is not thread-safe: concurrent `types` calls
//! corrupt it and crash inside AppKit. Clide reaches the clipboard from the
//! insertion worker and from the Copy command, so every entry point goes
//! through [`access`], and the borrow-paste-restore sequence holds the lock
//! for its whole duration so nothing can interleave with it.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Virtual key codes (Carbon `kVK_*`), stable across keyboard layouts.
const KEY_V: u16 = 0x09;
const PASTEBOARD_SETTLE_DELAY: Duration = Duration::from_millis(50);
const KEY_EVENT_DELAY: Duration = Duration::from_millis(12);
/// How long to let the user finish releasing a hold-to-talk shortcut.
const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(600);
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(15);
/// A single event carrying a very long string is truncated by some editors.
const TYPING_CHUNK_CHARS: usize = 20;
const TYPING_CHUNK_DELAY: Duration = Duration::from_millis(4);

static PASTEBOARD: Mutex<()> = Mutex::new(());

/// Proof that the caller holds the pasteboard lock.
///
/// Every operation hangs off this token, which makes it impossible to touch
/// `NSPasteboard` without having serialised first.
pub struct Pasteboard {
    _guard: MutexGuard<'static, ()>,
}

/// Run `work` with exclusive access to the system pasteboard.
pub fn access<R>(work: impl FnOnce(&Pasteboard) -> R) -> R {
    let guard = PASTEBOARD.lock().unwrap_or_else(|e| e.into_inner());
    let pasteboard = Pasteboard { _guard: guard };
    work(&pasteboard)
}

/// Everything that was on the pasteboard, by UTI.
pub struct ClipboardSnapshot {
    items: Vec<(String, Vec<u8>)>,
}

impl ClipboardSnapshot {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }
}

/// NSString -> String via the CFString toll-free bridge.
unsafe fn ns_string_to_rust(ns: *mut AnyObject) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(ns as CFStringRef).to_string())
}

/// String -> NSString, likewise.
fn ns(value: &str) -> CFString {
    CFString::new(value)
}

unsafe fn general_pasteboard() -> *mut AnyObject {
    msg_send![class!(NSPasteboard), generalPasteboard]
}

impl Pasteboard {
    /// Capture the current contents so they can be restored.
    pub fn snapshot(&self) -> ClipboardSnapshot {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return ClipboardSnapshot::empty();
            }

            let types: *mut AnyObject = msg_send![pasteboard, types];
            if types.is_null() {
                return ClipboardSnapshot::empty();
            }

            let count: usize = msg_send![types, count];
            let mut items = Vec::with_capacity(count);

            for index in 0..count {
                let uti: *mut AnyObject = msg_send![types, objectAtIndex: index];
                let Some(uti_string) = ns_string_to_rust(uti) else {
                    continue;
                };

                let data: *mut AnyObject = msg_send![pasteboard, dataForType: uti];
                if data.is_null() {
                    // Promised data the owning app has not produced. It cannot
                    // be restored, so it is not worth capturing.
                    continue;
                }

                let length: usize = msg_send![data, length];
                let bytes: *const c_void = msg_send![data, bytes];
                if bytes.is_null() || length == 0 {
                    continue;
                }
                items.push((
                    uti_string,
                    std::slice::from_raw_parts(bytes as *const u8, length).to_vec(),
                ));
            }

            ClipboardSnapshot { items }
        })
    }

    /// Put a snapshot back, replacing whatever is on the pasteboard now.
    pub fn restore(&self, snapshot: ClipboardSnapshot) {
        if snapshot.is_empty() {
            return;
        }
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return;
            }
            let _: isize = msg_send![pasteboard, clearContents];

            for (uti, bytes) in &snapshot.items {
                let uti_string = ns(uti);
                let data: *mut AnyObject = msg_send![
                    class!(NSData),
                    dataWithBytes: bytes.as_ptr() as *const c_void,
                    length: bytes.len()
                ];
                if data.is_null() {
                    continue;
                }
                let _: bool = msg_send![
                    pasteboard,
                    setData: data,
                    forType: uti_string.as_concrete_TypeRef() as *mut AnyObject
                ];
            }
        });
    }

    /// Replace the pasteboard with a single plain-text item.
    pub fn set_text(&self, text: &str) -> bool {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return false;
            }
            let _: isize = msg_send![pasteboard, clearContents];

            let value = ns(text);
            let uti = ns("public.utf8-plain-text");
            msg_send![
                pasteboard,
                setString: value.as_concrete_TypeRef() as *mut AnyObject,
                forType: uti.as_concrete_TypeRef() as *mut AnyObject
            ]
        })
    }

    /// Read the plain-text representation, if there is one.
    pub fn text(&self) -> Option<String> {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return None;
            }
            let uti = ns("public.utf8-plain-text");
            let value: *mut AnyObject = msg_send![
                pasteboard,
                stringForType: uti.as_concrete_TypeRef() as *mut AnyObject
            ];
            ns_string_to_rust(value)
        })
    }
}

/// Replace the clipboard with `text`. Used by every Copy affordance.
pub fn set_text(text: &str) -> bool {
    access(|pasteboard| pasteboard.set_text(text))
}

/// Read the clipboard's plain text.
pub fn text() -> Option<String> {
    access(|pasteboard| pasteboard.text())
}

/// Send Cmd+V through the system HID event stream to the currently focused app.
///
/// The Command key is pressed and released as real key events rather than only
/// setting the modifier flag: some applications watch for the physical
/// modifier and ignore a bare flagged keystroke.
pub fn send_paste_keystroke() -> Result<(), String> {
    // Hold-to-talk means the user may still be physically holding the shortcut
    // when the transcript lands. Their real Command (and whatever else) is down,
    // and an app that compares the event's modifiers against what it expects
    // sees Cmd+Opt+V, or Cmd+.+V, and ignores it. Let the keyboard settle first.
    wait_for_modifiers_to_clear();

    // A *private* source, so the synthesized event carries only the flags set
    // here. `CombinedSessionState` unions in the real hardware modifier state,
    // which is exactly the contamination above.
    let source = CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|_| "could not create a keyboard event source".to_string())?;

    // Only the V keystrokes, with Command as a *flag*. Synthesizing the Command
    // key itself as separate down/up events is what broke Chromium-based inputs:
    // they track `flagsChanged` on its own timeline, and a modifier press
    // arriving 10ms before the V often had not been applied yet, so the app saw
    // a bare "v". AppKit fields read the flag straight off the event and never
    // noticed the difference — hence "works in some inputs, not others".
    for stroke in paste_chord() {
        let event = CGEvent::new_keyboard_event(source.clone(), stroke.key, stroke.down)
            .map_err(|_| "could not create the paste keystroke".to_string())?;
        event.set_flags(CGEventFlags::from_bits_truncate(stroke.flags));
        event.post(CGEventTapLocation::HID);

        if stroke.down {
            thread::sleep(KEY_EVENT_DELAY);
        }
    }

    Ok(())
}

/// One synthesized key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stroke {
    key: u16,
    down: bool,
    flags: u64,
}

/// Exactly what gets posted for a paste.
///
/// Split out so the shape can be tested: posting requires Accessibility, which
/// a `cargo test` binary does not have, but the decision about *what* to post
/// is the part that was wrong.
fn paste_chord() -> [Stroke; 2] {
    [
        Stroke { key: KEY_V, down: true, flags: FLAG_COMMAND },
        Stroke { key: KEY_V, down: false, flags: FLAG_COMMAND },
    ]
}

extern "C" {
    /// Current modifier state of the real keyboard.
    ///
    /// Not exposed by the `core-graphics` crate, but a stable CoreGraphics
    /// symbol.
    fn CGEventSourceFlagsState(state: u32) -> u64;
}

/// `CGEventFlags` masks, from `CGEventTypes.h`. Named rather than inlined
/// because getting one wrong silently changes which modifiers are waited on.
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

/// The modifiers that corrupt a synthetic Cmd+V when physically held.
const CONTAMINATING_MODIFIERS: u64 = FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND;

/// Wait, briefly, for the user to finish releasing their shortcut.
///
/// Bounded: if a modifier is genuinely held — someone resting a thumb on
/// Command — the paste is still attempted rather than silently dropped. Late is
/// better than never, and the transcript is on the clipboard either way.
fn wait_for_modifiers_to_clear() {
    // 1 is HIDSystemState — the physical keyboard. (CombinedSessionState is 0
    // and includes synthetic events, including our own.)
    const HID_SYSTEM_STATE: u32 = 1;
    let deadline = std::time::Instant::now() + MODIFIER_RELEASE_TIMEOUT;

    while std::time::Instant::now() < deadline {
        let held = unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) };
        if held & CONTAMINATING_MODIFIERS == 0 {
            return;
        }
        thread::sleep(MODIFIER_POLL_INTERVAL);
    }

    tracing::debug!("pasting while a modifier is still held; the shortcut may be stuck down");
}

/// Type `text` directly, as though the user had entered it.
///
/// # Why this exists alongside paste
///
/// Cmd+V is a *command*: the target app has to recognise the chord, route it
/// through its menu system, and choose to read the pasteboard. Electron and
/// Chromium apps do that on their own terms and reject synthetic chords that
/// do not match what they expect — which is why paste worked in AppKit fields
/// and silently did nothing in the Claude app.
///
/// Unicode typing skips all of it. `CGEventKeyboardSetUnicodeString` attaches
/// the text to a keystroke that carries no keycode and no modifiers, so the
/// app receives it as ordinary text input. There is no chord to reject.
///
/// Sent in chunks because a single event carrying a very long string is
/// truncated by some editors.
pub fn type_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    // The same contamination that broke paste would attach live modifiers to
    // these keystrokes and turn typed characters into shortcuts.
    wait_for_modifiers_to_clear();

    post_unicode_keystrokes(text)
}

/// Type while the user may still be holding the dictation shortcut.
///
/// Live typing cannot wait for the modifiers to clear: in hold-to-talk they
/// are down for the whole recording. Every event carries an empty flag set, so
/// the held keys are not applied to the characters.
pub fn type_text_while_held(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    post_unicode_keystrokes(text)
}

fn post_unicode_keystrokes(text: &str) -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "could not create a keyboard event source".to_string())?;

    let characters: Vec<char> = text.chars().collect();

    for chunk in characters.chunks(TYPING_CHUNK_CHARS) {
        let piece: String = chunk.iter().collect();
        let utf16: Vec<u16> = piece.encode_utf16().collect();

        for down in [true, false] {
            // Keycode 0 with an attached string: the text is the payload, not
            // the key. Flags are cleared so nothing reads as a shortcut.
            let event = CGEvent::new_keyboard_event(source.clone(), 0, down)
                .map_err(|_| "could not create the typing event".to_string())?;
            event.set_flags(CGEventFlags::CGEventFlagNull);
            event.set_string_from_utf16_unchecked(&utf16);
            event.post(CGEventTapLocation::HID);
        }

        thread::sleep(TYPING_CHUNK_DELAY);
    }

    Ok(())
}

/// Give AppKit/WebKit time to observe the new pasteboard change count before
/// the paste chord arrives. Fast back-to-back write/event delivery is dropped
/// intermittently by real editors even though both calls report success.
pub fn wait_until_pasteboard_is_ready() {
    thread::sleep(PASTEBOARD_SETTLE_DELAY);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These four masks decide which held keys delay a paste. A wrong value
    /// means waiting on the wrong modifier, which reintroduces the bug where
    /// Cmd+V arrived contaminated and Chromium-based inputs ignored it.
    /// The regression this fixes: clide used to synthesize the Command **key**
    /// (0x37) as its own down/up pair around the V. Chromium-based inputs track
    /// `flagsChanged` on a separate timeline and frequently had not applied the
    /// modifier by the time the V arrived, so they saw a bare "v" and ignored
    /// it — while AppKit fields, which read the flag straight off the event,
    /// worked fine. Hence "pastes into Firefox's search but not this input".
    /// Transcripts routinely exceed one chunk, and chunking must not lose or
    /// reorder anything — including multi-byte characters, where a naive
    /// byte-split would corrupt the text.
    #[test]
    fn chunking_preserves_the_text_exactly() {
        for text in [
            "short",
            "a transcript long enough to span several chunks of twenty characters",
            "unicode survives: café — naïve — 日本語 — 🎙️ emoji",
        ] {
            let characters: Vec<char> = text.chars().collect();
            let rejoined: String = characters
                .chunks(TYPING_CHUNK_CHARS)
                .flat_map(|chunk| chunk.iter())
                .collect();
            assert_eq!(rejoined, text, "chunking altered the text");
        }
    }

    #[test]
    fn typing_nothing_is_a_no_op_rather_than_an_error() {
        assert!(type_text("").is_ok());
    }

    #[test]
    fn the_paste_chord_never_synthesizes_the_command_key() {
        const KEY_COMMAND: u16 = 0x37;

        for stroke in paste_chord() {
            assert_ne!(
                stroke.key, KEY_COMMAND,
                "synthesizing the Command key is what broke Chromium inputs"
            );
            assert_eq!(stroke.key, KEY_V);
        }
    }

    #[test]
    fn the_paste_chord_is_one_balanced_press_carrying_only_command() {
        let chord = paste_chord();

        assert_eq!(chord.len(), 2, "a paste is one press and one release");
        assert!(chord[0].down && !chord[1].down, "down must precede up");

        for stroke in chord {
            // Exactly Command. Any extra bit here is the contamination that
            // made apps reject the chord.
            assert_eq!(
                stroke.flags, FLAG_COMMAND,
                "the paste must carry Command and nothing else"
            );
        }
    }

    #[test]
    fn the_modifier_masks_match_cgeventtypes_h() {
        assert_eq!(FLAG_SHIFT, 1 << 17);
        assert_eq!(FLAG_CONTROL, 1 << 18);
        assert_eq!(FLAG_OPTION, 1 << 19);
        assert_eq!(FLAG_COMMAND, 1 << 20);

        // Four distinct bits, nothing overlapping or repeated.
        assert_eq!(CONTAMINATING_MODIFIERS.count_ones(), 4);
    }

    #[test]
    fn every_shortcut_modifier_is_waited_on() {
        for (name, flag) in [
            ("shift", FLAG_SHIFT),
            ("control", FLAG_CONTROL),
            ("option", FLAG_OPTION),
            ("command", FLAG_COMMAND),
        ] {
            assert!(
                CONTAMINATING_MODIFIERS & flag != 0,
                "{name} can be part of a hold-to-talk shortcut but is not waited on"
            );
        }
    }

    /// The wait must be bounded: a genuinely stuck modifier has to degrade to a
    /// late paste, never to no paste.
    #[test]
    fn waiting_for_modifiers_is_bounded() {
        assert!(MODIFIER_RELEASE_TIMEOUT <= Duration::from_secs(1));
        assert!(MODIFIER_POLL_INTERVAL < MODIFIER_RELEASE_TIMEOUT);

        let started = std::time::Instant::now();
        wait_for_modifiers_to_clear();
        assert!(started.elapsed() <= MODIFIER_RELEASE_TIMEOUT + Duration::from_millis(200));
    }

    #[test]
    fn text_survives_a_snapshot_and_restore_round_trip() {
        access(|pasteboard| {
            let original = pasteboard.snapshot();

            assert!(pasteboard.set_text("clide round trip"));
            assert_eq!(pasteboard.text().as_deref(), Some("clide round trip"));

            let borrowed = pasteboard.snapshot();
            assert!(pasteboard.set_text("something else"));
            assert_eq!(pasteboard.text().as_deref(), Some("something else"));

            pasteboard.restore(borrowed);
            assert_eq!(
                pasteboard.text().as_deref(),
                Some("clide round trip"),
                "restoring the snapshot did not bring the clipboard back"
            );

            pasteboard.restore(original);
        });
    }

    #[test]
    fn restoring_an_empty_snapshot_leaves_the_clipboard_alone() {
        access(|pasteboard| {
            let original = pasteboard.snapshot();

            pasteboard.set_text("untouched");
            pasteboard.restore(ClipboardSnapshot::empty());
            assert_eq!(pasteboard.text().as_deref(), Some("untouched"));

            pasteboard.restore(original);
        });
    }

    /// The crash this lock exists to prevent: AppKit's pasteboard type cache
    /// is not thread-safe, and Clide reaches the clipboard from more than one
    /// thread.
    #[test]
    fn concurrent_access_is_serialised() {
        let threads: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    for _ in 0..12 {
                        let captured = access(|pasteboard| {
                            let before = pasteboard.snapshot();
                            pasteboard.set_text(&format!("thread {i}"));
                            let seen = pasteboard.text();
                            pasteboard.restore(before);
                            seen
                        });
                        // Under the lock, a thread always reads back its own
                        // write; without it, this is where the data races.
                        assert_eq!(captured.as_deref(), Some(format!("thread {i}").as_str()));
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().expect("a clipboard thread crashed");
        }
    }
}
