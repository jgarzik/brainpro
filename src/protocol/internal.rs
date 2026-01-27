//! Internal protocol between Gateway and Agent daemon.
//! Uses NDJSON (newline-delimited JSON) for streaming responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request from Gateway to Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Unique request identifier for correlation
    pub id: String,
    /// Method to invoke
    pub method: AgentMethod,
    /// Session ID for context
    pub session_id: String,
    /// Conversation messages
    #[serde(default)]
    pub messages: Vec<Value>,
    /// Target model@backend
    #[serde(default)]
    pub target: Option<String>,
    /// Tool schemas to provide
    #[serde(default)]
    pub tools: Option<Vec<Value>>,
    /// Working directory for tool execution
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Methods the agent can execute
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMethod {
    /// Run a single turn of the agent loop
    RunTurn,
    /// Cancel an in-flight request
    Cancel,
    /// Health check
    Ping,
}

/// Streaming response events from Agent to Gateway (NDJSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Request ID this event belongs to
    pub id: String,
    /// Event type and payload
    #[serde(flatten)]
    pub event: AgentEventType,
}

/// Types of events the agent can emit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEventType {
    /// Agent is thinking (content before tool calls)
    Thinking { content: String },
    /// Agent is calling a tool
    ToolCall {
        name: String,
        args: Value,
        tool_call_id: String,
    },
    /// Tool execution result
    ToolResult {
        name: String,
        tool_call_id: String,
        result: Value,
        ok: bool,
        duration_ms: u64,
    },
    /// Final text content from the agent
    Content { text: String },
    /// Agent turn completed successfully
    Done { usage: UsageStats },
    /// Agent needs user input (AskUserQuestion)
    AwaitingInput {
        tool_call_id: String,
        questions: Vec<Value>,
    },
    /// Error occurred
    Error { code: String, message: String },
    /// Pong response to ping
    Pong,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_uses: u64,
}

impl AgentEvent {
    pub fn thinking(id: &str, content: &str) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::Thinking {
                content: content.to_string(),
            },
        }
    }

    pub fn tool_call(id: &str, name: &str, args: Value, tool_call_id: &str) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::ToolCall {
                name: name.to_string(),
                args,
                tool_call_id: tool_call_id.to_string(),
            },
        }
    }

    pub fn tool_result(
        id: &str,
        name: &str,
        tool_call_id: &str,
        result: Value,
        ok: bool,
        duration_ms: u64,
    ) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::ToolResult {
                name: name.to_string(),
                tool_call_id: tool_call_id.to_string(),
                result,
                ok,
                duration_ms,
            },
        }
    }

    pub fn content(id: &str, text: &str) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::Content {
                text: text.to_string(),
            },
        }
    }

    pub fn done(id: &str, usage: UsageStats) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::Done { usage },
        }
    }

    pub fn awaiting_input(id: &str, tool_call_id: &str, questions: Vec<Value>) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::AwaitingInput {
                tool_call_id: tool_call_id.to_string(),
                questions,
            },
        }
    }

    pub fn error(id: &str, code: &str, message: &str) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::Error {
                code: code.to_string(),
                message: message.to_string(),
            },
        }
    }

    pub fn pong(id: &str) -> Self {
        Self {
            id: id.to_string(),
            event: AgentEventType::Pong,
        }
    }

    /// Serialize to NDJSON line (with trailing newline)
    pub fn to_ndjson(&self) -> String {
        let mut json = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        json.push('\n');
        json
    }
}

impl AgentRequest {
    /// Parse from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Create a run_turn request
    pub fn run_turn(
        id: &str,
        session_id: &str,
        messages: Vec<Value>,
        target: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            method: AgentMethod::RunTurn,
            session_id: session_id.to_string(),
            messages,
            target,
            tools: None,
            working_dir: None,
        }
    }

    /// Create a cancel request
    pub fn cancel(id: &str, session_id: &str) -> Self {
        Self {
            id: id.to_string(),
            method: AgentMethod::Cancel,
            session_id: session_id.to_string(),
            messages: Vec::new(),
            target: None,
            tools: None,
            working_dir: None,
        }
    }

    /// Create a ping request
    pub fn ping(id: &str) -> Self {
        Self {
            id: id.to_string(),
            method: AgentMethod::Ping,
            session_id: String::new(),
            messages: Vec::new(),
            target: None,
            tools: None,
            working_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEvent::thinking("req-1", "Analyzing the code...");
        let json = event.to_ndjson();
        assert!(json.contains("thinking"));
        assert!(json.contains("req-1"));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn test_agent_request_parsing() {
        let json = r#"{"id":"1","method":"run_turn","session_id":"s1","messages":[]}"#;
        let req = AgentRequest::from_json(json).unwrap();
        assert_eq!(req.id, "1");
        assert_eq!(req.method, AgentMethod::RunTurn);
    }

    #[test]
    fn test_done_event() {
        let usage = UsageStats {
            input_tokens: 100,
            output_tokens: 50,
            tool_uses: 3,
        };
        let event = AgentEvent::done("req-1", usage);
        let json = event.to_ndjson();
        assert!(json.contains("done"));
        assert!(json.contains("100"));
    }
}
