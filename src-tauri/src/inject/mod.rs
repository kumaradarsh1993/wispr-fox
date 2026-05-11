//! Text-injection dispatcher.
//!
//! Strategy per platform:
//!
//! - **Windows**: try Win32 `SendInput` with Unicode keystrokes first —
//!   works everywhere including PowerShell/cmd/Windows Terminal, where
//!   clipboard-paste often fails. Fall back to clipboard + Ctrl+V if
//!   SendInput is rejected (e.g. UIPI on elevated targets), or if text
//!   is long enough that per-codepoint injection would be slow.
//!
//! - **macOS**: try `CGEvent` unicode keystrokes first — same idea.
//!   Falls back to clipboard + Cmd+V if Accessibility permission hasn't
//!   been granted yet (the event silently doesn't reach the focused
//!   app) or for very long text.
//!
//! Other targets get stubs that error.

pub mod clipboard;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod sendinput;

use anyhow::Result;

/// Channel chosen by the dispatcher. Stored on the recording row for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    SendInput,
    Clipboard,
}

const CLIPBOARD_PASTE_THRESHOLD: usize = 500;

#[cfg(windows)]
pub fn inject(text: &str) -> Result<Channel> {
    if text.is_empty() {
        return Ok(Channel::SendInput);
    }

    // Long text: clipboard is much faster than per-codepoint SendInput.
    if text.chars().count() > CLIPBOARD_PASTE_THRESHOLD {
        clipboard::paste(text)?;
        return Ok(Channel::Clipboard);
    }

    match sendinput::send(text) {
        Ok(()) => Ok(Channel::SendInput),
        Err(e) => {
            tracing::warn!("SendInput rejected ({e:?}); falling back to clipboard paste");
            clipboard::paste(text)?;
            Ok(Channel::Clipboard)
        }
    }
}

#[cfg(target_os = "macos")]
pub fn inject(text: &str) -> Result<Channel> {
    if text.is_empty() {
        return Ok(Channel::SendInput);
    }

    if text.chars().count() > CLIPBOARD_PASTE_THRESHOLD {
        clipboard::paste(text)?;
        return Ok(Channel::Clipboard);
    }

    match macos::send(text) {
        Ok(()) => Ok(Channel::SendInput),
        Err(e) => {
            tracing::warn!("CGEvent send rejected ({e:?}); falling back to clipboard paste");
            clipboard::paste(text)?;
            Ok(Channel::Clipboard)
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn inject(_text: &str) -> Result<Channel> {
    anyhow::bail!("text injection is Windows/macOS-only in this build")
}
