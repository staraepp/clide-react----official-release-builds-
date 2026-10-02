//! The floating recording HUD.
//!
//! It is a real window, but it must never behave like one: no focus, no dock
//! presence, no interception of clicks except when it is showing an error the
//! user has to act on.

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::state::AppState;

pub const LABEL: &str = "hud";

/// How far above the bottom of the usable screen area the pill sits, in
/// logical pixels. The usable area stops at the Dock, so this keeps the pill
/// close to the screen edge without ever covering the Dock.
const BOTTOM_MARGIN: f64 = 10.0;

/// The window is only as big as what it shows. A large transparent window
/// would swallow clicks meant for the app underneath it.
const PILL_SIZE: (f64, f64) = (340.0, 56.0);
const CARD_SIZE: (f64, f64) = (400.0, 250.0);

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Show the HUD without taking focus from the app being dictated into.
///
/// Two things keep the caret where the user left it. `show()` is deliberately
/// never paired with `set_focus()`. And the window is declared `focusable:
/// false` in `tauri.conf.json`, which makes tao override `canBecomeKeyWindow`
/// on the underlying `NSWindow` — without that, macOS hands the HUD key status
/// as it appears and the transcript has nowhere to land.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else {
        tracing::warn!("HUD window is missing");
        return;
    };

    fit_to_state(app, &window);
    position(app, &window);
    sync_interactivity(app, &window);

    if let Err(error) = window.show() {
        tracing::warn!(?error, "could not show the HUD");
    }
    let _ = window.set_always_on_top(true);
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        if let Err(error) = window.hide() {
            tracing::warn!(?error, "could not hide the HUD");
        }
    }
}

/// Whether the HUD is showing a failure card rather than the pill.
fn showing_card(app: &AppHandle) -> bool {
    use crate::dictation::DictationState;

    matches!(
        app.state::<AppState>().session.state(),
        DictationState::CaptureFailed { .. }
            | DictationState::TranscriptionFailed { .. }
            | DictationState::ProcessingFailed { .. }
            | DictationState::InsertionFailed { .. }
    )
}

fn target_size(app: &AppHandle) -> (f64, f64) {
    if showing_card(app) {
        CARD_SIZE
    } else {
        PILL_SIZE
    }
}

fn fit_to_state(app: &AppHandle, window: &WebviewWindow) {
    let (width, height) = target_size(app);
    if let Err(error) = window.set_size(LogicalSize::new(width, height)) {
        tracing::debug!(?error, "could not size the HUD");
    }
}

/// Keep the HUD click-through except when it is offering Retry or Copy.
///
/// A HUD that swallows clicks while someone is trying to work is worse than no
/// HUD, so interactivity is opt-in per state rather than always on.
pub fn sync_interactivity(app: &AppHandle, window: &WebviewWindow) {
    // Only the failure card puts controls (Retry, Copy, the draggable
    // transcript) on screen.
    let needs_input = showing_card(app);

    if let Err(error) = window.set_ignore_cursor_events(!needs_input) {
        tracing::debug!(?error, "could not update HUD cursor behaviour");
    }
}

/// Place the HUD at the bottom centre of whichever display the pointer is on,
/// so it appears next to the work the user is actually doing.
fn position(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|point| app.monitor_from_point(point.x, point.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    // The size just requested, not `outer_size()`: a resize is applied
    // asynchronously, so reading the window back can return the old one.
    let scale = monitor.scale_factor();
    let (width, height) = target_size(app);
    let (width, height) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let area = monitor.work_area();

    let x = area.position.x + ((area.size.width as i32 - width) / 2);
    let y = area.position.y + area.size.height as i32
        - height
        - (BOTTOM_MARGIN * scale).round() as i32;

    if let Err(error) = window.set_position(PhysicalPosition::new(x, y)) {
        tracing::debug!(?error, "could not position the HUD");
    }
}
