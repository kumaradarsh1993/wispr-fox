//! OpenAI transcription client. Multipart POST -> /v1/audio/transcriptions.
//!
//! Uses `gpt-transcribe` for ordinary files and the specialized GPT-4o
//! diarization model for speaker-labelled meetings. OpenAI file uploads are
//! capped at 25 MB, so ordinary transcription mirrors the Groq chunking path.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio::fs;

use super::{chunk, render_turns, turns_from_words, SttError, SttOptions, SttProvider, Transcript};

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
pub const DEFAULT_MODEL: &str = "gpt-transcribe";
const MAX_BYTES: u64 = 25 * 1024 * 1024;
const ORDINARY_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
// Diarization analyzes the complete meeting in one request so speaker ids stay
// stable. Give half-hour meeting audio realistic processing headroom while
// keeping a finite wall-clock ceiling for a wedged provider connection.
const DIARIZED_REQUEST_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

fn language_field(model: &str) -> &'static str {
    // The current gpt-transcribe multipart contract is plural even for one
    // hint. Older transcription models, including the diarization model, use
    // the legacy singular field.
    if model == "gpt-transcribe" { "languages[]" } else { "language" }
}

pub struct OpenAiStt {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiStt {
    pub fn with_model(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("reqwest client construction is infallible with default config");
        Self { client, api_key, model }
    }

    async fn transcribe_one(
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

        let diarized = opts.diarize && self.model == "gpt-4o-transcribe-diarize";
        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", if diarized { "diarized_json" } else { "json" });

        if diarized {
            form = form.text("chunking_strategy", "auto");
        }

        if let Some(lang) = opts.language() {
            form = form.text(language_field(&self.model), lang.to_owned());
        }

        let request_timeout = if diarized {
            DIARIZED_REQUEST_TIMEOUT
        } else {
            ORDINARY_REQUEST_TIMEOUT
        };
        let resp = self
            .client
            .post(ENDPOINT)
            .timeout(request_timeout)
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

        if diarized {
            #[derive(Deserialize)]
            struct Segment {
                speaker: String,
                text: String,
                start: f64,
                end: f64,
            }
            #[derive(Deserialize)]
            struct DiarizedResponse {
                #[serde(default)]
                segments: Vec<Segment>,
            }
            let parsed: DiarizedResponse = resp
                .json()
                .await
                .map_err(|e| SttError::Decode(e.to_string()))?;
            let duration = parsed.segments.last().map(|s| s.end);
            let turns = turns_from_words(
                parsed.segments.into_iter().map(|s| (s.speaker, s.text, Some(s.start))),
            );
            let text = render_turns(&turns);
            return Ok(Transcript {
                text,
                language: opts.language.clone(),
                duration_seconds: duration,
                speakers: (!turns.is_empty()).then_some(turns),
            });
        }

        #[derive(Deserialize)]
        struct OpenAiResponse { text: String }
        let parsed: OpenAiResponse = resp.json().await.map_err(|e| SttError::Decode(e.to_string()))?;
        Ok(Transcript::plain(parsed.text, opts.language.clone(), None))
    }
}

#[cfg(test)]
mod tests {
    use super::language_field;

    #[test]
    fn current_transcribe_model_uses_plural_language_field() {
        assert_eq!(language_field("gpt-transcribe"), "languages[]");
    }

    #[test]
    fn legacy_and_diarized_models_use_singular_language_field() {
        assert_eq!(language_field("gpt-4o-transcribe"), "language");
        assert_eq!(language_field("gpt-4o-transcribe-diarize"), "language");
        assert_eq!(language_field("whisper-1"), "language");
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
        opts: &SttOptions,
    ) -> Result<Transcript, SttError> {
        let meta = fs::metadata(wav_path).await?;

        // Chunk-local speaker ids cannot be reconciled reliably across API
        // calls. Keep diarized OpenAI uploads bounded and steer long meetings
        // to Deepgram/ElevenLabs, which accept the whole file.
        if opts.diarize && meta.len() > chunk::TARGET_CHUNK_BYTES {
            return Err(SttError::FileTooLarge { bytes: meta.len(), max: chunk::TARGET_CHUNK_BYTES });
        }

        if meta.len() <= chunk::TARGET_CHUNK_BYTES {
            return self.transcribe_one(wav_path, opts).await;
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
            match self.transcribe_one(chunk_path, opts).await {
                Ok(t) => parts.push(t.text),
                Err(e) => {
                    chunk::cleanup_chunks(&chunks, wav_path);
                    return Err(e);
                }
            }
        }

        chunk::cleanup_chunks(&chunks, wav_path);
        Ok(Transcript::plain(
            parts.iter().map(|s| s.trim()).collect::<Vec<_>>().join(" "),
            opts.language.clone(),
            None,
        ))
    }
}
