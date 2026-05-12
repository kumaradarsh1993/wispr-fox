//! Clipboard-based fallback: save prior contents → write text → SendInput Ctrl+V → restore.
//!
//! Restores text + image clipboard formats; file-list and HTML formats are
//! dropped on fallback (one-time warning shown by the UI). The restore is
//! deferred so the target app's paste handler runs against our text before
//! we put the prior contents back.
//!
//! Timings are tuned for Outlook 365, which is the slowest mainstream paste
//! target on Windows: its paste pipeline runs DLP + HTML scanning on plain
//! text before consuming the clipboard, and clipboard listeners observe
//! `set_text` lazily. Empirically:
//!   - PRE_PASTE_SETTLE: give the OS clipboard a beat to propagate to
//!     listeners *before* we fire Ctrl+V. Without it, Outlook can start its
//!     paste pipeline against the previous contents.
//!   - RESTORE_DELAY: hold our text on the clipboard long enough for Outlook
//!     to finish reading it. 150 ms was fine for Notepad / VS Code but raced
//!     ahead of Outlook, causing F10 (long drafted text) to silently no-op.

use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};

const PRE_PASTE_SETTLE: Duration = Duration::from_millis(40);
const RESTORE_DELAY: Duration = Duration::from_millis(800);

enum Saved {
    None,
    Text(String),
    Image(StaticImage),
}

struct StaticImage {
    width: usize,
    height: usize,
    bytes: Vec<u8>,
}

impl StaticImage {
    fn from(img: ImageData<'_>) -> Self {
        Self {
            width: img.width,
            height: img.height,
            bytes: img.bytes.into_owned(),
        }
    }
    fn as_image_data(&self) -> ImageData<'_> {
        ImageData {
            width: self.width,
            height: self.height,
            bytes: std::borrow::Cow::Borrowed(&self.bytes),
        }
    }
}

#[cfg(windows)]
pub fn paste(text: &str, restore_prior_after: bool) -> Result<()> {
    let saved = if restore_prior_after { save_prior() } else { Saved::None };
    let saved_kind = match &saved {
        Saved::None => "none",
        Saved::Text(_) => "text",
        Saved::Image(_) => "image",
    };

    {
        let mut cb = Clipboard::new().context("opening clipboard")?;
        cb.set_text(text).context("writing text to clipboard")?;
    }
    tracing::debug!(
        chars = text.chars().count(),
        saved_kind,
        restore_prior_after,
        "clipboard primed; waiting for OS listeners to settle"
    );

    // Let Win32 clipboard listeners (Outlook DLP, OneDrive sync, etc.)
    // observe the new contents before we fire Ctrl+V. Without this, fast
    // paste handlers can start reading against the previous clipboard.
    thread::sleep(PRE_PASTE_SETTLE);

    super::sendinput::send_ctrl_v().context("sending Ctrl+V")?;
    tracing::debug!("Ctrl+V dispatched; scheduling clipboard restore");

    // Restore after the target app has consumed the paste. Outlook's
    // paste pipeline is slow (HTML/DLP detection on plain text), so we
    // hold the new clipboard for longer than feels necessary for snappy
    // targets like Notepad. Skipped when keep_in_clipboard is on — user
    // wants the dictation text to stay available for Ctrl+V anywhere.
    if restore_prior_after {
        thread::spawn(move || {
            thread::sleep(RESTORE_DELAY);
            match restore_prior(saved) {
                Ok(()) => tracing::debug!("prior clipboard restored"),
                Err(e) => tracing::warn!("prior clipboard restore failed: {e:#}"),
            }
        });
    } else {
        tracing::debug!("clipboard restore skipped (keep_in_clipboard=true)");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn paste(text: &str, restore_prior_after: bool) -> Result<()> {
    let saved = if restore_prior_after { save_prior() } else { Saved::None };

    {
        let mut cb = Clipboard::new().context("opening clipboard")?;
        cb.set_text(text).context("writing text to clipboard")?;
    }

    thread::sleep(PRE_PASTE_SETTLE);

    super::macos::send_cmd_v().context("sending Cmd+V")?;

    if restore_prior_after {
        thread::spawn(move || {
            thread::sleep(RESTORE_DELAY);
            let _ = restore_prior(saved);
        });
    }

    Ok(())
}

/// Write text to the clipboard WITHOUT pasting — used in the silent
/// cross-process path when the user navigated away during the LLM gap.
/// No prior-content save/restore: dictation stays on clipboard so the
/// user can Ctrl+V it wherever they are now.
pub fn set_only(text: &str) -> Result<()> {
    let mut cb = Clipboard::new().context("opening clipboard")?;
    cb.set_text(text).context("writing text to clipboard")?;
    tracing::debug!(chars = text.chars().count(), "clipboard set (silent delivery)");
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn paste(_text: &str, _restore_prior_after: bool) -> Result<()> {
    anyhow::bail!("clipboard paste is Windows/macOS-only in this build")
}

fn save_prior() -> Saved {
    let Ok(mut cb) = Clipboard::new() else { return Saved::None };
    if let Ok(t) = cb.get_text() {
        return Saved::Text(t);
    }
    if let Ok(img) = cb.get_image() {
        return Saved::Image(StaticImage::from(img));
    }
    Saved::None
}

fn restore_prior(saved: Saved) -> Result<()> {
    let mut cb = Clipboard::new().context("re-opening clipboard for restore")?;
    match saved {
        Saved::None => Ok(()),
        Saved::Text(t) => cb.set_text(t).context("restoring prior text"),
        Saved::Image(img) => cb
            .set_image(img.as_image_data())
            .context("restoring prior image"),
    }
}
