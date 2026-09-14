//! Groq chat-completions client. Defaults: GPT-OSS 20B for cleanup and 120B for drafts.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{LlmError, LlmOutput, LlmProvider, TokenUsage};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
// Groq is fast, but a long Draft rewrite plus tail latency can still exceed a
// tight 8s and trip a spurious `clippy_timeout`. 25s covers the long case; the
// fire-and-forget auto-title call shares this client and is unaffected.
const TIMEOUT: Duration = Duration::from_secs(25);

pub const DEFAULT_LIGHT_MODEL: &str = "openai/gpt-oss-20b";
pub const DEFAULT_ADVANCED_MODEL: &str = "openai/gpt-oss-120b";

/// Process-wide pooled client — shared across the cleanup/draft call and the
/// per-recording auto-title call so both reuse warm keep-alive connections to
/// api.groq.com instead of each `GroqLlm::new` opening a fresh DNS+TLS handshake.
/// Cheap to clone (Arc), pool shared across clones; key/model are per-request.
static SHARED_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(TIMEOUT)
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client construction is infallible with default config")
        })
        .clone()
}

pub struct GroqLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GroqLlm {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: shared_client(),
            api_key,
            model,
        }
    }
}

/// Groq's default `max_completion_tokens` is 1024 — and on its reasoning
/// models the *thinking* counts against that budget. A long dictation could
/// spend the whole cap reasoning and return `content: null`, which the old
/// `content: String` parser rejected as a decode error → "Cleanup failed".
/// 16k comfortably covers a full Draft rewrite of a 20-minute recording.
const MAX_COMPLETION_TOKENS: u32 = 16_384;

/// Every Groq model we offer is a reasoning model today (GPT-OSS 20B/120B,
/// Qwen 3.x). GPT-OSS ignores `reasoning_format` and only takes
/// `reasoning_effort: low|medium|high`; Qwen takes `reasoning_format` and
/// `reasoning_effort: none|default` (3.8 also low/medium/high). Groq returns
/// 400 for a parameter a model does not support, so these are per-family.
fn is_gpt_oss(model: &str) -> bool {
    model.starts_with("openai/gpt-oss")
}

fn is_qwen(model: &str) -> bool {
    model.starts_with("qwen/")
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_completion_tokens: u32,
    /// Qwen only. `hidden` = final answer only. Without this Qwen defaults to
    /// `raw` and dumps its chain of thought INSIDE `content` as `<think>…</think>`
    /// — which then fails the Light-mode length-drift guard and gets pasted raw.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_format: Option<&'static str>,
    /// GPT-OSS: `low` — cleanup is a rewrite, not a proof; low effort is
    /// faster and leaves the token budget for the answer. Qwen: `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
    /// GPT-OSS only (mutually exclusive with `reasoning_format`). Drops the
    /// `reasoning` field from the reply so we don't pay to download it.
    #[serde(skip_serializing_if = "Option::is_none")]
    include_reasoning: Option<bool>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    /// `null` when a reasoning model exhausts its budget before answering,
    /// or when a model returns only tool calls. Must not be a hard `String`.
    content: Option<String>,
}

/// Strip a leading `<think>…</think>` block if a model returned one anyway
/// (older Qwen builds, or a future model that ignores `reasoning_format`).
/// An unterminated `<think>` means the whole reply is thinking — return empty
/// so the caller reports a failure instead of pasting a monologue.
fn strip_think_block(s: &str) -> String {
    let t = s.trim_start();
    if !t.starts_with("<think>") {
        return s.to_owned();
    }
    match t.find("</think>") {
        Some(end) => t[end + "</think>".len()..].to_owned(),
        None => String::new(),
    }
}

#[async_trait]
impl LlmProvider for GroqLlm {
    fn name(&self) -> &'static str {
        "groq"
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<LlmOutput, LlmError> {
        let (reasoning_format, reasoning_effort, include_reasoning) =
            if is_gpt_oss(&self.model) {
                (None, Some("low"), Some(false))
            } else if is_qwen(&self.model) {
                (Some("hidden"), Some("none"), None)
            } else {
                (None, None, None)
            };
        let body = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system,
                },
                ChatMessage {
                    role: "user",
                    content: user,
                },
            ],
            temperature,
            max_completion_tokens: MAX_COMPLETION_TOKENS,
            reasoning_format,
            reasoning_effort,
            include_reasoning,
        };

        let resp = self
            .client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Http {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Decode(e.to_string()))?;

        let first = parsed.choices.into_iter().next();
        let finish = first
            .as_ref()
            .and_then(|c| c.finish_reason.clone())
            .unwrap_or_default();
        let text = first
            .and_then(|c| c.message.content)
            .map(|c| strip_think_block(&c))
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(LlmError::Decode(format!(
                "Groq returned no answer text (finish_reason={finish:?}, model={})",
                self.model
            )));
        }

        let usage = parsed.usage.map(|u| {
            let input_tokens = u.prompt_tokens.unwrap_or(0);
            let output_tokens = u.completion_tokens.unwrap_or(0);
            let total_tokens = u.total_tokens.unwrap_or(input_tokens + output_tokens);
            TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens,
            }
        });

        Ok(LlmOutput { text, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_content_parses_instead_of_failing_decode() {
        // What Groq returns when a reasoning model spends the whole budget
        // thinking. Before the fix this was a serde error → "Cleanup failed".
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":null,"reasoning":"..."},"finish_reason":"length"}],"usage":{"prompt_tokens":10,"completion_tokens":1024,"total_tokens":1034}}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).expect("must parse");
        assert!(parsed.choices[0].message.content.is_none());
        assert_eq!(parsed.choices[0].finish_reason.as_deref(), Some("length"));
    }

    #[test]
    fn think_block_is_stripped() {
        assert_eq!(
            strip_think_block("<think>hmm, punctuation</think>Hello, world."),
            "Hello, world."
        );
        assert_eq!(strip_think_block("  <think>x</think>\nBody"), "\nBody");
        assert_eq!(strip_think_block("Plain reply"), "Plain reply");
        // Unterminated = the whole reply was thinking; must not be pasted.
        assert_eq!(strip_think_block("<think>still going"), "");
    }

    #[test]
    fn reasoning_params_are_per_model_family() {
        assert!(is_gpt_oss("openai/gpt-oss-20b"));
        assert!(is_gpt_oss("openai/gpt-oss-120b"));
        assert!(!is_gpt_oss("qwen/qwen3.6-27b"));
        assert!(is_qwen("qwen/qwen3.6-27b"));
        assert!(is_qwen("qwen/qwen3.8-27b"));
        assert!(!is_qwen("openai/gpt-oss-20b"));
    }

    #[test]
    fn request_omits_unsupported_params() {
        // GPT-OSS rejects reasoning_format; Qwen must not get include_reasoning
        // alongside reasoning_format (Groq: mutually exclusive → 400).
        let body = ChatRequest {
            model: "openai/gpt-oss-20b",
            messages: vec![],
            temperature: 0.3,
            max_completion_tokens: MAX_COMPLETION_TOKENS,
            reasoning_format: None,
            reasoning_effort: Some("low"),
            include_reasoning: Some(false),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(!json.contains("reasoning_format"));
        assert!(json.contains("\"reasoning_effort\":\"low\""));
        assert!(json.contains("\"include_reasoning\":false"));
        assert!(json.contains("\"max_completion_tokens\":16384"));
    }
}
