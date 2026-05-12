//! Global-shortcut wrapper.
//!
//! Tauri's global-shortcut plugin emits Pressed / Released edges for each
//! registered combo. We register SIX combos total — three "main" hotkeys
//! (Light, Advanced, Drafting — push-to-talk by default) and three
//! "sticky-invoke" hotkeys (typically Win+main key) that always trigger
//! a press-once-start / press-again-stop toggle, regardless of the per-mode
//! sticky setting. The flow layer decides actual recording state — this
//! module just forwards edges + mode + sticky flag.

use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::Mode;

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

#[allow(clippy::too_many_arguments)]
pub fn register(
    app: &AppHandle,
    light: &str,
    advanced: &str,
    drafting: &str,
    light_sticky: &str,
    advanced_sticky: &str,
    drafting_sticky: &str,
    force_clean: &str,
    force_clean_sticky: &str,
    on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static,
) -> Result<()> {
    // Tuple shape: (combo, mode, sticky_invoke, force_clean)
    let combos: [(&str, Mode, bool, bool); 8] = [
        (light, Mode::Light, false, false),
        (advanced, Mode::Advanced, false, false),
        (drafting, Mode::Drafting, false, false),
        (light_sticky, Mode::Light, true, false),
        (advanced_sticky, Mode::Advanced, true, false),
        (drafting_sticky, Mode::Drafting, true, false),
        // Force-clean variants: same Light mode, but flag the press so the
        // flow layer treats it as auto_clean_in_light=true for this invocation.
        (force_clean, Mode::Light, false, true),
        (force_clean_sticky, Mode::Light, true, true),
    ];

    let on_event = std::sync::Arc::new(on_event);

    for (combo, mode, sticky_invoke, force_clean_flag) in combos {
        if combo.is_empty() {
            continue;
        }
        let sc = Shortcut::from_str(combo)
            .with_context(|| format!("parsing hotkey '{combo}'"))?;
        let sc_match = sc.clone();
        let on_event = on_event.clone();
        let mode_capture = mode;
        let sticky_capture = sticky_invoke;
        let force_capture = force_clean_flag;
        match app.global_shortcut().on_shortcut(sc.clone(), move |_app, fired, event| {
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
            Ok(()) => tracing::info!(combo, ?mode, sticky_invoke, force_clean = force_clean_flag, "hotkey registered"),
            Err(e) => tracing::warn!(combo, ?mode, "hotkey registration failed: {e}"),
        }
    }

    Ok(())
}
