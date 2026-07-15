//! ElevenLabs Scribe transcription client. Multipart POST -> /v1/speech-to-text.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio::fs;

use super::{SttError, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.elevenlabs.io/v1/speech-to-text";
pub const DEFAULT_MODEL: &str = "scribe_v2";
const TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

pub struct ElevenLabsStt {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl ElevenLabsStt {
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
struct ElevenLabsResponse {
    text: String,
    #[serde(default)]
    language_code: Option<String>,
}

#[async_trait]
impl SttProvider for ElevenLabsStt {
    fn name(&self) -> &'static str {
        "elevenlabs"
    }

    async fn transcribe(
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
            .mime_str(super::mime_for_audio(wav_path))
            .map_err(|e| SttError::Decode(e.to_string()))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model_id", self.model.clone());

        if let Some(lang) = hint_lang {
            form = form.text("language_code", lang.to_owned());
        }

        let resp = self
            .client
            .post(ENDPOINT)
            .header("xi-api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| SttError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SttError::Http { status: status.as_u16(), body });
        }

        let parsed: ElevenLabsResponse = resp
            .json()
            .await
            .map_err(|e| SttError::Decode(e.to_string()))?;

        Ok(Transcript {
            text: parsed.text,
            language: parsed.language_code.or_else(|| hint_lang.map(str::to_owned)),
            duration_seconds: None,
        })
    }
}
