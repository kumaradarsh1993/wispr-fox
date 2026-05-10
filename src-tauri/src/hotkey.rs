//! Global-shortcut wrapper.
//!
//! Tauri's global-shortcut plugin emits a single event per key combo on press,
//! with a `ShortcutState` indicating Pressed or Released (post 2.x). We forward
//! these as `HotkeyEvent`s tagged with the user's intended `Mode` so flow.rs
//! can build push-to-talk semantics.

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
}

/// Register both Light and Advanced hotkeys. Forwards key-down/key-up edges to
/// the flow layer through a shared callback.
pub fn register(
    app: &AppHandle,
    light: &str,
    advanced: &str,
    on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static,
) -> Result<()> {
    let light_sc = Shortcut::from_str(light)
        .with_context(|| format!("parsing light hotkey '{light}'"))?;
    let advanced_sc = Shortcut::from_str(advanced)
        .with_context(|| format!("parsing advanced hotkey '{advanced}'"))?;

    let on_event = std::sync::Arc::new(on_event);

    {
        let on_event = on_event.clone();
        let light_match = light_sc.clone();
        let advanced_match = advanced_sc.clone();
        app.global_shortcut().on_shortcut(light_sc.clone(), move |_app, sc, event| {
            let mode = if sc == &light_match {
                Mode::Light
            } else if sc == &advanced_match {
                Mode::Advanced
            } else {
                return;
            };
            let edge = match event.state() {
                ShortcutState::Pressed => Edge::Down,
                ShortcutState::Released => Edge::Up,
            };
            on_event(HotkeyEvent { mode, edge });
        })?;
    }
    {
        let on_event = on_event.clone();
        let light_match = light_sc.clone();
        let advanced_match = advanced_sc.clone();
        app.global_shortcut().on_shortcut(advanced_sc.clone(), move |_app, sc, event| {
            let mode = if sc == &advanced_match {
                Mode::Advanced
            } else if sc == &light_match {
                Mode::Light
            } else {
                return;
            };
            let edge = match event.state() {
                ShortcutState::Pressed => Edge::Down,
                ShortcutState::Released => Edge::Up,
            };
            on_event(HotkeyEvent { mode, edge });
        })?;
    }

    tracing::info!(%light, %advanced, "hotkeys registered");
    let _ = app; // suppress unused if Manager goes unused above
    Ok(())
}
