//! LLM client with retry logic, jittered backoff, and connection pooling.
//!
//! Uses reqwest::blocking for synchronous HTTP calls with built-in
//! connection pooling and timeout handling.

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use rand::Rng;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::thread;
use std::time::Duration;

use crate::claude_api;
use crate::config::ApiFormat;

// Retry configuration for rate limiting and transient errors
const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 1000; // 1 second
const MAX_BACKOFF_MS: u64 = 60000; // 60 seconds
const JITTER_FACTOR: f64 = 0.3; // ±30% jitter

/// Check if an HTTP status code is retryable (429 rate limit or 5xx server error)
fn is_retryable_status(code: u16) -> bool {
    code == 429 || (500..600).contains(&code)
}

/// Calculate jittered backoff delay
fn jittered_backoff(base_ms: u64) -> u64 {
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0.0..JITTER_FACTOR) * base_ms as f64;
    let jittered = base_ms as f64 + jitter;
    (jittered as u64).min(MAX_BACKOFF_MS)
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

// ============ Streaming Types ============

/// Delta content in a streaming response
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChoiceDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Partial tool call in a streaming response
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallDelta {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub call_type: Option<String>,
    #[serde(default)]
    pub function: Option<FunctionCallDelta>,
}

/// Partial function call in a streaming response
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FunctionCallDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

/// A single chunk in a streaming response
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// A choice within a streaming chunk
#[derive(Debug, Clone, Deserialize)]
pub struct ChunkChoice {
    pub index: usize,
    #[serde(default)]
    pub delta: ChoiceDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// Events emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Content text delta
    ContentDelta(String),
    /// Start of a new tool call
    ToolCallStart {
        index: usize,
        id: String,
        name: String,
    },
    /// Argument delta for a tool call
    ToolCallDelta { index: usize, arguments: String },
    /// Stream completed with usage info
    Done { usage: Option<Usage> },
}

/// Result of an LLM call including timing and retry info
#[derive(Debug)]
pub struct LlmCallResult {
    pub response: ChatResponse,
    pub latency_ms: u64,
    pub retries: u32,
}

/// Trait for LLM clients to allow mocking and abstraction
pub trait LlmClient {
    /// Synchronous chat call (may internally use async)
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse>;

    /// Chat call that returns additional metadata (latency, retries)
    fn chat_with_metadata(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        let start = std::time::Instant::now();
        let response = self.chat(request)?;
        Ok(LlmCallResult {
            response,
            latency_ms: start.elapsed().as_millis() as u64,
            retries: 0,
        })
    }
}

pub struct Client {
    base_url: String,
    /// API key wrapped in SecretString for secure memory handling.
    /// Will be zeroized on drop and won't leak via Debug/Display.
    api_key: SecretString,
    http_client: reqwest::blocking::Client,
    api_format: ApiFormat,
}

