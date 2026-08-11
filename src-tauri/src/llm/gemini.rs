//! Google Gemini chat-completions client.
//!
//! Different shape from Groq/OpenAI: uses `generateContent` endpoint, a
//! `contents` array of `parts`, and a separate `systemInstruction` field
//! rather than mixing system + user in one `messages` array.
//!
//! Default model: `gemini-3.6-flash`. Model ids here were re-verified against
//! <https://ai.google.dev/gemini-api/docs/models> on 2026-08-11.
//!
//! ## Why every Gemini call used to time out (fixed 2026-07-22)
//!
//! Gemini 2.5 and the whole Gemini 3 line have **thinking enabled by
//! default**. The model spends a long, silent thinking phase before emitting
//! its first output token, and those thinking tokens are billed against
//! `maxOutputTokens`. Two consequences bit us at once:
//!
//! 1. **Latency.** `clippy::clean` wraps every provider call in an outer
//!    timeout. That wall used to be a flat 8s for all providers, so a
//!    thinking model was killed mid-thought and the user got
//!    `clippy_timeout` and their raw transcript back — on *every* Gemini
//!    model, while Groq's Llama (no thinking phase, 1-3s) worked fine.
//!    That's why the bug looked like "Gemini is broken".
//! 2. **Starved output.** With a 2048-token cap, thinking could eat the
//!    whole budget and return `finishReason: MAX_TOKENS` with zero text
//!    parts, which surfaced as the equally unhelpful "empty response".
//!
//! The fix is to ask for the *least* thinking each model family allows —
//! cleanup and drafting are style transforms, they gain nothing from a
//! reasoning budget — plus real headroom on tokens and time. See
//! `minimal_thinking_for`.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{LlmError, LlmOutput, LlmProvider, TokenUsage};

const ENDPOINT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
// Transport-level ceiling. The *effective* deadline is the outer one in
// `clippy::clean` (see `timeout_hint` below), which is always shorter — this
// only exists so a genuinely wedged connection can't hang a task forever.
const TIMEOUT: Duration = Duration::from_secs(90);
// Thinking tokens count against `maxOutputTokens`, so the old 2048 was a
// budget shared between reasoning and the actual answer. Even at minimal
// thinking, leave room for a long draft plus whatever the model thinks first.
const MAX_OUTPUT_TOKENS: u32 = 8192;

pub const DEFAULT_MODEL: &str = "gemini-3.6-flash";

/// Model ids that Google deprecated since this codebase last shipped.
/// When the user's saved settings hold one of these, fall through to
/// `DEFAULT_MODEL` so transcription doesn't fail with a 404 on a tombstoned
/// endpoint. Re-checked against Google's model list 2026-08-11.
pub const DEPRECATED_MODELS: &[&str] = &[
    "gemini-2.0-flash", // "shut down soon" per Google's model list
    "gemini-2.0-flash-001",
    "gemini-2.0-flash-lite",
    "gemini-2.0-flash-lite-001",
    "gemini-3-pro-preview",        // superseded by gemini-3.1-pro-preview
    "gemini-3.1-flash-lite-preview", // graduated to gemini-3.1-flash-lite
    "gemini-3-flash", // stale/speculative UI id; use preview/default instead
    "gemini-3-pro",   // speculative id that never shipped
    "gemini-3.1-pro", // stale shorthand; current preview id has "-preview"
];

/// The smallest thinking allowance each model family accepts.
///
/// Google splits the knob across two mutually-exclusive fields — sending both
/// is a 400 — and the accepted values differ by family:
///
/// - **Gemini 3.x** uses `thinkingLevel`, and its flash models accept
///   `MINIMAL`. The Pro models only go down to `LOW`.
/// - **Gemini 2.5** uses `thinkingBudget`, where `0` disables thinking on the
///   flash models. 2.5 Pro cannot disable it, so we leave it alone rather
///   than send a value it will reject.
///
/// Returns `None` for anything unrecognised — an unknown field on an
/// unexpected model is a hard 400, and no thinking config at all still works
/// (just slower). `complete` also retries without this block if the API
/// rejects it, so a future model rename degrades to "slow" and never "broken".
fn minimal_thinking_for(model: &str) -> Option<ThinkingConfig> {
    if model.starts_with("gemini-3") {
        // Pro reasons no matter what; asking for MINIMAL there is rejected.
        let level = if model.contains("-pro") { "LOW" } else { "MINIMAL" };
        Some(ThinkingConfig {
            thinking_level: Some(level),
            thinking_budget: None,
        })
    } else if model.starts_with("gemini-2.5") && !model.contains("-pro") {
        Some(ThinkingConfig {
            thinking_level: None,
            thinking_budget: Some(0),
        })
    } else {
        None
    }
}

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
        Self {
            client,
            api_key,
            model,
        }
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
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<ThinkingConfig>,
}

