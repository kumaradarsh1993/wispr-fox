//! Deepgram pre-recorded transcription client. Binary POST -> /v1/listen.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;

use super::{SttError, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.deepgram.com/v1/listen";
pub const DEFAULT_MODEL: &str = "nova-3";
const TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

/// Process-wide pooled client. Previously a fresh `reqwest::Client` was built
/// for EVERY recording, so every dictation paid a full DNS + TCP + TLS
/// handshake to api.deepgram.com with zero connection reuse — a real chunk of
/// the intermittent tail latency, especially from a high-RTT link. A shared
/// client keeps warm keep-alive connections in its pool, so back-to-back
/// dictations skip the handshake entirely. Cloning it is cheap (it's an Arc)
/// and the pool is shared across clones. Key/model stay per-request, so one
/// client serves every user and model.
static SHARED_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .build()
                .expect("reqwest client construction is infallible with default config")
        })
        .clone()
}

pub struct DeepgramStt {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl DeepgramStt {
    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: shared_client(),
            api_key,
            model,
        }
    }
}

#[derive(Deserialize)]
struct DeepgramResponse {
    metadata: Option<DeepgramMetadata>,
    results: DeepgramResults,
}

#[derive(Deserialize)]
struct DeepgramMetadata {
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
    #[serde(default)]
    detected_language: Option<String>,
}

#[derive(Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

#[async_trait]
impl SttProvider for DeepgramStt {
    fn name(&self) -> &'static str {
        "deepgram"
    }

    async fn transcribe(
        &self,
        wav_path: &Path,
        hint_lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let bytes = fs::read(wav_path).await?;
        let mut req = self
            .client
            .post(ENDPOINT)
            .query(&[("model", self.model.as_str()), ("smart_format", "true")])
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", super::mime_for_audio(wav_path))
            .body(bytes);

        if let Some(lang) = hint_lang {
            req = req.query(&[("language", lang)]);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| SttError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SttError::Http { status: status.as_u16(), body });
        }

        let parsed: DeepgramResponse = resp
            .json()
            .await
            .map_err(|e| SttError::Decode(e.to_string()))?;

        let first_channel = parsed.results.channels.into_iter().next();
        let language = first_channel
            .as_ref()
            .and_then(|c| c.detected_language.clone())
            .or_else(|| hint_lang.map(str::to_owned));
        let text = first_channel
            .and_then(|c| c.alternatives.into_iter().next())
            .map(|a| a.transcript)
            .unwrap_or_default();

        Ok(Transcript {
            text,
            language,
            duration_seconds: parsed.metadata.and_then(|m| m.duration),
        })
    }
}
