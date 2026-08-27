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

    // Tooltip mentions the platform-correct hotkey so the user can find the
    // dictate trigger right from hovering the tray icon. (macOS uses ⌃⌥ chords
    // because the function row is media/volume by default; see settings.rs.)
    let tooltip = if cfg!(target_os = "macos") {
        "wispr-fox — hold ⌃⌥D to dictate"
    } else {
        "wispr-fox — hold F8 to dictate"
    };
    let _tray = TrayIconBuilder::with_id("wispr-tray")
        .icon(app.default_window_icon().cloned().expect("default icon"))
        .tooltip(tooltip)
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
                // bypasses that path and actually exits. If the floater is
                // visible, give it one beat to play its per-skin farewell
                // animation (EXIT_MS = 240ms in the clippy page) before dying.
                let farewell = app
                    .get_webview_window("clippy")
                    .map(|w| {
                        let visible = w.is_visible().unwrap_or(false);
                        if visible {
                            let _ = w.emit("wispr:farewell", ());
                        }
                        visible
                    })
                    .unwrap_or(false);
                if farewell {
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(420));
                        app.exit(0);
                    });
                } else {
                    app.exit(0);
                }
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
        // macOS: show() + set_focus() alone leave the window ordered behind the
        // frontmost app — `is_visible()` reports true while the user sees
        // nothing happen. Clicking the tray icon does not activate the app the
        // way clicking a Dock icon does, so we have to ask AppKit ourselves.
        // Without this the tray icon is simply dead on macOS.
        #[cfg(target_os = "macos")]
        crate::macos_activate_app();
    }
}

/// Show the window if hidden, focus it if visible-but-behind, hide it if it is
/// already the focused window. Shared by the tray icon and the global
/// show/hide hotkey.
pub fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let visible = w.is_visible().unwrap_or(false);
        let minimized = w.is_minimized().unwrap_or(false);
        let focused = w.is_focused().unwrap_or(false);
        // Only hide when the window is genuinely in front of the user. If it is
        // merely behind another app, "toggle" has to mean RAISE — hiding an
        // already-hidden-looking window is what makes a show/hide hotkey feel
        // broken (press once: nothing appears to happen; press again: it opens).
        if visible && !minimized && focused {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            // Same activation requirement as show_main — this is the path the
            // tray left-click and the show/hide hotkey both take.
            #[cfg(target_os = "macos")]
            crate::macos_activate_app();
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
