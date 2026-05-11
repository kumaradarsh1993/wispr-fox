//! API-key storage with automatic fallback.
//!
//! Primary: Windows Credential Manager via keyring-rs (secure, no plaintext on disk).
//! Fallback: JSON file in app-data dir (less secure but always works).
//!
//! On write: try keyring → always write file as backup.
//! On read: try keyring → if empty, check file.
//! This way if the user fixes their Credential Manager later, the secure path
//! kicks in automatically, while the app never blocks on a broken keyring.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "wispr-fox";
const FALLBACK_FILENAME: &str = ".keys.json";

/// Lazily resolved path to the fallback key file.
static FALLBACK_PATH: OnceLock<Mutex<PathBuf>> = OnceLock::new();

fn fallback_path() -> PathBuf {
    FALLBACK_PATH
        .get_or_init(|| {
            // Match Tauri's app-data convention: %APPDATA%/com.wispr-fox.app/
            let base = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("com.wispr-fox.app");
            fs::create_dir_all(&base).ok();
            Mutex::new(base.join(FALLBACK_FILENAME))
        })
        .lock()
        .clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKey {
    GroqStt,
    GroqLlm,
    GeminiLlm,
}

impl SecretKey {
    fn entry_name(self) -> &'static str {
        match self {
            SecretKey::GroqStt => "groq_stt_key",
            SecretKey::GroqLlm => "groq_llm_key",
            SecretKey::GeminiLlm => "gemini_llm_key",
        }
    }
}

// ── Keyring helpers (best-effort) ──────────────────────────────────────────

fn keyring_entry(key: SecretKey) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, key.entry_name())
        .with_context(|| format!("opening keyring entry for {:?}", key))
}

fn keyring_set(key: SecretKey, value: &str) -> Result<()> {
    keyring_entry(key)?
        .set_password(value)
        .with_context(|| format!("storing secret {:?} in keyring", key))
}

fn keyring_get(key: SecretKey) -> Option<String> {
    keyring_entry(key)
        .ok()
        .and_then(|e| e.get_password().ok())
}

// ── File-based fallback ────────────────────────────────────────────────────

fn file_read_all() -> HashMap<String, String> {
    let path = fallback_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn file_write_all(map: &HashMap<String, String>) -> Result<()> {
    let path = fallback_path();
    let json = serde_json::to_string_pretty(map)?;
    fs::write(&path, json).with_context(|| format!("writing fallback key file {}", path.display()))
}

fn file_set(key: SecretKey, value: &str) -> Result<()> {
    let mut map = file_read_all();
    map.insert(key.entry_name().to_string(), value.to_string());
    file_write_all(&map)
}

fn file_get(key: SecretKey) -> Option<String> {
    file_read_all().get(key.entry_name()).cloned()
}

fn file_delete(key: SecretKey) -> Result<()> {
    let mut map = file_read_all();
    map.remove(key.entry_name());
    file_write_all(&map)
}

// ── Public API (keyring-first, file-fallback) ──────────────────────────────

pub fn set(key: SecretKey, value: &str) -> Result<()> {
    // Always write file fallback so reads never fail.
    file_set(key, value)?;
    // Best-effort keyring write — log but don't block on failure.
    if let Err(e) = keyring_set(key, value) {
        tracing::warn!("keyring write failed for {:?}, using file fallback: {e:#}", key);
    }
    Ok(())
}

pub fn get(key: SecretKey) -> Result<Option<String>> {
    // Try keyring first (secure path).
    if let Some(v) = keyring_get(key) {
        return Ok(Some(v));
    }
    // Fallback to file.
    Ok(file_get(key))
}

pub fn delete(key: SecretKey) -> Result<()> {
    // Clean up file.
    file_delete(key)?;
    // Best-effort keyring delete.
    if let Ok(entry) = keyring_entry(key) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => tracing::warn!("keyring delete failed for {:?}: {e:#}", key),
        }
    }
    Ok(())
}

pub fn has(key: SecretKey) -> bool {
    matches!(get(key), Ok(Some(_)))
}
