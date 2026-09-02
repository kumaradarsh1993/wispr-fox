//! Does macOS think the floater is on the desktop the user is looking at?
//!
//! Three releases were spent fixing the floater's Space pinning, and a
//! diagnostic on the reporter's Mac then showed the pin was correct the whole
//! time — `NSWindow.level == 25`, `collectionBehavior == 0x101`
//! (`CanJoinAllSpaces | FullScreenAuxiliary`) — while the avatar still only
//! ever appeared on the desktop the app launched on.
//!
//! Two very different failures produce that same report, and no amount of
//! reading the window's *configuration* can tell them apart:
//!
//!   1. macOS is not putting the window on the active Space at all. The
//!      collection behavior is being overridden or ignored, and the fix is
//!      something at the application level, not the window level.
//!   2. macOS *is* putting it there and nothing is drawn. This app has a
//!      documented history of exactly that — a transparent + `macOSPrivateApi`
//!      window that AppKit reports as visible while the WindowServer composites
//!      no pixels for it. The fix for that is a surface re-registration, which
//!      has nothing to do with Spaces.
//!
//! `NSWindow.isOnActiveSpace` answers it in one boolean. Sampling it on a timer
//! and keeping a short history means the question can be settled by a user who
//! swipes to another desktop, tries a dictation, swipes back, and reads the
//! diagnostic — rather than by another guess from a Windows machine that cannot
//! observe any of this.
//!
//! Cheap enough to leave running: one Objective-C message send every two
//! seconds, on the main thread, holding two minutes of history.

use std::collections::VecDeque;

use parking_lot::Mutex;

/// 2 minutes at [`SAMPLE_INTERVAL`] — long enough to cover "swipe away, try a
/// dictation, swipe back, open Settings", which is the whole point.
const HISTORY: usize = 60;
const SAMPLE_INTERVAL_MS: u64 = 2_000;

#[derive(Clone, Copy)]
pub struct Sample {
    pub on_active_space: bool,
    pub visible: bool,
}

#[derive(Default)]
pub struct SpaceProbe {
    samples: Mutex<VecDeque<Sample>>,
}

impl SpaceProbe {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&self, s: Sample) {
        let mut q = self.samples.lock();
        if q.len() == HISTORY {
            q.pop_front();
        }
        q.push_back(s);
    }

    /// Oldest → newest. `1` = macOS reported the floater on the active Space,
    /// `0` = it did not, `·` = the window was hidden at that moment (neither
    /// answer means anything then, and reading a hidden window as a failure is
    /// exactly the kind of false signal that sent this investigation sideways).
    pub fn timeline(&self) -> String {
        self.samples
            .lock()
            .iter()
            .map(|s| {
                if !s.visible {
                    '·'
                } else if s.on_active_space {
                    '1'
                } else {
                    '0'
                }
            })
            .collect()
    }

    /// (on-active, off-active) counts across visible samples only.
    pub fn counts(&self) -> (usize, usize) {
        let q = self.samples.lock();
        let on = q.iter().filter(|s| s.visible && s.on_active_space).count();
        let off = q.iter().filter(|s| s.visible && !s.on_active_space).count();
        (on, off)
    }

    pub fn latest(&self) -> Option<Sample> {
        self.samples.lock().back().copied()
    }
}

/// Start the sampler. No-op off macOS, where Spaces do not exist.
pub fn spawn(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn(async move {
            let mut ticker =
                tokio::time::interval(std::time::Duration::from_millis(SAMPLE_INTERVAL_MS));
            loop {
                ticker.tick().await;
                if let Some(s) = sample_once(&app) {
                    if let Some(probe) = tauri::Manager::try_state::<SpaceProbe>(&app) {
                        probe.push(s);
                    }
                }
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

/// One reading of `isOnActiveSpace`, marshalled to the main thread.
#[cfg(target_os = "macos")]
pub fn sample_once(app: &tauri::AppHandle) -> Option<Sample> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use tauri::Manager;

    let w = app.get_webview_window("clippy")?;
    let visible = w.is_visible().unwrap_or(false);

    let (tx, rx) = std::sync::mpsc::channel::<Option<bool>>();
    let win = w.clone();
    if w.run_on_main_thread(move || {
        let read = (|| {
            let ptr = win.ns_window().ok()?;
            if ptr.is_null() {
                return None;
            }
            // SAFETY: main thread, live NSWindow. `isOnActiveSpace` returns
            // ObjC BOOL, which objc2 maps to Rust bool for this signature.
            unsafe {
                let ns_window = ptr as *mut AnyObject;
                let on: bool = msg_send![ns_window, isOnActiveSpace];
                Some(on)
            }
        })();
        let _ = tx.send(read);
    })
    .is_err()
    {
        return None;
    }

    let on_active_space = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .ok()
        .flatten()?;
    Some(Sample {
        on_active_space,
        visible,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn sample_once(_app: &tauri::AppHandle) -> Option<Sample> {
    None
}
