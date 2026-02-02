//! Channel manager for message routing and event forwarding.

use super::auth::{AuthStatus, ChannelAuthManager};
use super::config::ChannelsConfig;
use super::plugin::{
    ApprovalRequest, ApprovalResponse, ChannelContext, ChannelEvent, ChannelPlugin, ChannelTarget,
    InboundMessage, OutboundMessage,
};
use super::session_map::ChannelSessionMap;
use crate::gateway::agent_conn::AsyncAgentConnection;
use crate::protocol::internal::{AgentEventType, AgentRequest, ResumeData, YieldReason};
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

/// Manages channel plugins and message routing
pub struct ChannelManager {
    /// Configuration
    config: ChannelsConfig,
    /// Registered plugins
    plugins: RwLock<HashMap<String, Arc<dyn ChannelPlugin>>>,
    /// Session mapping
    sessions: Arc<ChannelSessionMap>,
    /// Authorization manager
    auth: Arc<ChannelAuthManager>,
    /// Agent connection
    agent: Arc<AsyncAgentConnection>,
    /// Inbound message receiver
    message_rx: RwLock<Option<mpsc::UnboundedReceiver<InboundMessage>>>,
    /// Approval response receiver
    approval_rx: RwLock<Option<mpsc::UnboundedReceiver<ApprovalResponse>>>,
    /// Message sender for plugins
    message_tx: mpsc::UnboundedSender<InboundMessage>,
    /// Approval sender for plugins
    approval_tx: mpsc::UnboundedSender<ApprovalResponse>,
    /// Streaming state: target -> (last_update, buffer)
    streaming: RwLock<HashMap<String, StreamingState>>,
}

