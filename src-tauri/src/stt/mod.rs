//! Speech-to-text provider abstraction.
//!
//! Primary impl is Groq whisper-large-v3-turbo (see `groq.rs`). Adding a new
//! provider = implementing `SttProvider` for it. The flow layer holds a boxed
//! trait object so providers can be swapped at runtime per user settings.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod groq;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("missing API key for provider {0}")]
    MissingKey(&'static str),
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("file too large: {bytes} bytes (max {max})")]
    FileTooLarge { bytes: u64, max: u64 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait SttProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// Transcribe `wav_path` (16 kHz mono i16 WAV expected).
    /// `hint_lang` is an ISO-639-1 code or `None` for auto-detect.
    async fn transcribe(
        &self,
        wav_path: &Path,
        hint_lang: Option<&str>,
    ) -> Result<Transcript, SttError>;
}
