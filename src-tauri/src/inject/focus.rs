//! Capture-and-restore foreground focus across the dictation lifecycle.
//!
//! Problem this solves: there is a 200ms–8s gap between hotkey-down (user
//! starts speaking into a text field) and inject (we paste the result).
//! In that window the foreground/focused control can drift:
//!   - F10 release activates Outlook's ribbon keytips → cursor leaves
//!     the compose body silently.
//!   - User clicks another window while LLM is processing.
//!   - The foreground app auto-focuses a sibling control (To: line, etc.).
//!   - A notification briefly steals focus.
//!
//! Without restore, our paste lands in the wrong place. With restore, the
//! caret returns to exactly where the user was talking.
//!
//! Strategy on Windows: snapshot `GetForegroundWindow` + the focused-control
//! HWND via `GetGUIThreadInfo` on hotkey-down. Before injection, call
//! `SetForegroundWindow` + `SetFocus`, using the `AttachThreadInput` trick
//! to defeat Win32's foreground-lock (which would otherwise no-op our
//! foreground request because our process isn't the most recently
//! foregrounded one).
//!
//! On non-Windows, capture returns None and restore is a no-op. We rely on
//! whatever happens to be focused at inject time. macOS has a similar
//! NSWorkspace + AX API path but isn't implemented yet (lower priority —
//! macOS doesn't have the F10 menu quirk).

use anyhow::Result;

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct CapturedFocus {
    /// Foreground window HWND value, stored as `usize` (a raw pointer cast).
    /// We re-wrap into `HWND` on use. Stored as integer so the type is
    /// trivially `Send + Sync` and not tied to the windows-rs raw-pointer
    /// representation.
    foreground_hwnd: usize,
    /// Focused control HWND inside the foreground window, or 0 if the
    /// foreground app didn't have a child with keyboard focus when we
    /// asked. Restore still puts the window in foreground in that case.
    focused_ctrl: usize,
    /// PID of the captured process. Used at inject time to detect whether
    /// the user has navigated to a different app during the LLM gap — if
    /// they have, we silent-deliver instead of yanking focus.
    pid: u32,
}

#[cfg(windows)]
impl CapturedFocus {
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy)]
pub struct CapturedFocus;

#[cfg(not(windows))]
impl CapturedFocus {
    pub fn pid(&self) -> u32 {
        0
    }
}

#[cfg(windows)]
pub fn capture() -> Option<CapturedFocus> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        tracing::debug!("focus capture: no foreground window");
        return None;
    }

    // Skip if foreground belongs to wispr-fox itself — restoring back to
    // our own window after the user dictated into our settings page would
    // be wrong; let the OS choose whatever was previously focused.
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == std::process::id() {
        tracing::debug!("focus capture: skipping our own window");
        return None;
    }

    // Get the keyboard-focused child of the foreground window. Many apps
    // (Outlook, Slack, browsers) have a top-level frame HWND distinct from
    // the focused edit-control HWND. Restoring just the frame would leave
    // focus on whichever sibling the app decided to give it.
    let tid = unsafe { GetWindowThreadProcessId(hwnd, None) };
    let mut gui_info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let focused_ctrl: usize = if unsafe { GetGUIThreadInfo(tid, &mut gui_info) }.is_ok() {
        gui_info.hwndFocus.0 as usize
    } else {
        0
    };

    let foreground_hwnd = hwnd.0 as usize;
    tracing::debug!(
        fg_hwnd = foreground_hwnd,
        focused_ctrl,
        pid,
        "focus captured"
    );

    Some(CapturedFocus {
        foreground_hwnd,
        focused_ctrl,
        pid,
    })
}

/// PID currently owning the foreground window. Used at inject time to
/// decide whether the user has navigated away from where they started
/// speaking. Returns 0 on non-Windows and on Win32 query failure.
#[cfg(windows)]
pub fn current_foreground_pid() -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return 0;
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

#[cfg(not(windows))]
pub fn current_foreground_pid() -> u32 {
    0
}

