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
    OpenAiStt,
    OpenAiLlm,
    DeepgramStt,
    ElevenLabsStt,
}

impl SecretKey {
    fn entry_name(self) -> &'static str {
        match self {
            SecretKey::GroqStt => "groq_stt_key",
            SecretKey::GroqLlm => "groq_llm_key",
            SecretKey::GeminiLlm => "gemini_llm_key",
            SecretKey::OpenAiStt => "openai_stt_key",
            SecretKey::OpenAiLlm => "openai_llm_key",
            SecretKey::DeepgramStt => "deepgram_stt_key",
            SecretKey::ElevenLabsStt => "elevenlabs_stt_key",
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
    // Pure read — keyring first, then file fallback. NO side effects.
    //
    // The previous implementation did "opportunistic migration" — if the
    // value was found in the file, it would try to write it to the keyring
    // and delete the file. That deletion trusted `keyring_set`'s Ok return,
    // which (per the bug fixed in `set` above) doesn't actually prove the
    // credential persisted. On machines where the keyring silently fails,
    // the file got deleted after the first read and the next read returned
    // None — exactly matching the reported "save works, demo transcribes
    // fine, real transcription says key not found" pattern in nightly.2/.3.
    //
    // Now: reads are pure. Migration only happens on explicit `set()` calls,
    // which use verify-readback to decide whether the file is still needed.
    if let Some(v) = keyring_get(key) {
        return Ok(Some(v));
    }
    let Some(v) = file_get(key) else {
        return Ok(None);
    };

    // Migrate stale plaintext entries only after verified keyring readback.
    // If this machine's keyring accepts writes but does not persist them, keep
    // the file fallback so the saved key does not vanish on the next launch.
    match keyring_set(key, &v) {
        Ok(()) if keyring_get(key).as_deref() == Some(v.as_str()) => {
            if let Err(e) = file_delete(key) {
                tracing::warn!("couldn't clean migrated plaintext entry for {:?}: {e:#}", key);
            }
        }
        Ok(()) => tracing::warn!(
            "keyring migration write didn't persist for {:?}; keeping fallback file entry",
            key
        ),
        Err(e) => tracing::debug!(
            "keyring migration failed for {:?} ({e:#}); keeping fallback file entry",
            key
        ),
    }

    Ok(Some(v))
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

// ── Diagnostics ────────────────────────────────────────────────────────────

/// Where a given secret is currently stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretLocation {
    /// Stored in the OS keyring (secure path).
    Keyring,
    /// Stored in the file fallback (keyring write didn't persist).
    File,
    /// Not stored anywhere.
    None,
}

/// Per-secret diagnostic info — where the value lives, used by the
/// Settings page "secret storage status" panel to help users diagnose
/// Windows Credential Manager / macOS Keychain reliability issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsDiagnostic {
    pub stt: SecretLocation,
    pub llm: SecretLocation,
    pub gemini: SecretLocation,
    pub openai_stt: SecretLocation,
    pub openai_llm: SecretLocation,
    pub deepgram_stt: SecretLocation,
    pub elevenlabs_stt: SecretLocation,
    /// Whether the keyring backend appears to work on this machine at all.
    /// True if at least one secret could be read from the keyring.
    pub keyring_works: bool,
    /// Absolute path to the file fallback (whether or not it currently exists).
    pub fallback_path: String,
    /// True if the file fallback actually exists on disk right now.
    pub fallback_exists: bool,
}

fn location_of(key: SecretKey) -> SecretLocation {
    if keyring_get(key).is_some() {
        SecretLocation::Keyring
    } else if file_get(key).is_some() {
        SecretLocation::File
    } else {
        SecretLocation::None
    }
}

pub fn diagnostic() -> SecretsDiagnostic {
    let stt = location_of(SecretKey::GroqStt);
    let llm = location_of(SecretKey::GroqLlm);
    let gemini = location_of(SecretKey::GeminiLlm);
    let openai_stt = location_of(SecretKey::OpenAiStt);
    let openai_llm = location_of(SecretKey::OpenAiLlm);
    let deepgram_stt = location_of(SecretKey::DeepgramStt);
    let elevenlabs_stt = location_of(SecretKey::ElevenLabsStt);
    let keyring_works = [
        stt,
        llm,
        gemini,
        openai_stt,
        openai_llm,
        deepgram_stt,
        elevenlabs_stt,
    ]
    .iter()
    .any(|loc| matches!(loc, SecretLocation::Keyring));
    let path = fallback_path();
    SecretsDiagnostic {
        stt,
        llm,
        gemini,
        openai_stt,
        openai_llm,
        deepgram_stt,
        elevenlabs_stt,
        keyring_works,
        fallback_exists: path.exists(),
        fallback_path: path.display().to_string(),
    }
}
