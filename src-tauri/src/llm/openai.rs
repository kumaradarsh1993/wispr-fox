//! OpenAI Responses API client for cleanup/drafting.

use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use super::{LlmError, LlmOutput, LlmProvider, TokenUsage};

const ENDPOINT: &str = "https://api.openai.com/v1/responses";
const TIMEOUT: Duration = Duration::from_secs(10);

pub const DEFAULT_MODEL: &str = "gpt-5.4-mini";

pub struct OpenAiLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiLlm {
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
struct ResponsesRequest<'a> {
    model: &'a str,
    instructions: &'a str,
    input: &'a str,
    temperature: f32,
    #[serde(rename = "max_output_tokens")]
    max_output_tokens: u32,
    reasoning: Reasoning<'a>,
}

#[derive(Serialize)]
struct Reasoning<'a> {
    effort: &'a str,
}

fn extract_output_text(v: &Value) -> String {
    if let Some(s) = v.get("output_text").and_then(Value::as_str) {
        if !s.trim().is_empty() {
            return s.to_owned();
        }
    }

    let mut out = String::new();
    if let Some(items) = v.get("output").and_then(Value::as_array) {
        for item in items {
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                continue;
            };
            for part in content {
                if part.get("type").and_then(Value::as_str) == Some("output_text") {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        out.push_str(text);
                    }
                }
            }
        }
    }
    out
}

fn extract_usage(v: &Value) -> Option<TokenUsage> {
    let usage = v.get("usage")?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens);
    Some(TokenUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

#[async_trait]
impl LlmProvider for OpenAiLlm {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
    ) -> Result<LlmOutput, LlmError> {
        let body = ResponsesRequest {
            model: &self.model,
            instructions: system,
            input: user,
            temperature,
            max_output_tokens: 2048,
            reasoning: Reasoning { effort: "low" },
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

        let parsed: Value = resp
            .json()
            .await
            .map_err(|e| LlmError::Decode(e.to_string()))?;

        let text = extract_output_text(&parsed);
        if text.trim().is_empty() {
            return Err(LlmError::Decode("empty response from OpenAI".into()));
        }
        let usage = extract_usage(&parsed);
        Ok(LlmOutput { text, usage })
    }
}