/// State for message streaming/batching
struct StreamingState {
    /// Accumulated content
    buffer: String,
    /// Last time we sent an update
    last_update: Instant,
    /// Message ID being edited
    message_id: Option<String>,
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new(config: ChannelsConfig, agent: Arc<AsyncAgentConnection>) -> Arc<Self> {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (approval_tx, approval_rx) = mpsc::unbounded_channel();

        Arc::new(Self {
            auth: ChannelAuthManager::new(config.auth.clone()),
            sessions: ChannelSessionMap::new(),
            agent,
            plugins: RwLock::new(HashMap::new()),
            message_rx: RwLock::new(Some(message_rx)),
            approval_rx: RwLock::new(Some(approval_rx)),
            message_tx,
            approval_tx,
            streaming: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Get the channel context for plugins
    pub fn get_context(&self) -> Arc<ChannelContext> {
        Arc::new(ChannelContext {
            message_tx: self.message_tx.clone(),
            approval_tx: self.approval_tx.clone(),
        })
    }

    /// Register a channel plugin
    pub async fn register_plugin(&self, plugin: Arc<dyn ChannelPlugin>) {
        let name = plugin.name().to_string();
        self.plugins.write().await.insert(name, plugin);
    }

    /// Start all registered plugins
    pub async fn start_all(&self) -> Result<()> {
        let ctx = self.get_context();
        let plugins = self.plugins.read().await;

        for (name, plugin) in plugins.iter() {
            eprintln!("[channels] Starting {} plugin", name);
            if let Err(e) = plugin.start(ctx.clone()).await {
                eprintln!("[channels] Failed to start {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Stop all plugins
    pub async fn stop_all(&self) -> Result<()> {
        let plugins = self.plugins.read().await;

        for (name, plugin) in plugins.iter() {
            eprintln!("[channels] Stopping {} plugin", name);
            if let Err(e) = plugin.stop().await {
                eprintln!("[channels] Failed to stop {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Run the message processing loop
    pub async fn run(self: Arc<Self>) -> Result<()> {
        // Take ownership of receivers
        let mut message_rx = self
            .message_rx
            .write()
            .await
            .take()
            .ok_or_else(|| anyhow::anyhow!("Manager already running"))?;

        let mut approval_rx = self
            .approval_rx
            .write()
            .await
            .take()
            .ok_or_else(|| anyhow::anyhow!("Manager already running"))?;

        // Spawn streaming flush task
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                if let Err(e) = manager.flush_streaming_buffers().await {
                    eprintln!("[channels] Streaming flush error: {}", e);
                }
            }
        });

        // Spawn session cleanup task
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                let removed = manager.sessions.cleanup_stale(Duration::from_secs(3600)); // 1 hour
                if removed > 0 {
                    eprintln!("[channels] Cleaned up {} stale sessions", removed);
                }
            }
        });

        loop {
            tokio::select! {
                Some(msg) = message_rx.recv() => {
                    if let Err(e) = self.handle_inbound_message(msg).await {
                        eprintln!("[channels] Error handling message: {}", e);
                    }
                }
                Some(approval) = approval_rx.recv() => {
                    if let Err(e) = self.handle_approval_response(approval).await {
                        eprintln!("[channels] Error handling approval: {}", e);
                    }
                }
            }
        }
    }

    /// Handle an inbound message from a channel
    async fn handle_inbound_message(&self, msg: InboundMessage) -> Result<()> {
        let target = &msg.target;

        // Check authorization
        match self.auth.check_auth(target) {
            AuthStatus::Authorized => {
                // Proceed with message
                self.process_authorized_message(msg).await
            }
            AuthStatus::PendingPairing(code) => {
                // Remind about pairing
                self.send_pairing_reminder(target, &code).await
            }
            AuthStatus::Denied => {
                // Initiate pairing
                let code = self.auth.request_pairing(target);
                self.send_pairing_request(target, &code).await
            }
        }
    }

    /// Process an authorized message
    async fn process_authorized_message(&self, msg: InboundMessage) -> Result<()> {
        let target = msg.target.clone();

        // Get or create session
        let session_info = self.sessions.get_or_create_session(&target);
        self.sessions.touch_session(&target);

        // Build agent request
        let req_id = uuid::Uuid::new_v4().to_string();
        let agent_request = AgentRequest::run_turn(
            &req_id,
            &session_info.session_id,
            vec![json!({
                "role": "user",
                "content": msg.content
            })],
            None,
        );

        // Send to agent and process events
        self.process_agent_turn(&target, agent_request).await
    }

    /// Process an agent turn and forward events to the channel
    async fn process_agent_turn(
        &self,
        target: &ChannelTarget,
        request: AgentRequest,
    ) -> Result<()> {
        let mut event_rx = self.agent.send_request(request).await?;

        // Initialize streaming state
        {
            let mut streaming = self.streaming.write().await;
            streaming.insert(
                target.session_key(),
                StreamingState {
                    buffer: String::new(),
                    last_update: Instant::now(),
                    message_id: None,
                },
            );
        }

        while let Some(agent_event) = event_rx.recv().await {
            let channel_event = self.map_agent_event(&agent_event.event);

            if let Some(event) = channel_event {
                self.handle_channel_event(target, event).await?;
            }

            // Check for terminal events
            if matches!(
                agent_event.event,
                AgentEventType::Done { .. }
                    | AgentEventType::Error { .. }
                    | AgentEventType::Yield { .. }
            ) {
                break;
            }
        }

        // Flush any remaining content
        self.flush_target_buffer(target).await?;

        // Clear streaming state
        {
            let mut streaming = self.streaming.write().await;
            streaming.remove(&target.session_key());
        }

        Ok(())
    }

    /// Map agent event to channel event
    fn map_agent_event(&self, event: &AgentEventType) -> Option<ChannelEvent> {
        match event {
            AgentEventType::Thinking { content } => Some(ChannelEvent::Thinking(content.clone())),
            AgentEventType::Content { text } => Some(ChannelEvent::Content(text.clone())),
            AgentEventType::TokenDelta { text } => Some(ChannelEvent::TokenDelta(text.clone())),
            AgentEventType::ToolCall {
                name,
                args,
                tool_call_id,
            } => Some(ChannelEvent::ToolCall {
                name: name.clone(),
                args: args.clone(),
                tool_call_id: tool_call_id.clone(),
            }),
            AgentEventType::ToolResult {
                name,
                tool_call_id,
                ok,
                duration_ms,
                ..
            } => Some(ChannelEvent::ToolResult {
                name: name.clone(),
                tool_call_id: tool_call_id.clone(),
                ok: *ok,
                duration_ms: *duration_ms,
            }),
            AgentEventType::Yield {
                turn_id,
                reason,
                tool_call_id,
                tool_name,
                tool_args,
                policy_rule,
                ..
            } => {
                if *reason == YieldReason::AwaitingApproval {
                    Some(ChannelEvent::AwaitingApproval(ApprovalRequest {
                        turn_id: turn_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                        tool_name: tool_name.clone(),
                        tool_args: tool_args.clone(),
                        policy_rule: policy_rule.clone(),
                    }))
                } else {
                    // AwaitingInput is not yet supported in channels
                    None
                }
            }
            AgentEventType::Done { usage } => Some(ChannelEvent::Done {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }),
            AgentEventType::Error { code, message } => Some(ChannelEvent::Error {
                code: code.clone(),
                message: message.clone(),
            }),
            _ => None,
        }
    }

    /// Handle a channel event
    async fn handle_channel_event(
        &self,
        target: &ChannelTarget,
        event: ChannelEvent,
    ) -> Result<()> {
        match event {
            ChannelEvent::TokenDelta(text) => {
                // Buffer token deltas
                let mut streaming = self.streaming.write().await;
                if let Some(state) = streaming.get_mut(&target.session_key()) {
                    state.buffer.push_str(&text);
                }
            }
            ChannelEvent::Content(text) => {
                // Append to buffer and flush
                {
                    let mut streaming = self.streaming.write().await;
                    if let Some(state) = streaming.get_mut(&target.session_key()) {
                        state.buffer.push_str(&text);
                    }
                }
                self.flush_target_buffer(target).await?;
            }
            ChannelEvent::Thinking(content) => {
                // Send thinking indicator
                self.send_to_target(
                    target,
                    &OutboundMessage::text(format!("_Thinking: {}_", content)),
                )
                .await?;
            }
            ChannelEvent::ToolCall { name, .. } => {
                // Send tool call indicator
                self.send_to_target(
                    target,
                    &OutboundMessage::text(format!("_Using tool: {}_", name)),
                )
                .await?;
            }
            ChannelEvent::AwaitingApproval(req) => {
                // Store pending turn
                self.sessions
                    .set_pending_turn(target, Some(req.turn_id.clone()));
                // Send approval request with buttons
                self.send_approval_to_target(target, &req).await?;
            }
            ChannelEvent::Error { code, message } => {
                self.send_to_target(
                    target,
                    &OutboundMessage::markdown(format!("**Error** ({}): {}", code, message)),
                )
                .await?;
            }
            ChannelEvent::Done { .. } => {
                // Clear pending state
                self.sessions.set_pending_turn(target, None);
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle an approval response from a channel
    async fn handle_approval_response(&self, response: ApprovalResponse) -> Result<()> {
        let target = &response.source;

        // Get session
        let session_info = self
            .sessions
            .get_session(target)
            .ok_or_else(|| anyhow::anyhow!("No session for target"))?;

        // Build resume request
        let req_id = uuid::Uuid::new_v4().to_string();
        let resume_data = ResumeData {
            turn_id: response.turn_id,
            tool_call_id: response.tool_call_id,
            approved: Some(response.approved),
            answers: None,
        };

        let agent_request =
            AgentRequest::resume_turn(&req_id, &session_info.session_id, resume_data);

        // Process continued turn
        self.process_agent_turn(target, agent_request).await
    }

    /// Flush streaming buffers for all targets
    async fn flush_streaming_buffers(&self) -> Result<()> {
        let now = Instant::now();
        let update_interval = Duration::from_millis(500);

        let mut to_flush = Vec::new();

        {
            let streaming = self.streaming.read().await;
            for (key, state) in streaming.iter() {
                if !state.buffer.is_empty()
                    && now.duration_since(state.last_update) >= update_interval
                {
                    to_flush.push(key.clone());
                }
            }
        }

        for key in to_flush {
            if let Some(target) = self.target_from_key(&key) {
                self.flush_target_buffer(&target).await?;
            }
        }

        Ok(())
    }

    /// Flush the streaming buffer for a specific target
    async fn flush_target_buffer(&self, target: &ChannelTarget) -> Result<()> {
        let key = target.session_key();
        let (content, message_id) = {
            let mut streaming = self.streaming.write().await;
            if let Some(state) = streaming.get_mut(&key) {
                if state.buffer.is_empty() {
                    return Ok(());
                }
                let content = std::mem::take(&mut state.buffer);
                state.last_update = Instant::now();
                (content, state.message_id.clone())
            } else {
                return Ok(());
            }
        };

        // Send or edit message
        let msg = if let Some(msg_id) = &message_id {
            OutboundMessage::markdown(&content).into_edit(msg_id)
        } else {
            OutboundMessage::markdown(&content)
        };

        if let Ok(sent_msg) = self.send_to_target(target, &msg).await {
            // Update message ID for future edits
            let mut streaming = self.streaming.write().await;
            if let Some(state) = streaming.get_mut(&key) {
                state.message_id = Some(sent_msg.id);
            }
        }

        Ok(())
    }

    /// Send a message to a target via its plugin
    async fn send_to_target(
        &self,
        target: &ChannelTarget,
        msg: &OutboundMessage,
    ) -> Result<super::plugin::MessageId> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(&target.channel)
            .ok_or_else(|| anyhow::anyhow!("No plugin for channel: {}", target.channel))?;

        plugin.send_message(target, msg).await
    }

    /// Send an approval request to a target
    async fn send_approval_to_target(
        &self,
        target: &ChannelTarget,
        req: &ApprovalRequest,
    ) -> Result<()> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(&target.channel)
            .ok_or_else(|| anyhow::anyhow!("No plugin for channel: {}", target.channel))?;

        plugin.send_approval_request(target, req).await?;
        Ok(())
    }

    /// Send pairing request to a target
    async fn send_pairing_request(&self, target: &ChannelTarget, code: &str) -> Result<()> {
        let msg = OutboundMessage::markdown(format!(
            "**Authorization Required**\n\n\
            To use this bot, enter the following pairing code in the gateway:\n\n\
            `{}`\n\n\
            This code expires in 5 minutes.",
            code
        ));

        self.send_to_target(target, &msg).await?;
        Ok(())
    }

    /// Send pairing reminder to a target
    async fn send_pairing_reminder(&self, target: &ChannelTarget, code: &str) -> Result<()> {
        let msg = OutboundMessage::markdown(format!(
            "Waiting for authorization. Your pairing code is: `{}`",
            code
        ));

        self.send_to_target(target, &msg).await?;
        Ok(())
    }

    /// Convert session key back to target
    fn target_from_key(&self, key: &str) -> Option<ChannelTarget> {
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(ChannelTarget {
                channel: parts[0].to_string(),
                chat_id: parts[1].to_string(),
                user_id: None,
                username: None,
            })
        } else {
            None
        }
    }

    /// Get authorization manager
    pub fn auth(&self) -> &Arc<ChannelAuthManager> {
        &self.auth
    }

    /// Get session map
    pub fn sessions(&self) -> &Arc<ChannelSessionMap> {
        &self.sessions
    }

    /// Get channel status for HTTP endpoint
    pub async fn status(&self) -> ChannelStatus {
        let plugins = self.plugins.read().await;
        ChannelStatus {
            plugins: plugins.keys().cloned().collect(),
            active_sessions: self.sessions.session_count(),
            pending_pairings: self.auth.pending_count(),
            authorizations: self.auth.list_authorizations().len(),
        }
    }
}

/// Channel manager status
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChannelStatus {
    pub plugins: Vec<String>,
    pub active_sessions: usize,
    pub pending_pairings: usize,
    pub authorizations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would require mocking the agent connection
    // For now, we just test that types compile correctly

    #[test]
    fn test_streaming_state() {
        let _state = StreamingState {
            buffer: String::new(),
            last_update: Instant::now(),
            message_id: None,
        };
    }
}
