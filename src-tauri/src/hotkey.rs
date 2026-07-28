//! Global-shortcut wrapper.
//!
//! Tauri's global-shortcut plugin emits Pressed / Released edges for each
//! registered combo. We register EIGHT combos total — three "main" hotkeys
//! (Light, Advanced, Drafting — push-to-talk by default), three
//! "sticky-invoke" hotkeys (typically Win+main key) that always trigger
//! a press-once-start / press-again-stop toggle regardless of the per-mode
//! sticky setting, and two force-clean variants. The flow layer decides
//! actual recording state — this module just forwards edges + mode + flags.
//!
//! **Registration is live, not boot-only.** The combos can be re-applied at
//! any time (`apply`) and temporarily torn down (`suspend`). Both exist for
//! the hotkey-rebinding UI: while the user is pressing keys into the capture
//! dialog, a still-registered F8 would fire a *recording* instead of being
//! captured — the shortcut is global, so it wins over the focused webview and
//! the keypress never reaches the DOM listener. Every app with a rebind UI
//! (Discord, Raycast, Alfred, OBS) solves this the same way: unregister for
//! the duration of the capture, re-register after. `suspend`/`apply` are that
//! pair, and `apply` doubles as "the user saved a new binding, make it live
//! now" so rebinding no longer needs an app restart.

