//! Translation layer between internal OpenAI-compatible format and
//! the Anthropic Messages API.
//!
//! All translation happens at the HTTP boundary — internal types
//! (ChatRequest, ChatResponse, StreamEvent) remain unchanged.

use crate::llm::{
    ChatResponse, Choice, FunctionCall, Message, StreamEvent, ToolCall, Usage,
};
use serde::Deserialize;
use serde_json::Value;

// ============ Request Translation ============

/// Build the Anthropic Messages API URL from a base URL.
/// Anthropic uses `/v1/messages` instead of `/v1/chat/completions`.
/// For OAuth tokens, appends `?beta=true` query parameter.
pub fn messages_url(base_url: &str, is_oauth: bool) -> String {
    let base = base_url.trim_end_matches('/');
    let url = format!("{}/messages", base);
    if is_oauth {
        format!("{}?beta=true", url)
    } else {
        url
    }
}

/// Check if an API key is an OAuth token (vs a regular API key).
pub fn is_oauth_token(api_key: &str) -> bool {
    api_key.starts_with("sk-ant-oat")
}

/// Build Anthropic-specific headers.
/// Returns a list of (header_name, header_value) pairs.
///
/// OAuth tokens (sk-ant-oat01-...) use `Authorization: Bearer` and require
/// beta headers + a Claude CLI user-agent. Regular API keys use `x-api-key`.
pub fn build_headers(api_key: &str) -> Vec<(&'static str, String)> {
    let oauth = is_oauth_token(api_key);
    let mut headers = Vec::new();

    if oauth {
        headers.push(("Authorization", format!("Bearer {}", api_key)));
        headers.push((
            "anthropic-beta",
            "oauth-2025-04-20,interleaved-thinking-2025-05-14,claude-code-20250219,fine-grained-tool-streaming-2025-05-14".to_string(),
        ));
        headers.push((
            "User-Agent",
            "claude-cli/2.1.2 (external, cli)".to_string(),
        ));
    } else {
        headers.push(("x-api-key", api_key.to_string()));
    }

    headers.push(("anthropic-version", "2023-06-01".to_string()));
    headers.push(("Content-Type", "application/json".to_string()));
    headers
}

/// OAuth tool name prefix required by the Claude Code API.
const OAUTH_TOOL_PREFIX: &str = "mcp_";

/// Required system prompt prefix for OAuth authentication.
const CLAUDE_CODE_SYSTEM_PREFIX: &str =
    "You are Claude Code, Anthropic's official CLI for Claude.";

/// Translate an internal ChatRequest into an Anthropic Messages API request body.
pub fn translate_request(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    tool_choice: Option<&str>,
    stream: bool,
) -> Value {
    translate_request_inner(model, messages, tools, tool_choice, stream, false)
}

/// Translate request with OAuth tool-name prefixing when needed.
pub fn translate_request_oauth(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    tool_choice: Option<&str>,
    stream: bool,
) -> Value {
    translate_request_inner(model, messages, tools, tool_choice, stream, true)
}

fn translate_request_inner(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    tool_choice: Option<&str>,
    stream: bool,
    prefix_tools: bool,
) -> Value {
    let mut system_parts: Vec<String> = Vec::new();
    let mut translated_messages: Vec<Value> = Vec::new();

    // First pass: extract system messages and translate others
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        match role {
            "system" => {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    system_parts.push(content.to_string());
                }
            }
            "assistant" => {
                translated_messages.push(translate_assistant_message(msg, prefix_tools));
            }
            "tool" => {
                translated_messages.push(translate_tool_message(msg));
            }
            "user" => {
                translated_messages.push(msg.clone());
            }
            _ => {
                // Unknown role, pass through as user
                let mut m = msg.clone();
                if let Some(obj) = m.as_object_mut() {
                    obj.insert("role".to_string(), Value::String("user".to_string()));
                }
                translated_messages.push(m);
            }
        }
    }

    // Merge consecutive same-role messages (Anthropic disallows them)
    translated_messages = merge_consecutive_roles(translated_messages);

    // Build the request body
    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "messages": translated_messages,
    });

    // Build system prompt
    if prefix_tools {
        // OAuth mode: prepend required Claude Code system prefix
        system_parts.insert(0, CLAUDE_CODE_SYSTEM_PREFIX.to_string());
    }
    if !system_parts.is_empty() {
        // Send as array of text blocks (matches Anthropic SDK format)
        let system_blocks: Vec<Value> = system_parts
            .iter()
            .map(|text| serde_json::json!({"type": "text", "text": text}))
            .collect();
        body["system"] = Value::Array(system_blocks);
    }

    // Translate tools
    if let Some(tools) = tools {
        let claude_tools: Vec<Value> = tools
            .iter()
            .filter_map(|t| translate_tool_schema(t, prefix_tools))
            .collect();
        if !claude_tools.is_empty() {
            body["tools"] = Value::Array(claude_tools);
        }
    }

    // Translate tool_choice
    if let Some(tc) = tool_choice {
        body["tool_choice"] = match tc {
            "auto" => serde_json::json!({"type": "auto"}),
            "none" => serde_json::json!({"type": "none"}),
            "required" => serde_json::json!({"type": "any"}),
            _ => serde_json::json!({"type": "auto"}),
        };
    }

    if stream {
        body["stream"] = Value::Bool(true);
    }

    body
}

