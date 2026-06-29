//! API-key storage with keyring-first semantics and a local-only fallback.
//!
//! Primary: OS keychain via keyring-rs
//!   - Windows: Credential Manager
//!   - macOS:   Keychain Services
//!   - Linux:   Secret Service / kwallet
//!
//! Fallback: used only after a verified keyring write fails. On Windows the
//! fallback is encrypted with DPAPI and can only be decrypted by the same
//! Windows user profile. Older plaintext `.keys.json` files are read once,
//! migrated to keyring or the encrypted fallback, then removed only after the
//! replacement has been verified.

use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "wispr-fox";
const ENCRYPTED_FALLBACK_FILENAME: &str = ".keys.enc.json";
const LEGACY_FALLBACK_FILENAME: &str = ".keys.json";
const AUDIT_LOG_FILENAME: &str = "secret-audit.jsonl";

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static AUDIT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .get_or_init(|| {
            let base = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("com.wispr-fox.app");
            fs::create_dir_all(&base).ok();
            base
        })
        .clone()
}

fn encrypted_fallback_path() -> PathBuf {
    app_data_dir().join(ENCRYPTED_FALLBACK_FILENAME)
}

fn legacy_fallback_path() -> PathBuf {
    app_data_dir().join(LEGACY_FALLBACK_FILENAME)
}

fn audit_log_path() -> PathBuf {
    app_data_dir().join(AUDIT_LOG_FILENAME)
}

#[cfg(windows)]
mod platform_fallback {
    use anyhow::{Context, Result};
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub const PROTECTION: &str = "dpapi";

    pub fn protect(plaintext: &str) -> Result<String> {
        let bytes = plaintext.as_bytes();
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .context("DPAPI protect failed")?;

            let protected =
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(HLOCAL(output.pbData.cast()));
            Ok(STANDARD.encode(protected))
        }
    }

    pub fn unprotect(stored: &str) -> Result<String> {
        let mut protected = STANDARD
            .decode(stored)
            .context("DPAPI fallback value is not valid base64")?;
        let input = CRYPT_INTEGER_BLOB {
            cbData: protected.len() as u32,
            pbData: protected.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .context("DPAPI unprotect failed")?;

            let clear = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(HLOCAL(output.pbData.cast()));
            String::from_utf8(clear).context("DPAPI fallback value is not UTF-8")
        }
    }
}

#[cfg(not(windows))]
mod platform_fallback {
    use anyhow::Result;

    pub const PROTECTION: &str = "plaintext";

    pub fn protect(plaintext: &str) -> Result<String> {
        Ok(plaintext.to_string())
    }

    pub fn unprotect(stored: &str) -> Result<String> {
        Ok(stored.to_string())
    }
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

    fn display_name(self) -> &'static str {
        match self {
            SecretKey::GroqStt => "Groq STT",
            SecretKey::GroqLlm => "Groq cleanup",
            SecretKey::GeminiLlm => "Gemini cleanup",
            SecretKey::OpenAiStt => "OpenAI STT",
            SecretKey::OpenAiLlm => "OpenAI cleanup",
            SecretKey::DeepgramStt => "Deepgram STT",
            SecretKey::ElevenLabsStt => "ElevenLabs STT",
        }
    }
}

fn all_secret_keys() -> [SecretKey; 7] {
    [
        SecretKey::GroqStt,
        SecretKey::GroqLlm,
        SecretKey::GeminiLlm,
        SecretKey::OpenAiStt,
        SecretKey::OpenAiLlm,
        SecretKey::DeepgramStt,
        SecretKey::ElevenLabsStt,
    ]
}

// -- No-secret audit log -----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretAuditEvent {
    pub ts: String,
    pub key: String,
    pub label: String,
    pub action: String,
    pub storage: String,
    pub outcome: String,
    pub detail: String,
}

fn clean_detail(detail: impl AsRef<str>) -> String {
    let mut s: String = detail
        .as_ref()
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect();
    if s.len() > 240 {
        s.truncate(240);
        s.push_str("...");
    }
    s
}

