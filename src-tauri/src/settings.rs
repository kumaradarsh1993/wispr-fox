//! User settings — defaults the app boots with, mutated via the settings UI.
//!
//! Persisted on the JS side via `tauri-plugin-store`. The Rust side reads them
//! through commands when needed (e.g. flow.rs reads hotkey strings + Clippy mode).

use serde::{Deserialize, Serialize};

use crate::llm::ClippyMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // ── Hotkeys ────────────────────────────────────────────────────────────
    pub light_hotkey: String,
    pub advanced_hotkey: String,
    /// F10-style "drafting" mode — user gives a brief, LLM drafts a polished output.
    pub drafting_hotkey: String,

    /// Sticky-toggle mode per hotkey. When true, the MAIN hotkey behaves as
    /// press-to-start / press-to-stop instead of push-to-talk. Useful when
    /// the user wants sticky to be the default for that mode.
    pub sticky_light: bool,
    pub sticky_advanced: bool,
    pub sticky_drafting: bool,

    /// Ad-hoc sticky hotkeys. Pressing these ALWAYS triggers a sticky toggle
    /// for the corresponding mode, regardless of the per-mode sticky flag.
    /// Defaults: Win + the same key as the main hotkey.
    pub light_sticky_hotkey: String,
    pub advanced_sticky_hotkey: String,
    pub drafting_sticky_hotkey: String,

    // ── Cleanup behaviour per mode ────────────────────────────────────────
    // Whether each mode runs the LLM cleanup step. Light defaults to OFF
    // (raw Whisper transcript is good enough; user can opt in). Advanced
    // and Drafting default to ON because their whole purpose is the
    // transformative LLM pass.
    pub auto_clean_in_light: bool,
    pub auto_clean_in_advanced: bool,
    pub auto_clean_in_drafting: bool,

    // ── Models ────────────────────────────────────────────────────────────
    // Simplified May 2026: ONE STT choice + ONE LLM choice. All three modes
    // (F8 / F9 / F10) use the same LLM client; only the system prompt
    // differs per mode. Old per-mode fields kept for backwards compat but
    // ignored by the flow layer.
    pub stt_provider: String,
    pub stt_model: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub language_hint: Option<String>,

    // Legacy per-mode fields. Kept so old settings.json files still
    // deserialize cleanly. New code uses llm_provider / llm_model.
    #[serde(default)]
    pub clippy_light_model: String,
    #[serde(default)]
    pub clippy_advanced_model: String,
    #[serde(default)]
    pub clippy_drafting_model: String,
    #[serde(default)]
    pub light_provider: String,
    #[serde(default)]
    pub advanced_provider: String,
    #[serde(default)]
    pub drafting_provider: String,

    // ── Retention ──────────────────────────────────────────────────────────
    pub retention_days: u32,
    pub retention_max_mb: u64,

    // ── System integration ─────────────────────────────────────────────────
    pub autostart: bool,

    // ── Audio cues (filename within %APPDATA%/com.wispr-fox.app/sounds/) ──
    /// Empty string = use built-in generated tone.
    pub start_sound: String,
    pub stop_sound: String,
    pub cues_enabled: bool,

    // ── Look & feel ────────────────────────────────────────────────────────
    /// "auto" | "light" | "dark" | "retro"
    pub theme: String,

    // ── Custom prompts (override built-in defaults) ───────────────────────
    // Empty string = use the baked-in default from prompts.rs.
    // Set via Settings UI. Reset button clears these strings.
    #[serde(default)]
    pub custom_light_prompt: String,
    #[serde(default)]
    pub custom_advanced_prompt: String,
    #[serde(default)]
    pub custom_drafting_prompt: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // F-keys are nearly always free system-wide; Win+Space collides with
            // the Windows IME picker, and Ctrl+Alt+Space is grabbed by various
            // background apps (NVIDIA GeForce, Logitech, Discord etc) on common
            // setups. F8/F9/F10 work out of the box; rebind from Settings if needed.
            light_hotkey: "F8".to_string(),
            advanced_hotkey: "F9".to_string(),
            drafting_hotkey: "F10".to_string(),
            sticky_light: false,
            sticky_advanced: false,
            sticky_drafting: false,
            light_sticky_hotkey: "Super+F8".to_string(),
            advanced_sticky_hotkey: "Super+F9".to_string(),
            drafting_sticky_hotkey: "Super+F10".to_string(),
            // F8 default OFF — raw Whisper is fast + accurate, no LLM tax.
            auto_clean_in_light: false,
            auto_clean_in_advanced: true,
            auto_clean_in_drafting: true,
            // Simplified globals.
            stt_provider: "groq".to_string(),
            stt_model: "whisper-large-v3-turbo".to_string(),
            llm_provider: "groq".to_string(),
            llm_model: crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string(),
            language_hint: None,
            // Legacy per-mode fields — mirror the globals so any code that
            // still reads them gets a sane value.
            clippy_light_model: crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string(),
            clippy_advanced_model: crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string(),
            clippy_drafting_model: crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string(),
            light_provider: "groq".to_string(),
            advanced_provider: "groq".to_string(),
            drafting_provider: "groq".to_string(),
            retention_days: 7,
            retention_max_mb: 500,
            autostart: false,
            start_sound: String::new(),
            stop_sound: String::new(),
            cues_enabled: true,
            // Light by default — user feedback May 2026: many surfaces still
            // need dark-mode polish, so don't auto-flip on system dark.
            theme: "light".to_string(),
            custom_light_prompt: String::new(),
            custom_advanced_prompt: String::new(),
            custom_drafting_prompt: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    Light,
    Advanced,
    Drafting,
}

impl From<Mode> for ClippyMode {
    fn from(m: Mode) -> ClippyMode {
        match m {
            Mode::Light => ClippyMode::Light,
            Mode::Advanced => ClippyMode::Advanced,
            // Drafting uses the Advanced cleanup pipeline but with its own prompt.
            Mode::Drafting => ClippyMode::Drafting,
        }
    }
}
