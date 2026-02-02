//! Channel plugin trait and core types for messaging integrations.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Identifies a target for sending messages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelTarget {
    /// Channel type: "telegram" or "discord"
    pub channel: String,
    /// Platform-specific chat/channel ID
    pub chat_id: String,
    /// Optional user ID (for DM authorization)
    pub user_id: Option<String>,
    /// Optional username/display name
    pub username: Option<String>,
}

impl ChannelTarget {
    /// Create a new Telegram target
    pub fn telegram(chat_id: i64, user_id: Option<i64>) -> Self {
        Self {
            channel: "telegram".to_string(),
            chat_id: chat_id.to_string(),
            user_id: user_id.map(|id| id.to_string()),
            username: None,
        }
    }

    /// Create a new Discord target
    pub fn discord(channel_id: u64, user_id: Option<u64>) -> Self {
        Self {
            channel: "discord".to_string(),
            chat_id: channel_id.to_string(),
            user_id: user_id.map(|id| id.to_string()),
            username: None,
        }
    }

    /// Get a unique key for session mapping
    pub fn session_key(&self) -> String {
        format!("{}:{}", self.channel, self.chat_id)
    }
}

/// Message ID returned after sending
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageId {
    /// Platform-specific message ID
    pub id: String,
    /// Channel type
    pub channel: String,
}

/// Outbound message to send to a channel
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    /// Message content (may be markdown)
    pub content: String,
    /// Whether to use markdown formatting
    pub markdown: bool,
    /// Optional message ID to reply to
    pub reply_to: Option<String>,
    /// Whether this is an edit to an existing message
    pub edit_message_id: Option<String>,
}

impl OutboundMessage {
    /// Create a simple text message
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            markdown: false,
            reply_to: None,
            edit_message_id: None,
        }
    }

    /// Create a markdown message
    pub fn markdown(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            markdown: true,
            reply_to: None,
            edit_message_id: None,
        }
    }

    /// Set reply target
    pub fn with_reply(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Make this an edit to an existing message
    pub fn into_edit(mut self, message_id: impl Into<String>) -> Self {
        self.edit_message_id = Some(message_id.into());
        self
    }
}

/// Approval request to display with interactive buttons
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// Turn ID for correlation
    pub turn_id: String,
    /// Tool call ID
    pub tool_call_id: String,
    /// Tool name being called
    pub tool_name: String,
    /// Tool arguments (JSON)
    pub tool_args: serde_json::Value,
    /// Policy rule that triggered the ask (if any)
    pub policy_rule: Option<String>,
}

impl ApprovalRequest {
    /// Format the approval request for display
    pub fn format_message(&self) -> String {
        let args_display = if self.tool_args.is_object() {
            serde_json::to_string_pretty(&self.tool_args).unwrap_or_else(|_| "{}".to_string())
        } else {
            self.tool_args.to_string()
        };

        let mut msg = format!("**Tool Approval Required**\n\n`{}`\n", self.tool_name);

        if args_display.len() < 500 {
            msg.push_str(&format!("```json\n{}\n```\n", args_display));
        } else {
            // Use char-aware truncation to avoid panic on multi-byte chars
            let truncated: String = args_display.chars().take(500).collect();
            msg.push_str(&format!("```json\n{}...\n```\n", truncated));
        }

        if let Some(rule) = &self.policy_rule {
            msg.push_str(&format!("\n*Policy: {}*\n", rule));
        }

        msg
    }

    /// Get the callback data for approve button
    pub fn approve_callback(&self) -> String {
        format!("approve:{}:{}", self.turn_id, self.tool_call_id)
    }

    /// Get the callback data for deny button
    pub fn deny_callback(&self) -> String {
        format!("deny:{}:{}", self.turn_id, self.tool_call_id)
    }

    /// Parse callback data into (action, turn_id, tool_call_id)
    pub fn parse_callback(data: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = data.splitn(3, ':').collect();
        if parts.len() == 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }
}