fn record_secret_audit(
    key: SecretKey,
    action: &str,
    storage: &str,
    outcome: &str,
    detail: impl AsRef<str>,
) {
    let _guard = AUDIT_LOCK.get_or_init(|| Mutex::new(())).lock();
    let event = SecretAuditEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        key: key.entry_name().to_string(),
        label: key.display_name().to_string(),
        action: action.to_string(),
        storage: storage.to_string(),
        outcome: outcome.to_string(),
        detail: clean_detail(detail),
    };

    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    match serde_json::to_string(&event) {
        Ok(line) => {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(file, "{line}");
            }
        }
        Err(e) => tracing::warn!("secret audit event serialization failed: {e:#}"),
    }
}

pub fn audit_log(limit: Option<usize>) -> Vec<SecretAuditEvent> {
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let Ok(text) = fs::read_to_string(audit_log_path()) else {
        return Vec::new();
    };

    let mut out = VecDeque::with_capacity(limit);
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<SecretAuditEvent>(line) else {
            continue;
        };
        if out.len() == limit {
            out.pop_front();
        }
        out.push_back(event);
    }
    out.into_iter().collect()
}

// -- Keyring helpers ---------------------------------------------------------

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
    keyring_entry(key).ok().and_then(|e| e.get_password().ok())
}

fn keyring_delete(key: SecretKey) -> Result<()> {
    let entry = keyring_entry(key)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("keyring delete failed for {:?}: {e:#}", key)),
    }
}

fn keyring_probe() -> bool {
    const PROBE_ENTRY: &str = "__wispr_fox_keyring_probe__";
    let Ok(entry) = keyring::Entry::new(SERVICE, PROBE_ENTRY) else {
        return false;
    };
    let token = format!("probe-{}", uuid::Uuid::new_v4());
    let ok = entry.set_password(&token).is_ok()
        && entry
            .get_password()
            .map(|stored| stored == token)
            .unwrap_or(false);
    let _ = entry.delete_credential();
    ok
}

// -- File fallback -----------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedFallbackFile {
    version: u8,
    protection: String,
    entries: HashMap<String, String>,
}

fn decode_fallback_value(stored: &str, protection: &str) -> Result<String> {
    match protection {
        p if p == platform_fallback::PROTECTION => platform_fallback::unprotect(stored),
        // Defensive migration support if a prerelease ever wrote plaintext
        // using the new envelope. The next write re-encrypts on Windows.
        "plaintext" => Ok(stored.to_string()),
        other => Err(anyhow!("unsupported secret fallback protection '{other}'")),
    }
}

fn file_read_all() -> HashMap<String, String> {
    let path = encrypted_fallback_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let parsed: EncryptedFallbackFile = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "encrypted key fallback parse failed at {}: {e:#}",
                path.display()
            );
            return HashMap::new();
        }
    };

    let mut out = HashMap::new();
    for (name, stored) in parsed.entries {
        match decode_fallback_value(&stored, &parsed.protection) {
            Ok(value) => {
                out.insert(name, value);
            }
            Err(e) => {
                tracing::warn!("encrypted key fallback decrypt failed for {name}: {e:#}");
            }
        }
    }
    out
}

fn file_write_all(map: &HashMap<String, String>) -> Result<()> {
    let path = encrypted_fallback_path();
    if map.is_empty() {
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("removing empty encrypted fallback file {}", path.display())
            })?;
        }
        return Ok(());
    }

    let mut entries = HashMap::new();
    for (name, value) in map {
        let protected = platform_fallback::protect(value)
            .with_context(|| format!("encrypting fallback value for {name}"))?;
        entries.insert(name.clone(), protected);
    }
    let file = EncryptedFallbackFile {
        version: 1,
        protection: platform_fallback::PROTECTION.to_string(),
        entries,
    };
    let json = serde_json::to_string_pretty(&file)?;
    fs::write(&path, json)
        .with_context(|| format!("writing encrypted fallback key file {}", path.display()))
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

fn file_has(key: SecretKey) -> bool {
    file_read_all().contains_key(key.entry_name())
}

fn fallback_storage_name() -> &'static str {
    if cfg!(windows) {
        "encrypted_file"
    } else {
        "file"
    }
}

fn fallback_location() -> SecretLocation {
    if cfg!(windows) {
        SecretLocation::EncryptedFile
    } else {
        SecretLocation::File
    }
}

// -- Legacy plaintext fallback migration ------------------------------------

