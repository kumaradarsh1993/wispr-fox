//! Tray icon + menu: Toggle Show/Hide, Open Settings, Quit.
//!
//! The pulse animation during recording is driven by the frontend (it listens
//! to `wispr:state` events emitted by flow.rs and toggles a CSS class). Keeping
//! it in JS avoids fighting the Win32 tray icon refresh limits.

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn install(app: &AppHandle) -> Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show wispr-fox", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show_item, &settings_item, &history_item, &quit_item],
    )?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("default icon"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "settings" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                    let _ = w.eval("window.location.hash = '#/settings'");
                }
            }
            "history" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                    let _ = w.eval("window.location.hash = '#/history'");
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
