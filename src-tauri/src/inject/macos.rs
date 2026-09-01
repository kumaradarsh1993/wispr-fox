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

/// Bundle identifier. Must match `identifier` in `tauri.conf.json` — it is the
/// key macOS files this app's privacy grants under, and `tccutil` takes it as
/// the argument that says which app to forget.
pub const BUNDLE_ID: &str = "com.wispr-fox.app";

/// Ask macOS to prompt for Accessibility, registering the CURRENT binary.
///
/// `AXIsProcessTrusted` only reports; this variant, with the prompt option on,
/// makes macOS surface its own "wants to control this computer" dialog and add
/// the running executable to the Accessibility list. That matters because the
/// list is keyed to a code-signing identity, not to a name: after an update
/// changes the app's hash, the old entry stays visible and switched on while
/// referring to a binary that no longer exists.
///
/// Returns the trust state as macOS sees it right now. A freshly granted
/// permission usually does not take effect in this process until it restarts,
/// so a `false` here immediately after granting is expected, not a failure.
pub fn prompt_for_accessibility() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    type Boolean = std::os::raw::c_uchar;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> Boolean;
    }

    // The literal value of `kAXTrustedCheckOptionPrompt`. Using the string
    // rather than linking the exported global keeps this to one FFI symbol;
    // the constant is part of the public API and has not changed.
    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);

    // SAFETY: `options` outlives the call, and the dictionary is exactly the
    // shape AXIsProcessTrustedWithOptions documents.
    unsafe {
        AXIsProcessTrustedWithOptions(
            options.as_concrete_TypeRef() as *const std::ffi::c_void
        ) != 0
    }
}

/// Make macOS forget this app's Accessibility grant.
///
/// The repair half of [`prompt_for_accessibility`]. When an update changes the
/// app's signature, the existing entry is stale: System Settings shows
/// wispr-fox switched **on** while `AXIsProcessTrusted` says false, and
/// toggling the switch off and on does not rebind it — the entry still points
/// at the old binary. Removing the entry outright is what clears that, and
/// `tccutil reset` is the scriptable form of the `−` button.
///
/// Runs as the user against the user's own TCC store, so it needs no
/// privileges. Returns the command's stderr on a non-zero exit.
pub fn reset_accessibility_grant() -> Result<()> {
    let out = std::process::Command::new("tccutil")
        .args(["reset", "Accessibility", BUNDLE_ID])
        .output()
        .map_err(|e| anyhow!("could not run tccutil: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "tccutil reset Accessibility {BUNDLE_ID} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}

/// Whether the app currently holds macOS **Accessibility** permission.
///
/// Our CGEvent keystroke injection — and the Cmd+V clipboard fallback, which
/// also posts CGEvents — only reach other apps when the user has granted
/// Accessibility (System Settings → Privacy & Security → Accessibility).
/// Without it, dictated text still lands on the clipboard but won't auto-
/// paste. The frontend uses this to show a one-time setup nudge.
///
/// `AXIsProcessTrusted` is a read-only check (no prompt, no side effects) so
/// it's safe to call on every launch.
pub fn is_accessibility_trusted() -> bool {
    // `Boolean` in the macOS SDK is an unsigned char (0/1), not Rust's bool —
    // bind it as c_uchar and compare to avoid relying on bool's ABI.
    type Boolean = std::os::raw::c_uchar;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> Boolean;
    }
    unsafe { AXIsProcessTrusted() != 0 }
}