fn legacy_file_read_all() -> HashMap<String, String> {
    let path = legacy_fallback_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn legacy_file_write_all(map: &HashMap<String, String>) -> Result<()> {
    let path = legacy_fallback_path();
    if map.is_empty() {
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("removing legacy plaintext fallback {}", path.display())
            })?;
        }
        return Ok(());
    }
    let json = serde_json::to_string_pretty(map)?;
    fs::write(&path, json)
        .with_context(|| format!("writing legacy plaintext fallback {}", path.display()))
}

fn legacy_file_get(key: SecretKey) -> Option<String> {
    legacy_file_read_all().get(key.entry_name()).cloned()
}

fn legacy_file_delete(key: SecretKey) -> Result<()> {
    let mut map = legacy_file_read_all();
    map.remove(key.entry_name());
    legacy_file_write_all(&map)
}

fn legacy_file_has(key: SecretKey) -> bool {
    legacy_file_read_all().contains_key(key.entry_name())
}

fn delete_fallback_copies(key: SecretKey) {
    if file_has(key) {
        match file_delete(key) {
            Ok(()) => record_secret_audit(
                key,
                "cleanup",
                fallback_storage_name(),
                "ok",
                "Removed stale fallback after keyring write verified",
            ),
            Err(e) => record_secret_audit(
                key,
                "cleanup",
                fallback_storage_name(),
                "failed",
                e.to_string(),
            ),
        }
    }
    if legacy_file_has(key) {
        match legacy_file_delete(key) {
            Ok(()) => record_secret_audit(
                key,
                "cleanup",
                "legacy_file",
                "ok",
                "Removed stale plaintext fallback after replacement verified",
            ),
            Err(e) => record_secret_audit(key, "cleanup", "legacy_file", "failed", e.to_string()),
        }
    }
}

fn migrate_from_file(key: SecretKey, value: &str, source: &str) {
    match keyring_set(key, value) {
        Ok(()) if keyring_get(key).as_deref() == Some(value) => {
            record_secret_audit(
                key,
                "migrate",
                "keyring",
                "ok",
                format!("Migrated {source} entry to OS keyring"),
            );
            match source {
                "legacy_file" => {
                    let _ = legacy_file_delete(key);
                }
                _ => {
                    let _ = file_delete(key);
                }
            }
            return;
        }
        Ok(()) => record_secret_audit(
            key,
            "migrate",
            "keyring",
            "verify_failed",
            "Keyring accepted migration write but readback did not match",
        ),
        Err(e) => record_secret_audit(key, "migrate", "keyring", "failed", e.to_string()),
    }

    if source == "legacy_file" {
        match file_set(key, value) {
            Ok(()) if file_get(key).as_deref() == Some(value) => {
                record_secret_audit(
                    key,
                    "migrate",
                    fallback_storage_name(),
                    "ok",
                    "Migrated legacy plaintext fallback to local fallback",
                );
                if let Err(e) = legacy_file_delete(key) {
                    record_secret_audit(key, "cleanup", "legacy_file", "failed", e.to_string());
                }
            }
            Ok(()) => record_secret_audit(
                key,
                "migrate",
                fallback_storage_name(),
                "verify_failed",
                "Fallback write completed but readback did not match",
            ),
            Err(e) => record_secret_audit(
                key,
                "migrate",
                fallback_storage_name(),
                "failed",
                e.to_string(),
            ),
        }
    }
}

// -- Public API --------------------------------------------------------------

pub fn set(key: SecretKey, value: &str) -> Result<()> {
    record_secret_audit(
        key,
        "save",
        "keyring",
        "started",
        "Attempting verified OS keyring write",
    );

    let keyring_ok = match keyring_set(key, value) {
        Ok(()) if keyring_get(key).as_deref() == Some(value) => {
            record_secret_audit(
                key,
                "save",
                "keyring",
                "ok",
                "OS keyring write verified by readback",
            );
            true
        }
        Ok(()) => {
            record_secret_audit(
                key,
                "save",
                "keyring",
                "verify_failed",
                "Keyring accepted the write but readback did not return the saved value",
            );
            false
        }
        Err(e) => {
            record_secret_audit(key, "save", "keyring", "failed", e.to_string());
            false
        }
    };

    if keyring_ok {
        delete_fallback_copies(key);
        return Ok(());
    }

    record_secret_audit(
        key,
        "save",
        fallback_storage_name(),
        "started",
        "Using local fallback because keyring write did not verify",
    );
    file_set(key, value).with_context(|| format!("fallback file write failed for {:?}", key))?;
    if file_get(key).as_deref() != Some(value) {
        record_secret_audit(
            key,
            "save",
            fallback_storage_name(),
            "verify_failed",
            "Fallback write completed but readback did not match",
        );
        return Err(anyhow!("fallback file write did not verify for {:?}", key));
    }
    record_secret_audit(
        key,
        "save",
        fallback_storage_name(),
        "ok",
        "Local fallback write verified by readback",
    );
    if legacy_file_has(key) {
        let _ = legacy_file_delete(key);
    }
    Ok(())
}

