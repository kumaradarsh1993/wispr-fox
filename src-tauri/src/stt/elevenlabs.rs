//! ElevenLabs Scribe transcription client. Multipart POST -> /v1/speech-to-text.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio::fs;

use super::{SttError, SttOptions, SttProvider, Transcript};

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
    #[serde(default)]
    words: Vec<ElevenLabsWord>,
}

#[derive(Deserialize)]
struct ElevenLabsWord {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    speaker_id: Option<String>,
    #[serde(default)]
    start: Option<f64>,
    /// "word" | "spacing" | "audio_event". Only real words carry meaning for
    /// turn grouping; spacing entries are whitespace and audio events are
    /// things like (laughter).
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

#[async_trait]
impl SttProvider for ElevenLabsStt {
    fn name(&self) -> &'static str {
        "elevenlabs"
    }

    async fn transcribe(
        &self,
        wav_path: &Path,
        opts: &SttOptions,
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

        if let Some(lang) = opts.language() {
            form = form.text("language_code", lang.to_owned());
        }

        if opts.diarize {
            // Scribe includes diarization in the base per-hour price — no
            // surcharge — and v2 handles up to 32 speakers. `num_speakers` is
            // left unset so the model decides; we don't know how many people
            // were in the room.
            form = form.text("diarize", "true");
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

        let language = parsed.language_code.or_else(|| opts.language.clone());

        if opts.diarize {
            let turns = super::turns_from_words(parsed.words.into_iter().filter_map(|w| {
                // Skip spacing and audio-event entries — only real words carry
                // speaker attribution worth grouping on.
                if w.kind.as_deref().is_some_and(|k| k != "word") {
                    return None;
                }
                let speaker = w.speaker_id?;
                let text = w.text?;
                Some((speaker, text, w.start))
            }));
            if !turns.is_empty() {
                return Ok(Transcript {
                    text: super::render_turns(&turns),
                    language,
                    duration_seconds: None,
                    speakers: Some(turns),
                });
            }
            tracing::info!("diarization requested but ElevenLabs returned no speaker labels");
        }

        Ok(Transcript::plain(parsed.text, language, None))
    }
}
