//! Gateway client for connecting CLI to gateway via WebSocket.

use crate::protocol::client::{events, methods, ClientCapabilities, ClientRole};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Run the CLI in gateway client mode
pub fn run_gateway_mode(gateway_url: &str, prompt: Option<&str>) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        run_gateway_async(gateway_url, prompt).await
    })
}

async fn run_gateway_async(gateway_url: &str, prompt: Option<&str>) -> Result<()> {
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
        .send(Message::Text(hello.to_string().into()))
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
        run_prompt(&mut write, &mut read, prompt).await?;
    } else {
        // REPL mode
        run_repl(&mut write, &mut read).await?;
    }

    Ok(())
}

async fn run_prompt(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    prompt: &str,
) -> Result<()> {
    send_chat_and_stream(write, read, prompt).await
}

async fn run_repl(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<()> {
    use std::io::{self, BufRead, Write};

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

        if let Err(e) = send_chat_and_stream(write, read, input).await {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

async fn send_chat_and_stream(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    message: &str,
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
        .send(Message::Text(request.to_string().into()))
        .await
        .map_err(|e| anyhow!("Failed to send request: {}", e))?;

    // Stream responses
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
                                input_tokens =
                                    data.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
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
                            _ => {}
                        }
                    }
                    "res" => {
                        // Response received, we're done
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
