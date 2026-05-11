//! macOS text injection via CGEvent unicode keystrokes.
//!
//! Each segment of text between newlines is posted as a single
//! `CGKeyboardEvent` whose unicode payload is set via
//! `set_string_from_utf16_unchecked`. This is the macOS analog of Windows'
//! `SendInput` with `KEYEVENTF_UNICODE`: the OS treats the event as a
//! synthesized character insertion, so it works in any focused text field
//! regardless of keyboard layout.
//!
//! Newlines are translated into the Return virtual key (kVK_Return = 0x24)
//! because some apps swallow a literal U+000A in the unicode payload.
//!
//! **Permission requirement:** the user must grant our app
//! "Accessibility" access (System Settings → Privacy & Security →
//! Accessibility). Without it, CGEventPost silently fails — events are
//! accepted but never reach the focused window. The clipboard fallback
//! (`send_cmd_v` + clipboard contents) covers the unauth'd case.
//!
//! Counterpart: `sendinput.rs` for Windows. Keep the public API
//! (`send`, `send_cmd_v` / `send_ctrl_v`) symmetric across platforms.

use anyhow::{anyhow, Result};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

const KV_RETURN: CGKeyCode = 0x24;
const KV_V: CGKeyCode = 0x09;

pub fn send(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    // Split on '\n' so we can emit explicit Return keystrokes between
    // segments. `split` preserves empty segments which we handle below.
    let mut first = true;
    for segment in text.split('\n') {
        if !first {
            press_keycode(KV_RETURN, CGEventFlags::empty())?;
        }
        first = false;

        // Strip any stray '\r' inside a segment (CRLF inputs).
        let cleaned: String = segment.chars().filter(|&c| c != '\r').collect();
        if cleaned.is_empty() {
            continue;
        }

        emit_unicode_segment(&cleaned)?;
    }
    Ok(())
}

/// Send a Cmd+V keystroke combo. Used by the clipboard fallback (the
/// macOS analog of `send_ctrl_v` on Windows).
pub fn send_cmd_v() -> Result<()> {
    press_keycode(KV_V, CGEventFlags::CGEventFlagCommand)
}

fn emit_unicode_segment(segment: &str) -> Result<()> {
    let source = make_source()?;
    let utf16: Vec<u16> = segment.encode_utf16().collect();

    let down = CGEvent::new_keyboard_event(source.clone(), 0, true)
        .map_err(|_| anyhow!("CGEvent (keydown) creation failed"))?;
    down.set_string_from_utf16_unchecked(&utf16);
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, 0, false)
        .map_err(|_| anyhow!("CGEvent (keyup) creation failed"))?;
    up.set_string_from_utf16_unchecked(&utf16);
    up.post(CGEventTapLocation::HID);
    Ok(())
}

fn press_keycode(keycode: CGKeyCode, flags: CGEventFlags) -> Result<()> {
    let source = make_source()?;
    let down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .map_err(|_| anyhow!("CGEvent (keycode down) creation failed"))?;
    if !flags.is_empty() {
        down.set_flags(flags);
    }
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, keycode, false)
        .map_err(|_| anyhow!("CGEvent (keycode up) creation failed"))?;
    if !flags.is_empty() {
        up.set_flags(flags);
    }
    up.post(CGEventTapLocation::HID);
    Ok(())
}

fn make_source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| anyhow!("CGEventSource::new(HIDSystemState) failed"))
}
