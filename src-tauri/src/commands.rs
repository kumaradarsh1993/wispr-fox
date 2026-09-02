//! Frontend command surface. Thin wrappers — all logic lives in domain modules.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio;
use crate::flow::{Flow, FlowSnapshot};
use crate::history::{History, Recording};
use crate::secrets::{self, SecretKey};
use crate::settings::AppSettings;
use crate::usage::{DailyUsage, UsageTracker};

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
pub fn get_flow_snapshot(flow: State<'_, Flow>) -> FlowSnapshot {
    flow.get_flow_snapshot()
}

/// Called every 10s by the floater's JS to signal the webview is alive.
/// The Rust-side watchdog checks the staleness of this timestamp to decide
/// whether a full force_repaint is needed.
#[tauri::command]
pub fn js_heartbeat_ping(ping_state: State<'_, crate::power::JsPingState>) {
    ping_state.ping();
}

/// Nudge a transparent, always-on-top window so WebView2 rebuilds its
/// composition surface. On Windows the floater's DirectComposition surface
/// is torn down when DWM restarts (system sleep/resume, RDP reconnect, fast
/// user-switching, GPU driver reset) — the window stays "visible" but paints
/// nothing, so the fox vanishes. A plain `show()` is NOT enough to bring it
/// back (that's why the tray's "Toggle Clippy" didn't help); the reliable fix
/// is to change the window size, which forces WebView2 to recreate its swap
/// chain and repaint. We bump by 1px then restore the exact size.
pub(crate) fn force_repaint<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    // macOS path: hide() → show() in sequence forces the WKWebView surface to
    // re-register with the WindowServer. This recovers from the zero-alpha
    // "ghost window" state where AppKit thinks the window is visible but no
    // pixels are composited (reported on M4 Pro for transparent + macOSPrivateApi
    // + alwaysOnTop floaters). A plain `show()` is a no-op on an already-
    // visible-from-AppKit's-POV window, so the previous version of this
    // function silently failed to recover on Mac.
    #[cfg(target_os = "macos")]
    {
        let _ = window.hide();
        let _ = window.show();
        // Re-assert the Space pin too: a recovered window that only floats on
        // the Space it was created in is still invisible to a user who has
        // moved to a fullscreen app. NOT set_always_on_top — see pin_floater.
        crate::pin_floater(window);
        return;
    }
    #[allow(unreachable_code)]
    {
        let _ = window.show();
        crate::pin_floater(window);
        // Windows-only: WebView2 loses its DirectComposition surface after
        // sleep and only a resize forces it to rebuild.
        #[cfg(windows)]
        if let Ok(size) = window.outer_size() {
            let bumped = tauri::PhysicalSize::new(size.width + 1, size.height + 1);
            let _ = window.set_size(bumped);
            let _ = window.set_size(size);
        }
    }
}

/// Force the Clippy floater to repaint after the machine resumes from sleep.
/// Called from the floater's own resume-watchdog (a clock-drift timer in the
/// webview) when it detects a large time gap — the tell-tale of a suspend.
/// No-op when the floater is intentionally hidden (user toggled it off via
/// the tray) so we never resurrect a window they dismissed.
#[tauri::command]
pub fn recover_clippy_window(app: AppHandle) {
    let Some(w) = app.get_webview_window("clippy") else {
        return;
    };
    if !w.is_visible().unwrap_or(true) {
        return;
    }
    tracing::info!("recovering Clippy floater (resume / surface-loss repaint)");
    force_repaint(&w);
}

/// Show the floater and pin it to the Space the user is looking at RIGHT NOW.
///
/// Every JS caller that used to do `getCurrentWindow().show()` on the floater
/// goes through this instead. A bare `show()` is not enough on macOS: the
/// NSWindow keeps whatever level and collection behavior it last had, and
/// anything that touched `always_on_top` in between (the watchdog, a resume
/// recovery) will have knocked it back down to `NSFloatingWindowLevel` on its
/// birth Space. Re-pinning at the moment of showing means the avatar appears
/// wherever the user actually is, even if every other safeguard has lapsed.
///
/// No-op-safe: showing an already-visible window just re-asserts the pin.
#[tauri::command]
pub fn show_floater(app: AppHandle) {
    let Some(w) = app.get_webview_window("clippy") else {
        return;
    };
    // Pin BEFORE showing. The collection behavior is what decides which Space
    // a window is ordered onto, so setting it afterwards is a frame too late:
    // the window has already been placed, and on macOS a background app's
    // window is placed on the Space it belongs to, not the one in front of the
    // user. All three calls queue onto the main thread in this order.
    crate::pin_floater(&w);

    // On macOS, `show()` on an ALREADY-visible window does nothing at the
    // WindowServer level, so a floater that has been on screen since launch is
    // never re-registered and stays wherever it was first composited. That is
    // one of the two candidate explanations for the avatar being stuck on the
    // desktop the app started on. hide() → show() is this app's established
    // remedy for a stale surface (the same cycle runs at startup); it is
    // imperceptible and costs nothing when the window was hidden anyway.
    #[cfg(target_os = "macos")]
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    }

    let _ = w.show();
    #[cfg(target_os = "macos")]
    crate::macos_order_front(&w);
}

