//! Unix socket server for the agent daemon.
//! Listens for NDJSON requests and streams NDJSON events back.

use crate::agent_service::worker;
use crate::protocol::internal::{AgentEvent, AgentRequest};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

/// Configuration for the agent server
pub struct AgentServerConfig {
    /// Path to the Unix socket
    pub socket_path: String,
    /// Maximum concurrent requests
    pub max_concurrent: usize,
}

impl Default for AgentServerConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/brainpro.sock".to_string(),
            max_concurrent: 4,
        }
    }
}

/// The agent daemon server
pub struct AgentServer {
    config: AgentServerConfig,
    /// Track in-flight requests for cancellation
    in_flight: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
}

impl AgentServer {
    pub fn new(config: AgentServerConfig) -> Self {
        Self {
            config,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Run the server (blocking)
    pub fn run(&self) -> std::io::Result<()> {
        // Remove existing socket if present
        let socket_path = Path::new(&self.config.socket_path);
        if socket_path.exists() {
            std::fs::remove_file(socket_path)?;
        }

        // Create parent directory if needed
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(socket_path)?;
        eprintln!(
            "[agent] Listening on Unix socket: {}",
            self.config.socket_path
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let in_flight = Arc::clone(&self.in_flight);
                    thread::spawn(move || {
                        if let Err(e) = handle_connection(stream, in_flight) {
                            eprintln!("[agent] Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[agent] Accept error: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// Handle a single connection from the gateway
fn handle_connection(
    stream: UnixStream,
    in_flight: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            // Connection closed
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse the request
        let request: AgentRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                let error = AgentEvent::error("unknown", "parse_error", &e.to_string());
                let _ = writer.write_all(error.to_ndjson().as_bytes());
                continue;
            }
        };

        let request_id = request.id.clone();

        // Handle cancellation requests specially
        if request.method == crate::protocol::internal::AgentMethod::Cancel {
            let mut flights = in_flight.lock().unwrap();
            if let Some(cancel_tx) = flights.remove(&request.session_id) {
                let _ = cancel_tx.send(());
                let event = AgentEvent::done(
                    &request_id,
                    crate::protocol::internal::UsageStats::default(),
                );
                let _ = writer.write_all(event.to_ndjson().as_bytes());
            } else {
                let event = AgentEvent::error(
                    &request_id,
                    "not_found",
                    "No in-flight request to cancel",
                );
                let _ = writer.write_all(event.to_ndjson().as_bytes());
            }
            continue;
        }

        // Spawn worker and collect events
        let handle = worker::spawn_worker(request);

        // Stream events back to the client
        for event in handle.events {
            let json = event.to_ndjson();
            if let Err(e) = writer.write_all(json.as_bytes()) {
                eprintln!("[agent] Write error: {}", e);
                break;
            }
            if let Err(e) = writer.flush() {
                eprintln!("[agent] Flush error: {}", e);
                break;
            }

            // Check if this was the terminal event
            match &event.event {
                crate::protocol::internal::AgentEventType::Done { .. }
                | crate::protocol::internal::AgentEventType::Error { .. } => {
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Run the agent server with default configuration
pub fn run_default() -> std::io::Result<()> {
    let server = AgentServer::new(AgentServerConfig::default());
    server.run()
}

/// Run the agent server with a custom socket path
pub fn run_with_socket(socket_path: &str) -> std::io::Result<()> {
    let server = AgentServer::new(AgentServerConfig {
        socket_path: socket_path.to_string(),
        ..Default::default()
    });
    server.run()
}
