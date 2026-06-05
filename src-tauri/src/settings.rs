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

    /// Force-clean variants of the Light hotkey. Pressing these triggers
    /// F8 with `auto_clean_in_light` overridden to TRUE for this single
    /// invocation — doesn't persist. Used when the user normally wants
    /// raw F8 output but occasionally needs a cleaned version on-demand.
    /// Defaults: Shift+F8 and Shift+Super+F8.
    #[serde(default = "default_force_clean_hotkey")]
    pub force_clean_hotkey: String,
    #[serde(default = "default_force_clean_sticky_hotkey")]
    pub force_clean_sticky_hotkey: String,

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

    /// When true (default): main window stays hidden on launch — only the
    /// Clippy floater + tray icon show up. User clicks tray or double-clicks
    /// Clippy to open the main window. When false: open the main window on
    /// startup like a normal app.
    #[serde(default = "default_open_silently")]
    pub open_silently: bool,

    // ── Audio cues (filename within %APPDATA%/com.wispr-fox.app/sounds/) ──
    /// Empty string = use built-in generated tone.
    pub start_sound: String,
    pub stop_sound: String,
    pub cues_enabled: bool,

    // ── Look & feel ────────────────────────────────────────────────────────
    /// "auto" | "light" | "dark" | "retro"
    pub theme: String,

    // ── Injection behaviour ────────────────────────────────────────────────
    /// When true: if the user has navigated to a different app during the
    /// LLM gap, yank focus back to where they started speaking and inject
    /// there. When false (default): respect the new app, deliver the result
    /// silently to the clipboard and show a Clippy bubble. Off by default
    /// because pulling Chrome (or whatever) back to Outlook feels hostile.
    #[serde(default)]
    pub pull_back_on_navigation: bool,

    /// When true (default): after every successful dictation, leave the
    /// cleaned text on the clipboard so the user can Ctrl+V it again
    /// anywhere — even if injection already pasted it once. Sacrifices the
    /// user's prior clipboard contents (no restore-to-prior). When false:
    /// restore prior clipboard ~800ms after paste (legacy behaviour).
    #[serde(default = "default_keep_in_clipboard")]
    pub keep_in_clipboard: bool,

    /// When true (default): F9 drafting reads the foreground app's process
    /// name + window title at hotkey-down time, classifies it into a coarse
    /// bucket (email / chat / social / doc / code / AI), and nudges the
    /// LLM's tone to match (formal email, casual WhatsApp message, punchy
    /// LinkedIn post, etc.). Only the bucket NAME is sent to the LLM —
    /// never the raw process or title. Disable if you want the LLM to
    /// pick the register from your brief's content alone.
    #[serde(default = "default_adapt_to_app")]
    pub adapt_to_app: bool,

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
        // Platform-split hotkey defaults.
        //
        // Windows: F-keys are nearly always free system-wide. (F10 is the one
        // exception — Windows reserves it as the menu-activation key, which
        // stole focus to Outlook's ribbon, so it was retired in favour of F9.)
        //   F8 = transcribe (Light), F9 = draft (Drafting).
        //
        // macOS: the function row sends media/volume events by default, NOT
        // F8/F9 keycodes — so a global F8/F9 shortcut never fires (it just
        // plays/pauses music). We default Mac to ⌃⌥ (Control+Option) chords,
        // which fire regardless of the "use F1/F2 as standard function keys"
        // setting and rarely collide with app shortcuts:
        //   ⌃⌥D = dictate (Light), ⌃⌥F = draft (Drafting), ⌃⌥C = force-clean.
        // Sticky-toggle variants add Shift. (Existing Mac installs are
        // migrated off F8/F9 by the frontend settings store.)
        let mac = cfg!(target_os = "macos");
        Self {
            // macOS: a single ⌥ chord (Option+Space / Option+Enter) instead of
            // ⌃⌥ three-key combos — one modifier + one key, works out of the box
            // (no "use F-keys as standard" setting needed, unlike bare F8/F9).
            // ⌘ is the sticky modifier (the Mac analogue of Win+F8).
            light_hotkey: if mac { "Alt+Space" } else { "F8" }.to_string(),
            advanced_hotkey: String::new(),
            drafting_hotkey: if mac { "Alt+Enter" } else { "F9" }.to_string(),
            sticky_light: false,
            sticky_advanced: false,
            sticky_drafting: false,
            light_sticky_hotkey: if mac { "Super+Alt+Space" } else { "Super+F8" }.to_string(),
            advanced_sticky_hotkey: String::new(),
            drafting_sticky_hotkey: if mac { "Super+Alt+Enter" } else { "Super+F9" }.to_string(),
            force_clean_hotkey: default_force_clean_hotkey(),
            force_clean_sticky_hotkey: default_force_clean_sticky_hotkey(),
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
            pull_back_on_navigation: false,
            keep_in_clipboard: default_keep_in_clipboard(),
            open_silently: default_open_silently(),
            adapt_to_app: default_adapt_to_app(),
        }
    }
}

fn default_adapt_to_app() -> bool {
    true
}

fn default_keep_in_clipboard() -> bool {
    true
}

fn default_open_silently() -> bool {
    // Windows: hide main window on launch — users find the tray icon easily,
    // and there's no Dock to give them a re-entry point. Silent start is the
    // expected "background utility" pattern.
    //
    // macOS: the menu-bar item (our equivalent of tray) is easy to miss, and
    // Dock clicks don't auto-reopen a hidden window without explicit Reopen
    // handling. Default to showing the main window on launch so users have an
    // obvious entry point. RunEvent::Reopen in lib.rs covers subsequent Dock
    // clicks even when the user later flips this off.
    !cfg!(target_os = "macos")
}

fn default_force_clean_hotkey() -> String {
    if cfg!(target_os = "macos") { "Shift+Alt+Space" } else { "Shift+F8" }.to_string()
}

fn default_force_clean_sticky_hotkey() -> String {
    if cfg!(target_os = "macos") { "Super+Shift+Alt+Space" } else { "Shift+Super+F8" }.to_string()
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
