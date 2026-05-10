//! LLM provider abstraction for Clippy cleanup.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod groq;
pub mod prompts;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClippyMode {
    Light,
    Advanced,
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
