/// Paste transcribed text into the previously focused app.
/// Strategy: write to clipboard, then simulate Cmd+V (macOS).
use enigo::{Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

/// Simulate a Cmd+V keypress to paste clipboard content.
pub fn paste_to_cursor() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Other(format!("Enigo init failed: {e}")))?;

    // Small delay to allow focus to return to the target app
    std::thread::sleep(std::time::Duration::from_millis(150));

    enigo
        .key(Key::Meta, enigo::Direction::Press)
        .map_err(|e| AppError::Other(e.to_string()))?;
    // Use the hardware keycode (kVK_ANSI_V = 0x09) instead of Key::Unicode('v').
    // Key::Unicode triggers a keyboard-layout lookup via TSMGetInputSourceProperty,
    // which requires the main dispatch queue — causing a crash on background threads.
    enigo
        .key(Key::Other(0x09), enigo::Direction::Click)
        .map_err(|e| AppError::Other(e.to_string()))?;
    enigo
        .key(Key::Meta, enigo::Direction::Release)
        .map_err(|e| AppError::Other(e.to_string()))?;

    Ok(())
}
