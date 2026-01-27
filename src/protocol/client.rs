//! Client protocol for WebSocket communication between clients and Gateway.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client roles
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientRole {
    /// Full access: chat, approve tools, manage sessions/cron/devices
    #[default]
    Operator,
    /// Limited: execute delegated tools, heartbeat only
    Node,
}

/// Client capabilities advertised during handshake
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Tools this client can execute (for nodes)
    #[serde(default)]
    pub tools: Vec<String>,
    /// Supported protocol version
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
}

fn default_protocol_version() -> u32 {
    1
}

/// Handshake: Client hello
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub role: ClientRole,
    pub device_id: String,
    #[serde(default)]
    pub caps: ClientCapabilities,
}

/// Handshake: Server challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub nonce: String,
}

/// Handshake: Client auth response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub signature: String,
}

/// Handshake: Server welcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    pub session_id: String,
    pub policy: PolicyInfo,
}

/// Policy info sent to client
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyInfo {
    pub mode: String,
    pub max_turns: usize,
}

/// Client → Gateway request frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRequest {
    #[serde(rename = "type")]
    pub frame_type: String, // Always "req"
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Gateway → Client response frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientResponse {
    #[serde(rename = "type")]
    pub frame_type: String, // Always "res"
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

/// Error information in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
}

/// Gateway → Client event frame (server push)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEvent {
    #[serde(rename = "type")]
    pub frame_type: String, // Always "event"
    pub event: String,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Incoming WebSocket message types (can be any of the above)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    // Handshake
    Hello(Hello),
    Challenge(Challenge),
    Auth(Auth),
    Welcome(Welcome),
    // Request/Response
    Req(RequestPayload),
    Res(ResponsePayload),
    // Events
    Event(EventPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPayload {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePayload {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event: String,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Method names for client requests
pub mod methods {
    pub const CHAT_SEND: &str = "chat.send";
    pub const SESSION_CREATE: &str = "session.create";
    pub const SESSION_LIST: &str = "session.list";
    pub const SESSION_GET: &str = "session.get";
    pub const TOOL_APPROVE: &str = "tool.approve";
    pub const TURN_RESUME: &str = "turn.resume";
    pub const CRON_ADD: &str = "cron.add";
    pub const CRON_REMOVE: &str = "cron.remove";
    pub const CRON_LIST: &str = "cron.list";
    pub const DEVICE_PAIR: &str = "device.pair";
    pub const HEALTH_STATUS: &str = "health.status";
}

/// Event names for server push
pub mod events {
    pub const AGENT_THINKING: &str = "agent.thinking";
    pub const AGENT_TOOL_CALL: &str = "agent.tool_call";
    pub const AGENT_TOOL_RESULT: &str = "agent.tool_result";
    pub const AGENT_MESSAGE: &str = "agent.message";
    pub const AGENT_DONE: &str = "agent.done";
    pub const AGENT_ERROR: &str = "agent.error";
    pub const AGENT_AWAITING_APPROVAL: &str = "agent.awaiting_approval";
    pub const AGENT_AWAITING_INPUT: &str = "agent.awaiting_input";
    pub const PRESENCE_UPDATE: &str = "presence.update";
    pub const HEALTH_TICK: &str = "health.tick";
    pub const CRON_FIRED: &str = "cron.fired";
}

impl ClientRequest {
    pub fn new(id: &str, method: &str, params: Value) -> Self {
        Self {
            frame_type: "req".to_string(),
            id: id.to_string(),
            method: method.to_string(),
            params,
        }
    }
}

impl ClientResponse {
    pub fn ok(id: &str, payload: Value) -> Self {
        Self {
            frame_type: "res".to_string(),
            id: id.to_string(),
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn error(id: &str, code: &str, message: &str) -> Self {
        Self {
            frame_type: "res".to_string(),
            id: id.to_string(),
            ok: false,
            payload: None,
            error: Some(ErrorInfo {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

impl ClientEvent {
    pub fn new(event: &str, data: Value, session_id: Option<String>) -> Self {
        Self {
            frame_type: "event".to_string(),
            event: event.to_string(),
            data,
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_client_request_serialization() {
        let req = ClientRequest::new("1", methods::CHAT_SEND, json!({"message": "hello"}));
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("chat.send"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_client_response_ok() {
        let res = ClientResponse::ok("1", json!({"status": "sent"}));
        assert!(res.ok);
        assert!(res.error.is_none());
    }

    #[test]
    fn test_client_response_error() {
        let res = ClientResponse::error("1", "invalid_session", "Session not found");
        assert!(!res.ok);
        assert!(res.error.is_some());
    }

    #[test]
    fn test_client_event() {
        let event = ClientEvent::new(
            events::AGENT_THINKING,
            json!({"content": "..."}),
            Some("s1".into()),
        );
        assert_eq!(event.event, events::AGENT_THINKING);
        assert!(event.session_id.is_some());
    }
}