/// Resize the floater window from Rust, optionally keeping its visual centre
/// fixed. We do this in Rust instead of the JS window API because on the
/// floater webview `WebviewWindow.outerSize()` / `setSize()` from JS were
/// silently failing (outerSize() rejected, so the JS resize aborted before it
/// ever called setSize — the "got 0×0, window never resizes" bug). Native
/// calls here are reliable.
///
/// `width`/`height` are LOGICAL pixels; Tauri converts to physical using the
/// window's scale factor. Returns the ACTUAL outer size in PHYSICAL px plus
/// the scale factor so the frontend debug overlay can show requested-vs-actual.
#[tauri::command]
pub fn resize_floater(
    window: tauri::WebviewWindow,
    width: f64,
    height: f64,
    center: bool,
) -> Result<(u32, u32, f64), String> {
    let sf = window.scale_factor().unwrap_or(1.0);
    // Snapshot current geometry BEFORE resizing so we can re-anchor.
    let cur_pos = window.outer_position().ok();
    let cur_size = window.outer_size().ok();

    let new_w = (width * sf).round() as i32;
    let new_h = (height * sf).round() as i32;

    // Bottom-CENTRE anchored target top-left (physical). The avatar is
    // horizontally centred and sits on the window's bottom edge, so to keep
    // it visually still: keep the horizontal centre fixed (x shifts by half
    // of dx) and the BOTTOM edge fixed (y shifts by the FULL dy). The window
    // then grows UPWARD for the bubble and the character doesn't move.
    let (nx, ny) = match (center, cur_pos, cur_size) {
        (true, Some(pos), Some(old)) => {
            let dx = new_w - old.width as i32;
            let dy = new_h - old.height as i32;
            (pos.x - dx / 2, pos.y - dy)
        }
        _ => cur_pos.map(|p| (p.x, p.y)).unwrap_or((0, 0)),
    };

    // Unlock the size bounds first so the resize below isn't clamped by the
    // PREVIOUS state's lock (which would otherwise force an intermediate
    // resize/paint — a flicker source). We relock to the new size afterwards.
    let _ = window.set_resizable(true);
    let _ = window.set_maximizable(false);
    let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
    let _ = window.set_max_size(None::<tauri::LogicalSize<f64>>);

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOOWNERZORDER, SWP_NOZORDER,
        };
        match window.hwnd() {
            Ok(h) => unsafe {
                // Rebuild the HWND in OUR windows-crate version from the raw
                // pointer — Tauri's hwnd() may come from a different `windows`
                // version, so passing it straight to our SetWindowPos fails the
                // type/Param bound.
                let hwnd = HWND(h.0 as *mut core::ffi::c_void);
                // ONE atomic move+size = a single paint, far smoother than a
                // separate set_size then set_position (two paints, and the
                // window flashes briefly mis-placed in between — the flicker).
                //
                // SWP_NOCOPYBITS: without it, Windows BLITS the old client-area
                // pixels into the new geometry while WebView2 is still
                // repainting — and because the window grows UPWARD (top-left
                // moves), the stale avatar lands shifted up for a frame or two
                // before snapping back. That was the visible "glitched view"
                // on every F8 grow (v1.4.0-nightly.1 feedback). Discarding the
                // old bits means the worst case is one transparent frame, which
                // disappears into the avatar's own state cross-fade.
                let _ = SetWindowPos(
                    hwnd,
                    HWND::default(),
                    nx,
                    ny,
                    new_w,
                    new_h,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOCOPYBITS,
                );
            },
            Err(_) => {
                let _ = window.set_size(tauri::LogicalSize::new(width, height));
                if center {
                    let _ = window.set_position(tauri::PhysicalPosition::new(nx, ny));
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| format!("set_size: {e}"))?;
        if center {
            let _ = window.set_position(tauri::PhysicalPosition::new(nx, ny));
        }
    }

    // Relock to exactly the new size so the user can't drag-resize the
    // borderless floater. Equal to the size we just applied → no extra resize.
    let logical = tauri::LogicalSize::new(width, height);
    let _ = window.set_min_size(Some(logical));
    let _ = window.set_max_size(Some(logical));

    let after = window
        .outer_size()
        .map_err(|e| format!("outer_size: {e}"))?;
    Ok((after.width, after.height, sf))
}

/// Toggle the floater window's clickthrough mode. When `ignore=true` the
/// window passes all clicks through to whatever app is behind it — and stops
/// receiving any mouse events itself. Used by the JS hit-test in
/// `clippy/+page.svelte` to make ONLY the avatar shape (and the bubble while
/// it's visible) catch clicks; empty pixels around the avatar become
/// transparent to mouse input.
///
/// When ignore goes true we also kick off a background OS-cursor poller so
/// the moment the user moves their mouse back over the floater bounds we can
/// re-enable catching — without the poller, a cursor-ignored window can
/// never notice the cursor returning.
#[tauri::command]
pub fn set_clickthrough(window: tauri::WebviewWindow, ignore: bool) {
    let _ = window.set_ignore_cursor_events(ignore);
    if ignore {
        crate::cursor_poller::spawn_if_needed(window);
    }
}

/// Trigger a recording from the floater's right-click context menu (or any
/// non-hotkey caller). This is an explicit toggle, independent of physical
/// key-edge interpretation. `mode` is "light", "advanced", or "drafting".
#[tauri::command]
pub fn floater_trigger(
    app: AppHandle,
    flow: State<'_, Flow>,
    mode: String,
) -> Result<(), String> {
    use crate::settings::Mode;
    let m = match mode.as_str() {
        "light" => Mode::Light,
        "advanced" => Mode::Advanced,
        "drafting" => Mode::Drafting,
        other => return Err(format!("unknown mode '{other}'")),
    };
    flow.toggle_recording(&app, m, false);
    Ok(())
}

/// Whether text auto-paste will work. On macOS this reflects the
/// Accessibility permission (required for CGEvent injection + the Cmd+V
/// fallback). On Windows/Linux there's no such gate, so it's always `true`.
/// The frontend shows a setup nudge when this is `false`.
#[tauri::command]
pub fn accessibility_ok() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::inject::macos::is_accessibility_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Repair a macOS Accessibility grant that System Settings shows as enabled
/// but that the app does not actually hold.
///
/// This state is normal after an update, not a corruption: macOS files the
/// grant against the app's code-signing identity, our builds are ad-hoc signed,
/// and an ad-hoc signature changes hash on every build. The entry survives,
/// pointing at the binary that no longer exists — which is why the switch reads
/// ON while `AXIsProcessTrusted` reads false, and why toggling the switch does
/// not help. Removing the entry and re-granting is what rebinds it.
///
/// Returns whether the app is trusted immediately afterwards. Expect `false`:
/// the grant normally does not reach a running process until it restarts, so
/// the caller should ask the user to relaunch rather than treat this as failure.
#[tauri::command]
pub fn repair_accessibility() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = crate::inject::macos::reset_accessibility_grant() {
            // Worth continuing rather than bailing: the prompt below can still
            // register the current binary if no stale entry was in the way.
            tracing::warn!("accessibility reset failed, prompting anyway: {e:#}");
        }
        let trusted = crate::inject::macos::prompt_for_accessibility();
        tracing::info!(trusted, "accessibility repair requested");
        Ok(trusted)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// What this build actually is and what the OS actually thinks of it.
