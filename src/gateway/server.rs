//! Gateway WebSocket server using axum.

use crate::gateway::agent_conn::AsyncAgentConnection;
use crate::gateway::client_mgr::{ClientManager, ClientMessage};
use crate::protocol::client::{
    events, methods, ClientEvent, ClientRequest, ClientResponse, Hello, PolicyInfo, Welcome,
};
use crate::protocol::internal::{AgentEventType, AgentRequest, ResumeData, YieldReason};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Gateway server configuration
pub struct GatewayConfig {
    pub port: u16,
    pub agent_socket: String,
    pub auth_token: Option<String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: 18789,
            agent_socket: "/run/brainpro.sock".to_string(),
            auth_token: std::env::var("BRAINPRO_GATEWAY_TOKEN").ok(),
        }
    }
}

/// Shared state for the gateway
pub struct GatewayState {
    pub config: GatewayConfig,
    pub clients: Arc<ClientManager>,
    pub agent: AsyncAgentConnection,
}

impl GatewayState {
    pub fn new(config: GatewayConfig) -> Arc<Self> {
        let agent = AsyncAgentConnection::new(&config.agent_socket);
        Arc::new(Self {
            config,
            clients: ClientManager::new(),
            agent,
        })
    }
}

/// Run the gateway server
pub async fn run(config: GatewayConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = GatewayState::new(config);
    let port = state.config.port;

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    eprintln!("[gateway] Listening on port {}", port);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Brainpro Gateway</title>
    <style>
        body { font-family: system-ui, sans-serif; max-width: 800px; margin: 2rem auto; padding: 1rem; }
        h1 { color: #333; }
        .status { padding: 1rem; background: #f0f0f0; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>Brainpro Gateway</h1>
    <div class="status">
        <p>WebSocket endpoint: <code>ws://localhost:18789/ws</code></p>
        <p>Health check: <code>GET /health</code></p>
    </div>
</body>
</html>"#,
    )
}

async fn health_handler(State(state): State<Arc<GatewayState>>) -> impl IntoResponse {
    let agent_ok = state.agent.is_available();
    let client_count = state.clients.client_count();

    let status = if agent_ok { "healthy" } else { "degraded" };

    axum::Json(json!({
        "status": status,
        "agent_available": agent_ok,
        "connected_clients": client_count,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<GatewayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: Arc<GatewayState>) {
    let (mut sender, mut receiver) = socket.split();

    // Generate client ID
    let client_id = uuid::Uuid::new_v4().to_string();

    // Create channel for sending messages to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<ClientMessage>();

    // Spawn task to forward messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.json.into())).await.is_err() {
                break;
            }
        }
    });

    // Wait for Hello message
    let hello: Hello = match wait_for_hello(&mut receiver).await {
        Some(h) => h,
        None => {
            eprintln!("[gateway] Client {} failed handshake", client_id);
            return;
        }
    };

    // Register client
    state.clients.register(
        &client_id,
        hello.role,
        &hello.device_id,
        hello.caps.clone(),
        tx.clone(),
    );

    // Send Welcome
    let session_id = uuid::Uuid::new_v4().to_string();
    let welcome = Welcome {
        session_id: session_id.clone(),
        policy: PolicyInfo {
            mode: "default".to_string(),
            max_turns: 12,
        },
    };

    let welcome_json = serde_json::to_string(&json!({
        "type": "welcome",
        "session_id": welcome.session_id,
        "policy": welcome.policy,
    }))
    .unwrap();

    if tx.send(ClientMessage { json: welcome_json }).is_err() {
        state.clients.unregister(&client_id);
        return;
    }

    // Join session
    state.clients.join_session(&client_id, &session_id);

    eprintln!(
        "[gateway] Client {} connected (role={:?}, session={})",
        client_id, hello.role, session_id
    );

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) =
                    handle_client_message(&state, &client_id, &session_id, &text, &tx).await
                {
                    eprintln!("[gateway] Error handling message: {}", e);
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                eprintln!("[gateway] WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    state.clients.unregister(&client_id);
    send_task.abort();
    eprintln!("[gateway] Client {} disconnected", client_id);
}

async fn wait_for_hello(
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
) -> Option<Hello> {
    // Wait up to 10 seconds for hello
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(msg) = receiver.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("type").and_then(|t| t.as_str()) == Some("hello") {
                        if let Ok(hello) = serde_json::from_value::<Hello>(value) {
                            return Some(hello);
                        }
                    }
                }
            }
        }
        None
    });

    timeout.await.ok().flatten()
}

