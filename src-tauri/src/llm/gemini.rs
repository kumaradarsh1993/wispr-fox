//! Google Gemini chat-completions client.
//!
//! Different shape from Groq/OpenAI: uses `generateContent` endpoint, a
//! `contents` array of `parts`, and a separate `systemInstruction` field
//! rather than mixing system + user in one `messages` array.
//!
//! Default model: `gemini-2.5-flash` — the most generous free tier model
//! (15 RPM, 1,500 RPD as of May 2026). Pro models (gemini-2.5-pro,
//! gemini-3-pro, gemini-3.1-pro) were removed from the free tier in
//! April 2026 — they require billing enabled.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{LlmError, LlmProvider};

const ENDPOINT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const TIMEOUT: Duration = Duration::from_secs(10);

pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

pub struct GeminiLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GeminiLlm {
    pub fn new(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client construction is infallible with default config");
        Self { client, api_key, model }
    }
}

#[derive(Serialize)]
struct GenerateContentRequest<'a> {
    contents: Vec<Content<'a>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct Content<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'a str>,
    parts: Vec<Part<'a>>,
}

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<PromptFeedback>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Option<Vec<CandidatePart>>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct PromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
}

#[async_trait]
impl LlmProvider for GeminiLlm {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<String, LlmError> {
        let body = GenerateContentRequest {
            contents: vec![Content {
                role: Some("user"),
                parts: vec![Part { text: user }],
            }],
            system_instruction: if system.is_empty() {
                None
            } else {
                Some(Content {
                    role: None,
                    parts: vec![Part { text: system }],
                })
            },
            generation_config: GenerationConfig {
                temperature,
                max_output_tokens: 2048,
            },
        };

        let url = format!("{ENDPOINT_BASE}/{model}:generateContent", model = self.model);

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Http { status: status.as_u16(), body });
        }

        let parsed: GenerateContentResponse =
            resp.json().await.map_err(|e| LlmError::Decode(e.to_string()))?;

        // Safety blocks return a candidate with no content + a blockReason.
        if let Some(pf) = parsed.prompt_feedback {
            if let Some(reason) = pf.block_reason {
                return Err(LlmError::Decode(format!("gemini blocked: {reason}")));
            }
        }

        let text = parsed
            .candidates
            .and_then(|cs| cs.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|ps| ps.into_iter().next())
            .and_then(|p| p.text)
            .unwrap_or_default();

        if text.is_empty() {
            return Err(LlmError::Decode("empty response from gemini".into()));
        }
        Ok(text)
    }
}