/// Inbound message from a channel
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Source of the message
    pub target: ChannelTarget,
    /// Message content
    pub content: String,
    /// Platform-specific message ID
    pub message_id: String,
}

/// Events that a channel plugin receives from the manager
#[derive(Debug, Clone)]
pub enum ChannelEvent {
    /// Agent is thinking
    Thinking(String),
    /// Agent produced content
    Content(String),
    /// Token delta for streaming
    TokenDelta(String),
    /// Tool is being called
    ToolCall {
        name: String,
        args: serde_json::Value,
        tool_call_id: String,
    },
    /// Tool completed
    ToolResult {
        name: String,
        tool_call_id: String,
        ok: bool,
        duration_ms: u64,
    },
    /// Awaiting approval (yield)
    AwaitingApproval(ApprovalRequest),
    /// Turn completed
    Done {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// Error occurred
    Error { code: String, message: String },
}

/// Context passed to channel plugins for interacting with the manager
pub struct ChannelContext {
    /// Sender for inbound messages
    pub message_tx: mpsc::UnboundedSender<InboundMessage>,
    /// Sender for approval responses
    pub approval_tx: mpsc::UnboundedSender<ApprovalResponse>,
}

/// Response to an approval request
#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    /// Turn ID
    pub turn_id: String,
    /// Tool call ID
    pub tool_call_id: String,
    /// Whether approved
    pub approved: bool,
    /// Source target (for logging)
    pub source: ChannelTarget,
}

/// Trait for channel plugins (Telegram, Discord, etc.)
#[async_trait::async_trait]
pub trait ChannelPlugin: Send + Sync {
    /// Plugin name (e.g., "telegram", "discord")
    fn name(&self) -> &str;

    /// Start the plugin (connect to platform, start polling/webhooks)
    async fn start(&self, ctx: Arc<ChannelContext>) -> anyhow::Result<()>;

    /// Stop the plugin gracefully
    async fn stop(&self) -> anyhow::Result<()>;

    /// Send a message to a target
    async fn send_message(
        &self,
        target: &ChannelTarget,
        msg: &OutboundMessage,
    ) -> anyhow::Result<MessageId>;

    /// Send an approval request with interactive buttons
    async fn send_approval_request(
        &self,
        target: &ChannelTarget,
        req: &ApprovalRequest,
    ) -> anyhow::Result<MessageId>;

    /// Edit a previously sent message
    async fn edit_message(
        &self,
        target: &ChannelTarget,
        message_id: &str,
        msg: &OutboundMessage,
    ) -> anyhow::Result<()>;

    /// Delete a message
    async fn delete_message(&self, target: &ChannelTarget, message_id: &str) -> anyhow::Result<()>;
}

/// Result type for plugin operations
pub type PluginResult<T> = anyhow::Result<T>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_target_session_key() {
        let target = ChannelTarget::telegram(12345, Some(67890));
        assert_eq!(target.session_key(), "telegram:12345");
    }

    #[test]
    fn test_approval_callback_parsing() {
        let req = ApprovalRequest {
            turn_id: "turn-123".to_string(),
            tool_call_id: "tc-456".to_string(),
            tool_name: "Bash".to_string(),
            tool_args: serde_json::json!({"command": "ls"}),
            policy_rule: None,
        };

        let callback = req.approve_callback();
        let parsed = ApprovalRequest::parse_callback(&callback).unwrap();
        assert_eq!(parsed.0, "approve");
        assert_eq!(parsed.1, "turn-123");
        assert_eq!(parsed.2, "tc-456");
    }

    #[test]
    fn test_outbound_message_builders() {
        let msg = OutboundMessage::markdown("**Hello**")
            .with_reply("msg-123")
            .into_edit("msg-456");

        assert!(msg.markdown);
        assert_eq!(msg.reply_to, Some("msg-123".to_string()));
        assert_eq!(msg.edit_message_id, Some("msg-456".to_string()));
    }
}
