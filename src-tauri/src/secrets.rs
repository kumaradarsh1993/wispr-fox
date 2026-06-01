//! API-key storage with secure-first, fall-back-to-file-only-if-needed semantics.
//!
//! Primary: OS keychain via keyring-rs
//!   - Windows: Credential Manager (DPAPI under the hood)
//!   - macOS:   Keychain Services
//!   - Linux:   Secret Service (libsecret) / kwallet
//!
//! Fallback: JSON file in app-data dir, used ONLY when the keyring write
//! fails (uncommon — broken Credential Manager, headless Linux without a
//! secret service, sandboxed Keychain, etc). We deliberately do not write
//! the file on the happy path: an unconditional plaintext fallback meant
//! any other process running as the user could read the keys.
//!
//! Reads prefer the keyring; if a value is only present in the fallback
//! file, we opportunistically migrate it into the keyring and delete it
//! from the file so subsequent reads/writes converge on the secure path.

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

// ── Keyring helpers ────────────────────────────────────────────────────────

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

fn keyring_delete(key: SecretKey) {
    if let Ok(entry) = keyring_entry(key) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => tracing::warn!("keyring delete failed for {:?}: {e:#}", key),
        }
    }
}

// ── File-based fallback (used ONLY when keyring is unavailable) ────────────

fn file_read_all() -> HashMap<String, String> {
    let path = fallback_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn file_write_all(map: &HashMap<String, String>) -> Result<()> {
    let path = fallback_path();
    if map.is_empty() {
        // Don't leave an empty file on disk if everything migrated to the
        // keyring — fewer footprints for forensics, and it's the most
        // honest signal that no plaintext is being kept.
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("removing empty fallback file {}", path.display()))?;
        }
        return Ok(());
    }
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

/// Returns true if there's an entry for `key` in the fallback file.
fn file_has(key: SecretKey) -> bool {
    file_read_all().contains_key(key.entry_name())
}

// ── Public API (keyring-first; file only on keyring failure) ───────────────

pub fn set(key: SecretKey, value: &str) -> Result<()> {
    // Try the secure path first. Only if that genuinely fails do we accept
    // the plaintext-on-disk cost.
    //
    // CRITICAL: we VERIFY the keyring write actually persisted by reading it
    // back. Some platforms accept `set_password()` and return Ok even when
    // the credential doesn't survive — Windows Credential Manager corruption,
    // sandboxed macOS apps without the keychain-access entitlement, headless
    // Linux without a running Secret Service. Without verify-readback, the
    // file fallback gets deleted and the next `get()` returns None, so the
    // UI reports "no key saved" even though the user just clicked Save.
    // Reported by a user on v1.1.0-nightly.2 — file fallback was the only
    // thing keeping things working pre-security-pass.
    let keyring_ok = match keyring_set(key, value) {
        Ok(()) => keyring_get(key).as_deref() == Some(value),
        Err(e) => {
            tracing::warn!(
                "keyring write failed for {:?} ({e:#}); falling back to file storage",
                key
            );
            false
        }
    };

    if keyring_ok {
        // Belt and braces: drop any stale plaintext copy from a previous
        // version that always wrote the file.
        if file_has(key) {
            if let Err(e) = file_delete(key) {
                tracing::warn!("couldn't clean stale plaintext entry for {:?}: {e:#}", key);
            }
        }
        Ok(())
    } else {
        // Either keyring write errored, OR it returned Ok but the readback
        // didn't return our value — credential didn't persist. Save to file
        // so the user's key isn't silently dropped.
        tracing::warn!(
            "keyring write didn't persist for {:?} — falling back to file storage",
            key
        );
        file_set(key, value)
            .with_context(|| format!("fallback file write failed for {:?}", key))
    }
}

pub fn get(key: SecretKey) -> Result<Option<String>> {
    // Secure path first.
    if let Some(v) = keyring_get(key) {
        // If a stale plaintext copy survives from pre-fix installs, drop it.
        if file_has(key) {
            if let Err(e) = file_delete(key) {
                tracing::warn!("couldn't drop stale plaintext for {:?}: {e:#}", key);
            }
        }
        return Ok(Some(v));
    }
    // Fallback only. Opportunistically migrate so future reads hit keyring.
    if let Some(v) = file_get(key) {
        if keyring_set(key, &v).is_ok() {
            if let Err(e) = file_delete(key) {
                tracing::warn!("migrated {:?} to keyring; couldn't drop file copy: {e:#}", key);
            } else {
                tracing::info!("migrated {:?} from plaintext fallback into keyring", key);
            }
        }
        return Ok(Some(v));
    }
    Ok(None)
}

pub fn delete(key: SecretKey) -> Result<()> {
    keyring_delete(key);
    // Always sweep the file too, in case a value was ever stored there.
    file_delete(key)?;
    Ok(())
}

pub fn has(key: SecretKey) -> bool {
    matches!(get(key), Ok(Some(_)))
}
