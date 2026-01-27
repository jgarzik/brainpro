//! Unix socket server for the agent daemon.
//! Listens for NDJSON requests and streams NDJSON events back.

use crate::agent_service::turn_state::TurnStateStore;
use crate::agent_service::worker::{self, WorkerConfig};
use crate::protocol::internal::{AgentEvent, AgentMethod, AgentRequest};
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
    /// Enable gateway mode (yields on ask decisions)
    pub gateway_mode: bool,
    /// Personality to use (mrcode or mrbot)
    pub personality: String,
}

impl Default for AgentServerConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/brainpro.sock".to_string(),
            max_concurrent: 4,
            gateway_mode: false,
            personality: "mrbot".to_string(),
        }
    }
}

/// The agent daemon server
pub struct AgentServer {
    config: AgentServerConfig,
    /// Track in-flight requests for cancellation
    in_flight: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
    /// Turn state store for yield/resume
    turn_store: Arc<TurnStateStore>,
}

impl AgentServer {
    pub fn new(config: AgentServerConfig) -> Self {
        let turn_store = Arc::new(TurnStateStore::default());

        // Start cleanup task
        TurnStateStore::start_cleanup_task(Arc::clone(&turn_store));

        Self {
            config,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            turn_store,
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
            "[agent] Listening on Unix socket: {} (gateway_mode={}, personality={})",
            self.config.socket_path, self.config.gateway_mode, self.config.personality
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let in_flight = Arc::clone(&self.in_flight);
                    let turn_store = Arc::clone(&self.turn_store);
                    let gateway_mode = self.config.gateway_mode;
                    let personality = self.config.personality.clone();
                    thread::spawn(move || {
                        if let Err(e) =
                            handle_connection(stream, in_flight, turn_store, gateway_mode, &personality)
                        {
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
    turn_store: Arc<TurnStateStore>,
    gateway_mode: bool,
    personality: &str,
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
        if request.method == AgentMethod::Cancel {
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

        // Create worker config
        let worker_config = WorkerConfig {
            gateway_mode,
            turn_store: Arc::clone(&turn_store),
            personality: personality.to_string(),
        };

        // Spawn worker and collect events
        let handle = worker::spawn_worker_with_config(request, worker_config);

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
                | crate::protocol::internal::AgentEventType::Error { .. }
                | crate::protocol::internal::AgentEventType::Yield { .. } => {
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Run the agent server with a custom socket path
pub fn run_with_socket(socket_path: &str) -> std::io::Result<()> {
    let server = AgentServer::new(AgentServerConfig {
        socket_path: socket_path.to_string(),
        ..Default::default()
    });
    server.run()
}

/// Run the agent server with gateway mode enabled
pub fn run_gateway_mode(socket_path: &str) -> std::io::Result<()> {
    let server = AgentServer::new(AgentServerConfig {
        socket_path: socket_path.to_string(),
        gateway_mode: true,
        ..Default::default()
    });
    server.run()
}

/// Run the agent server with a specific personality
pub fn run_with_personality(socket_path: &str, gateway_mode: bool, personality: &str) -> std::io::Result<()> {
    let server = AgentServer::new(AgentServerConfig {
        socket_path: socket_path.to_string(),
        gateway_mode,
        personality: personality.to_string(),
        ..Default::default()
    });
    server.run()
}
