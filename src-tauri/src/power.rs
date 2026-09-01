//! Floater visibility watchdog.  Three layers defend against the Clippy
//! window vanishing after system sleep or WebView2 surface loss:
//!
//! 1. **Resume detector** — a dedicated OS thread sleeps in 2-second ticks
//!    and checks wall-clock time.  If a tick takes 10+ seconds, the machine
//!    was almost certainly suspended.  On resume, an aggressive hide → show →
//!    repaint cycle recovers the floater from the Rust side, independent of
//!    whether the webview's JS runtime is alive.
//!
//! 2. **Periodic watchdog** — a tokio task (spawned in lib.rs) re-asserts
//!    `always_on_top` every 30 s and force-repaints if the JS heartbeat has
//!    gone stale (> 45 s without a ping).
//!
//! 3. **JS heartbeat** — the floater sends `js_heartbeat_ping` every 10 s so
//!    the Rust watchdog knows the webview is healthy.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

// ── JS-ping state (cross-platform) ─────────────────────────────────────

/// Tracks the last JS heartbeat ping from the floater webview.
pub struct JsPingState {
    last_ping_ms: AtomicU64,
}

impl JsPingState {
    pub fn new() -> Self {
        Self {
            last_ping_ms: AtomicU64::new(Self::now_ms()),
        }
    }

    pub fn ping(&self) {
        self.last_ping_ms.store(Self::now_ms(), Ordering::Relaxed);
    }

    pub fn ms_since_last_ping(&self) -> u64 {
        Self::now_ms().saturating_sub(self.last_ping_ms.load(Ordering::Relaxed))
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// ── Resume detector thread (cross-platform) ─────────────────────────────

/// Spawn a dedicated thread that detects system resume via wall-clock gaps.
/// Unlike the JS heartbeat, this thread is NOT hosted inside WebView2 so it
/// keeps running even when the webview's JS runtime is frozen.
pub fn spawn_resume_detector(app: AppHandle) {
    std::thread::Builder::new()
        .name("resume-detector".into())
        .spawn(move || resume_loop(&app))
        .ok();
}

fn resume_loop(app: &AppHandle) {
    let tick = Duration::from_secs(2);
    let threshold = Duration::from_secs(10);

    loop {
        let before = SystemTime::now();
        std::thread::sleep(tick);
        let gap = SystemTime::now()
            .duration_since(before)
            .unwrap_or(Duration::ZERO);

        if gap > threshold {
            tracing::info!(
                "resume detector: wall-clock gap {:?} — system likely resumed from sleep",
                gap
            );
            // Let the system stabilise (DWM, GPU, WebView2 host).
            std::thread::sleep(Duration::from_secs(2));
            recover_floater(app);
        }
    }
}

/// Aggressive recovery cycle: hide the floater, wait, then show + repaint.
/// A bare `force_repaint` (size-nudge) sometimes isn't enough when the
/// WebView2 DirectComposition surface is truly dead; the hide → show cycle
/// forces a full surface recreation.
fn recover_floater(app: &AppHandle) {
    let Some(w) = app.get_webview_window("clippy") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        return;
    }

    tracing::info!("resume recovery: hide → show → repaint");
    let _ = w.hide();
    std::thread::sleep(Duration::from_millis(500));
    let _ = w.show();
    // pin_floater, NOT set_always_on_top — the latter resets the macOS window
    // level and would strand the recovered floater on its birth Space.
    crate::pin_floater(&w);
    crate::commands::force_repaint(&w);
}