///
/// Exists because the two hardest bugs in this app are both invisible from the
/// machine most of its code is written on: a macOS window's Space pinning, and
/// whether the running binary holds Accessibility. Both were diagnosed by
/// reasoning rather than measurement, and one of those diagnoses was wrong. The
/// numbers here are read back from the live NSWindow and from macOS itself, so
/// a user can copy them into a bug report and end the argument.
#[derive(Serialize)]
pub struct PlatformDiagnostic {
    /// `CARGO_PKG_VERSION` — the string the updater compares, and the one that
    /// settles "did the update actually install".
    pub version: String,
    pub os: String,
    /// The running executable, and the `.app` bundle containing it. A grant
    /// filed against a *different* copy of the app looks exactly like a grant
    /// that does not work, so the path is part of the evidence.
    pub exe_path: String,
    pub bundle_path: Option<String>,
    pub accessibility_trusted: bool,
    pub floater_visible: bool,
    /// macOS `NSWindow.level`. 25 (`NSStatusWindowLevel`) is the pinned value;
    /// 3 (`NSFloatingWindowLevel`) means something reset it and the floater
    /// cannot paint over a full-screen Space.
    pub floater_level: Option<i64>,
    /// macOS `NSWindow.collectionBehavior`. Bit 0 (`CanJoinAllSpaces`) is the
    /// one that makes the avatar follow the user between desktops; bit 8
    /// (`FullScreenAuxiliary`) is what lets it sit over a full-screen app.
    pub floater_collection_behavior: Option<u64>,
    /// Both of the above are what we asked for.
    pub floater_pinned: bool,
    /// macOS `NSWindow.isOnActiveSpace` right now — does the OS consider the
    /// floater present on the desktop in front of the user?
    pub floater_on_active_space: Option<bool>,
    /// The last two minutes of that answer, oldest → newest, sampled every 2s.
    /// `1` on the active Space, `0` not, `·` hidden. Swipe to another desktop,
    /// wait, swipe back, and read it: a run of `0`s means macOS is not honouring
    /// the window's collection behavior and the fix is at the application
    /// level; a solid run of `1`s means it IS there and simply is not being
    /// drawn, which is a compositing bug and has nothing to do with Spaces.
    pub floater_space_timeline: String,
    /// Visible samples counted (on-active, off-active).
    pub floater_space_on: usize,
    pub floater_space_off: usize,
}

#[tauri::command]
pub fn platform_diagnostic(app: AppHandle) -> PlatformDiagnostic {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| format!("<unavailable: {e}>"));

    // Walk up to the enclosing `.app`, if there is one. A build run straight
    // from `target/` has none, which is itself worth seeing.
    let bundle_path = std::env::current_exe().ok().and_then(|p| {
        p.ancestors()
            .find(|a| a.extension().map(|e| e == "app").unwrap_or(false))
            .map(|a| a.display().to_string())
    });

    let clippy = app.get_webview_window("clippy");
    let floater_visible = clippy
        .as_ref()
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    #[cfg(target_os = "macos")]
    let (floater_level, floater_collection_behavior) = clippy
        .as_ref()
        .and_then(read_floater_pin)
        .map(|(l, b)| (Some(l), Some(b)))
        .unwrap_or((None, None));
    #[cfg(not(target_os = "macos"))]
    let (floater_level, floater_collection_behavior) = (None, None);

    // 25 and the CanJoinAllSpaces | FullScreenAuxiliary bits — see
    // `crate::macos_pin_floater`, which is where these values come from.
    let floater_pinned = floater_level == Some(25)
        && floater_collection_behavior
            .map(|b| b & 1 != 0 && b & (1 << 8) != 0)
            .unwrap_or(false);

    let probe = app.try_state::<crate::space_probe::SpaceProbe>();
    let (on_count, off_count) = probe.as_ref().map(|p| p.counts()).unwrap_or((0, 0));

    PlatformDiagnostic {
        floater_on_active_space: crate::space_probe::sample_once(&app).map(|s| s.on_active_space),
        floater_space_timeline: probe
            .as_ref()
            .map(|p| p.timeline())
            .unwrap_or_default(),
        floater_space_on: on_count,
        floater_space_off: off_count,
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        exe_path: exe,
        bundle_path,
        accessibility_trusted: accessibility_ok(),
        floater_visible,
        floater_level,
        floater_collection_behavior,
        floater_pinned,
    }
}

/// Read the floater's live `level` and `collectionBehavior` off the NSWindow.
///
/// Both are main-thread-only reads, and this command runs on a worker thread,
/// so the value comes back over a channel. The timeout is the point: a missed
/// main-thread hop must degrade to "unknown" rather than hanging a settings
/// page forever.
#[cfg(target_os = "macos")]
fn read_floater_pin<R: tauri::Runtime>(w: &tauri::WebviewWindow<R>) -> Option<(i64, u64)> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let (tx, rx) = std::sync::mpsc::channel::<Option<(i64, u64)>>();
    let win = w.clone();
    if w.run_on_main_thread(move || {
        let read = (|| {
            let ptr = win.ns_window().ok()?;
            if ptr.is_null() {
                return None;
            }
            // SAFETY: on the main thread, against the live NSWindow.
            unsafe {
                let ns_window = ptr as *mut AnyObject;
                let level: isize = msg_send![ns_window, level];
                let behavior: usize = msg_send![ns_window, collectionBehavior];
                Some((level as i64, behavior as u64))
            }
        })();
        let _ = tx.send(read);
    })
    .is_err()
    {
        return None;
    }
    rx.recv_timeout(std::time::Duration::from_secs(2)).ok().flatten()
}