/// Translate an assistant message, converting tool_calls to tool_use content blocks.
fn translate_assistant_message(msg: &Value, prefix_tools: bool) -> Value {
    let mut content_blocks: Vec<Value> = Vec::new();

    // Add text content if present
    if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
        if !text.is_empty() {
            content_blocks.push(serde_json::json!({
                "type": "text",
                "text": text
            }));
        }
    }

    // Convert tool_calls to tool_use blocks
    if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
        for tc in tool_calls {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let func = tc.get("function").unwrap_or(&Value::Null);
            let raw_name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let name = if prefix_tools {
                format!("{}{}", OAUTH_TOOL_PREFIX, raw_name)
            } else {
                raw_name.to_string()
            };
            let args_str = func
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(args_str).unwrap_or(Value::Object(
                serde_json::Map::new(),
            ));

            content_blocks.push(serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));
        }
    }

    if content_blocks.is_empty() {
        // Empty assistant message — Anthropic requires content
        content_blocks.push(serde_json::json!({
            "type": "text",
            "text": ""
        }));
    }

    serde_json::json!({
        "role": "assistant",
        "content": content_blocks
    })
}

/// Translate a tool-role message to a user message with tool_result content block.
fn translate_tool_message(msg: &Value) -> Value {
    let tool_call_id = msg
        .get("tool_call_id")
        .and_then(|i| i.as_str())
        .unwrap_or("");
    let content = msg
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    serde_json::json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": content
        }]
    })
}

/// Translate an OpenAI-format tool schema to Anthropic format.
/// OpenAI: `{type: "function", function: {name, description, parameters}}`
/// Anthropic: `{name, description, input_schema}`
fn translate_tool_schema(tool: &Value, prefix_tools: bool) -> Option<Value> {
    let func = tool.get("function")?;
    let raw_name = func.get("name")?.as_str()?;
    let name = if prefix_tools {
        format!("{}{}", OAUTH_TOOL_PREFIX, raw_name)
    } else {
        raw_name.to_string()
    };

    let mut claude_tool = serde_json::json!({
        "name": name,
    });

    if let Some(desc) = func.get("description") {
        claude_tool["description"] = desc.clone();
    }

    if let Some(params) = func.get("parameters") {
        claude_tool["input_schema"] = params.clone();
    } else {
        // Anthropic requires input_schema
        claude_tool["input_schema"] = serde_json::json!({
            "type": "object",
            "properties": {}
        });
    }

    Some(claude_tool)
}

/// Merge consecutive messages with the same role by combining their content.
/// Anthropic requires alternating user/assistant roles.
fn merge_consecutive_roles(messages: Vec<Value>) -> Vec<Value> {
    if messages.is_empty() {
        return messages;
    }

    let mut merged: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();

        let should_merge = merged.last().map_or(false, |last: &Value| {
            last.get("role")
                .and_then(|r| r.as_str())
                .map_or(false, |r| r == role)
        });

        if should_merge {
            let last = merged.last_mut().unwrap();
            // Merge content
            let existing_content = last.get("content").cloned().unwrap_or(Value::Null);
            let new_content = msg.get("content").cloned().unwrap_or(Value::Null);

            let merged_content = merge_content(existing_content, new_content);
            last.as_object_mut()
                .unwrap()
                .insert("content".to_string(), merged_content);
        } else {
            merged.push(msg);
        }
    }

    merged
}

