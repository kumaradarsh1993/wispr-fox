//! Frontend command surface. Thin wrappers — all logic lives in domain modules.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::audio;
use crate::flow::Flow;
use crate::history::{History, Recording};
use crate::secrets::{self, SecretKey};
use crate::settings::AppSettings;
use crate::usage::{DailyUsage, UsageTracker};

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
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
        let _ = window.set_always_on_top(true);
        return;
    }
    #[allow(unreachable_code)]
    {
        let _ = window.show();
        let _ = window.set_always_on_top(true);
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
/// non-hotkey caller). Behaves like a sticky-invoke hotkey press: a second
/// call toggles recording off. `mode` is "light", "advanced", or "drafting".
#[tauri::command]
pub fn floater_trigger(
    app: AppHandle,
    flow: State<'_, Flow>,
    mode: String,
) -> Result<(), String> {
    use crate::hotkey::{Edge, HotkeyEvent};
    use crate::settings::Mode;
    let m = match mode.as_str() {
        "light" => Mode::Light,
        "advanced" => Mode::Advanced,
        "drafting" => Mode::Drafting,
        other => return Err(format!("unknown mode '{other}'")),
    };
    flow.handle_hotkey(
        &app,
        HotkeyEvent {
            mode: m,
            edge: Edge::Down,
            sticky_invoke: true,
            force_clean: false,
        },
    );
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
pub fn set_settings(flow: State<'_, Flow>, settings: AppSettings) {
    flow.set_settings(settings);
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
) -> Result<String, String> {
    flow.generate_alt_version(&id, &kind)
        .await
        .map_err(|e| e.to_string())
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
    let bytes = std::fs::read(&rec.audio_path)
        .map_err(|e| format!("read {}: {e}", rec.audio_path.display()))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:audio/wav;base64,{b64}"))
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
}

/// Return the baked-in default system prompts for each mode. Used by the
/// Settings UI to seed the editor + power the Reset button.
#[tauri::command]
pub fn get_default_prompts() -> DefaultPrompts {
    DefaultPrompts {
        light: crate::llm::prompts::LIGHT_SYSTEM,
        advanced: crate::llm::prompts::ADVANCED_SYSTEM,
        drafting: crate::llm::prompts::DRAFTING_SYSTEM,
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

// ──────────────────────────────────────────────────────────────────────────
// Update check (manual — auto-updater is separate work)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UpdateInfo {
    /// Version currently installed (from `CARGO_PKG_VERSION`).
    pub current: String,
    /// Newest release tag name on GitHub, e.g. `"v1.2.0"` or `"v1.2.0-nightly.3"`.
    pub latest: String,
    /// Whether `latest` is strictly newer than `current` (semver-aware,
    /// pre-releases sort below their plain version per semver 11).
    pub newer: bool,
    /// HTML URL of the latest release page — Settings opens this if the user
    /// clicks the "Open release page" link.
    pub html_url: String,
    /// `true` if the latest release is a pre-release (nightly / RC / beta).
    /// Settings shows a slightly different copy ("Nightly available") so the
    /// user knows it isn't a stable.
    pub prerelease: bool,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    prerelease: bool,
}

/// Hit GitHub's "latest releases" endpoint and compare against the current
/// build's version. We deliberately use `/releases` (not `/releases/latest`)
/// because that endpoint hides pre-releases; we want users on a nightly
/// channel to see a newer nightly too. The first entry is always the most
/// recent regardless of channel.
///
/// No auto-update — we only report. Auto-update is a separate Tauri plugin
/// (`tauri-plugin-updater`) and a bigger trust-decision than we want to make
/// without explicit owner sign-off.
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent(concat!("wispr-fox/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let url = "https://api.github.com/repos/kumaradarsh1993/wispr-fox/releases?per_page=1";
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", resp.status().as_u16()));
    }

    let releases: Vec<GhRelease> = resp
        .json()
        .await
        .map_err(|e| format!("decode: {e}"))?;

    let Some(latest) = releases.into_iter().next() else {
        return Err("no releases found".into());
    };

    // Strip leading "v" if present so we compare "1.2.0" to "1.2.0".
    let latest_clean = latest.tag_name.trim_start_matches('v').to_string();
    let newer = version_is_newer(&current, &latest_clean);

    Ok(UpdateInfo {
        current,
        latest: latest.tag_name,
        newer,
        html_url: latest.html_url,
        prerelease: latest.prerelease,
    })
}

/// Tiny semver-ish comparator. Splits on `.` and `-`, compares dotted numeric
/// parts first then any pre-release suffix lexicographically. Enough for our
/// "1.1.0-nightly.5 vs 1.1.0-nightly.6" comparisons; not a full semver
/// implementation.
fn version_is_newer(current: &str, candidate: &str) -> bool {
    fn parts(v: &str) -> (Vec<u32>, &str) {
        let (head, tail) = v.split_once('-').unwrap_or((v, ""));
        let nums: Vec<u32> = head.split('.').filter_map(|s| s.parse().ok()).collect();
        (nums, tail)
    }
    fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let mut aa = a.split('.');
        let mut bb = b.split('.');
        loop {
            match (aa.next(), bb.next()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(x), Some(y)) => {
                    let ord = match (x.parse::<u32>(), y.parse::<u32>()) {
                        (Ok(xn), Ok(yn)) => xn.cmp(&yn),
                        _ => x.cmp(y),
                    };
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
        }
    }
    let (cnums, ctail) = parts(current);
    let (lnums, ltail) = parts(candidate);
    let max_len = cnums.len().max(lnums.len());
    for i in 0..max_len {
        let c = cnums.get(i).copied().unwrap_or(0);
        let l = lnums.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    // Dotted-numeric parts equal — fall through to pre-release suffix.
    // Per semver 11: NO suffix beats ANY suffix (1.0.0 > 1.0.0-rc.1).
    match (ctail.is_empty(), ltail.is_empty()) {
        (false, true) => true,   // current is a pre-release, latest is stable → newer
        (true, false) => false,  // current is stable, latest is pre-release → not newer
        _ => compare_prerelease(ltail, ctail).is_gt(),
    }
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