/// Open the OS pane where the user grants the permission auto-paste needs.
/// macOS deep-links straight to Privacy & Security → Accessibility; other
/// platforms are a no-op (the gate doesn't exist there).
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretCheck {
    pub stt: bool,
    pub llm: bool,
    pub gemini: bool,
    pub openai_stt: bool,
    pub openai_llm: bool,
    pub deepgram_stt: bool,
    pub elevenlabs_stt: bool,
    pub any_stt: bool,
}

#[tauri::command]
pub fn check_secrets() -> SecretCheck {
    let stt = secrets::has(SecretKey::GroqStt);
    let openai_stt = secrets::has(SecretKey::OpenAiStt);
    let openai_llm = secrets::has(SecretKey::OpenAiLlm);
    let deepgram_stt = secrets::has(SecretKey::DeepgramStt);
    let elevenlabs_stt = secrets::has(SecretKey::ElevenLabsStt);
    SecretCheck {
        stt,
        llm: secrets::has(SecretKey::GroqLlm),
        gemini: secrets::has(SecretKey::GeminiLlm),
        openai_stt,
        openai_llm,
        deepgram_stt,
        elevenlabs_stt,
        any_stt: stt || openai_stt || openai_llm || deepgram_stt || elevenlabs_stt,
    }
}

/// Where each saved secret currently lives (keyring / file / none) plus
/// whether the keyring backend works on this machine. Used by the Settings
/// page's "Storage status" panel and during support diagnostics.
#[tauri::command]
pub fn secrets_diagnostic() -> secrets::SecretsDiagnostic {
    secrets::diagnostic()
}

/// Recent no-secret key-storage events for Settings -> Security.
#[tauri::command]
pub fn secret_audit_log(limit: Option<usize>) -> Vec<secrets::SecretAuditEvent> {
    secrets::audit_log(limit)
}

fn parse_secret_key(name: &str) -> Result<SecretKey, String> {
    match name {
        "groq_stt" => Ok(SecretKey::GroqStt),
        "groq_llm" => Ok(SecretKey::GroqLlm),
        "gemini_llm" => Ok(SecretKey::GeminiLlm),
        "openai_stt" => Ok(SecretKey::OpenAiStt),
        "openai_llm" => Ok(SecretKey::OpenAiLlm),
        "deepgram_stt" => Ok(SecretKey::DeepgramStt),
        "elevenlabs_stt" => Ok(SecretKey::ElevenLabsStt),
        other => Err(format!("unknown secret key '{other}'")),
    }
}