/// Merge two content values (could be strings or arrays of content blocks).
fn merge_content(existing: Value, new: Value) -> Value {
    let mut blocks = content_to_blocks(existing);
    blocks.extend(content_to_blocks(new));
    Value::Array(blocks)
}

/// Convert content (string or array) to a vec of content blocks.
fn content_to_blocks(content: Value) -> Vec<Value> {
    match content {
        Value::String(s) => {
            if s.is_empty() {
                vec![]
            } else {
                vec![serde_json::json!({"type": "text", "text": s})]
            }
        }
        Value::Array(arr) => arr,
        Value::Null => vec![],
        other => vec![serde_json::json!({"type": "text", "text": other.to_string()})],
    }
}

// ============ Response Translation ============

/// Anthropic Messages API response structure.
#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize)]
pub struct ClaudeUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// Strip the OAuth tool name prefix if present.
fn strip_tool_prefix(name: &str) -> String {
    name.strip_prefix(OAUTH_TOOL_PREFIX)
        .unwrap_or(name)
        .to_string()
}

/// Translate an Anthropic Messages API response to internal ChatResponse format.
pub fn translate_response(claude_resp: ClaudeResponse) -> ChatResponse {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in &claude_resp.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id: id.clone(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: strip_tool_prefix(name),
                        arguments: serde_json::to_string(input).unwrap_or_default(),
                    },
                });
            }
        }
    }

    let content = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };

    let finish_reason = claude_resp.stop_reason.map(|sr| match sr.as_str() {
        "end_turn" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        "max_tokens" => "length".to_string(),
        "stop_sequence" => "stop".to_string(),
        other => other.to_string(),
    });

    let usage = claude_resp.usage.map(|u| Usage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
    });

    ChatResponse {
        choices: vec![Choice {
            message: Message {
                role: "assistant".to_string(),
                content,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
            },
            finish_reason,
        }],
        usage,
    }
}

// ============ Streaming Translation ============

/// Anthropic SSE event types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStartData },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStartData,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        #[allow(dead_code)]
        index: usize,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaData,
        #[serde(default)]
        usage: Option<ClaudeUsage>,
    },
    #[serde(rename = "message_stop")]
    MessageStop {},
    #[serde(rename = "ping")]
    Ping {},
    #[serde(rename = "error")]
    Error { error: ClaudeError },
}

