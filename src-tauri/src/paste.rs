//! Paste transcribed text into the previously focused app via Cmd+V (macOS).
//!
//! ## Why direct CGEvent instead of enigo
//!
//! enigo's macOS backend (`raw()` in macos_impl.rs) creates the V keyDown
//! event with `CGEvent::new_keyboard_event` and posts it without ever calling
//! `CGEventSetFlags`. The Cmd modifier is sent as a separate plain keyDown
//! for the Command keycode — not as a `flagsChanged` event, and crucially
//! not as a flag on the V event itself.
//!
//! On macOS, many applications read `[event modifierFlags]` directly off the
//! keyDown event rather than polling the global modifier state. Those apps
//! see the V event with `flags = 0` and interpret it as a literal "v"
//! character — the user-visible bug.
//!
//! The fix: synthesize a single V keyDown/keyUp pair and call
//! `set_flags(CGEventFlagCommand)` on each. macOS treats this as "V was
//! pressed while Cmd was held", which every application handles correctly.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use crate::error::{AppError, Result};

/// kVK_ANSI_V — hardware virtual keycode for the V key, layout-independent.
const KVK_ANSI_V: u16 = 0x09;

/// Simulate Cmd+V to paste the clipboard at the cursor.
pub fn paste_to_cursor() -> Result<()> {
    // Wait for the previously-focused app to regain key-window status after
    // the global hotkey is released. ~150ms is enough on every machine we've
    // tested; the bug we used to chase here was NOT timing-related.
    std::thread::sleep(std::time::Duration::from_millis(150));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AppError::Other("CGEventSource init failed".into()))?;

    let down = CGEvent::new_keyboard_event(source.clone(), KVK_ANSI_V, true)
        .map_err(|_| AppError::Other("CGEvent V-down creation failed".into()))?;
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, KVK_ANSI_V, false)
        .map_err(|_| AppError::Other("CGEvent V-up creation failed".into()))?;
    up.set_flags(CGEventFlags::CGEventFlagCommand);
    up.post(CGEventTapLocation::HID);

    Ok(())
}
