mod audio;
mod clippy;
mod commands;
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
mod tray;
#[cfg(target_os = "macos")]
mod touchbar;
mod usage;

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{Manager, WindowEvent};

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
                // Show by default; users can hide via the X button or tray menu.
                let _ = clippy.show();
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
            commands::recover_clippy_window,
            commands::accessibility_ok,
            commands::floater_trigger,
            commands::open_accessibility_settings,
            commands::check_secrets,
            commands::secrets_diagnostic,
            commands::save_secret,
            commands::delete_secret,
            commands::get_settings,
            commands::set_settings,
            commands::list_history,
            commands::delete_recording,
            commands::retry_recording,
            commands::generate_alt_version,
            commands::audio_url_for,
            commands::audio_data_url_for,
            commands::list_input_devices,
            commands::app_paths,
            commands::daily_usage,
            commands::current_models,
            commands::clear_all_history,
            commands::get_default_prompts,
            commands::list_notification_sounds,
            commands::add_notification_sound,
            commands::test_groq_key,
            commands::test_gemini_key,
            commands::test_saved_groq_key,
            commands::test_saved_gemini_key,
            commands::configure_cues,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!("error while running wispr-fox: {e}");
    }
}
