//! Frontend command surface. Thin wrappers — all logic lives in domain modules.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::audio;
use crate::flow::Flow;
use crate::history::{History, Recording};
use crate::secrets::{self, SecretKey};
use crate::settings::AppSettings;

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretCheck {
    pub stt: bool,
    pub llm: bool,
}

#[tauri::command]
pub fn check_secrets() -> SecretCheck {
    SecretCheck {
        stt: secrets::has(SecretKey::GroqStt),
        llm: secrets::has(SecretKey::GroqLlm),
    }
}

#[tauri::command]
pub fn save_secret(key: String, value: String) -> Result<(), String> {
    let key = match key.as_str() {
        "groq_stt" => SecretKey::GroqStt,
        "groq_llm" => SecretKey::GroqLlm,
        other => return Err(format!("unknown secret key '{other}'")),
    };
    secrets::set(key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_secret(key: String) -> Result<(), String> {
    let key = match key.as_str() {
        "groq_stt" => SecretKey::GroqStt,
        "groq_llm" => SecretKey::GroqLlm,
        other => return Err(format!("unknown secret key '{other}'")),
    };
    secrets::delete(key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(flow: State<'_, Flow>) -> AppSettings {
    flow.settings()
}

#[tauri::command]
pub fn set_settings(flow: State<'_, Flow>, settings: AppSettings) {
    flow.set_settings(settings);
}

#[tauri::command]
pub fn list_history(history: State<'_, History>, limit: Option<i64>) -> Result<Vec<Recording>, String> {
    history
        .list_recent(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_recording(history: State<'_, History>, id: String) -> Result<(), String> {
    if let Some(r) = history.get(&id).map_err(|e| e.to_string())? {
        let _ = std::fs::remove_file(&r.audio_path);
    }
    history.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<audio::devices::InputDeviceInfo>, String> {
    audio::devices::list().map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AppPaths {
    pub audio_dir: PathBuf,
    pub db_path: PathBuf,
}

#[tauri::command]
pub fn app_paths(app: AppHandle) -> Result<AppPaths, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(AppPaths {
        audio_dir: dir.join("audio"),
        db_path: dir.join("history.sqlite"),
    })
}
