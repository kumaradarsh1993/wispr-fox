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
    on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static,
) -> Result<()> {
    let combos = [
        (light, Mode::Light, false),
        (advanced, Mode::Advanced, false),
        (drafting, Mode::Drafting, false),
        (light_sticky, Mode::Light, true),
        (advanced_sticky, Mode::Advanced, true),
        (drafting_sticky, Mode::Drafting, true),
    ];

    let on_event = std::sync::Arc::new(on_event);

    for (combo, mode, sticky_invoke) in combos {
        if combo.is_empty() {
            continue;
        }
        let sc = Shortcut::from_str(combo)
            .with_context(|| format!("parsing hotkey '{combo}'"))?;
        let sc_match = sc.clone();
        let on_event = on_event.clone();
        let mode_capture = mode;
        let sticky_capture = sticky_invoke;
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
            });
        }) {
            Ok(()) => tracing::info!(combo, ?mode, sticky_invoke, "hotkey registered"),
            Err(e) => tracing::warn!(combo, ?mode, "hotkey registration failed: {e}"),
        }
    }

    Ok(())
}
