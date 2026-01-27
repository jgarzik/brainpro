//! Gateway client for connecting CLI to gateway via WebSocket.

use crate::protocol::client::methods;
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write as IoWrite};
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Run the CLI in gateway client mode
pub fn run_gateway_mode(gateway_url: &str, prompt: Option<&str>, auto_approve: bool) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async { run_gateway_async(gateway_url, prompt, auto_approve).await })
}

async fn run_gateway_async(
    gateway_url: &str,
    prompt: Option<&str>,
    auto_approve: bool,
) -> Result<()> {
    eprintln!("[gateway-client] Connecting to {}...", gateway_url);

    // Connect to WebSocket
    let (ws_stream, _) = connect_async(gateway_url)
        .await
        .map_err(|e| anyhow!("Failed to connect to gateway: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    // Send Hello
    let device_id = uuid::Uuid::new_v4().to_string();
    let hello = json!({
        "type": "hello",
        "role": "operator",
        "device_id": device_id,
        "caps": {
            "tools": [],
            "protocol_version": 1
        }
    });

    write
        .send(Message::Text(hello.to_string()))
        .await
        .map_err(|e| anyhow!("Failed to send hello: {}", e))?;

    // Wait for Welcome
    let welcome = loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                let msg: Value = serde_json::from_str(&text)?;
                if msg.get("type").and_then(|t| t.as_str()) == Some("welcome") {
                    break msg;
                }
            }
            Some(Err(e)) => return Err(anyhow!("WebSocket error: {}", e)),
            None => return Err(anyhow!("Connection closed before welcome")),
            _ => {}
        }
    };

    let session_id = welcome
        .get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    eprintln!("[gateway-client] Connected, session: {}", session_id);

    if let Some(prompt) = prompt {
        // One-shot mode
        run_prompt(&mut write, &mut read, prompt, auto_approve).await?;
    } else {
        // REPL mode
        run_repl(&mut write, &mut read, auto_approve).await?;
    }

    Ok(())
}

type WsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

type WsRead = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

async fn run_prompt(
    write: &mut WsWrite,
    read: &mut WsRead,
    prompt: &str,
    auto_approve: bool,
) -> Result<()> {
    send_chat_and_stream(write, read, prompt, auto_approve).await
}

