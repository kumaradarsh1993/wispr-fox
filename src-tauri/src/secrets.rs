//! Windows Credential Manager wrapper via keyring-rs.
//!
//! All API keys live here and never touch disk in plaintext. Each key is one
//! credential entry under the service `wispr-fox`, addressed by a stable string
//! id from `SecretKey`. Adding a new provider = adding a `SecretKey` variant.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SERVICE: &str = "wispr-fox";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKey {
    GroqStt,
    GroqLlm,
}

impl SecretKey {
    fn entry_name(self) -> &'static str {
        match self {
            SecretKey::GroqStt => "groq_stt_key",
            SecretKey::GroqLlm => "groq_llm_key",
        }
    }
}

fn entry(key: SecretKey) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, key.entry_name())
        .with_context(|| format!("opening keyring entry for {:?}", key))
}

pub fn set(key: SecretKey, value: &str) -> Result<()> {
    entry(key)?
        .set_password(value)
        .with_context(|| format!("storing secret {:?}", key))
}

pub fn get(key: SecretKey) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading secret {:?}", key)),
    }
}

pub fn delete(key: SecretKey) -> Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("deleting secret {:?}", key)),
    }
}

pub fn has(key: SecretKey) -> bool {
    matches!(get(key), Ok(Some(_)))
}
