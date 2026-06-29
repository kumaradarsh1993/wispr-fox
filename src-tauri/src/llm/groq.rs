//! Groq chat-completions client. Default models: 8B for Light (fast), 70B for Advanced.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{LlmError, LlmOutput, LlmProvider, TokenUsage};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const TIMEOUT: Duration = Duration::from_secs(8);

pub const DEFAULT_LIGHT_MODEL: &str = "llama-3.3-70b-versatile";
pub const DEFAULT_ADVANCED_MODEL: &str = "llama-3.3-70b-versatile";

pub struct GroqLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GroqLlm {
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
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
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
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
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

        let text = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

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