async fn run_repl(write: &mut WsWrite, read: &mut WsRead, auto_approve: bool) -> Result<()> {
    println!("brainpro (gateway mode) - type /exit to quit");

    loop {
        print!(">>> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "/exit" || input == "/quit" {
            break;
        }

        if let Err(e) = send_chat_and_stream(write, read, input, auto_approve).await {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

async fn send_chat_and_stream(
    write: &mut WsWrite,
    read: &mut WsRead,
    message: &str,
    auto_approve: bool,
) -> Result<()> {
    let start = Instant::now();
    let req_id = uuid::Uuid::new_v4().to_string();

    // Send chat.send request
    let request = json!({
        "type": "req",
        "id": req_id,
        "method": methods::CHAT_SEND,
        "params": {
            "message": message
        }
    });

    write
        .send(Message::Text(request.to_string()))
        .await
        .map_err(|e| anyhow!("Failed to send request: {}", e))?;

    // Stream responses, handling yields
    stream_response(write, read, &req_id, start, auto_approve).await
}

/// Stream response events, handling yields by prompting user and sending resume
async fn stream_response(
    write: &mut WsWrite,
    read: &mut WsRead,
    _req_id: &str,
    start: Instant,
    auto_approve: bool,
) -> Result<()> {
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut tool_uses = 0u64;

    loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                let msg: Value = serde_json::from_str(&text)?;
                let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match msg_type {
                    "event" => {
                        let event_name = msg.get("event").and_then(|e| e.as_str()).unwrap_or("");
                        let data = msg.get("data").cloned().unwrap_or(json!({}));

                        match event_name {
                            "agent.thinking" => {
                                if let Some(content) = data.get("content").and_then(|c| c.as_str())
                                {
                                    if !content.is_empty() {
                                        println!("{}", content);
                                    }
                                }
                            }
                            "agent.tool_call" => {
                                tool_uses += 1;
                                let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                                let args = data.get("args").cloned().unwrap_or(json!({}));
                                eprintln!("⏺ {}({})", name, format_args_brief(&args));
                            }
                            "agent.tool_result" => {
                                let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                                let ok = data.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                                let symbol = if ok { "✓" } else { "✗" };
                                eprintln!("  ⎿ {} {}", symbol, name);
                            }
                            "agent.message" => {
                                if let Some(text) = data.get("text").and_then(|t| t.as_str()) {
                                    println!("{}", text);
                                }
                            }
                            "agent.done" => {
                                input_tokens = data
                                    .get("input_tokens")
                                    .and_then(|t| t.as_u64())
                                    .unwrap_or(0);
                                output_tokens = data
                                    .get("output_tokens")
                                    .and_then(|t| t.as_u64())
                                    .unwrap_or(0);
                            }
                            "agent.error" => {
                                let code =
                                    data.get("code").and_then(|c| c.as_str()).unwrap_or("error");
                                let message = data
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown error");
                                eprintln!("Error [{}]: {}", code, message);
                            }
                            "agent.awaiting_approval" => {
                                // Prompt user for tool approval
                                let turn_id =
                                    data.get("turn_id").and_then(|t| t.as_str()).unwrap_or("");
                                let tool_call_id = data
                                    .get("tool_call_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let tool_name = data
                                    .get("tool_name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("?");
                                let tool_args = data.get("tool_args").cloned().unwrap_or(json!({}));

                                let approved = if auto_approve {
                                    eprintln!(
                                        "⚠ Auto-approved: {}({})",
                                        tool_name,
                                        format_args_brief(&tool_args)
                                    );
                                    true
                                } else {
                                    eprintln!(
                                        "⚠ Permission required: {}({})",
                                        tool_name,
                                        format_args_brief(&tool_args)
                                    );

                                    // Show more details for certain tools
                                    if tool_name == "Bash" {
                                        if let Some(cmd) =
                                            tool_args.get("command").and_then(|c| c.as_str())
                                        {
                                            eprintln!("  Command: {}", cmd);
                                        }
                                    } else if tool_name == "Write" || tool_name == "Edit" {
                                        if let Some(path) =
                                            tool_args.get("file_path").and_then(|p| p.as_str())
                                        {
                                            eprintln!("  File: {}", path);
                                        }
                                    }

                                    prompt_approval()
                                };

                                // Send resume request
                                let resume_req_id = uuid::Uuid::new_v4().to_string();
                                let resume_request = json!({
                                    "type": "req",
                                    "id": resume_req_id,
                                    "method": methods::TURN_RESUME,
                                    "params": {
                                        "turn_id": turn_id,
                                        "tool_call_id": tool_call_id,
                                        "response_type": "approval",
                                        "approved": approved
                                    }
                                });

                                write
                                    .send(Message::Text(resume_request.to_string()))
                                    .await
                                    .map_err(|e| anyhow!("Failed to send resume: {}", e))?;

                                // Continue streaming with new request ID
                                // (The loop will continue processing events)
                            }
                            "agent.awaiting_input" => {
                                // Prompt user for answers
                                let turn_id =
                                    data.get("turn_id").and_then(|t| t.as_str()).unwrap_or("");
                                let tool_call_id = data
                                    .get("tool_call_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let questions = data
                                    .get("questions")
                                    .and_then(|q| q.as_array())
                                    .cloned()
                                    .unwrap_or_default();

                                let answers = prompt_questions(&questions);

                                // Send resume request with answers
                                let resume_req_id = uuid::Uuid::new_v4().to_string();
                                let resume_request = json!({
                                    "type": "req",
                                    "id": resume_req_id,
                                    "method": methods::TURN_RESUME,
                                    "params": {
                                        "turn_id": turn_id,
                                        "tool_call_id": tool_call_id,
                                        "response_type": "answers",
                                        "answers": answers
                                    }
                                });

                                write
                                    .send(Message::Text(resume_request.to_string()))
                                    .await
                                    .map_err(|e| anyhow!("Failed to send resume: {}", e))?;

                                // Continue streaming
                            }
                            _ => {}
                        }
                    }
                    "res" => {
                        // Check if this is a yield response (turn paused)
                        let status = msg
                            .get("payload")
                            .and_then(|p| p.get("status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");

                        if status == "yielded" {
                            // Turn is paused, waiting for user input
                            // The awaiting_* event handler above will send resume
                            // Continue processing events
                            continue;
                        }

                        // Response received (completed or error), we're done
                        let duration = start.elapsed();
                        let total_tokens = input_tokens + output_tokens;
                        let token_display = if total_tokens >= 1000 {
                            format!("{:.1}k", total_tokens as f64 / 1000.0)
                        } else {
                            total_tokens.to_string()
                        };
                        eprintln!(
                            "[Duration: {:.1}s | Tokens: {} | Tools: {}]",
                            duration.as_secs_f64(),
                            token_display,
                            tool_uses
                        );
                        break;
                    }
                    _ => {}
                }
            }
            Some(Ok(Message::Close(_))) => {
                return Err(anyhow!("Connection closed by server"));
            }
            Some(Err(e)) => {
                return Err(anyhow!("WebSocket error: {}", e));
            }
            None => {
                return Err(anyhow!("Connection closed"));
            }
            _ => {}
        }
    }

    Ok(())
}

/// Prompt user for tool approval
fn prompt_approval() -> bool {
    print!("Allow? [y/N]: ");
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim().to_lowercase();
        input == "y" || input == "yes"
    } else {
        false
    }
}

/// Prompt user for question answers
fn prompt_questions(questions: &[Value]) -> Value {
    let mut answers = json!({});

    for (i, q) in questions.iter().enumerate() {
        let question = q.get("question").and_then(|q| q.as_str()).unwrap_or("?");
        let header = q.get("header").and_then(|h| h.as_str()).unwrap_or("");
        let options = q
            .get("options")
            .and_then(|o| o.as_array())
            .cloned()
            .unwrap_or_default();
        let multi_select = q
            .get("multi_select")
            .and_then(|m| m.as_bool())
            .unwrap_or(false);

        println!("\n[{}] {}", header, question);

        for (j, opt) in options.iter().enumerate() {
            let label = opt.get("label").and_then(|l| l.as_str()).unwrap_or("?");
            let desc = opt
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            println!("  {}. {} - {}", j + 1, label, desc);
        }
        println!("  {}. Other (type your answer)", options.len() + 1);

        print!(
            "Choice{}: ",
            if multi_select {
                "(s, comma-separated)"
            } else {
                ""
            }
        );
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            continue;
        }
        let input = input.trim();

        let key = format!("q{}", i);

        if multi_select {
            // Parse comma-separated choices
            let choices: Vec<String> = input
                .split(',')
                .filter_map(|s| {
                    let s = s.trim();
                    if let Ok(idx) = s.parse::<usize>() {
                        if idx > 0 && idx <= options.len() {
                            return options
                                .get(idx - 1)
                                .and_then(|o| o.get("label"))
                                .and_then(|l| l.as_str())
                                .map(|s| s.to_string());
                        } else if idx == options.len() + 1 {
                            // Other - prompt for custom input
                            print!("Enter custom answer: ");
                            io::stdout().flush().ok();
                            let mut custom = String::new();
                            if io::stdin().read_line(&mut custom).is_ok() {
                                return Some(custom.trim().to_string());
                            }
                        }
                    }
                    None
                })
                .collect();
            answers[&key] = json!(choices);
        } else {
            // Single choice
            if let Ok(idx) = input.parse::<usize>() {
                if idx > 0 && idx <= options.len() {
                    if let Some(label) = options
                        .get(idx - 1)
                        .and_then(|o| o.get("label"))
                        .and_then(|l| l.as_str())
                    {
                        answers[&key] = json!(label);
                    }
                } else if idx == options.len() + 1 {
                    // Other - prompt for custom input
                    print!("Enter custom answer: ");
                    io::stdout().flush().ok();
                    let mut custom = String::new();
                    if io::stdin().read_line(&mut custom).is_ok() {
                        answers[&key] = json!(custom.trim());
                    }
                }
            } else {
                // Assume direct text input
                answers[&key] = json!(input);
            }
        }
    }

    answers
}

fn format_args_brief(args: &Value) -> String {
    match args {
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(2)
                .map(|(k, v)| {
                    let v_str = match v {
                        Value::String(s) => {
                            if s.len() > 30 {
                                format!("\"{}...\"", &s[..27])
                            } else {
                                format!("\"{}\"", s)
                            }
                        }
                        _ => v.to_string(),
                    };
                    format!("{}: {}", k, v_str)
                })
                .collect();
            parts.join(", ")
        }
        _ => args.to_string(),
    }
}