pub fn get(key: SecretKey) -> Result<Option<String>> {
    if let Some(value) = keyring_get(key) {
        return Ok(Some(value));
    }

    if let Some(value) = file_get(key) {
        record_secret_audit(
            key,
            "read",
            fallback_storage_name(),
            "ok",
            "Read from local fallback because keyring had no value",
        );
        migrate_from_file(key, &value, fallback_storage_name());
        return Ok(Some(value));
    }

    if let Some(value) = legacy_file_get(key) {
        record_secret_audit(
            key,
            "read",
            "legacy_file",
            "ok",
            "Read from legacy plaintext fallback; migration attempted",
        );
        migrate_from_file(key, &value, "legacy_file");
        return Ok(Some(value));
    }

    Ok(None)
}

pub fn delete(key: SecretKey) -> Result<()> {
    record_secret_audit(
        key,
        "delete",
        "all",
        "started",
        "Deleting keyring and local fallback entries",
    );
    if let Err(e) = keyring_delete(key) {
        record_secret_audit(key, "delete", "keyring", "failed", e.to_string());
    } else {
        record_secret_audit(key, "delete", "keyring", "ok", "Keyring entry removed");
    }
    file_delete(key)?;
    legacy_file_delete(key)?;
    record_secret_audit(key, "delete", "all", "ok", "Local fallback entries removed");
    Ok(())
}

pub fn has(key: SecretKey) -> bool {
    matches!(get(key), Ok(Some(_)))
}

// -- Diagnostics -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretLocation {
    Keyring,
    EncryptedFile,
    File,
    LegacyFile,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsDiagnostic {
    pub stt: SecretLocation,
    pub llm: SecretLocation,
    pub gemini: SecretLocation,
    pub openai_stt: SecretLocation,
    pub openai_llm: SecretLocation,
    pub deepgram_stt: SecretLocation,
    pub elevenlabs_stt: SecretLocation,
    pub keyring_works: bool,
    /// Compatibility field for older UI code. Points to the current fallback.
    pub fallback_path: String,
    pub fallback_exists: bool,
    pub encrypted_fallback_path: String,
    pub encrypted_fallback_exists: bool,
    pub legacy_fallback_path: String,
    pub legacy_fallback_exists: bool,
    pub audit_log_path: String,
}

fn location_of(key: SecretKey) -> SecretLocation {
    if keyring_get(key).is_some() {
        SecretLocation::Keyring
    } else if file_get(key).is_some() {
        fallback_location()
    } else if legacy_file_get(key).is_some() {
        SecretLocation::LegacyFile
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
    let encrypted_path = encrypted_fallback_path();
    let legacy_path = legacy_fallback_path();
    SecretsDiagnostic {
        stt,
        llm,
        gemini,
        openai_stt,
        openai_llm,
        deepgram_stt,
        elevenlabs_stt,
        keyring_works: keyring_probe()
            || all_secret_keys()
                .into_iter()
                .any(|key| matches!(location_of(key), SecretLocation::Keyring)),
        fallback_path: encrypted_path.display().to_string(),
        fallback_exists: encrypted_path.exists(),
        encrypted_fallback_path: encrypted_path.display().to_string(),
        encrypted_fallback_exists: encrypted_path.exists(),
        legacy_fallback_path: legacy_path.display().to_string(),
        legacy_fallback_exists: legacy_path.exists(),
        audit_log_path: audit_log_path().display().to_string(),
    }
}
