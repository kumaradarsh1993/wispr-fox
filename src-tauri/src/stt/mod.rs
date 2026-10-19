//! Speech-to-text provider abstraction.
//!
//! Primary impl is Groq whisper-large-v3-turbo (see `groq.rs`). Adding a new
//! provider = implementing `SttProvider` for it. The flow layer holds a boxed
//! trait object so providers can be swapped at runtime per user settings.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod chunk;
pub mod deepgram;
pub mod elevenlabs;
pub mod groq;
pub mod openai;
pub mod speakers;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub duration_seconds: Option<f64>,
    /// Speaker-attributed turns, present only when diarization was requested
    /// AND the provider returned per-word speaker labels. `text` is rendered
    /// from these when they exist, so downstream code that only reads `text`
    /// transparently gets the labelled version.
    #[serde(default)]
    pub speakers: Option<Vec<SpeakerTurn>>,
}

impl Transcript {
    /// Plain transcript with no speaker attribution — the common case.
    pub fn plain(text: String, language: Option<String>, duration_seconds: Option<f64>) -> Self {
        Self { text, language, duration_seconds, speakers: None }
    }
}

/// Per-request STT knobs. Grouped into a struct rather than added as more
/// positional parameters so adding the next option doesn't touch all four
/// provider implementations again.
#[derive(Debug, Clone, Default)]
pub struct SttOptions {
    /// ISO-639-1 hint, or `None` for auto-detect.
    pub language: Option<String>,
    /// Ask the provider to attribute speech to speakers. Silently ignored by
    /// providers that can't do it — the caller is responsible for not offering
    /// the option there (see `provider_supports_diarization`).
    pub diarize: bool,
}

impl SttOptions {
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}

/// Whole-request deadline for providers that take the entire file in one
/// request (Deepgram, ElevenLabs): 90 s plus 30 s per 10 MB of audio, capped
/// at 15 minutes. A flat 90 s covered the upload as well as the provider's
/// processing, so an hour-long meeting on a slow uplink timed out before the
/// provider had finished receiving it.
pub fn scaled_request_timeout(bytes: u64) -> std::time::Duration {
    const BASE_SECS: f64 = 90.0;
    const PER_10_MB_SECS: f64 = 30.0;
    const CAP_SECS: f64 = 15.0 * 60.0;
    let ten_mb = 10.0 * 1024.0 * 1024.0;
    let secs = BASE_SECS + PER_10_MB_SECS * (bytes as f64 / ten_mb);
    std::time::Duration::from_secs_f64(secs.min(CAP_SECS))
}

#[cfg(test)]
mod timeout_tests {
    use super::scaled_request_timeout;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn small_files_keep_the_old_90s() {
        assert_eq!(scaled_request_timeout(0).as_secs(), 90);
    }

    #[test]
    fn adds_30s_per_10_mb() {
        assert_eq!(scaled_request_timeout(10 * MB).as_secs(), 120);
        assert_eq!(scaled_request_timeout(50 * MB).as_secs(), 240);
        assert_eq!(scaled_request_timeout(5 * MB).as_secs(), 105);
    }

    #[test]
    fn caps_at_15_minutes() {
        assert_eq!(scaled_request_timeout(270 * MB).as_secs(), 900);
        assert_eq!(scaled_request_timeout(2_000 * MB).as_secs(), 900);
    }
}

pub use speakers::{provider_supports_diarization, render_turns, turns_from_words, SpeakerTurn};

/// Audio container extensions we accept for uploaded files. The cloud STT
/// providers (Groq/OpenAI Whisper, Deepgram, ElevenLabs) all decode these
/// server-side, so we forward the original bytes untouched rather than
/// transcoding locally. `.wav` is what the mic recorder writes.
pub const SUPPORTED_AUDIO_EXTS: &[&str] = &[
    "wav", "mp3", "m4a", "mp4", "aac", "ogg", "oga", "opus", "flac", "webm", "mpga", "mpeg",
];

/// Whether `ext` (lowercase, no dot) is an audio container we can transcribe.
pub fn is_supported_audio_ext(ext: &str) -> bool {
    SUPPORTED_AUDIO_EXTS.contains(&ext)
}

/// MIME type to advertise for an audio file, keyed off its extension. Used
/// both for the multipart upload part (Groq/OpenAI) and the raw-body
/// Content-Type (Deepgram). Providers sniff the actual container too, but a
/// correct hint avoids ambiguity for formats like m4a. Unknown extensions
/// fall back to a generic audio type.
pub fn mime_for_audio(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("mp3") | Some("mpga") | Some("mpeg") => "audio/mpeg",
        Some("m4a") | Some("mp4") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("flac") => "audio/flac",
        Some("webm") => "audio/webm",
        _ => "audio/wav",
    }
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
    async fn transcribe(
        &self,
        wav_path: &Path,
        opts: &SttOptions,
    ) -> Result<Transcript, SttError>;
}
