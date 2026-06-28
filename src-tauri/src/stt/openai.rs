//! OpenAI transcription client. Multipart POST -> /v1/audio/transcriptions.
//!
//! Uses the GPT-4o transcription snapshots. OpenAI file uploads are capped at
//! 25 MB, so this mirrors the Groq chunking path for longer WAVs.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio::fs;

use super::{chunk, SttError, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
pub const DEFAULT_MODEL: &str = "gpt-4o-transcribe";
const MAX_BYTES: u64 = 25 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(45);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

pub struct OpenAiStt {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiStt {
    pub fn with_model(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("reqwest client construction is infallible with default config");
        Self { client, api_key, model }
    }

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
            .text("response_format", "json");

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

        #[derive(Deserialize)]
        struct OpenAiResponse {
            text: String,
        }

        let parsed: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| SttError::Decode(e.to_string()))?;

        Ok(Transcript {
            text: parsed.text,
            language: hint_lang.map(str::to_owned),
            duration_seconds: None,
        })
    }
}

#[async_trait]
impl SttProvider for OpenAiStt {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn transcribe(
        &self,
        wav_path: &Path,
        hint_lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let meta = fs::metadata(wav_path).await?;

        if meta.len() <= chunk::TARGET_CHUNK_BYTES {
            return self.transcribe_one(wav_path, hint_lang).await;
        }

        let chunks = chunk::split_wav_if_needed(wav_path, chunk::TARGET_CHUNK_BYTES)
            .map_err(|e| SttError::Decode(format!("WAV chunking failed: {e}")))?;

        for c in &chunks {
            if let Ok(m) = std::fs::metadata(c) {
                if m.len() > MAX_BYTES {
                    chunk::cleanup_chunks(&chunks, wav_path);
                    return Err(SttError::FileTooLarge { bytes: m.len(), max: MAX_BYTES });
                }
            }
        }

        let mut parts = Vec::with_capacity(chunks.len());
        for chunk_path in &chunks {
            match self.transcribe_one(chunk_path, hint_lang).await {
                Ok(t) => parts.push(t.text),
                Err(e) => {
                    chunk::cleanup_chunks(&chunks, wav_path);
                    return Err(e);
                }
            }
        }

        chunk::cleanup_chunks(&chunks, wav_path);
        Ok(Transcript {
            text: parts.iter().map(|s| s.trim()).collect::<Vec<_>>().join(" "),
            language: hint_lang.map(str::to_owned),
            duration_seconds: None,
        })
    }
}