async fn handle_client_message(
    state: &Arc<GatewayState>,
    client_id: &str,
    session_id: &str,
    text: &str,
    tx: &mpsc::UnboundedSender<ClientMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value: Value = serde_json::from_str(text)?;

    // Check message type
    let msg_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

    if msg_type == "req" {
        let request: ClientRequest = serde_json::from_value(value)?;
        handle_request(state, client_id, session_id, request, tx).await?;
    }

    Ok(())
}

async fn handle_request(
    state: &Arc<GatewayState>,
    client_id: &str,
    session_id: &str,
    request: ClientRequest,
    tx: &mpsc::UnboundedSender<ClientMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let req_id = request.id.clone();

    match request.method.as_str() {
        methods::HEALTH_STATUS => {
            let response = ClientResponse::ok(
                &req_id,
                json!({
                    "agent_available": state.agent.is_available(),
                    "connected_clients": state.clients.client_count(),
                }),
            );
            send_response(tx, &response);
        }

        methods::CHAT_SEND => {
            handle_chat_send(state, session_id, &req_id, request.params, tx).await?;
        }

        methods::SESSION_CREATE => {
            let new_session_id = uuid::Uuid::new_v4().to_string();
            state.clients.join_session(client_id, &new_session_id);
            let response = ClientResponse::ok(&req_id, json!({ "session_id": new_session_id }));
            send_response(tx, &response);
        }

        methods::SESSION_LIST => {
            // For now, just return the current session
            let response = ClientResponse::ok(&req_id, json!({ "sessions": [session_id] }));
            send_response(tx, &response);
        }

        methods::TURN_RESUME => {
            handle_turn_resume(state, session_id, &req_id, request.params, tx).await?;
        }

        _ => {
            let response = ClientResponse::error(
                &req_id,
                "unknown_method",
                &format!("Unknown method: {}", request.method),
            );
            send_response(tx, &response);
        }
    }

    Ok(())
}

