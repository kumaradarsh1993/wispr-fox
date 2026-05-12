//! Groq Whisper REST client. Multipart POST → /openai/v1/audio/transcriptions.
//!
//! Free tier (May 2026): 2,000 req/day, 7,200 audio-seconds/hour, 25 MB/file.
//! No `language` param by default → auto-detect (we mix Hindi sometimes).

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio::fs;

use super::{chunk, SttError, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";
/// Groq's documented per-request file ceiling. We don't actually use this
/// as a rejection threshold any more — instead, `TARGET_CHUNK_BYTES` (in
/// `stt::chunk`) tells us when to split. Kept here for documentation and
/// for the rare case where a single chunk somehow still exceeds 25 MB
/// (e.g. unusually high sample rate) — we'd surface FileTooLarge then
/// instead of letting Groq return a cryptic 413.
const MAX_BYTES: u64 = 25 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct GroqStt {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GroqStt {
    pub fn new(api_key: String) -> Self {
        Self::with_model(api_key, DEFAULT_MODEL.into())
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("reqwest client construction is infallible with default config");
        Self { client, api_key, model }
    }
}

#[derive(Deserialize)]
struct GroqResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

impl GroqStt {
    /// Transcribe a single WAV that's already under the size limit. Inner
    /// helper used both directly (small clips) and by the chunked path
    /// (long clips).
    async fn transcribe_one(
        &self,
        wav_path: &Path,
        hint_lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let bytes = fs::read(wav_path).await?;
        let filename = wav_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("clip.wav")
            .to_owned();

        let file_part = multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str("audio/wav")
            .map_err(|e| SttError::Decode(e.to_string()))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        if let Some(lang) = hint_lang {
            form = form.text("language", lang.to_owned());
        }

        let resp = self
            .client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| SttError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SttError::Http { status: status.as_u16(), body });
        }

        let parsed: GroqResponse = resp
            .json()
            .await
            .map_err(|e| SttError::Decode(e.to_string()))?;

        Ok(Transcript {
            text: parsed.text,
            language: parsed.language,
            duration_seconds: parsed.duration,
        })
    }
}

#[async_trait]
impl SttProvider for GroqStt {
    fn name(&self) -> &'static str {
        "groq"
    }

    async fn transcribe(
        &self,
        wav_path: &Path,
        hint_lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let meta = fs::metadata(wav_path).await?;

        // Fast path: file fits in one request. The vast majority of clips
        // hit this — typical dictation is < 60s.
        if meta.len() <= chunk::TARGET_CHUNK_BYTES {
            return self.transcribe_one(wav_path, hint_lang).await;
        }

        // Slow path: file exceeds the per-request limit. Split into
        // chunks, transcribe each, concatenate the text. We do chunks
        // SEQUENTIALLY rather than in parallel — Groq's free tier rate
        // limit (30 RPM) is tight, and serial keeps us nice. For a typical
        // 5-min recording this is 2 chunks back-to-back ≈ 4-6 s extra.
        let chunks = chunk::split_wav_if_needed(wav_path, chunk::TARGET_CHUNK_BYTES)
            .map_err(|e| SttError::Decode(format!("WAV chunking failed: {e}")))?;

        // Belt-and-suspenders: a single chunk that still exceeds MAX_BYTES
        // is genuinely unsendable (extreme sample rate, exotic format).
        // Surface FileTooLarge rather than burning a request that 413s.
        for c in &chunks {
            if let Ok(m) = std::fs::metadata(c) {
                if m.len() > MAX_BYTES {
                    chunk::cleanup_chunks(&chunks, wav_path);
                    return Err(SttError::FileTooLarge { bytes: m.len(), max: MAX_BYTES });
                }
            }
        }

        tracing::info!(
            n_chunks = chunks.len(),
            original_bytes = meta.len(),
            "transcribing in chunks (file exceeds single-request limit)"
        );

        let mut parts: Vec<String> = Vec::with_capacity(chunks.len());
        let mut detected_lang: Option<String> = None;
        let mut total_duration: f64 = 0.0;

        for (idx, chunk_path) in chunks.iter().enumerate() {
            let t = match self.transcribe_one(chunk_path, hint_lang).await {
                Ok(t) => t,
                Err(e) => {
                    chunk::cleanup_chunks(&chunks, wav_path);
                    return Err(e);
                }
            };
            tracing::info!(chunk_index = idx, chars = t.text.len(), "chunk transcribed");
            if detected_lang.is_none() {
                detected_lang = t.language.clone();
            }
            if let Some(d) = t.duration_seconds {
                total_duration += d;
            }
            parts.push(t.text);
        }

        chunk::cleanup_chunks(&chunks, wav_path);

        // Join with a space. Chunks may split mid-word; Whisper's tokeniser
        // usually trims/normalises whitespace at boundaries, so a single
        // space between chunk texts produces clean concatenated output.
        let mut joined = String::with_capacity(parts.iter().map(|s| s.len() + 1).sum());
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                joined.push(' ');
            }
            joined.push_str(p.trim());
        }

        Ok(Transcript {
            text: joined,
            language: detected_lang,
            duration_seconds: if total_duration > 0.0 { Some(total_duration) } else { None },
        })
    }
}
