/// Paste transcribed text into the previously focused app.
/// Strategy: write to clipboard, then simulate Cmd+V (macOS).
use enigo::{Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

/// Simulate a Cmd+V keypress to paste clipboard content.
pub fn paste_to_cursor() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Other(format!("Enigo init failed: {e}")))?;

    // Wait for the previously-focused app to regain key-window status after the
    // global hotkey is released. 150ms was too short on slower machines — when
    // the app wasn't focused yet, the Meta press was dropped and the bare V
    // keycode below landed as a literal "v" in whatever window was foreground.
    std::thread::sleep(std::time::Duration::from_millis(300));

    enigo
        .key(Key::Meta, enigo::Direction::Press)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // Give macOS a tick to register the Cmd modifier flag before the V chord.
    // Without this, Enigo can fire V before the modifier latches → bare "v".
    std::thread::sleep(std::time::Duration::from_millis(25));

    // Use the hardware keycode (kVK_ANSI_V = 0x09) instead of Key::Unicode('v').
    // Key::Unicode triggers a keyboard-layout lookup via TSMGetInputSourceProperty,
    // which requires the main dispatch queue — causing a crash on background threads.
    enigo
        .key(Key::Other(0x09), enigo::Direction::Click)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // Let the V keystroke be consumed before lifting Cmd.
    std::thread::sleep(std::time::Duration::from_millis(25));

    enigo
        .key(Key::Meta, enigo::Direction::Release)
        .map_err(|e| AppError::Other(e.to_string()))?;

    Ok(())
}
