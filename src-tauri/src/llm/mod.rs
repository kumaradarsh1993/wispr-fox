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

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<String, LlmError>;
}
