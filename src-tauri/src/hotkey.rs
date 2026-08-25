//! Global-shortcut wrapper.
//!
//! Tauri's global-shortcut plugin emits Pressed / Released edges for each
//! registered combo. We register the three mode hotkeys plus force-clean.
//! Every physical combo uses the same adaptive tap/hold contract; the flow
//! layer decides recording state and this module only forwards edges.
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
    /// Stable identity of the physical shortcut. Used as a key-down latch so
    /// OS auto-repeat cannot masquerade as another press.
    pub trigger_id: String,
    pub mode: Mode,
    pub edge: Edge,
    /// True when fired from a force-clean combo (Shift+F8 by default).
    /// Tells the flow layer to override `auto_clean_in_light` to TRUE for
    /// this single invocation — gives the user on-demand cleanup without
    /// changing the global setting. Only meaningful for Light mode.
    #[serde(default)]
    pub force_clean: bool,
}

/// Active adaptive combo strings, pulled out of `AppSettings` so this module never
/// needs the whole settings struct at a call site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotkeyConfig {
    pub light: String,
    pub advanced: String,
    pub drafting: String,
    pub force_clean: String,
    /// Show/hide the main window. Not a dictation binding: it carries no Mode
    /// and never reaches the flow layer, so it lives outside `combos()`.
    pub toggle_window: String,
}

impl HotkeyConfig {
    pub fn from_settings(s: &AppSettings) -> Self {
        Self {
            light: s.light_hotkey.clone(),
            advanced: s.advanced_hotkey.clone(),
            drafting: s.drafting_hotkey.clone(),
            force_clean: s.force_clean_hotkey.clone(),
            toggle_window: s.toggle_window_hotkey.clone(),
        }
    }

    /// Tuple shape: (combo, mode, force_clean).
    fn combos(&self) -> [(&str, Mode, bool); 4] {
        [
            (&self.light, Mode::Light, false),
            (&self.advanced, Mode::Advanced, false),
            (&self.drafting, Mode::Drafting, false),
            // Force-clean variants: same Light mode, but flag the press so the
            // flow layer treats it as auto_clean_in_light=true for this run.
            (&self.force_clean, Mode::Light, true),
        ]
    }
}

type Callback = std::sync::Arc<dyn Fn(HotkeyEvent) + Send + Sync + 'static>;

/// The flow-layer dispatcher, stored once at install so `apply` can re-register
/// without the caller having to hand it back every time.
static CALLBACK: std::sync::OnceLock<Callback> = std::sync::OnceLock::new();

/// Registration and explicit capture-suspension are one atomic state. Keeping
/// the flag under the same mutex as the owned shortcuts prevents a settings
/// refresh from racing a rebinding capture and accidentally turning hotkeys
/// back on while the capture overlay is open.
struct RegistrationState {
    registered: Vec<Shortcut>,
    suspended: bool,
}

static REGISTRATION: Mutex<RegistrationState> = Mutex::new(RegistrationState {
    registered: Vec::new(),
    suspended: false,
});

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
    let mut state = REGISTRATION.lock();
    state.suspended = false;
    rebuild_locked(app, cfg, &mut state)
}

/// Refresh bindings after settings change, but only when registration is live.
/// A capture-suspended registrar stays suspended; `apply` is the explicit
/// resume operation and will rebuild from Flow's latest settings afterwards.
pub fn refresh_if_live(app: &AppHandle, cfg: &HotkeyConfig) -> Result<bool> {
    let mut state = REGISTRATION.lock();
    if state.suspended {
        return Ok(false);
    }
    rebuild_locked(app, cfg, &mut state)?;
    Ok(true)
}

fn rebuild_locked(
    app: &AppHandle,
    cfg: &HotkeyConfig,
    state: &mut RegistrationState,
) -> Result<()> {
    unregister_locked(app, state);

    let Some(on_event) = CALLBACK.get().cloned() else {
        // apply() before install() — nothing to dispatch to. Not fatal, but it
        // means hotkeys would be inert, so make it loud in the log.
        tracing::error!("hotkey::apply called before install; no dispatcher registered");
        return Ok(());
    };

    for (combo, mode, force_clean_flag) in cfg.combos() {
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
        let trigger_capture = combo.to_owned();
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
                    trigger_id: trigger_capture.clone(),
                    mode: mode_capture,
                    edge,
                    force_clean: force_capture,
                });
            }) {
            Ok(()) => {
                tracing::info!(
                    combo,
                    ?mode,
                    force_clean = force_clean_flag,
                    "hotkey registered"
                );
                state.registered.push(sc);
            }
            Err(e) => tracing::warn!(combo, ?mode, "hotkey registration failed: {e}"),
        }
    }

    register_window_toggle(app, cfg, state);
    Ok(())
}

/// Register the show/hide-window combo.
///
/// Separate from the dictation loop because it dispatches to the window layer,
/// not the flow layer — it has no Mode, no edge semantics, and must not be
/// able to start a recording.
fn register_window_toggle(app: &AppHandle, cfg: &HotkeyConfig, state: &mut RegistrationState) {
    let combo = cfg.toggle_window.trim();
    if combo.is_empty() {
        return;
    }
    let sc = match Shortcut::from_str(combo) {
        Ok(sc) => sc,
        Err(e) => {
            tracing::warn!(combo, "window-toggle hotkey parse failed, skipping: {e}");
            return;
        }
    };
    let sc_match = sc.clone();
    match app.global_shortcut().on_shortcut(sc.clone(), move |app, fired, event| {
        if fired != &sc_match || event.state() != ShortcutState::Pressed {
            return;
        }
        // Hop to the main thread before touching windows.
        //
        // This callback runs while the global-shortcut plugin still holds its
        // internal registry lock, on the same thread that drives the tray and
        // every window. Doing window work inline here is exactly the shape of
        // the bug that froze the app on the first keypress in v3.3.0-nightly.2
        // (see its release notes). `run_on_main_thread` queues instead, so it
        // cannot deadlock whichever thread we are called on.
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            crate::tray::toggle_main(&handle);
        });
    }) {
        Ok(()) => {
            tracing::info!(combo, "window-toggle hotkey registered");
            state.registered.push(sc);
        }
        Err(e) => tracing::warn!(combo, "window-toggle hotkey registration failed: {e}"),
    }
}

/// Unregister every combo this module owns, leaving the OS free to deliver
/// those keys to the focused window (which is exactly what the rebinding
/// capture dialog needs). Safe to call when nothing is registered.
pub fn suspend(app: &AppHandle) {
    let mut state = REGISTRATION.lock();
    state.suspended = true;
    unregister_locked(app, &mut state);
}

fn unregister_locked(app: &AppHandle, state: &mut RegistrationState) {
    for sc in state.registered.drain(..) {
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
    let state = REGISTRATION.lock();
    !state.suspended && !state.registered.is_empty()
}