#[tauri::command]
pub fn save_secret(key: String, value: String) -> Result<(), String> {
    let k = parse_secret_key(&key)?;
    secrets::set(k, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_secret(key: String) -> Result<(), String> {
    let k = parse_secret_key(&key)?;
    secrets::delete(k).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(flow: State<'_, Flow>) -> AppSettings {
    flow.settings()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    flow: State<'_, Flow>,
    settings: AppSettings,
) -> Result<(), String> {
    let new_hotkeys = crate::hotkey::HotkeyConfig::from_settings(&settings);
    let old = flow.set_settings(settings);
    let old_hotkeys = crate::hotkey::HotkeyConfig::from_settings(&old);

    // Startup initially registers Rust defaults. The first settings-store push
    // replaces those with the persisted/custom bindings. Later unrelated
    // preference writes do not churn registrations, and a binding capture
    // remains suspended until its explicit apply_hotkeys resume.
    if old_hotkeys != new_hotkeys {
        crate::hotkey::refresh_if_live(&app, &new_hotkeys)
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_history(history: State<'_, History>, limit: Option<i64>) -> Result<Vec<Recording>, String> {
    history
        .list_recent(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_recording(history: State<'_, History>, id: String) -> Result<(), String> {
    if let Some(r) = history.get(&id).map_err(|e| e.to_string())? {
        let _ = std::fs::remove_file(&r.audio_path);
    }
    history.delete(&id).map_err(|e| e.to_string())
}

// ──────────────────────────────────────────────────────────────────────────
// Accounts + cross-device sync (v3.0.0)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct AuthStatus {
    /// Whether this build has a real Supabase project baked in. When false,
    /// the account UI shows "Sync not configured in this build" and behaves
    /// signed-out no matter what.
    pub configured: bool,
    pub signed_in: bool,
    /// A stored session is being restored right now (launch-time refresh in
    /// flight). `signed_in` is still false, but the UI must NOT render "Not
    /// signed in" — that's the transient state that made the app look like it
    /// had logged itself out on every restart.
    pub restoring: bool,
    pub email: Option<String>,
    pub user_id: Option<String>,
}

#[tauri::command]
pub fn auth_status() -> AuthStatus {
    let configured = crate::sync::config::is_configured();
    let user = if configured {
        crate::sync::auth::current_user()
    } else {
        None
    };
    AuthStatus {
        configured,
        signed_in: user.is_some(),
        restoring: configured && user.is_none() && crate::sync::auth::is_restoring(),
        email: user.as_ref().map(|u| u.email.clone()),
        user_id: user.as_ref().map(|u| u.user_id.clone()),
    }
}

/// Broadcast the current auth status to every webview. The frontend polls
/// `auth_status` once on mount, which is not enough on its own: the launch
/// restore resolves asynchronously (and sign-in/sign-out originate in one
/// window but affect the sidebar in another). Anything that changes the
/// signed-in state calls this.
pub fn emit_auth_status(app: &AppHandle) {
    let _ = app.emit("wispr:auth_status", auth_status());
}

/// Shared post-sign-in bookkeeping: on first sign-in, mark all existing
/// `done` rows dirty so the user's whole local history pushes up, then kick
/// a sync cycle in the background.
fn after_sign_in(app: &AppHandle) {
    // Tell every window immediately — the sidebar's account state and the
    // History page's "Everywhere" delete option must not wait for a remount.
    emit_auth_status(app);
    if let Some(history) = app.try_state::<History>() {
        match history.mark_all_done_dirty() {
            Ok(n) => tracing::info!(rows = n, "sync: marked existing history dirty for initial push"),
            Err(e) => tracing::warn!("sync: initial dirty-mark failed: {e:#}"),
        }
    }
    if let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() {
        let engine = engine.inner().clone();
        tauri::async_runtime::spawn(async move {
            engine.sync_once().await;
        });
    }
}

#[tauri::command]
pub async fn sign_in_email(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<AuthStatus, String> {
    crate::sync::auth::sign_in_email(email, password)
        .await
        .map_err(|e| format!("{e:#}"))?;
    after_sign_in(&app);
    Ok(auth_status())
}

#[tauri::command]
pub async fn sign_up_email(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<AuthStatus, String> {
    crate::sync::auth::sign_up_email(email, password)
        .await
        .map_err(|e| format!("{e:#}"))?;
    after_sign_in(&app);
    Ok(auth_status())
}

#[tauri::command]
pub async fn sign_in_google(app: AppHandle) -> Result<AuthStatus, String> {
    crate::sync::auth::sign_in_google(app.clone())
        .await
        .map_err(|e| format!("{e:#}"))?;
    after_sign_in(&app);
    Ok(auth_status())
}

#[tauri::command]
pub fn cancel_google_sign_in() {
    crate::sync::auth::cancel_google_sign_in();
}

#[tauri::command]
pub async fn sign_out(app: AppHandle) -> Result<AuthStatus, String> {
    crate::sync::auth::sign_out().await;
    // Drop the cached fleet with the session. Without this, signing out and
    // into a DIFFERENT account shows the previous account's device list until
    // the first sync lands.
    if let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() {
        crate::sync::fleet::clear_cache(engine.history_ref());
    }
    emit_auth_status(&app);
    Ok(auth_status())
}

/// Every device signed into this account, with its assigned icon/label and
/// its published analytics. Hits the network; falls back to the local cache
/// when offline so Settings still renders something truthful.
#[tauri::command]
pub async fn list_devices(app: AppHandle) -> Result<Vec<crate::sync::fleet::FleetDevice>, String> {
    let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() else {
        return Ok(Vec::new());
    };
    let engine = engine.inner().clone();
    Ok(engine.fleet_now().await)
}

/// The cached fleet with no network round-trip — for painting the UI on mount
/// before `list_devices` resolves.
#[tauri::command]
pub fn list_devices_cached(app: AppHandle) -> Vec<crate::sync::fleet::FleetDevice> {
    app.try_state::<crate::sync::engine::SyncEngine>()
        .map(|e| e.fleet_cached())
        .unwrap_or_default()
}

/// Assign an icon and/or display label to any device in the account (not just
/// this one — the user is as likely to label the laptop from the desktop).
#[tauri::command]
pub async fn set_device_meta(
    app: AppHandle,
    device_id: String,
    icon: Option<String>,
    label: Option<String>,
) -> Result<Vec<crate::sync::fleet::FleetDevice>, String> {
    let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() else {
        return Err("sync is not configured in this build".to_string());
    };
    let engine = engine.inner().clone();
    // Empty strings from a cleared text field mean "unset", not "set to
    // empty" — otherwise clearing a label leaves a blank name on the card.
    let meta = crate::sync::fleet::DeviceMeta {
        icon: icon.filter(|s| !s.trim().is_empty()),
        label: label.filter(|s| !s.trim().is_empty()),
    };
    engine
        .set_device_meta(&device_id, &meta)
        .await
        .map_err(|e| e.to_string())
}

/// Manual "Sync now" from Settings → Account.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<(), String> {
    if let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() {
        let engine = engine.inner().clone();
        engine.sync_once().await;
    }
    Ok(())
}

/// Update this install's device name (shown as "Desktop · <name>" on synced
/// rows). Persists into the live Flow settings; the frontend also writes it
/// into its own settings store so it survives a restart.
#[tauri::command]
pub fn set_device_name(flow: State<'_, Flow>, name: String) {
    let mut s = flow.settings();
    s.device_name = name;
    flow.set_settings(s);
}

/// Ownership-scoped delete (v3.0.0, supersedes the What/Where matrix). See
/// SYNC_DESIGN.md "Delete — ownership-scoped": a client may delete only the
/// transcripts it originated. Locally that is any non-`remote` row — rows with
/// `remote = 1` were pulled from another device and belong to it, so they are
/// refused here (and the UI hides their delete control entirely). A transcript
/// and its recording die together — there is no audio-only option any more.
///
/// For owned rows we also tombstone the cloud copy (`deleted_at` + nulled text,
/// scoped to our `device_id`) so every other device drops it on its next pull.
/// Deleting a row whose local audio is already gone succeeds quietly.
///
/// `ids = None` means "all of THIS device's recordings" — other devices'
/// synced transcripts are untouched, locally and on the server.
#[tauri::command]
pub async fn delete_recordings(
    app: AppHandle,
    history: State<'_, History>,
    ids: Option<Vec<String>>,
) -> Result<u64, String> {
    // Resolve the target rows.
    let targets: Vec<Recording> = match ids {
        Some(list) => {
            let mut v = Vec::with_capacity(list.len());
            for id in list {
                if let Some(r) = history.get(&id).map_err(|e| e.to_string())? {
                    v.push(r);
                }
            }
            v
        }
        None => history.list_recent(100_000).map_err(|e| e.to_string())?,
    };

    // Ownership guard: only rows this device originated may be deleted. Remote
    // rows (pulled from another device) are silently skipped — the UI never
    // offers a delete control on them, so this is belt-and-suspenders.
    let owned: Vec<Recording> = targets.into_iter().filter(|r| !r.remote).collect();
    if owned.is_empty() {
        return Ok(0);
    }
    let owned_ids: Vec<String> = owned.iter().map(|r| r.id.clone()).collect();

    // Cloud tombstones first (best-effort) so other devices converge even if
    // the local half below hits a snag. Scoped to our device_id server-side.
    let signed_in = crate::sync::config::is_configured()
        && crate::sync::auth::current_user().is_some();
    if signed_in {
        if let Some(engine) = app.try_state::<crate::sync::engine::SyncEngine>() {
            let device_id = engine.device_id();
            crate::sync::engine::tombstone_remote(&owned_ids, &device_id).await;
        }
    }

    let mut affected = 0u64;
    for rec in &owned {
        // Transcript and audio die together. A row whose audio was already
        // pruned (GC) or never had a local file still deletes cleanly — the
        // remove_file error is ignored on purpose.
        let _ = std::fs::remove_file(&rec.audio_path);
        history.delete(&rec.id).map_err(|e| e.to_string())?;
        affected += 1;
    }

    // Wipe now-empty audio date-folders, matching clear_all_history's "leave
    // nothing behind".
    if let Ok(dir) = app.path().app_data_dir() {
        let audio = dir.join("audio");
        if audio.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&audio) {
                for e in entries.flatten() {
                    if e.path().is_dir() {
                        if let Ok(mut c) = std::fs::read_dir(e.path()) {
                            if c.next().is_none() {
                                let _ = std::fs::remove_dir(e.path());
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = app.emit("wispr:history_changed", ());
    Ok(affected)
}

/// Initiate an account-wide purge — the deliberate "reset my entire account
/// history everywhere" escape hatch, and the only delete allowed to cross
/// device ownership (it also clears undeletable orphans whose originating
/// device is gone). Sets the synced `purged_at` marker, hard-deletes every
/// note server-side, and wipes local state. Destructive and irreversible; the
/// UI gates it behind press-and-hold + an explicit confirm. No-op-safe when
/// signed out (returns an error the caller surfaces) — the control is only
/// shown while signed in.
#[tauri::command]
pub async fn purge_account(app: AppHandle) -> Result<(), String> {
    let engine = app
        .try_state::<crate::sync::engine::SyncEngine>()
        .ok_or_else(|| "sync engine unavailable".to_string())?
        .inner()
        .clone();
    engine.purge_all().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn retry_recording(
    app: AppHandle,
    flow: State<'_, Flow>,
    id: String,
) -> Result<(), String> {
    flow.retry_recording(&app, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rerun_transcription(
    app: AppHandle,
    flow: State<'_, Flow>,
    id: String,
    stt_provider: Option<String>,
    stt_model: Option<String>,
    diarize: bool,
) -> Result<(), String> {
    flow.retry_recording_with(
        &app,
        &id,
        stt_provider,
        stt_model,
        diarize,
        false,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Transcribe a user-supplied audio file (drag-and-drop or the file picker).
/// `path` is an absolute path on disk; the backend copies it into the audio
/// store, runs STT + optional cleanup/draft, and files it in History with an
/// "Uploaded" badge. Provider/model args are per-batch overrides (null = use
/// the current global setting). Returns the new recording id.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn transcribe_upload(
    app: AppHandle,
    flow: State<'_, Flow>,
    path: String,
    stt_provider: Option<String>,
    stt_model: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    draft_llm_provider: Option<String>,
    draft_llm_model: Option<String>,
    cleanup: bool,
    draft: bool,
    diarize: bool,
    meeting_notes: bool,
) -> Result<String, String> {
    flow.transcribe_file(
        &app,
        &path,
        stt_provider,
        stt_model,
        llm_provider,
        llm_model,
        draft_llm_provider,
        draft_llm_model,
        cleanup,
        draft,
        diarize,
        meeting_notes,
    )
    .await
    .map_err(|e| format!("{e:#}"))
}

/// Generate a "cleaned" or "drafted" variant for an existing recording.
/// Used by the History UI tabs: clicking a dimmed tab (Cleaned or Drafted
/// version not yet generated) calls this, the LLM runs against the raw
/// transcript with the appropriate prompt, the result is saved into the
/// matching column, and the returned text is shown in the tab.
#[tauri::command]
pub async fn generate_alt_version(
    flow: State<'_, Flow>,
    id: String,
    kind: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    flow.generate_alt_version(&id, &kind, provider, model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_speaker_names(
    app: AppHandle,
    history: State<'_, History>,
    id: String,
    names_json: String,
) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(&names_json)
        .map_err(|e| format!("invalid speaker names: {e}"))?;
    if !value.is_object() {
        return Err("speaker names must be a JSON object".to_string());
    }
    history.set_speaker_names(&id, &names_json).map_err(|e| e.to_string())?;
    let _ = app.emit("wispr:history_changed", ());
    Ok(())
}

/// Returns a `tauri://localhost` URL the frontend can use as an `<audio src>`
/// to play back a saved recording. Falls back to the file path if conversion
/// fails (the frontend then needs `convertFileSrc` from Tauri's API).
#[tauri::command]
pub fn audio_url_for(history: State<'_, History>, id: String) -> Result<String, String> {
    let rec = history
        .get(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "recording not found".to_string())?;
    Ok(rec.audio_path.to_string_lossy().into_owned())
}

/// Returns a `data:audio/wav;base64,...` URL for a recording's WAV file.
/// Bypasses the Tauri asset protocol entirely (which has scope/glob issues
/// on Windows for the AppData path). Slightly heavier than a streamed URL
/// because base64 inflates by ~33%, but dictation clips are short.
#[tauri::command]
pub fn audio_data_url_for(
    history: State<'_, History>,
    id: String,
) -> Result<String, String> {
    use base64::Engine;
    let rec = history
        .get(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "recording not found".to_string())?;
    // Remote rows (synced from another device) never have local audio, and
    // audio-only deletes blank the path — return a clear, expected error the
    // frontend surfaces gracefully rather than a raw filesystem failure.
    if rec.remote || rec.audio_path.as_os_str().is_empty() {
        return Err("audio not available on this device".to_string());
    }
    let bytes = std::fs::read(&rec.audio_path)
        .map_err(|e| format!("read {}: {e}", rec.audio_path.display()))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    // Uploaded files may be m4a/mp3/etc., not WAV — advertise the right MIME so
    // the <audio> element in History plays them back correctly.
    let mime = crate::stt::mime_for_audio(&rec.audio_path);
    Ok(format!("data:{mime};base64,{b64}"))
}

#[tauri::command]
pub fn daily_usage(usage: State<'_, UsageTracker>) -> DailyUsage {
    usage.snapshot()
}

/// Lifetime analytics rollup for the stats dashboard (per-day rows + totals).
/// Reads the dedicated `daily_stats` table, which survives history retention.
#[tauri::command]
pub fn stats_summary(history: State<'_, History>) -> Result<crate::history::StatsSummary, String> {
    history.stats_summary().map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct DefaultPrompts {
    pub light: &'static str,
    pub advanced: &'static str,
    pub drafting: &'static str,
    pub meeting: &'static str,
}

/// Return the baked-in default system prompts for each mode. Used by the
/// Settings UI to seed the editor + power the Reset button.
#[tauri::command]
pub fn get_default_prompts() -> DefaultPrompts {
    DefaultPrompts {
        light: crate::llm::prompts::LIGHT_SYSTEM,
        advanced: crate::llm::prompts::ADVANCED_SYSTEM,
        drafting: crate::llm::prompts::DRAFTING_SYSTEM,
        meeting: crate::llm::prompts::MEETING_NOTES_SYSTEM,
    }
}

#[derive(Serialize)]
pub struct CurrentModels {
    pub stt: String,
    pub llm_light: String,
    pub llm_advanced: String,
}

#[tauri::command]
pub fn current_models(flow: State<'_, Flow>) -> CurrentModels {
    let s = flow.settings();
    CurrentModels {
        stt: s.stt_model,
        llm_light: s.llm_model.clone(),
        llm_advanced: s.llm_model,
    }
}

#[tauri::command]
pub fn clear_all_history(app: AppHandle, history: State<'_, History>) -> Result<u64, String> {
    let recs = history.list_recent(10_000).map_err(|e| e.to_string())?;
    let mut removed = 0u64;
    for r in &recs {
        let _ = std::fs::remove_file(&r.audio_path);
        if history.delete(&r.id).is_ok() {
            removed += 1;
        }
    }
    // Hard clear: also wipe the entire audio directory so NO orphaned .wav
    // files or date-folders survive on disk (the DB only tracks files it
    // created — a real "clear everything" should leave nothing behind). The
    // folder is recreated empty so the next recording has somewhere to land.
    if let Ok(dir) = app.path().app_data_dir() {
        let audio = dir.join("audio");
        if audio.is_dir() {
            let _ = std::fs::remove_dir_all(&audio);
            let _ = std::fs::create_dir_all(&audio);
        }
    }
    Ok(removed)
}

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<audio::devices::InputDeviceInfo>, String> {
    audio::devices::list().map_err(|e| e.to_string())
}

/// Open a metering-only capture stream on `device` (null = system default) so
/// the user can confirm the mic actually hears them before it costs them a
/// dictation. Returns the RESOLVED device name.
///
/// This is the only reliable way to catch a phantom-connected Bluetooth mic:
/// after a sleep/wake cycle a transmitter can keep its "connected" indicator
/// and stay listed by the OS while delivering no audio whatsoever. Enumeration
/// says it's there; only a live meter says it works.
#[tauri::command]
pub async fn start_mic_test(
    flow: State<'_, Flow>,
    device: Option<String>,
) -> Result<String, String> {
    flow.start_mic_test(device)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn stop_mic_test(flow: State<'_, Flow>) -> Result<(), String> {
    flow.stop_mic_test().await.map_err(|e| format!("{e:#}"))
}

/// Tear down every dictation hotkey until `apply_hotkeys` puts them back.
///
/// Called by the rebinding UI. Global shortcuts win over the focused webview,
/// so with F8 still registered, pressing F8 into the "press your hotkey" dialog
/// started a *recording* and the keypress never reached the DOM listener —
/// making the picker look broken for exactly the keys people actually use.
/// Every app with a rebind UI suspends its shortcuts for the capture; so do we.
#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) {
    crate::hotkey::suspend(&app);
}

/// Re-register the dictation hotkeys from the CURRENT settings. Doubles as
/// "resume after a capture" and "the user saved a new binding, make it live" —
/// which is why rebinding no longer asks for an app restart.
#[tauri::command]
pub fn apply_hotkeys(app: AppHandle, flow: State<'_, Flow>) -> Result<(), String> {
    let cfg = crate::hotkey::HotkeyConfig::from_settings(&flow.settings());
    crate::hotkey::apply(&app, &cfg).map_err(|e| format!("{e:#}"))
}

/// Whether dictation hotkeys are currently live. Lets the settings UI show
/// honest state instead of assuming.
#[tauri::command]
pub fn hotkeys_active() -> bool {
    crate::hotkey::is_active()
}

#[derive(Serialize)]
pub struct AppPaths {
    pub audio_dir: PathBuf,
    pub db_path: PathBuf,
    pub sounds_dir: PathBuf,
}

#[tauri::command]
pub fn app_paths(app: AppHandle) -> Result<AppPaths, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(AppPaths {
        audio_dir: dir.join("audio"),
        db_path: dir.join("history.sqlite"),
        sounds_dir: dir.join("sounds"),
    })
}

/// Reveal a wispr-fox folder in the OS file manager. `kind` is one of:
///   "audio"   → %APPDATA%/com.wispr-fox.app/audio/
///   "sounds"  → %APPDATA%/com.wispr-fox.app/sounds/
///   "data"    → %APPDATA%/com.wispr-fox.app/ (parent)
///   "avatars" → %APPDATA%/com.wispr-fox.app/avatars/ (user-installed avatars)
///
/// Creates the directory if missing (some, like `sounds` and `avatars`, are
/// only auto-created the first time the user drops a file in — the button
/// has to create them so explorer.exe doesn't silently bounce to Documents).
#[tauri::command]
pub fn reveal_folder(app: AppHandle, kind: String) -> Result<(), String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let path = match kind.as_str() {
        "audio" => base.join("audio"),
        "sounds" => base.join("sounds"),
        "avatars" => base.join("avatars"),
        "data" => base,
        other => return Err(format!("unknown folder kind '{other}'")),
    };
    if let Err(e) = std::fs::create_dir_all(&path) {
        return Err(format!("create_dir_all({}) failed: {e}", path.display()));
    }
    // Use tauri-plugin-opener via the Rust API. The `_ = ` pattern keeps the
    // result quiet — opener returns Ok even when explorer.exe can't open the
    // path (silent fallback to Documents on Windows). The mkdir above is the
    // real safety net.
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("open failed: {e}"))
}

/// Copy a user-picked file into the sounds folder so it shows up in the picker.
/// Returns the final filename (preserving the source basename, deduplicated
/// with a numeric suffix if needed).
#[tauri::command]
pub fn add_notification_sound(src_path: String) -> Result<String, String> {
    let src = PathBuf::from(&src_path);
    if !src.is_file() {
        return Err(format!("not a file: {src_path}"));
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "wav" | "mp3" | "ogg") {
        return Err("only .wav / .mp3 / .ogg files supported".to_string());
    }
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.wispr-fox.app")
        .join("sounds");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("sound");
    let mut name = format!("{stem}.{ext}");
    let mut counter = 1;
    while base.join(&name).exists() {
        name = format!("{stem}-{counter}.{ext}");
        counter += 1;
    }
    let dest = base.join(&name);
    std::fs::copy(&src, &dest).map_err(|e| format!("copy: {e}"))?;
    Ok(name)
}

/// List filenames in the user's sounds folder, filtered to audio extensions
/// (.wav / .mp3 / .ogg). Frontend uses this for the notification-sound picker.
#[tauri::command]
pub fn list_notification_sounds() -> Vec<String> {
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.wispr-fox.app")
        .join("sounds");
    let _ = std::fs::create_dir_all(&base);

    let mut out: Vec<String> = std::fs::read_dir(&base)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.ends_with(".wav") || lower.ends_with(".mp3") || lower.ends_with(".ogg") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    out.sort();
    out
}

/// Push audio-cue config into the cue worker. Called whenever the user
/// saves a sound choice in Settings.
#[tauri::command]
pub fn configure_cues(start: String, stop: String, enabled: bool) {
    crate::audio::cues::configure(&start, &stop, enabled);
}

/// Test the currently-saved Groq key (no need to paste it again).
/// Reads the key from secret storage, runs the same test as test_groq_key.
#[tauri::command]
pub async fn test_saved_groq_key() -> Result<Vec<String>, String> {
    let key = secrets::get(SecretKey::GroqLlm)
        .map_err(|e| e.to_string())?
        .or_else(|| secrets::get(SecretKey::GroqStt).ok().flatten())
        .ok_or_else(|| "No Groq key saved yet — paste one above first.".to_string())?;
    test_groq_key(key).await
}

/// Test the currently-saved Gemini key.
#[tauri::command]
pub async fn test_saved_gemini_key() -> Result<Vec<String>, String> {
    let key = secrets::get(SecretKey::GeminiLlm)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No Gemini key saved yet — paste one above first.".to_string())?;
    test_gemini_key(key).await
}

/// Test the currently-saved OpenAI key.
#[tauri::command]
pub async fn test_saved_openai_key() -> Result<Vec<String>, String> {
    let key = secrets::get(SecretKey::OpenAiLlm)
        .map_err(|e| e.to_string())?
        .or_else(|| secrets::get(SecretKey::OpenAiStt).ok().flatten())
        .ok_or_else(|| "No OpenAI key saved yet - paste one above first.".to_string())?;
    test_openai_key(key).await
}

/// Test the currently-saved Deepgram key.
#[tauri::command]
pub async fn test_saved_deepgram_key() -> Result<Vec<String>, String> {
    let key = secrets::get(SecretKey::DeepgramStt)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No Deepgram key saved yet - paste one above first.".to_string())?;
    test_deepgram_key(key).await
}

/// Test the currently-saved ElevenLabs key.
#[tauri::command]
pub async fn test_saved_elevenlabs_key() -> Result<Vec<String>, String> {
    let key = secrets::get(SecretKey::ElevenLabsStt)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No ElevenLabs key saved yet - paste one above first.".to_string())?;
    test_elevenlabs_key(key).await
}

/// Test a Google Gemini API key by listing available models.
#[tauri::command]
pub async fn test_gemini_key(key: String) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("key is empty".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .header("x-goog-api-key", key.trim())
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), body));
    }
    #[derive(Deserialize)]
    struct Resp {
        models: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        name: String,
    }
    let parsed: Resp = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|m| m.name.trim_start_matches("models/").to_string())
        .collect())
}

/// Test an OpenAI API key by listing available models.
#[tauri::command]
pub async fn test_openai_key(key: String) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("key is empty".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(key.trim())
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), body));
    }
    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
    }
    let parsed: ModelsResponse = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    Ok(parsed.data.into_iter().map(|m| m.id).collect())
}

/// Test a Deepgram API key by listing projects.
#[tauri::command]
pub async fn test_deepgram_key(key: String) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("key is empty".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.deepgram.com/v1/projects")
        .header("Authorization", format!("Token {}", key.trim()))
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), body));
    }
    #[derive(Deserialize)]
    struct ProjectsResponse {
        projects: Vec<ProjectEntry>,
    }
    #[derive(Deserialize)]
    struct ProjectEntry {
        #[serde(default)]
        project_id: String,
        #[serde(default)]
        name: Option<String>,
    }
    let parsed: ProjectsResponse = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    Ok(parsed
        .projects
        .into_iter()
        .map(|p| p.name.unwrap_or(p.project_id))
        .collect())
}

/// Test an ElevenLabs API key by fetching account metadata.
#[tauri::command]
pub async fn test_elevenlabs_key(key: String) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("key is empty".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.elevenlabs.io/v1/user")
        .header("xi-api-key", key.trim())
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), body));
    }
    Ok(vec!["account reachable".to_string()])
}

/// Test a Groq API key by making a minimal authenticated request. Returns the
/// list of model ids the key has access to, or an error message. Used by the
/// Settings page "Test connection" button.
#[tauri::command]
pub async fn test_groq_key(key: String) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("key is empty".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.groq.com/openai/v1/models")
        .bearer_auth(key.trim())
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), body));
    }
    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
    }
    let parsed: ModelsResponse = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    Ok(parsed.data.into_iter().map(|m| m.id).collect())
}
