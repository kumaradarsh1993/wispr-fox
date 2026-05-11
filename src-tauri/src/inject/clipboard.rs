//! Clipboard-based fallback: save prior contents → write text → SendInput Ctrl+V → restore.
//!
//! Restores text + image clipboard formats; file-list and HTML formats are
//! dropped on fallback (one-time warning shown by the UI). The restore is
//! deferred ~150 ms so the target app's paste handler runs against our text
//! before we put the prior contents back.

use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};

const RESTORE_DELAY: Duration = Duration::from_millis(150);

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
pub fn paste(text: &str) -> Result<()> {
    let saved = save_prior();

    {
        let mut cb = Clipboard::new().context("opening clipboard")?;
        cb.set_text(text).context("writing text to clipboard")?;
    }

    super::sendinput::send_ctrl_v().context("sending Ctrl+V")?;

    // Restore after the target app has consumed the paste.
    thread::spawn(move || {
        thread::sleep(RESTORE_DELAY);
        let _ = restore_prior(saved);
    });

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn paste(text: &str) -> Result<()> {
    let saved = save_prior();

    {
        let mut cb = Clipboard::new().context("opening clipboard")?;
        cb.set_text(text).context("writing text to clipboard")?;
    }

    super::macos::send_cmd_v().context("sending Cmd+V")?;

    thread::spawn(move || {
        thread::sleep(RESTORE_DELAY);
        let _ = restore_prior(saved);
    });

    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn paste(_text: &str) -> Result<()> {
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
