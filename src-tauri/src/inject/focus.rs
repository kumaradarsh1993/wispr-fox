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
#[derive(Debug, Clone)]
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
    /// Filename of the foreground process executable (e.g. "OUTLOOK.EXE",
    /// "chrome.exe"). Empty if the OpenProcess query failed. Used by the
    /// app-context classifier to coarse-bucket the user's target app.
    process_name: String,
    /// Foreground window title at capture time (e.g. "Inbox - foo@bar -
    /// Outlook"). Used as fallback for browser-based classification when
    /// the process is just "chrome.exe". Sent ONLY to the local classifier
    /// — never leaves the machine.
    window_title: String,
}

#[cfg(windows)]
impl CapturedFocus {
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn process_name(&self) -> &str {
        &self.process_name
    }
    pub fn window_title(&self) -> &str {
        &self.window_title
    }
}

#[cfg(not(windows))]
#[derive(Debug, Clone)]
pub struct CapturedFocus;

#[cfg(not(windows))]
impl CapturedFocus {
    pub fn pid(&self) -> u32 {
        0
    }
    pub fn process_name(&self) -> &str {
        ""
    }
    pub fn window_title(&self) -> &str {
        ""
    }
}

/// Coarse bucket telling us what kind of surface the user is dictating
/// into. Used to nudge the LLM's tone for F9 drafting. We pass ONLY the
/// enum variant name to the LLM (never the raw process or title), so
/// nothing identifying leaks beyond "email-like target".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    Email,
    ChatCasual,
    ChatWork,
    SocialPublic,
    SocialCasual,
    Document,
    Code,
    AiChat,
    /// No match — caller should skip context augmentation entirely.
    Default,
}

#[cfg(windows)]
pub fn classify_app(ctx: &CapturedFocus) -> AppKind {
    classify_inner(&ctx.process_name, &ctx.window_title)
}

#[cfg(not(windows))]
pub fn classify_app(_ctx: &CapturedFocus) -> AppKind {
    AppKind::Default
}

fn classify_inner(process_name: &str, window_title: &str) -> AppKind {
    let exe = process_name.to_ascii_lowercase();
    let title = window_title.to_ascii_lowercase();

    // Direct exe matches — most reliable signal.
    match exe.as_str() {
        "outlook.exe" | "thunderbird.exe" => return AppKind::Email,
        "whatsapp.exe" | "telegram.exe" | "signal.exe" => return AppKind::ChatCasual,
        "teams.exe" | "ms-teams.exe" | "slack.exe" | "discord.exe" => return AppKind::ChatWork,
        "winword.exe" | "notion.exe" | "obsidian.exe" => return AppKind::Document,
        "code.exe" | "cursor.exe" | "idea64.exe" | "devenv.exe" | "rustrover64.exe" | "pycharm64.exe" => return AppKind::Code,
        _ => {}
    }

    // Browsers — process name doesn't distinguish, so fall back to title.
    let is_browser = matches!(
        exe.as_str(),
        "chrome.exe" | "msedge.exe" | "firefox.exe" | "brave.exe"
            | "arc.exe" | "opera.exe" | "vivaldi.exe" | "applicationframehost.exe"
    );
    if is_browser || exe.is_empty() {
        if title.contains("linkedin") { return AppKind::SocialPublic; }
        if title.contains("twitter") || title.contains(" / x") || title.starts_with("x (") {
            return AppKind::SocialPublic;
        }
        if title.contains("reddit") { return AppKind::SocialCasual; }
        if title.contains("hacker news") || title.contains("news.ycombinator") {
            return AppKind::SocialCasual;
        }
        if title.contains("chatgpt") || title.contains("claude.ai") || title.contains("gemini") {
            return AppKind::AiChat;
        }
        if title.contains("gmail") || title.contains("outlook") { return AppKind::Email; }
        if title.contains("whatsapp web") || title.contains("telegram") { return AppKind::ChatCasual; }
        if title.contains("discord") { return AppKind::ChatWork; }
        if title.contains("notion") || title.contains("google docs") {
            return AppKind::Document;
        }
    }

    AppKind::Default
}

/// Convert an `AppKind` to a short hint line that gets prepended to the
/// LLM's system prompt for F9 drafting. Returns None for `Default` so the
/// caller can skip augmentation entirely.
pub fn context_hint(kind: AppKind) -> Option<&'static str> {
    Some(match kind {
        AppKind::Email => "Context: the user is dictating into an EMAIL composer. Match register: formal-to-professional. Use proper greetings + sign-offs where natural. Structured paragraphs.",
        AppKind::ChatCasual => "Context: the user is dictating into a CASUAL CHAT (WhatsApp, Telegram, etc). Match register: short, conversational, lowercase often OK, no formal preamble, no sign-off.",
        AppKind::ChatWork => "Context: the user is dictating into a WORK CHAT (Slack, Teams, Discord). Match register: brief and professional. No formal greeting unless this is clearly a first message. Skip sign-off.",
        AppKind::SocialPublic => "Context: the user is dictating into a PUBLIC SOCIAL POST (LinkedIn, X/Twitter). Match register: polished, punchy. Short hooks, no fluff, line breaks for rhythm.",
        AppKind::SocialCasual => "Context: the user is dictating into a CASUAL SOCIAL POST (Reddit, HN, casual forum). Match register: conversational, opinionated OK. Match the platform's relaxed tone.",
        AppKind::Document => "Context: the user is dictating into a LONG-FORM DOCUMENT (Word, Notion, Google Docs). Match register: structured prose. Sub-headings or sections where the content naturally calls for them.",
        AppKind::Code => "Context: the user is dictating into a CODE EDITOR. Match register: concise and technical. If this reads like a code comment, format as one. No prose padding.",
        AppKind::AiChat => "Context: the user is dictating into an AI CHAT input box (ChatGPT, Claude, Gemini). Match register: direct, first-person instruction to the AI. Skip preamble — ask the question or state the task.",
        AppKind::Default => return None,
    })
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

    // Pull process name + window title for app-context classification.
    // Both are best-effort: if Win32 refuses (higher-integrity process,
    // missing privilege, etc.) we just record empty strings and classify
    // as Default.
    let process_name = process_name_for_pid(pid);
    let window_title = window_title_of(hwnd);

    tracing::debug!(
        fg_hwnd = foreground_hwnd,
        focused_ctrl,
        pid,
        process_name = %process_name,
        title_len = window_title.len(),
        "focus captured"
    );

    Some(CapturedFocus {
        foreground_hwnd,
        focused_ctrl,
        pid,
        process_name,
        window_title,
    })
}

#[cfg(windows)]
fn window_title_of(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let got = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if got <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..got as usize])
}

#[cfg(windows)]
fn process_name_for_pid(pid: u32) -> String {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid == 0 {
        return String::new();
    }
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return String::new(),
    };
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
    };
    let _ = unsafe { CloseHandle(handle) };
    if result.is_err() || size == 0 {
        return String::new();
    }
    let full_path = String::from_utf16_lossy(&buf[..size as usize]);
    // Take just the file name (after the last backslash) — the classifier
    // works on basename, not full path.
    match full_path.rsplit_once('\\') {
        Some((_, n)) => n.to_string(),
        None => full_path,
    }
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