impl Client {
    /// Create a new LLM client.
    /// The API key is stored as a SecretString for secure memory handling.
    pub fn new(base_url: &str, api_key: SecretString, api_format: ApiFormat) -> Self {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http_client,
            api_format,
        }
    }

    /// Internal sync implementation with retry logic
    fn chat_sync(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        let start = std::time::Instant::now();

        let mut attempt = 0;
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let mut total_retries = 0;

        loop {
            attempt += 1;

            let resp = match self.api_format {
                ApiFormat::Claude => {
                    let key = self.api_key.expose_secret();
                    let oauth = claude_api::is_oauth_token(key);
                    let url = claude_api::messages_url(&self.base_url, oauth);
                    let translate = if oauth {
                        claude_api::translate_request_oauth
                    } else {
                        claude_api::translate_request
                    };
                    let body = translate(
                        &request.model,
                        &request.messages,
                        request.tools.as_deref(),
                        request.tool_choice.as_deref(),
                        false,
                    );
                    let mut req_builder = self.http_client.post(&url);
                    for (name, value) in claude_api::build_headers(key) {
                        req_builder = req_builder.header(name, value);
                    }
                    req_builder.json(&body).send()
                }
                ApiFormat::OpenAI => {
                    let url = format!("{}/chat/completions", self.base_url);
                    self.http_client
                        .post(&url)
                        .header(
                            "Authorization",
                            format!("Bearer {}", self.api_key.expose_secret()),
                        )
                        .header("Content-Type", "application/json")
                        .json(request)
                        .send()
                }
            };

            match resp {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let body = match self.api_format {
                            ApiFormat::Claude => {
                                let claude_resp: claude_api::ClaudeResponse =
                                    response.json()?;
                                claude_api::translate_response(claude_resp)
                            }
                            ApiFormat::OpenAI => response.json::<ChatResponse>()?,
                        };
                        return Ok(LlmCallResult {
                            response: body,
                            latency_ms: start.elapsed().as_millis() as u64,
                            retries: total_retries,
                        });
                    }

                    let code = status.as_u16();
                    if is_retryable_status(code) {
                        if attempt >= MAX_RETRIES {
                            let body = response.text().unwrap_or_default();
                            return Err(anyhow!(
                                "API error {} after {} retries: {}",
                                code,
                                MAX_RETRIES,
                                body
                            ));
                        }

                        // Check for Retry-After header (common in 429 responses)
                        let retry_after = response
                            .headers()
                            .get("Retry-After")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .map(|s| s * 1000); // Convert seconds to ms

                        let wait_ms = retry_after.unwrap_or_else(|| jittered_backoff(backoff_ms));

                        eprintln!(
                            "[llm] {} error, retrying in {}ms (attempt {}/{})",
                            code, wait_ms, attempt, MAX_RETRIES
                        );

                        thread::sleep(Duration::from_millis(wait_ms));
                        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                        total_retries += 1;
                    } else {
                        // Non-retryable HTTP error (4xx except 429)
                        let body = response.text().unwrap_or_default();
                        return Err(anyhow!("API error {}: {}", code, body));
                    }
                }
                Err(e) => {
                    // Connection/network error - retryable
                    if attempt >= MAX_RETRIES {
                        return Err(anyhow!(
                            "Connection error after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    let wait_ms = jittered_backoff(backoff_ms);
                    eprintln!(
                        "[llm] Connection error, retrying in {}ms (attempt {}/{}): {}",
                        wait_ms, attempt, MAX_RETRIES, e
                    );

                    thread::sleep(Duration::from_millis(wait_ms));
                    backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                    total_retries += 1;
                }
            }
        }
    }
}

impl LlmClient for Client {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let result = self.chat_sync(request)?;
        Ok(result.response)
    }

    fn chat_with_metadata(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        self.chat_sync(request)
    }
}

// ============ Streaming Client ============