/// `thinkingLevel` and `thinkingBudget` are mutually exclusive — sending both
/// is a 400, hence both are optional and exactly one is ever populated.
#[derive(Serialize, Clone, Copy)]
struct ThinkingConfig {
    #[serde(rename = "thinkingLevel", skip_serializing_if = "Option::is_none")]
    thinking_level: Option<&'static str>,
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<u32>,
}

#[derive(Deserialize)]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<PromptFeedback>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u64>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u64>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<u64>,
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
    /// Set on reasoning parts. They are only returned when thought summaries
    /// are requested (we never ask), but filtering them costs nothing and
    /// stops a thought leaking into the user's pasted text if that default
    /// ever changes.
    #[serde(default)]
    thought: Option<bool>,
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

    fn timeout_hint(&self) -> Duration {
        // Even at minimal thinking, Gemini has a longer floor than Groq's
        // Llama. The old shared 8s deadline is what made every Gemini call
        // fail; see the module docs.
        Duration::from_secs(25)
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<LlmOutput, LlmError> {
        let url = format!(
            "{ENDPOINT_BASE}/{model}:generateContent",
            model = self.model
        );

        let mut thinking = minimal_thinking_for(&self.model);
        let parsed: GenerateContentResponse = loop {
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
                    max_output_tokens: MAX_OUTPUT_TOKENS,
                    thinking_config: thinking,
                },
            };

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
                let err_body = resp.text().await.unwrap_or_default();
                // A 400 while we're sending a thinking block almost certainly
                // means this model doesn't accept the field or value we
                // guessed. Drop it and try once more: a slow answer beats no
                // answer, and it keeps a future model rename from bricking
                // Gemini entirely.
                if status.as_u16() == 400 && thinking.is_some() {
                    tracing::warn!(
                        "gemini: {model} rejected the thinking config, retrying without it: {err}",
                        model = self.model,
                        err = err_body.chars().take(300).collect::<String>()
                    );
                    thinking = None;
                    continue;
                }
                return Err(LlmError::Http {
                    status: status.as_u16(),
                    body: err_body,
                });
            }

            break resp
                .json()
                .await
                .map_err(|e| LlmError::Decode(e.to_string()))?;
        };

        // Safety blocks return a candidate with no content + a blockReason.
        if let Some(pf) = parsed.prompt_feedback {
            if let Some(reason) = pf.block_reason {
                return Err(LlmError::Decode(format!("gemini blocked: {reason}")));
            }
        }

        let usage = parsed.usage_metadata.map(|u| {
            let input_tokens = u.prompt_token_count.unwrap_or(0);
            let output_tokens = u.candidates_token_count.unwrap_or(0);
            let total_tokens = u.total_token_count.unwrap_or(input_tokens + output_tokens);
            TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            }
        });

        let candidate = parsed.candidates.and_then(|cs| cs.into_iter().next());
        let finish_reason = candidate.as_ref().and_then(|c| c.finish_reason.clone());

        // Join every non-thought part. A long answer arrives split across
        // several parts, so taking only the first (what we used to do) could
        // silently truncate the user's draft mid-sentence.
        let text = candidate
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .map(|ps| {
                ps.into_iter()
                    .filter(|p| p.thought != Some(true))
                    .filter_map(|p| p.text)
                    .collect::<String>()
            })
            .unwrap_or_default();

        if text.trim().is_empty() {
            // Name the actual failure. `MAX_TOKENS` with no text means
            // thinking consumed the whole budget — worth saying out loud,
            // because the generic "empty response" sent us hunting the wrong
            // bug for a while.
            return Err(LlmError::Decode(match finish_reason.as_deref() {
                Some("MAX_TOKENS") => format!(
                    "gemini {model} produced no text — the token budget went to \
                     thinking. Try a flash model or raise MAX_OUTPUT_TOKENS.",
                    model = self.model
                ),
                Some(reason) => {
                    format!("gemini {model} returned no text (finishReason: {reason})", model = self.model)
                }
                None => format!("empty response from gemini {model}", model = self.model),
            }));
        }
        Ok(LlmOutput { text, usage })
    }
}
