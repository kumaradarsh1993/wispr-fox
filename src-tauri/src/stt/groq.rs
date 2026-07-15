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
/// How long to wait for the TCP+TLS handshake specifically. The overall
/// `TIMEOUT` covers the whole request (including the upload + Whisper's
/// processing time), but without a separate connect cap a half-dead socket
/// — common in the first seconds after resume, or on flaky Wi-Fi — can hang
/// for many seconds before failing. Bounding the connect to 6s means a bad
/// connection is abandoned quickly and we move on to a fresh retry instead
/// of stalling. A healthy handshake to Groq is well under a second even
/// from India, so 6s is comfortable headroom.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);
/// How many times we'll attempt a single transcription request before
/// giving up. The first try plus two retries. Tuned against the outer 120s
/// STT cap in `flow.rs` — 3 attempts × 30s timeout + backoff stays well
/// under it.
const MAX_ATTEMPTS: usize = 3;
/// Base backoff between retries; multiplied by the attempt number so the
/// gaps grow (~400ms, ~800ms). Short enough not to feel laggy, long enough
/// to let a just-resumed Wi-Fi adapter or a momentary DNS blip settle.
const RETRY_BASE_DELAY_MS: u64 = 400;

/// True for reqwest errors worth retrying — transport-level failures
/// (TCP connect refused/reset, request never made it out, timeout) that
/// commonly happen on flaky Wi-Fi or in the first few seconds after the
/// machine resumes from sleep, when the network stack is briefly
/// unavailable. These are exactly the "error sending request for url …"
/// failures users hit: a fresh attempt after a short pause almost always
/// succeeds. We deliberately do NOT retry decode/redirect errors — those
/// won't fix themselves.
fn is_transient(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}

/// Sleep before the next retry. Gap grows with the attempt index
/// (~400ms after attempt 1, ~800ms after attempt 2).
async fn sleep_backoff(attempt: usize) {
    let delay = Duration::from_millis(RETRY_BASE_DELAY_MS * attempt as u64);
    tokio::time::sleep(delay).await;
}

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
            .connect_timeout(CONNECT_TIMEOUT)
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

        // Retry loop. `multipart::Form` is consumed by `.send()` (its parts
        // hold the file bytes), so we rebuild the form from the in-memory
        // `bytes` on every attempt rather than trying to clone a sent form.
        let mut last_err: Option<SttError> = None;

        for attempt in 1..=MAX_ATTEMPTS {
            let file_part = multipart::Part::bytes(bytes.clone())
                .file_name(filename.clone())
                .mime_str(super::mime_for_audio(wav_path))
                .map_err(|e| SttError::Decode(e.to_string()))?;

            let mut form = multipart::Form::new()
                .part("file", file_part)
                .text("model", self.model.clone())
                .text("response_format", "verbose_json");

            if let Some(lang) = hint_lang {
                form = form.text("language", lang.to_owned());
            }

            let send_result = self
                .client
                .post(ENDPOINT)
                .bearer_auth(&self.api_key)
                .multipart(form)
                .send()
                .await;

            match send_result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let parsed: GroqResponse = resp
                            .json()
                            .await
                            .map_err(|e| SttError::Decode(e.to_string()))?;
                        if attempt > 1 {
                            tracing::info!(attempt, "Groq STT succeeded after retry");
                        }
                        return Ok(Transcript {
                            text: parsed.text,
                            language: parsed.language,
                            duration_seconds: parsed.duration,
                        });
                    }

                    let code = status.as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    // Retry only on server-side hiccups (5xx). 4xx (bad key,
                    // malformed request) and 429 (rate limit) won't improve on
                    // an immediate retry — surface them straight away so the
                    // user gets the right message instead of a delayed failure.
                    if (500..=599).contains(&code) && attempt < MAX_ATTEMPTS {
                        tracing::warn!(attempt, status = code, "Groq STT 5xx — retrying");
                        last_err = Some(SttError::Http { status: code, body });
                        sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(SttError::Http { status: code, body });
                }
                Err(e) => {
                    if is_transient(&e) && attempt < MAX_ATTEMPTS {
                        tracing::warn!(
                            attempt,
                            error = %e,
                            "Groq STT transient network error — retrying"
                        );
                        last_err = Some(SttError::Network(e.to_string()));
                        sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(SttError::Network(e.to_string()));
                }
            }
        }

        // Loop only falls through here if every attempt set `last_err` and
        // hit the `attempt < MAX_ATTEMPTS` guard on the final pass — i.e. we
        // exhausted retries on a transient/5xx failure.
        Err(last_err.unwrap_or_else(|| SttError::Network("STT failed after retries".into())))
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
