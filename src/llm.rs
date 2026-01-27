use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Retry configuration for rate limiting and transient errors
const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 1000; // 1 second
const MAX_BACKOFF_MS: u64 = 60000; // 60 seconds

/// Check if an HTTP status code is retryable (429 rate limit or 5xx server error)
fn is_retryable_status(code: u16) -> bool {
    code == 429 || (500..600).contains(&code)
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

/// Token usage statistics from the API response
#[derive(Debug, Deserialize, Default, Clone)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Trait for LLM clients to allow mocking and abstraction
pub trait LlmClient {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse>;
}

pub struct Client {
    base_url: String,
    api_key: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            agent: ureq::Agent::new(),
        }
    }
}

impl LlmClient for Client {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut attempt = 0;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        loop {
            attempt += 1;

            let resp = self
                .agent
                .post(&url)
                .set("Authorization", &format!("Bearer {}", self.api_key))
                .set("Content-Type", "application/json")
                .send_json(serde_json::to_value(request)?);

            match resp {
                Ok(r) => {
                    let body: ChatResponse = r.into_json()?;
                    return Ok(body);
                }
                Err(ureq::Error::Status(code, resp)) if is_retryable_status(code) => {
                    if attempt >= MAX_RETRIES {
                        let body = resp.into_string().unwrap_or_default();
                        return Err(anyhow!(
                            "API error {} after {} retries: {}",
                            code,
                            MAX_RETRIES,
                            body
                        ));
                    }

                    // Check for Retry-After header (common in 429 responses)
                    let retry_after = resp
                        .header("Retry-After")
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(|s| s * 1000); // Convert seconds to ms

                    let wait_ms = retry_after.unwrap_or(backoff_ms).min(MAX_BACKOFF_MS);

                    eprintln!(
                        "[llm] {} error, retrying in {}ms (attempt {}/{})",
                        code, wait_ms, attempt, MAX_RETRIES
                    );

                    std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                    backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                }
                Err(ureq::Error::Status(code, resp)) => {
                    // Non-retryable HTTP error (4xx except 429)
                    let body = resp.into_string().unwrap_or_default();
                    return Err(anyhow!("API error {}: {}", code, body));
                }
                Err(ureq::Error::Transport(e)) => {
                    // Connection/network error - retryable
                    if attempt >= MAX_RETRIES {
                        return Err(anyhow!(
                            "Connection error after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    eprintln!(
                        "[llm] Connection error, retrying in {}ms (attempt {}/{}): {}",
                        backoff_ms, attempt, MAX_RETRIES, e
                    );

                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                }
            }
        }
    }
}
