mod audio;
mod clippy;
mod commands;
mod cursor_poller;
mod flow;
mod gc;
mod history;
mod hotkey;
mod inject;
mod llm;
mod power;
mod secrets;
mod settings;
mod stt;
mod sync;
mod tray;
#[cfg(target_os = "macos")]
mod touchbar;
mod usage;

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{Manager, WindowEvent};
#[cfg(target_os = "macos")]
use tauri::RunEvent;

use crate::audio::AudioController;
use crate::flow::Flow;
use crate::history::History;
use crate::settings::AppSettings;
use crate::usage::UsageTracker;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,wispr_fox_lib=debug")),
        )
        .init();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }

    let result = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .expect("app_data_dir available on desktop");
            std::fs::create_dir_all(&app_data).ok();
            let db_path = app_data.join("history.sqlite");
            let audio_dir = app_data.join("audio");
            std::fs::create_dir_all(&audio_dir).ok();

            let history = History::open(&db_path)?;

            // Sweep any rows left stranded by a previous crash/force-quit.
            // Without this, recordings stuck at `transcribing` or `cleaning`
            // would persist as "in-flight" forever and never expose the
            // Retry button. Idempotent — safe to call on every launch.
            match history.recover_stranded() {
                Ok(0) => {}
                Ok(n) => tracing::info!(rows = n, "history: recovered stranded rows on startup"),
                Err(e) => tracing::warn!("history recovery scan failed (non-fatal): {e:#}"),
            }

            let settings = AppSettings::default();
            let audio_ctrl = AudioController::spawn();

            // `wispr:level` feed for the wave-bar avatar. The cpal callback
            // writes an RMS level into a shared atomic while recording (0.0
            // otherwise); this task samples it every 90ms and emits only
            // around activity — one trailing 0.0 lets the wave settle, then
            // it goes quiet, so idle costs nothing on the event bus.
            {
                use tauri::Emitter;
                let level = audio_ctrl.level_handle();
                let app_for_level = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut ticker =
                        tokio::time::interval(std::time::Duration::from_millis(90));
                    let mut last = 0.0f32;
                    loop {
                        ticker.tick().await;
                        let v = f32::from_bits(
                            level.load(std::sync::atomic::Ordering::Relaxed),
                        );
                        if v > 0.0 || last > 0.0 {
                            let _ = app_for_level.emit("wispr:level", v);
                        }
                        last = v;
                    }
                });
            }
            let usage = UsageTracker::open().unwrap_or_else(|e| {
                tracing::warn!("usage tracker init failed: {e:#} (continuing)");
                UsageTracker::open().expect("retry usage tracker init")
            });
            let flow = Flow::new(
                history.clone(),
                settings.clone(),
                audio_dir.clone(),
                audio_ctrl,
                usage.clone(),
            );

            // Spawn retention sweeper.
            let settings_arc: Arc<Mutex<AppSettings>> = Arc::new(Mutex::new(settings.clone()));
            gc::spawn(history.clone(), settings_arc);

            // Hotkeys: 3 main + 3 sticky-invoke combos. The sticky-invoke
            // variants (typically Win+main key) always trigger sticky toggle
            // behaviour for that mode, independent of the per-mode setting.
            let app_for_hotkey = app.handle().clone();
            let flow_for_hotkey = flow.clone();
            if let Err(e) = hotkey::register(
                app.handle(),
                &settings.light_hotkey,
                &settings.advanced_hotkey,
                &settings.drafting_hotkey,
                &settings.light_sticky_hotkey,
                &settings.advanced_sticky_hotkey,
                &settings.drafting_sticky_hotkey,
                &settings.force_clean_hotkey,
                &settings.force_clean_sticky_hotkey,
                move |evt| {
                    flow_for_hotkey.handle_hotkey(&app_for_hotkey, evt);
                },
            ) {
                tracing::warn!("hotkey registration failed: {e:#}");
            }

            // Tray icon.
            if let Err(e) = tray::install(app.handle()) {
                tracing::warn!("tray install failed: {e:#}");
            }

            // Touch Bar (macOS only — no-op on non-Touch-Bar hardware).
            #[cfg(target_os = "macos")]
            touchbar::install(app.handle(), &flow);

            // Accounts + cross-device sync (v3.0.0). Fully inert unless the
            // user signs in AND this build has a real Supabase project baked
            // into sync/config.rs — see SYNC_DESIGN.md. Cloned BEFORE
            // `history`/`flow` are moved into managed state below; both are
            // cheap `Clone` (Arc-backed internally).
            let sync_engine =
                sync::engine::SyncEngine::new(history.clone(), flow.clone(), app.handle().clone());
            app.manage(sync_engine.clone());
            {
                let engine_for_launch = sync_engine.clone();
                tauri::async_runtime::spawn(async move {
                    // Restore a previous session (if any) before the first
                    // sync attempt — both are no-ops when signed out or when
                    // this build has no Supabase project configured.
                    sync::auth::try_restore_session().await;
                    engine_for_launch.sync_once().await;
                });
            }
            sync::engine::spawn_background_poll(sync_engine);

            app.manage(history);
            app.manage(flow);
            app.manage(usage);
            app.manage(power::JsPingState::new());

            // Intercept main-window close: hide instead of quit. The app keeps
            // running as a tray-resident service. Real quit goes through the
            // tray menu's "Quit" item which calls app.exit().
            if let Some(main) = app.get_webview_window("main") {
                let main_for_handler = main.clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_for_handler.hide();
                    }
                });
            }

            // Same for the Clippy floating window — close = hide so it can be
            // brought back via the tray menu without restarting the app.
            if let Some(clippy) = app.get_webview_window("clippy") {
                let clippy_for_handler = clippy.clone();
                clippy.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = clippy_for_handler.hide();
                    }
                });

                // Belt-and-suspenders position fix — place the floater in a
                // guaranteed-visible bottom-right slot BEFORE we call show()
                // or the JS in /clippy gets a chance to (mis)position it.
                // Reason: nightly.10 surfaced that the JS placement code in
                // clippy/+page.svelte mixed PHYSICAL (from availableMonitors)
                // and LOGICAL (in setPosition(LogicalPosition)) pixels, which
                // on a 2× Retina Mac placed the window ~1700 px past the right
                // edge of the screen. The webview still loaded (heartbeat
                // fires!) and the avatar still painted — the user just
                // couldn't see it because it was off-screen. JS is fixed too
                // (nightly.12), but doing this from Rust first means there's
                // a sane fallback even if the JS placement fails for any
                // reason (Tauri API change, monitor enumeration race, etc).
                if let Ok(Some(monitor)) = clippy.primary_monitor() {
                    let size = monitor.size();
                    let pos = monitor.position();
                    let sf = monitor.scale_factor();
                    let win_w_phys = (190.0 * sf) as i32;
                    let win_h_phys = (210.0 * sf) as i32;
                    let margin_x = (24.0 * sf) as i32;
                    let margin_y = (60.0 * sf) as i32;
                    let x = pos.x + size.width as i32 - win_w_phys - margin_x;
                    let y = pos.y + size.height as i32 - win_h_phys - margin_y;
                    tracing::info!(
                        x, y, sf, mw = size.width, mh = size.height,
                        "positioning clippy floater (Rust-side, physical px)"
                    );
                    let _ = clippy.set_position(tauri::PhysicalPosition::new(x, y));
                }

                // Show by default; users can hide via the X button or tray menu.
                let _ = clippy.show();
                // macOS: transparent + alwaysOnTop + macOSPrivateApi windows
                // need a kick to actually composite. Without this, `show()`
                // marks the window visible at the AppKit level but no pixels
                // are drawn — the user sees nothing, the WKWebView gets
                // throttled (heartbeat goes stale), and `recover_clippy_window`
                // from the JS heartbeat doesn't help because `show()` on an
                // already-shown-but-invisible window is a no-op. The reliable
                // wake-up is hide()→show() in sequence so the WindowServer
                // re-registers the surface fresh.
                #[cfg(target_os = "macos")]
                {
                    let c = clippy.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                        let _ = c.hide();
                        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                        let _ = c.show();
                        let _ = c.set_always_on_top(true);
                    });
                }
            }

            // macOS first-launch surface. The main window has visible: false
            // in tauri.conf.json so it doesn't flash if `open_silently` is on.
            // But on a Mac with no on-disk settings yet (fresh install), the
            // frontend's "if !open_silently { show() }" path runs from inside
            // the main window's webview — which doesn't actually load until
            // the window is first shown. Classic chicken-and-egg. Solve from
            // Rust: show + explicitly activate the app so the user sees the
            // homepage on first launch. RunEvent::Reopen handles subsequent
            // Dock clicks.
            #[cfg(target_os = "macos")]
            {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.unminimize();
                    let _ = main.set_focus();
                }
                macos_activate_app();
            }

            // Layer 1: resume detector — detects system sleep/wake from the
            // Rust side (independent of the webview's JS runtime).
            power::spawn_resume_detector(app.handle().clone());

            // Layer 2: periodic watchdog — re-asserts always-on-top every 30s
            // and force-repaints if the JS heartbeat has gone stale.
            let watchdog_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    if let Some(w) = watchdog_handle.get_webview_window("clippy") {
                        if w.is_visible().unwrap_or(false) {
                            let _ = w.set_always_on_top(true);
                            if let Some(ps) = watchdog_handle.try_state::<power::JsPingState>() {
                                if ps.ms_since_last_ping() > 45_000 {
                                    tracing::info!("watchdog: JS heartbeat stale, forcing repaint");
                                    commands::force_repaint(&w);
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::js_heartbeat_ping,
            commands::set_clickthrough,
            commands::recover_clippy_window,
            commands::resize_floater,
            commands::accessibility_ok,
            commands::floater_trigger,
            commands::open_accessibility_settings,
            commands::check_secrets,
            commands::secrets_diagnostic,
            commands::secret_audit_log,
            commands::save_secret,
            commands::delete_secret,
            commands::get_settings,
            commands::set_settings,
            commands::list_history,
            commands::delete_recording,
            commands::delete_recordings,
            commands::purge_account,
            commands::retry_recording,
            commands::auth_status,
            commands::sign_in_email,
            commands::sign_up_email,
            commands::sign_in_google,
            commands::cancel_google_sign_in,
            commands::sign_out,
            commands::sync_now,
            commands::set_device_name,
            commands::transcribe_upload,
            commands::generate_alt_version,
            commands::audio_url_for,
            commands::audio_data_url_for,
            commands::list_input_devices,
            commands::app_paths,
            commands::reveal_folder,
            commands::check_for_updates,
            commands::daily_usage,
            commands::stats_summary,
            commands::current_models,
            commands::clear_all_history,
            commands::get_default_prompts,
            commands::list_notification_sounds,
            commands::add_notification_sound,
            commands::test_groq_key,
            commands::test_gemini_key,
            commands::test_openai_key,
            commands::test_deepgram_key,
            commands::test_elevenlabs_key,
            commands::test_saved_groq_key,
            commands::test_saved_gemini_key,
            commands::test_saved_openai_key,
            commands::test_saved_deepgram_key,
            commands::test_saved_elevenlabs_key,
            commands::configure_cues,
        ])
        .build(tauri::generate_context!());

    let app = match result {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("error while building wispr-fox: {e}");
            return;
        }
    };

    // RunEvent loop. The handler runs for every Tauri/AppKit event we care
    // about. We use this primarily for macOS Dock-icon clicks: with no Reopen
    // handler, clicking the Dock icon on a running app whose main window is
    // hidden does nothing — leaving the user stuck (reported on M4 Pro Mac,
    // v1.1.0-nightly.8). Reopen fires for every Dock click; we show the main
    // window AND nudge the clippy floater back if it's set to be visible.
    app.run(|_app_handle, _event| {
        #[cfg(target_os = "macos")]
        {
            // Dock-icon click. We do NOT gate on `has_visible_windows` because
            // a transparent + macOSPrivateApi window can be "visible" at the
            // AppKit level while invisible to the user (zero-alpha surface
            // never composited). In that situation `has_visible_windows = true`
            // would skip the re-show and leave the user stuck — which is
            // exactly the M4 Pro bug reported on nightly.8 and nightly.9.
            // Always re-show and activate so the click ALWAYS lands the user
            // on the main window.
            if let RunEvent::Reopen { .. } = &_event {
                if let Some(w) = _app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
                // Kick the clippy floater too — hide + show forces the
                // WKWebView surface to re-register with the WindowServer,
                // recovering from any zero-alpha "ghost window" state.
                if let Some(c) = _app_handle.get_webview_window("clippy") {
                    let _ = c.hide();
                    let _ = c.show();
                    let _ = c.set_always_on_top(true);
                }
                macos_activate_app();
            }
        }
    });
}

/// Explicitly activate the macOS app via `[NSApplication.shared
/// activateIgnoringOtherApps:YES]`. Tauri's `window.show()` marks the window
/// visible with AppKit but doesn't bring the app to the foreground — and
/// without foreground activation, transparent + macOSPrivateApi windows
/// don't get composited, so the user sees no pixels even though Tauri
/// reports `is_visible() == true`. Calling this immediately after `show()`
/// is what makes the window actually appear on screen on a fresh M-series
/// MacBook (reported on M4 Pro, nightly.8/9).
#[cfg(target_os = "macos")]
fn macos_activate_app() {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2::ClassType;
    // SAFETY: NSApplication.sharedApplication is documented to be safe to
    // call from any thread; activate(ignoringOtherApps:) is documented as
    // main-thread-only. We're invoked from the Tauri setup closure or the
    // RunEvent loop, both of which run on the main thread.
    unsafe {
        let ns_app: *mut AnyObject =
            msg_send![NSApplication::class(), sharedApplication];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    }
}
