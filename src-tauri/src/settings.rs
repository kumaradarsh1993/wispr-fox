//! User settings — defaults the app boots with, mutated via the settings UI.
//!
//! Persisted on the JS side via `tauri-plugin-store`. The Rust side reads them
//! through commands when needed (e.g. flow.rs reads hotkey strings + Clippy mode).

use serde::{Deserialize, Serialize};

use crate::llm::ClippyMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub light_hotkey: String,
    pub advanced_hotkey: String,
    pub auto_clean_in_light: bool,
    pub clippy_light_model: String,
    pub clippy_advanced_model: String,
    pub stt_model: String,
    pub language_hint: Option<String>,
    pub retention_days: u32,
    pub retention_max_mb: u64,
    pub autostart: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // F-keys are nearly always free system-wide; Win+Space collides with
            // the Windows IME picker, and Ctrl+Alt+Space is grabbed by various
            // background apps (NVIDIA GeForce, Logitech, Discord etc) on common
            // setups. F8/F9 work out of the box; rebind from Settings if needed.
            light_hotkey: "F8".to_string(),
            advanced_hotkey: "F9".to_string(),
            auto_clean_in_light: true,
            clippy_light_model: crate::llm::groq::DEFAULT_LIGHT_MODEL.to_string(),
            clippy_advanced_model: crate::llm::groq::DEFAULT_ADVANCED_MODEL.to_string(),
            stt_model: "whisper-large-v3-turbo".to_string(),
            language_hint: None,
            retention_days: 7,
            retention_max_mb: 500,
            autostart: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    Light,
    Advanced,
}

impl From<Mode> for ClippyMode {
    fn from(m: Mode) -> ClippyMode {
        match m {
            Mode::Light => ClippyMode::Light,
            Mode::Advanced => ClippyMode::Advanced,
        }
    }
}
