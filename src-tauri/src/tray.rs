//! Tray icon + menu — the app's persistent presence in the Windows system tray
//! (the area near the clock). Left-clicking the icon toggles the main window
//! between visible / hidden. Right-click opens the menu.
//!
//! The pulse animation during recording is driven by the frontend (it listens
//! to `wispr:state` events emitted by flow.rs and toggles a CSS class). Keeping
//! it in JS avoids fighting the Win32 tray icon refresh limits.

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn install(app: &AppHandle) -> Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show wispr-fox", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let toggle_clippy_item =
        MenuItem::with_id(app, "toggle-clippy", "Toggle Clippy", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit wispr-fox", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &history_item,
            &settings_item,
            &toggle_clippy_item,
            &separator,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::with_id("wispr-tray")
        .icon(app.default_window_icon().cloned().expect("default icon"))
        .tooltip("wispr-fox — hold F8 to dictate")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "history" => {
                show_main(app);
                navigate(app, "/history");
            }
            "settings" => {
                show_main(app);
                navigate(app, "/settings");
            }
            "toggle-clippy" => {
                if let Some(w) = app.get_webview_window("clippy") {
                    match w.is_visible() {
                        Ok(true) => { let _ = w.hide(); }
                        _ => {
                            // force_repaint (show + size nudge) rather than a
                            // bare show() so a blank-after-resume floater comes
                            // back painted, giving the user a manual recovery
                            // path that actually works.
                            crate::commands::force_repaint(&w);
                            let _ = w.set_focus();
                        }
                    }
                }
            }
            "quit" => {
                // The window close handler intercepts and hides; explicit quit
                // bypasses that path and actually exits.
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click toggles main window visibility.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                toggle_main(app);
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => {
                let _ = w.hide();
            }
            _ => {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
    }
}

fn navigate(app: &AppHandle, path: &str) {
    if let Some(w) = app.get_webview_window("main") {
        // Use the SvelteKit goto-via-postMessage approach so client-side router
        // handles the navigation cleanly. The frontend listens for this event.
        let _ = w.emit("wispr:navigate", path);
    }
}