async fn handle_chat_send(
    state: &Arc<GatewayState>,
    session_id: &str,
    req_id: &str,
    params: Value,
    tx: &mpsc::UnboundedSender<ClientMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let message = params.get("message").and_then(|m| m.as_str()).unwrap_or("");

    if message.is_empty() {
        let response = ClientResponse::error(req_id, "invalid_params", "Missing message parameter");
        send_response(tx, &response);
        return Ok(());
    }

    // Build agent request
    let agent_request = AgentRequest::run_turn(
        req_id,
        session_id,
        vec![json!({
            "role": "user",
            "content": message
        })],
        None, // Use default target
    );

    // Send to agent and stream events back
    let mut event_rx = state.agent.send_request(agent_request).await?;

    // Forward agent events as client events
    while let Some(agent_event) = event_rx.recv().await {
        let client_event = match agent_event.event {
            AgentEventType::Thinking { content } => Some(ClientEvent::new(
                events::AGENT_THINKING,
                json!({ "content": content }),
                Some(session_id.to_string()),
            )),
            AgentEventType::ToolCall {
                name,
                args,
                tool_call_id,
            } => Some(ClientEvent::new(
                events::AGENT_TOOL_CALL,
                json!({ "name": name, "args": args, "tool_call_id": tool_call_id }),
                Some(session_id.to_string()),
            )),
            AgentEventType::ToolResult {
                name,
                tool_call_id,
                result,
                ok,
                duration_ms,
            } => Some(ClientEvent::new(
                events::AGENT_TOOL_RESULT,
                json!({
                    "name": name,
                    "tool_call_id": tool_call_id,
                    "result": result,
                    "ok": ok,
                    "duration_ms": duration_ms
                }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Content { text } => Some(ClientEvent::new(
                events::AGENT_MESSAGE,
                json!({ "text": text }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Done { usage } => {
                // Send done event
                let event = ClientEvent::new(
                    events::AGENT_DONE,
                    json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "tool_uses": usage.tool_uses
                    }),
                    Some(session_id.to_string()),
                );
                send_event(tx, &event);

                // Send success response
                let response = ClientResponse::ok(
                    req_id,
                    json!({
                        "status": "completed",
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        }
                    }),
                );
                send_response(tx, &response);
                None
            }
            AgentEventType::Error { code, message } => {
                // Send error event
                let event = ClientEvent::new(
                    events::AGENT_ERROR,
                    json!({ "code": code, "message": message }),
                    Some(session_id.to_string()),
                );
                send_event(tx, &event);

                // Send error response
                let response = ClientResponse::error(req_id, &code, &message);
                send_response(tx, &response);
                None
            }
            AgentEventType::AwaitingInput {
                tool_call_id,
                questions,
            } => Some(ClientEvent::new(
                events::AGENT_AWAITING_INPUT,
                json!({ "tool_call_id": tool_call_id, "questions": questions }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Yield {
                turn_id,
                reason,
                tool_call_id,
                tool_name,
                tool_args,
                questions,
                policy_rule,
            } => {
                // Map yield events to appropriate client events
                let (event_name, data) = match reason {
                    YieldReason::AwaitingApproval => (
                        events::AGENT_AWAITING_APPROVAL,
                        json!({
                            "turn_id": turn_id,
                            "tool_call_id": tool_call_id,
                            "tool_name": tool_name,
                            "tool_args": tool_args,
                            "policy_rule": policy_rule
                        }),
                    ),
                    YieldReason::AwaitingInput => (
                        events::AGENT_AWAITING_INPUT,
                        json!({
                            "turn_id": turn_id,
                            "tool_call_id": tool_call_id,
                            "questions": questions
                        }),
                    ),
                };

                // Send yield event but don't send response yet (turn is paused)
                let event = ClientEvent::new(event_name, data, Some(session_id.to_string()));
                send_event(tx, &event);

                // Send "yielded" response to indicate turn paused
                let response = ClientResponse::ok(
                    req_id,
                    json!({
                        "status": "yielded",
                        "turn_id": turn_id,
                        "reason": match reason {
                            YieldReason::AwaitingApproval => "awaiting_approval",
                            YieldReason::AwaitingInput => "awaiting_input",
                        }
                    }),
                );
                send_response(tx, &response);
                None
            }
            AgentEventType::Pong => None,
        };

        if let Some(event) = client_event {
            send_event(tx, &event);
        }
    }

    Ok(())
}

async fn handle_turn_resume(
    state: &Arc<GatewayState>,
    session_id: &str,
    req_id: &str,
    params: Value,
    tx: &mpsc::UnboundedSender<ClientMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let turn_id = params.get("turn_id").and_then(|t| t.as_str()).unwrap_or("");

    if turn_id.is_empty() {
        let response = ClientResponse::error(req_id, "invalid_params", "Missing turn_id parameter");
        send_response(tx, &response);
        return Ok(());
    }

    let tool_call_id = params
        .get("tool_call_id")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let response_type = params
        .get("response_type")
        .and_then(|t| t.as_str())
        .unwrap_or("approval");

    // Build resume data based on response type
    let resume_data = match response_type {
        "approval" => {
            let approved = params
                .get("approved")
                .and_then(|a| a.as_bool())
                .unwrap_or(false);
            ResumeData {
                turn_id: turn_id.to_string(),
                tool_call_id,
                approved: Some(approved),
                answers: None,
            }
        }
        "answers" => {
            let answers = params.get("answers").cloned().unwrap_or(json!({}));
            ResumeData {
                turn_id: turn_id.to_string(),
                tool_call_id,
                approved: None,
                answers: Some(answers),
            }
        }
        _ => {
            let response = ClientResponse::error(
                req_id,
                "invalid_params",
                &format!("Invalid response_type: {}", response_type),
            );
            send_response(tx, &response);
            return Ok(());
        }
    };

    // Build agent request
    let agent_request = AgentRequest::resume_turn(req_id, session_id, resume_data);

    // Send to agent and stream events back
    let mut event_rx = state.agent.send_request(agent_request).await?;

    // Forward agent events as client events (same as chat_send)
    while let Some(agent_event) = event_rx.recv().await {
        let client_event = match agent_event.event {
            AgentEventType::Thinking { content } => Some(ClientEvent::new(
                events::AGENT_THINKING,
                json!({ "content": content }),
                Some(session_id.to_string()),
            )),
            AgentEventType::ToolCall {
                name,
                args,
                tool_call_id,
            } => Some(ClientEvent::new(
                events::AGENT_TOOL_CALL,
                json!({ "name": name, "args": args, "tool_call_id": tool_call_id }),
                Some(session_id.to_string()),
            )),
            AgentEventType::ToolResult {
                name,
                tool_call_id,
                result,
                ok,
                duration_ms,
            } => Some(ClientEvent::new(
                events::AGENT_TOOL_RESULT,
                json!({
                    "name": name,
                    "tool_call_id": tool_call_id,
                    "result": result,
                    "ok": ok,
                    "duration_ms": duration_ms
                }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Content { text } => Some(ClientEvent::new(
                events::AGENT_MESSAGE,
                json!({ "text": text }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Done { usage } => {
                let event = ClientEvent::new(
                    events::AGENT_DONE,
                    json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "tool_uses": usage.tool_uses
                    }),
                    Some(session_id.to_string()),
                );
                send_event(tx, &event);

                let response = ClientResponse::ok(
                    req_id,
                    json!({
                        "status": "completed",
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        }
                    }),
                );
                send_response(tx, &response);
                None
            }
            AgentEventType::Error { code, message } => {
                let event = ClientEvent::new(
                    events::AGENT_ERROR,
                    json!({ "code": code, "message": message }),
                    Some(session_id.to_string()),
                );
                send_event(tx, &event);

                let response = ClientResponse::error(req_id, &code, &message);
                send_response(tx, &response);
                None
            }
            AgentEventType::AwaitingInput {
                tool_call_id,
                questions,
            } => Some(ClientEvent::new(
                events::AGENT_AWAITING_INPUT,
                json!({ "tool_call_id": tool_call_id, "questions": questions }),
                Some(session_id.to_string()),
            )),
            AgentEventType::Yield {
                turn_id,
                reason,
                tool_call_id,
                tool_name,
                tool_args,
                questions,
                policy_rule,
            } => {
                let (event_name, data) = match reason {
                    YieldReason::AwaitingApproval => (
                        events::AGENT_AWAITING_APPROVAL,
                        json!({
                            "turn_id": turn_id,
                            "tool_call_id": tool_call_id,
                            "tool_name": tool_name,
                            "tool_args": tool_args,
                            "policy_rule": policy_rule
                        }),
                    ),
                    YieldReason::AwaitingInput => (
                        events::AGENT_AWAITING_INPUT,
                        json!({
                            "turn_id": turn_id,
                            "tool_call_id": tool_call_id,
                            "questions": questions
                        }),
                    ),
                };

                let event = ClientEvent::new(event_name, data, Some(session_id.to_string()));
                send_event(tx, &event);

                let response = ClientResponse::ok(
                    req_id,
                    json!({
                        "status": "yielded",
                        "turn_id": turn_id,
                        "reason": match reason {
                            YieldReason::AwaitingApproval => "awaiting_approval",
                            YieldReason::AwaitingInput => "awaiting_input",
                        }
                    }),
                );
                send_response(tx, &response);
                None
            }
            AgentEventType::Pong => None,
        };

        if let Some(event) = client_event {
            send_event(tx, &event);
        }
    }

    Ok(())
}

fn send_response(tx: &mpsc::UnboundedSender<ClientMessage>, response: &ClientResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = tx.send(ClientMessage { json });
    }
}

fn send_event(tx: &mpsc::UnboundedSender<ClientMessage>, event: &ClientEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        let _ = tx.send(ClientMessage { json });
    }
}