/// Request for streaming chat completions
#[derive(Debug, Serialize)]
pub struct StreamingChatRequest {
    pub model: String,
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

/// Async streaming LLM client
pub struct StreamingClient {
    base_url: String,
    api_key: SecretString,
    http_client: reqwest::Client,
    api_format: ApiFormat,
}

impl StreamingClient {
    pub fn new(base_url: &str, api_key: SecretString, api_format: ApiFormat) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // Longer timeout for streaming
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create async HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http_client,
            api_format,
        }
    }

    /// Stream chat completions, sending events to the provided channel.
    /// Accumulates the full response and returns it when complete.
    pub async fn chat_stream(
        &self,
        request: &ChatRequest,
        event_tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<ChatResponse> {
        match self.api_format {
            ApiFormat::Claude => self.chat_stream_claude(request, event_tx).await,
            ApiFormat::OpenAI => self.chat_stream_openai(request, event_tx).await,
        }
    }

    /// OpenAI-compatible streaming implementation.
    async fn chat_stream_openai(
        &self,
        request: &ChatRequest,
        event_tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<ChatResponse> {
        use futures::StreamExt;

        let url = format!("{}/chat/completions", self.base_url);

        let streaming_request = StreamingChatRequest {
            model: request.model.clone(),
            messages: request.messages.clone(),
            tools: request.tools.clone(),
            tool_choice: request.tool_choice.clone(),
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        };

        let response = self
            .http_client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key.expose_secret()),
            )
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&streaming_request)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, body));
        }

        // Accumulate full response
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut final_usage: Option<Usage> = None;
        let mut finish_reason: Option<String> = None;

        // Parse SSE stream using extension trait
        use eventsource_stream::Eventsource;
        let byte_stream = response.bytes_stream();
        let mut event_stream = byte_stream.eventsource();

        while let Some(event_result) = event_stream.next().await {
            let event = match event_result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("[llm] SSE parse error: {}", e);
                    continue;
                }
            };

            let data = event.data.trim();

            // Check for [DONE] sentinel
            if data == "[DONE]" {
                break;
            }

            // Parse JSON chunk
            let chunk: ChatCompletionChunk = match serde_json::from_str(data) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[llm] JSON parse error: {} in: {}", e, data);
                    continue;
                }
            };

            // Process each choice (usually just one)
            for choice in &chunk.choices {
                // Handle content delta
                if let Some(delta_content) = &choice.delta.content {
                    if !delta_content.is_empty() {
                        content.push_str(delta_content);
                        let _ = event_tx
                            .send(StreamEvent::ContentDelta(delta_content.clone()))
                            .await;
                    }
                }

                // Handle tool call deltas
                if let Some(tc_deltas) = &choice.delta.tool_calls {
                    for tc_delta in tc_deltas {
                        let idx = tc_delta.index;

                        // Ensure we have a slot for this tool call
                        while tool_calls.len() <= idx {
                            tool_calls.push(ToolCall {
                                id: String::new(),
                                call_type: "function".to_string(),
                                function: FunctionCall {
                                    name: String::new(),
                                    arguments: String::new(),
                                },
                            });
                        }

                        // Update tool call ID if provided
                        if let Some(id) = &tc_delta.id {
                            tool_calls[idx].id = id.clone();
                        }

                        // Update function details if provided
                        if let Some(func_delta) = &tc_delta.function {
                            if let Some(name) = &func_delta.name {
                                tool_calls[idx].function.name = name.clone();
                                // Emit tool call start event
                                let _ = event_tx
                                    .send(StreamEvent::ToolCallStart {
                                        index: idx,
                                        id: tool_calls[idx].id.clone(),
                                        name: name.clone(),
                                    })
                                    .await;
                            }
                            if let Some(args) = &func_delta.arguments {
                                tool_calls[idx].function.arguments.push_str(args);
                                let _ = event_tx
                                    .send(StreamEvent::ToolCallDelta {
                                        index: idx,
                                        arguments: args.clone(),
                                    })
                                    .await;
                            }
                        }
                    }
                }

                // Track finish reason
                if choice.finish_reason.is_some() {
                    finish_reason = choice.finish_reason.clone();
                }
            }

            // Track usage from the final chunk
            if chunk.usage.is_some() {
                final_usage = chunk.usage;
            }
        }

        // Send done event
        let _ = event_tx
            .send(StreamEvent::Done {
                usage: final_usage.clone(),
            })
            .await;

        // Build accumulated response
        let message = Message {
            role: "assistant".to_string(),
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        };

        Ok(ChatResponse {
            choices: vec![Choice {
                message,
                finish_reason,
            }],
            usage: final_usage,
        })
    }

    /// Anthropic Messages API streaming implementation.
    async fn chat_stream_claude(
        &self,
        request: &ChatRequest,
        event_tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<ChatResponse> {
        use futures::StreamExt;

        let key = self.api_key.expose_secret();
        let oauth = claude_api::is_oauth_token(key);
        let url = claude_api::messages_url(&self.base_url, oauth);
        let translate = if oauth {
            claude_api::translate_request_oauth
        } else {
            claude_api::translate_request
        };
        let body = translate(
            &request.model,
            &request.messages,
            request.tools.as_deref(),
            request.tool_choice.as_deref(),
            true,
        );

        let mut req_builder = self
            .http_client
            .post(&url)
            .header("Accept", "text/event-stream");

        for (name, value) in claude_api::build_headers(key) {
            req_builder = req_builder.header(name, value);
        }

        let response = req_builder
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, body));
        }

        let mut accumulator = claude_api::StreamAccumulator::new();
        let mut stream_state = claude_api::StreamState::default();

        // Parse SSE stream
        use eventsource_stream::Eventsource;
        let byte_stream = response.bytes_stream();
        let mut event_stream = byte_stream.eventsource();

        while let Some(event_result) = event_stream.next().await {
            let event = match event_result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("[llm] SSE parse error: {}", e);
                    continue;
                }
            };

            let data = event.data.trim();
            if data.is_empty() {
                continue;
            }

            // Parse Anthropic event
            let claude_event: claude_api::ClaudeStreamEvent =
                match serde_json::from_str(data) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("[llm] Claude JSON parse error: {} in: {}", e, data);
                        continue;
                    }
                };

            // Update accumulator
            accumulator.process_event(&claude_event);

            // Translate to internal stream events and forward
            let internal_events =
                claude_api::translate_stream_event(&claude_event, &mut stream_state);
            for evt in internal_events {
                let _ = event_tx.send(evt).await;
            }
        }

        Ok(accumulator.into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jittered_backoff() {
        // Run multiple times to verify jitter is applied
        let base = 1000u64;
        for _ in 0..10 {
            let result = jittered_backoff(base);
            assert!(result >= base);
            assert!(result <= base + (base as f64 * JITTER_FACTOR) as u64);
        }
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
    }
}