#[derive(Debug, Deserialize)]
pub struct MessageStartData {
    #[serde(default)]
    pub usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStartData {
    #[serde(rename = "text")]
    Text {
        #[serde(default)]
        #[allow(dead_code)]
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaData {
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeError {
    #[serde(default)]
    pub message: String,
}

/// Mutable state for tracking the streaming response.
#[derive(Debug, Default)]
pub struct StreamState {
    /// Input tokens from message_start
    pub input_tokens: u64,
    /// Output tokens from message_delta
    pub output_tokens: u64,
}

/// Translate an Anthropic SSE event into zero or more internal StreamEvents.
pub fn translate_stream_event(
    event: &ClaudeStreamEvent,
    state: &mut StreamState,
) -> Vec<StreamEvent> {
    match event {
        ClaudeStreamEvent::MessageStart { message } => {
            if let Some(usage) = &message.usage {
                state.input_tokens = usage.input_tokens;
            }
            vec![]
        }
        ClaudeStreamEvent::ContentBlockStart {
            index,
            content_block,
        } => match content_block {
            ContentBlockStartData::Text { .. } => vec![],
            ContentBlockStartData::ToolUse { id, name } => {
                vec![StreamEvent::ToolCallStart {
                    index: *index,
                    id: id.clone(),
                    name: strip_tool_prefix(name),
                }]
            }
        },
        ClaudeStreamEvent::ContentBlockDelta { index, delta } => match delta {
            ContentBlockDelta::TextDelta { text } => {
                vec![StreamEvent::ContentDelta(text.clone())]
            }
            ContentBlockDelta::InputJsonDelta { partial_json } => {
                vec![StreamEvent::ToolCallDelta {
                    index: *index,
                    arguments: partial_json.clone(),
                }]
            }
        },
        ClaudeStreamEvent::ContentBlockStop { .. } => vec![],
        ClaudeStreamEvent::MessageDelta { usage, .. } => {
            if let Some(u) = usage {
                state.output_tokens = u.output_tokens;
            }
            vec![]
        }
        ClaudeStreamEvent::MessageStop {} => {
            vec![StreamEvent::Done {
                usage: Some(Usage {
                    prompt_tokens: state.input_tokens,
                    completion_tokens: state.output_tokens,
                }),
            }]
        }
        ClaudeStreamEvent::Ping {} => vec![],
        ClaudeStreamEvent::Error { error } => {
            eprintln!("[claude] Stream error: {}", error.message);
            vec![]
        }
    }
}

// ============ Streaming Response Accumulation ============

/// Accumulator for building a ChatResponse from streaming events.
#[derive(Debug, Default)]
pub struct StreamAccumulator {
    content: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: Option<String>,
    usage: Option<Usage>,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an Anthropic stream event and update accumulated state.
    pub fn process_event(&mut self, event: &ClaudeStreamEvent) {
        match event {
            ClaudeStreamEvent::ContentBlockStart {
                content_block: ContentBlockStartData::ToolUse { id, name },
                ..
            } => {
                self.tool_calls.push(ToolCall {
                    id: id.clone(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: strip_tool_prefix(name),
                        arguments: String::new(),
                    },
                });
            }
            ClaudeStreamEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentBlockDelta::TextDelta { text } => {
                    self.content.push_str(text);
                }
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    if let Some(last) = self.tool_calls.last_mut() {
                        last.function.arguments.push_str(partial_json);
                    }
                }
            },
            ClaudeStreamEvent::MessageDelta { delta, usage } => {
                if let Some(reason) = &delta.stop_reason {
                    self.finish_reason = Some(match reason.as_str() {
                        "end_turn" => "stop".to_string(),
                        "tool_use" => "tool_calls".to_string(),
                        "max_tokens" => "length".to_string(),
                        "stop_sequence" => "stop".to_string(),
                        other => other.to_string(),
                    });
                }
                if let Some(u) = usage {
                    self.usage = Some(Usage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: u.output_tokens,
                    });
                }
            }
            ClaudeStreamEvent::MessageStart { message } => {
                if let Some(u) = &message.usage {
                    // Store input tokens; output will come from message_delta
                    let existing = self.usage.take().unwrap_or_default();
                    self.usage = Some(Usage {
                        prompt_tokens: u.input_tokens,
                        completion_tokens: existing.completion_tokens,
                    });
                }
            }
            _ => {}
        }
    }

    /// Build the final ChatResponse from accumulated state.
    pub fn into_response(self) -> ChatResponse {
        let content = if self.content.is_empty() {
            None
        } else {
            Some(self.content)
        };

        ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: "assistant".to_string(),
                    content,
                    tool_calls: if self.tool_calls.is_empty() {
                        None
                    } else {
                        Some(self.tool_calls)
                    },
                },
                finish_reason: self.finish_reason,
            }],
            usage: self.usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_url() {
        assert_eq!(
            messages_url("https://api.anthropic.com/v1", false),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            messages_url("https://api.anthropic.com/v1/", false),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_messages_url_oauth() {
        assert_eq!(
            messages_url("https://api.anthropic.com/v1", true),
            "https://api.anthropic.com/v1/messages?beta=true"
        );
    }

    #[test]
    fn test_is_oauth_token() {
        assert!(is_oauth_token("sk-ant-oat01-abc123"));
        assert!(!is_oauth_token("sk-ant-api03-abc123"));
        assert!(!is_oauth_token("some-other-key"));
    }

    #[test]
    fn test_build_headers_api_key() {
        let headers = build_headers("sk-ant-api03-test");
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[0], ("x-api-key", "sk-ant-api03-test".to_string()));
        assert_eq!(headers[1], ("anthropic-version", "2023-06-01".to_string()));
    }

    #[test]
    fn test_build_headers_oauth_token() {
        let headers = build_headers("sk-ant-oat01-test");
        assert_eq!(headers.len(), 5);
        assert_eq!(
            headers[0],
            ("Authorization", "Bearer sk-ant-oat01-test".to_string())
        );
        assert!(headers[1].1.contains("claude-code-20250219"));
        assert!(headers[1].1.contains("oauth-2025-04-20"));
        assert!(headers[1].1.contains("fine-grained-tool-streaming-2025-05-14"));
        assert_eq!(
            headers[2],
            (
                "User-Agent",
                "claude-cli/2.1.2 (external, cli)".to_string()
            )
        );
        assert_eq!(headers[3], ("anthropic-version", "2023-06-01".to_string()));
    }

    #[test]
    fn test_translate_request_basic() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful."}),
            serde_json::json!({"role": "user", "content": "Hello"}),
        ];

        let body = translate_request("claude-3-5-sonnet-latest", &messages, None, None, false);

        assert_eq!(body["model"], "claude-3-5-sonnet-latest");
        assert_eq!(body["max_tokens"], 8192);

        // System is an array of text blocks
        let system = body["system"].as_array().unwrap();
        assert_eq!(system.len(), 1);
        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "You are helpful.");

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello");
    }

    #[test]
    fn test_translate_request_with_tools() {
        let messages = vec![serde_json::json!({"role": "user", "content": "List files"})];
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files in a directory",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    }
                }
            }
        })];

        let body = translate_request(
            "claude-3-5-sonnet-latest",
            &messages,
            Some(&tools),
            Some("auto"),
            false,
        );

        let api_tools = body["tools"].as_array().unwrap();
        assert_eq!(api_tools.len(), 1);
        assert_eq!(api_tools[0]["name"], "list_files");
        assert!(api_tools[0]["input_schema"].is_object());
        assert_eq!(body["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_translate_assistant_with_tool_calls() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Let me check.",
            "tool_calls": [{
                "id": "call_123",
                "type": "function",
                "function": {
                    "name": "read_file",
                    "arguments": "{\"path\":\"/tmp/test\"}"
                }
            }]
        });

        let translated = translate_assistant_message(&msg, false);
        assert_eq!(translated["role"], "assistant");
        let content = translated["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Let me check.");
        assert_eq!(content[1]["type"], "tool_use");
        assert_eq!(content[1]["id"], "call_123");
        assert_eq!(content[1]["name"], "read_file");
        assert_eq!(content[1]["input"]["path"], "/tmp/test");
    }

    #[test]
    fn test_translate_tool_message() {
        let msg = serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_123",
            "content": "file contents here"
        });

        let translated = translate_tool_message(&msg);
        assert_eq!(translated["role"], "user");
        let content = translated["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "call_123");
        assert_eq!(content[0]["content"], "file contents here");
    }

    #[test]
    fn test_merge_consecutive_roles() {
        let messages = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "1", "content": "result"}]}),
        ];

        let merged = merge_consecutive_roles(messages);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["role"], "user");
        let content = merged[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
    }

    #[test]
    fn test_translate_response_text_only() {
        let resp = ClaudeResponse {
            id: "msg_123".to_string(),
            content: vec![ContentBlock::Text {
                text: "Hello!".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: Some(ClaudeUsage {
                input_tokens: 10,
                output_tokens: 5,
            }),
        };

        let chat_resp = translate_response(resp);
        assert_eq!(chat_resp.choices.len(), 1);
        assert_eq!(
            chat_resp.choices[0].message.content.as_deref(),
            Some("Hello!")
        );
        assert_eq!(
            chat_resp.choices[0].finish_reason.as_deref(),
            Some("stop")
        );
        assert!(chat_resp.choices[0].message.tool_calls.is_none());
        let usage = chat_resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
    }

    #[test]
    fn test_translate_response_with_tool_use() {
        let resp = ClaudeResponse {
            id: "msg_456".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "Let me read that.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "toolu_123".to_string(),
                    name: "Read".to_string(),
                    input: serde_json::json!({"path": "/tmp/test"}),
                },
            ],
            stop_reason: Some("tool_use".to_string()),
            usage: Some(ClaudeUsage {
                input_tokens: 100,
                output_tokens: 50,
            }),
        };

        let chat_resp = translate_response(resp);
        assert_eq!(
            chat_resp.choices[0].message.content.as_deref(),
            Some("Let me read that.")
        );
        assert_eq!(
            chat_resp.choices[0].finish_reason.as_deref(),
            Some("tool_calls")
        );
        let tcs = chat_resp.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].id, "toolu_123");
        assert_eq!(tcs[0].function.name, "Read");
        assert!(tcs[0].function.arguments.contains("/tmp/test"));
    }

    #[test]
    fn test_translate_stream_event_text_delta() {
        let event = ClaudeStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };
        let mut state = StreamState::default();
        let events = translate_stream_event(&event, &mut state);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentDelta(text) => assert_eq!(text, "Hello"),
            _ => panic!("Expected ContentDelta"),
        }
    }

    #[test]
    fn test_translate_stream_event_tool_call() {
        let start = ClaudeStreamEvent::ContentBlockStart {
            index: 1,
            content_block: ContentBlockStartData::ToolUse {
                id: "toolu_123".to_string(),
                name: "Read".to_string(),
            },
        };
        let mut state = StreamState::default();
        let events = translate_stream_event(&start, &mut state);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolCallStart { index, id, name } => {
                assert_eq!(*index, 1);
                assert_eq!(id, "toolu_123");
                assert_eq!(name, "Read");
            }
            _ => panic!("Expected ToolCallStart"),
        }
    }

    #[test]
    fn test_translate_stream_event_message_stop() {
        let mut state = StreamState {
            input_tokens: 100,
            output_tokens: 50,
        };
        let event = ClaudeStreamEvent::MessageStop {};
        let events = translate_stream_event(&event, &mut state);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Done { usage } => {
                let u = usage.as_ref().unwrap();
                assert_eq!(u.prompt_tokens, 100);
                assert_eq!(u.completion_tokens, 50);
            }
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_stream_accumulator() {
        let mut acc = StreamAccumulator::new();

        // Simulate message_start
        acc.process_event(&ClaudeStreamEvent::MessageStart {
            message: MessageStartData {
                usage: Some(ClaudeUsage {
                    input_tokens: 50,
                    output_tokens: 0,
                }),
            },
        });

        // Simulate text delta
        acc.process_event(&ClaudeStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::TextDelta {
                text: "Hello ".to_string(),
            },
        });
        acc.process_event(&ClaudeStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::TextDelta {
                text: "world!".to_string(),
            },
        });

        // Simulate message_delta with stop reason
        acc.process_event(&ClaudeStreamEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".to_string()),
            },
            usage: Some(ClaudeUsage {
                input_tokens: 0,
                output_tokens: 10,
            }),
        });

        let resp = acc.into_response();
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello world!")
        );
        assert_eq!(resp.choices[0].finish_reason.as_deref(), Some("stop"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.completion_tokens, 10);
    }

    #[test]
    fn test_translate_tool_schema() {
        let openai_tool = serde_json::json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path"}
                    },
                    "required": ["path"]
                }
            }
        });

        let claude_tool = translate_tool_schema(&openai_tool, false).unwrap();
        assert_eq!(claude_tool["name"], "read_file");
        assert_eq!(claude_tool["description"], "Read a file");
        assert!(claude_tool["input_schema"]["properties"]["path"].is_object());
    }

    #[test]
    fn test_translate_request_stream() {
        let messages = vec![serde_json::json!({"role": "user", "content": "Hi"})];
        let body = translate_request("claude-3-5-sonnet-latest", &messages, None, None, true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_translate_request_oauth_prepends_system() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful."}),
            serde_json::json!({"role": "user", "content": "Hello"}),
        ];

        let body =
            translate_request_oauth("claude-sonnet-4-20250514", &messages, None, None, false);

        // OAuth mode should prepend the Claude Code system prefix
        let system = body["system"].as_array().unwrap();
        assert_eq!(system.len(), 2);
        assert_eq!(
            system[0]["text"],
            "You are Claude Code, Anthropic's official CLI for Claude."
        );
        assert_eq!(system[1]["text"], "You are helpful.");
    }

    #[test]
    fn test_translate_request_oauth_adds_system_when_none() {
        let messages = vec![serde_json::json!({"role": "user", "content": "Hello"})];

        let body =
            translate_request_oauth("claude-sonnet-4-20250514", &messages, None, None, false);

        // Even without user system prompt, OAuth should add the Claude Code prefix
        let system = body["system"].as_array().unwrap();
        assert_eq!(system.len(), 1);
        assert_eq!(
            system[0]["text"],
            "You are Claude Code, Anthropic's official CLI for Claude."
        );
    }
}
