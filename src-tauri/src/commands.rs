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
    let _ = window.show();
    let _ = window.set_always_on_top(true);
    // The size-nudge is a Windows-only workaround: WebView2 loses its
    // DirectComposition surface after sleep and only a resize forces it to
    // rebuild. On macOS/Linux (WKWebView / WebKitGTK) the surface survives
    // resume, so the nudge is pointless there — and a 1px resize on a
    // transparent always-on-top floater can cause a visible jitter. Skip it.
    #[cfg(windows)]
    if let Ok(size) = window.outer_size() {
        let bumped = tauri::PhysicalSize::new(size.width + 1, size.height + 1);
        let _ = window.set_size(bumped);
        let _ = window.set_size(size);
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
}

#[tauri::command]
pub fn check_secrets() -> SecretCheck {
    SecretCheck {
        stt: secrets::has(SecretKey::GroqStt),
        llm: secrets::has(SecretKey::GroqLlm),
        gemini: secrets::has(SecretKey::GeminiLlm),
    }
}

fn parse_secret_key(name: &str) -> Result<SecretKey, String> {
    match name {
        "groq_stt" => Ok(SecretKey::GroqStt),
        "groq_llm" => Ok(SecretKey::GroqLlm),
        "gemini_llm" => Ok(SecretKey::GeminiLlm),
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
        llm_light: s.clippy_light_model,
        llm_advanced: s.clippy_advanced_model,
    }
}

#[tauri::command]
pub fn clear_all_history(history: State<'_, History>) -> Result<u64, String> {
    let recs = history.list_recent(10_000).map_err(|e| e.to_string())?;
    let mut removed = 0u64;
    for r in &recs {
        let _ = std::fs::remove_file(&r.audio_path);
        if history.delete(&r.id).is_ok() {
            removed += 1;
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
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        urlencoding::encode(key.trim())
    );
    let resp = client.get(&url).send().await.map_err(|e| format!("network: {e}"))?;
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
