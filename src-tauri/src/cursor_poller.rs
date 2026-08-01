//! Background OS-cursor poller for the floater's clickthrough mode.
//!
//! Why this exists: the floater toggles `set_ignore_cursor_events(true)`
//! when the user's mouse moves OFF the avatar shape, so clicks fall through
//! to the apps behind. But once a window is in cursor-ignored mode it stops
//! receiving ANY mouse events itself — so JS can't detect when the cursor
//! moves back over the avatar to re-enable catching. Tauri/Cocoa/Win32
//! don't expose a "tell me when the cursor enters this window region"
//! event when the window is pass-through.
//!
//! Solution: a tiny OS-level cursor poller. When `set_clickthrough(true)`
//! is invoked, we spawn one OS thread that polls the global cursor position
//! at ~30 Hz, checks whether it's inside the floater's screen bounds, and
//! the moment it crosses back in: re-enables catching + emits an event so
//! JS can resume hit-testing + exits the thread. One poller at a time.
//!
//! Cost: 30 OS calls/sec while the cursor is OFF the floater. `CGEventCreate`
//! on macOS and `GetCursorPos` on Win32 are both microsecond-scale syscalls.
//! Negligible.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tauri::{Emitter, WebviewWindow};

/// Whether a poller thread is currently active. Prevents spawning more than
/// one at a time (e.g. if JS sends two `set_clickthrough(true)` calls in
/// rapid succession).
static POLLER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Spawn a poller for the given window if one isn't already running.
pub fn spawn_if_needed(window: WebviewWindow) {
    // Atomically claim the poller slot. If someone else already has it,
    // just return — their poller is doing the same job.
    if POLLER_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    thread::spawn(move || {
        // 30 Hz feels responsive without being wasteful. At 60 Hz the
        // re-entry-to-catching latency drops by ~16 ms but the CPU cost
        // doubles for a benefit users won't perceive.
        let interval = Duration::from_millis(33);
        // Bail out after ~30 seconds with no re-entry. If the user really
        // walked away that long, JS will respawn us when needed; meanwhile
        // we don't keep an idle thread alive forever.
        let max_iters = 900;
        let mut iters = 0;
        loop {
            thread::sleep(interval);
            iters += 1;
            if iters > max_iters {
                break;
            }

            let cursor = match get_cursor_pos() {
                Some(p) => p,
                None => continue,
            };
            // outer_position / outer_size on a window that was hidden / closed
            // mid-poll would return Err — treat as a signal to exit cleanly.
            let (wx, wy) = match window.outer_position() {
                Ok(p) => (p.x, p.y),
                Err(_) => break,
            };
            let (ww, wh) = match window.outer_size() {
                Ok(s) => (s.width as i32, s.height as i32),
                Err(_) => break,
            };
            // Small inset so that a re-entry exactly on the window edge,
            // where AppKit/Win32 jitter is most pronounced, doesn't toggle
            // ignore-state back and forth.
            let inset = 2;
            let in_window = cursor.0 >= wx + inset
                && cursor.0 < wx + ww - inset
                && cursor.1 >= wy + inset
                && cursor.1 < wy + wh - inset;
            if in_window {
                // Re-enable catching. The JS hit-test will then refine: if
                // the cursor is over the avatar shape we stay catching, else
                // it'll send us right back to ignore=true.
                let _ = window.set_ignore_cursor_events(false);
                let _ = window.emit("wispr:cursor_entered", ());
                break;
            }
        }
        POLLER_ACTIVE.store(false, Ordering::Release);
    });
}

#[cfg(target_os = "macos")]
fn get_cursor_pos() -> Option<(i32, i32)> {
    // Core Graphics CGEvent: creating an event with no source captures the
    // current input state, including mouse location. This is what most macOS
    // global-mouse-tracker apps use.
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let pt = event.location();
    // Cocoa screen coords have origin at TOP-LEFT for CGEvent, which matches
    // what Tauri's outer_position/outer_size use. No flip needed.
    Some((pt.x as i32, pt.y as i32))
}

#[cfg(windows)]
fn get_cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut p).is_ok() {
            Some((p.x, p.y))
        } else {
            None
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn get_cursor_pos() -> Option<(i32, i32)> {
    // Linux: X11/Wayland support would go here. For now the floater on Linux
    // simply won't enter clickthrough mode — the set_ignore_cursor_events call
    // still works, but without re-entry detection JS has to find another way
    // to un-ignore. Acceptable for the v1.x linux audience; revisit if anyone
    // actually uses the linux builds for daily dictation.
    None
}

#[allow(dead_code)]
pub fn is_active() -> bool {
    POLLER_ACTIVE.load(Ordering::Acquire)
}
