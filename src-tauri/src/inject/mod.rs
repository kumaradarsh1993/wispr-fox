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
pub mod focus;
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

/// `keep_in_clipboard` controls whether the clipboard-paste path restores
/// the user's prior clipboard contents 800ms after pasting. When true
/// (the default user-facing behaviour), restore is SKIPPED — the dictated
/// text stays on the clipboard so the user can Ctrl+V it again anywhere.
#[cfg(windows)]
pub fn inject(text: &str, keep_in_clipboard: bool) -> Result<Channel> {
    if text.is_empty() {
        return Ok(Channel::SendInput);
    }

    let restore_prior = !keep_in_clipboard;

    // Long text: clipboard is much faster than per-codepoint SendInput.
    if text.chars().count() > CLIPBOARD_PASTE_THRESHOLD {
        clipboard::paste(text, restore_prior)?;
        return Ok(Channel::Clipboard);
    }

    match sendinput::send(text) {
        Ok(()) => {
            // SendInput typed the chars directly — clipboard wasn't touched.
            // If keep_in_clipboard is on, ALSO write text to clipboard so
            // the user still has Ctrl+V as a backup delivery method.
            if keep_in_clipboard {
                if let Err(e) = clipboard::set_only(text) {
                    tracing::warn!("post-inject clipboard set failed: {e:#}");
                }
            }
            Ok(Channel::SendInput)
        }
        Err(e) => {
            tracing::warn!("SendInput rejected ({e:?}); falling back to clipboard paste");
            clipboard::paste(text, restore_prior)?;
            Ok(Channel::Clipboard)
        }
    }
}

#[cfg(target_os = "macos")]
pub fn inject(text: &str, keep_in_clipboard: bool) -> Result<Channel> {
    if text.is_empty() {
        return Ok(Channel::SendInput);
    }

    let restore_prior = !keep_in_clipboard;

    if text.chars().count() > CLIPBOARD_PASTE_THRESHOLD {
        clipboard::paste(text, restore_prior)?;
        return Ok(Channel::Clipboard);
    }

    match macos::send(text) {
        Ok(()) => {
            if keep_in_clipboard {
                if let Err(e) = clipboard::set_only(text) {
                    tracing::warn!("post-inject clipboard set failed: {e:#}");
                }
            }
            Ok(Channel::SendInput)
        }
        Err(e) => {
            tracing::warn!("CGEvent send rejected ({e:?}); falling back to clipboard paste");
            clipboard::paste(text, restore_prior)?;
            Ok(Channel::Clipboard)
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn inject(_text: &str, _keep_in_clipboard: bool) -> Result<Channel> {
    anyhow::bail!("text injection is Windows/macOS-only in this build")
}