/// `(fg_hwnd, focused_ctrl, pid)` for the foreground window right now.
/// Used at inject time to compare against the capture and decide whether
/// the user has moved at all. Returns `(0, 0, 0)` on failure / non-Windows.
#[cfg(windows)]
pub fn current_foreground_state() -> (usize, usize, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return (0, 0, 0);
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let tid = unsafe { GetWindowThreadProcessId(hwnd, None) };
    let mut gui_info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let focused = if unsafe { GetGUIThreadInfo(tid, &mut gui_info) }.is_ok() {
        gui_info.hwndFocus.0 as usize
    } else {
        0
    };
    (hwnd.0 as usize, focused, pid)
}

#[cfg(not(windows))]
pub fn current_foreground_state() -> (usize, usize, u32) {
    (0, 0, 0)
}

#[cfg(windows)]
impl CapturedFocus {
    pub fn foreground_hwnd(&self) -> usize {
        self.foreground_hwnd
    }
    pub fn focused_ctrl(&self) -> usize {
        self.focused_ctrl
    }
}

#[cfg(not(windows))]
impl CapturedFocus {
    pub fn foreground_hwnd(&self) -> usize {
        0
    }
    pub fn focused_ctrl(&self) -> usize {
        0
    }
}

#[cfg(not(windows))]
pub fn capture() -> Option<CapturedFocus> {
    None
}

#[cfg(windows)]
pub fn restore(cap: &CapturedFocus) -> Result<()> {
    use std::thread;
    use std::time::Duration;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    };

    let target = HWND(cap.foreground_hwnd as *mut core::ffi::c_void);

    // Window vanished between capture and now (user closed it). Nothing to
    // restore — let injection land wherever the OS picked next.
    if !unsafe { IsWindow(target) }.as_bool() {
        tracing::debug!("focus restore: captured window no longer exists");
        return Ok(());
    }

    let our_tid = unsafe { GetCurrentThreadId() };
    let target_tid = unsafe { GetWindowThreadProcessId(target, None) };

    if target_tid == 0 {
        return Ok(());
    }

    if target_tid != our_tid {
        // AttachThreadInput merges our input queue with the target's,
        // which makes Win32 treat SetForegroundWindow/SetFocus from our
        // process as if they came from inside the target. Without this,
        // SetForegroundWindow silently fails when wispr-fox isn't the
        // most-recently-foregrounded process (which it never is during
        // background injection).
        unsafe {
            let _ = AttachThreadInput(our_tid, target_tid, true);
        }
        unsafe {
            let _ = SetForegroundWindow(target);
        }
        unsafe {
            let _ = BringWindowToTop(target);
        }
        if cap.focused_ctrl != 0 {
            let ctrl = HWND(cap.focused_ctrl as *mut core::ffi::c_void);
            if unsafe { IsWindow(ctrl) }.as_bool() {
                unsafe {
                    let _ = SetFocus(ctrl);
                }
            }
        }
        unsafe {
            let _ = AttachThreadInput(our_tid, target_tid, false);
        }
    } else {
        // Same thread — no attach needed.
        unsafe {
            let _ = SetForegroundWindow(target);
        }
        if cap.focused_ctrl != 0 {
            let ctrl = HWND(cap.focused_ctrl as *mut core::ffi::c_void);
            if unsafe { IsWindow(ctrl) }.as_bool() {
                unsafe {
                    let _ = SetFocus(ctrl);
                }
            }
        }
    }

    // Give Windows a beat to process the focus change before SendInput /
    // clipboard paste fires. Without this, the keystrokes can outrun the
    // focus transition and land in the previously-focused control.
    thread::sleep(Duration::from_millis(25));

    tracing::debug!(
        fg_hwnd = cap.foreground_hwnd,
        focused_ctrl = cap.focused_ctrl,
        "focus restored"
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn restore(_cap: &CapturedFocus) -> Result<()> {
    Ok(())
}
