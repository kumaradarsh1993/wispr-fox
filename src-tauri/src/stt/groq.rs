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

use super::{SttError, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";
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
        if meta.len() > MAX_BYTES {
            return Err(SttError::FileTooLarge { bytes: meta.len(), max: MAX_BYTES });
        }

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
