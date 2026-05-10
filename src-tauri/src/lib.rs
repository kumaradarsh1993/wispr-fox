mod audio;
mod clippy;
mod commands;
mod flow;
mod gc;
mod history;
mod hotkey;
mod inject;
mod llm;
mod secrets;
mod settings;
mod stt;
mod tray;

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::Manager;

use crate::audio::AudioController;
use crate::flow::Flow;
use crate::history::History;
use crate::settings::AppSettings;

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
        .plugin(tauri_plugin_fs::init())
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
            let settings = AppSettings::default();
            let audio_ctrl = AudioController::spawn();
            let flow = Flow::new(
                history.clone(),
                settings.clone(),
                audio_dir.clone(),
                audio_ctrl,
            );

            // Spawn retention sweeper.
            let settings_arc: Arc<Mutex<AppSettings>> = Arc::new(Mutex::new(settings.clone()));
            gc::spawn(history.clone(), settings_arc);

            // Hotkeys: bridge events into flow.
            let app_for_hotkey = app.handle().clone();
            let flow_for_hotkey = flow.clone();
            if let Err(e) = hotkey::register(
                app.handle(),
                &settings.light_hotkey,
                &settings.advanced_hotkey,
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

            app.manage(history);
            app.manage(flow);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::check_secrets,
            commands::save_secret,
            commands::delete_secret,
            commands::get_settings,
            commands::set_settings,
            commands::list_history,
            commands::delete_recording,
            commands::list_input_devices,
            commands::app_paths,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!("error while running wispr-fox: {e}");
    }
}