use std::str::FromStr;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::{AppSettings, Mode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Down,
    Up,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyEvent {
    pub mode: Mode,
    pub edge: Edge,
    /// True when fired from a sticky-invoke combo (e.g. Win+F8). Flow layer
    /// uses this to force sticky toggle behaviour for THIS press, regardless
    /// of the per-mode sticky setting.
    pub sticky_invoke: bool,
    /// True when fired from a force-clean combo (Shift+F8 by default).
    /// Tells the flow layer to override `auto_clean_in_light` to TRUE for
    /// this single invocation — gives the user on-demand cleanup without
    /// changing the global setting. Only meaningful for Light mode.
    #[serde(default)]
    pub force_clean: bool,
}

/// The eight combo strings, pulled out of `AppSettings` so this module never
/// needs the whole settings struct at a call site.
#[derive(Debug, Clone, Default)]
pub struct HotkeyConfig {
    pub light: String,
    pub advanced: String,
    pub drafting: String,
    pub light_sticky: String,
    pub advanced_sticky: String,
    pub drafting_sticky: String,
    pub force_clean: String,
    pub force_clean_sticky: String,
}

impl HotkeyConfig {
    pub fn from_settings(s: &AppSettings) -> Self {
        Self {
            light: s.light_hotkey.clone(),
            advanced: s.advanced_hotkey.clone(),
            drafting: s.drafting_hotkey.clone(),
            light_sticky: s.light_sticky_hotkey.clone(),
            advanced_sticky: s.advanced_sticky_hotkey.clone(),
            drafting_sticky: s.drafting_sticky_hotkey.clone(),
            force_clean: s.force_clean_hotkey.clone(),
            force_clean_sticky: s.force_clean_sticky_hotkey.clone(),
        }
    }

    /// Tuple shape: (combo, mode, sticky_invoke, force_clean).
    fn combos(&self) -> [(&str, Mode, bool, bool); 8] {
        [
            (&self.light, Mode::Light, false, false),
            (&self.advanced, Mode::Advanced, false, false),
            (&self.drafting, Mode::Drafting, false, false),
            (&self.light_sticky, Mode::Light, true, false),
            (&self.advanced_sticky, Mode::Advanced, true, false),
            (&self.drafting_sticky, Mode::Drafting, true, false),
            // Force-clean variants: same Light mode, but flag the press so the
            // flow layer treats it as auto_clean_in_light=true for this run.
            (&self.force_clean, Mode::Light, false, true),
            (&self.force_clean_sticky, Mode::Light, true, true),
        ]
    }
}

type Callback = std::sync::Arc<dyn Fn(HotkeyEvent) + Send + Sync + 'static>;

/// The flow-layer dispatcher, stored once at install so `apply` can re-register
/// without the caller having to hand it back every time.
static CALLBACK: std::sync::OnceLock<Callback> = std::sync::OnceLock::new();

/// Combos this module currently owns. We unregister exactly these rather than
/// calling `unregister_all()`, which would also rip out the dynamically-armed
/// Escape-stop shortcut that `flow.rs` owns during a live recording.
static REGISTERED: Mutex<Vec<Shortcut>> = Mutex::new(Vec::new());

/// Install the dispatcher and register the initial set of combos. Call once,
/// at startup; use `apply` for every subsequent change.
pub fn install(
    app: &AppHandle,
    cfg: &HotkeyConfig,
    on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static,
) -> Result<()> {
    // `set` fails only if install() was called twice — in that case the first
    // dispatcher stays authoritative, which is the safe outcome.
    let _ = CALLBACK.set(std::sync::Arc::new(on_event));
    apply(app, cfg)
}

/// Tear down the current combos and register `cfg` instead. Idempotent and
/// safe to call repeatedly; a combo that fails to parse or register is logged
/// and skipped rather than aborting the rest (a single bad saved binding must
/// never leave the user with NO working hotkeys).
pub fn apply(app: &AppHandle, cfg: &HotkeyConfig) -> Result<()> {
    suspend(app);

    let Some(on_event) = CALLBACK.get().cloned() else {
        // apply() before install() — nothing to dispatch to. Not fatal, but it
        // means hotkeys would be inert, so make it loud in the log.
        tracing::error!("hotkey::apply called before install; no dispatcher registered");
        return Ok(());
    };

    let mut registered = REGISTERED.lock();
    for (combo, mode, sticky_invoke, force_clean_flag) in cfg.combos() {
        if combo.trim().is_empty() {
            continue;
        }
        let sc = match Shortcut::from_str(combo).with_context(|| format!("parsing hotkey '{combo}'"))
        {
            Ok(sc) => sc,
            Err(e) => {
                tracing::warn!(combo, "hotkey parse failed, skipping: {e:#}");
                continue;
            }
        };
        let sc_match = sc.clone();
        let on_event = on_event.clone();
        let mode_capture = mode;
        let sticky_capture = sticky_invoke;
        let force_capture = force_clean_flag;
        match app
            .global_shortcut()
            .on_shortcut(sc.clone(), move |_app, fired, event| {
                if fired != &sc_match {
                    return;
                }
                let edge = match event.state() {
                    ShortcutState::Pressed => Edge::Down,
                    ShortcutState::Released => Edge::Up,
                };
                on_event(HotkeyEvent {
                    mode: mode_capture,
                    edge,
                    sticky_invoke: sticky_capture,
                    force_clean: force_capture,
                });
            }) {
            Ok(()) => {
                tracing::info!(
                    combo,
                    ?mode,
                    sticky_invoke,
                    force_clean = force_clean_flag,
                    "hotkey registered"
                );
                registered.push(sc);
            }
            Err(e) => tracing::warn!(combo, ?mode, "hotkey registration failed: {e}"),
        }
    }

    Ok(())
}

/// Unregister every combo this module owns, leaving the OS free to deliver
/// those keys to the focused window (which is exactly what the rebinding
/// capture dialog needs). Safe to call when nothing is registered.
pub fn suspend(app: &AppHandle) {
    let mut registered = REGISTERED.lock();
    for sc in registered.drain(..) {
        if let Err(e) = app.global_shortcut().unregister(sc) {
            // Already gone is the common case on a double-suspend; not worth
            // more than a debug line.
            tracing::debug!("hotkey unregister failed (non-fatal): {e}");
        }
    }
}

/// Whether any dictation combo is currently registered. Used by the settings
/// UI to show honest state ("hotkeys paused while you pick a key").
pub fn is_active() -> bool {
    !REGISTERED.lock().is_empty()
}
