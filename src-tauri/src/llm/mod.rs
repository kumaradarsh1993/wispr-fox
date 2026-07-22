//! LLM provider abstraction for Clippy cleanup.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod gemini;
pub mod groq;
pub mod openai;
pub mod prompts;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClippyMode {
    Light,
    Advanced,
    /// "Drafting" — user gives a brief, LLM drafts a polished output (longer,
    /// more transformative than Advanced).
    Drafting,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("missing API key for provider {0}")]
    MissingKey(&'static str),
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("decode error: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOutput {
    pub text: String,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// How long the live dictation path should wait on this provider before
    /// giving up and pasting the raw transcript.
    ///
    /// This exists because a single shared deadline is wrong: Groq's Llama
    /// answers in 1-3s, while Gemini's thinking phase alone can outlast the
    /// old flat 8s wall — which is exactly why every Gemini cleanup used to
    /// come back as `clippy_timeout`. Providers that need more room say so.
    fn timeout_hint(&self) -> std::time::Duration {
        std::time::Duration::from_secs(8)
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<LlmOutput, LlmError>;
}
