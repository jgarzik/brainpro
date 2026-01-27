//! Agent connection - Unix socket client to communicate with agent daemon.

use crate::protocol::internal::{AgentEvent, AgentRequest};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use tokio::sync::mpsc;

/// Connection to the agent daemon
pub struct AgentConnection {
    socket_path: String,
}

impl AgentConnection {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }

    /// Send a request and stream events back
    /// Returns a channel receiver for streaming events
    pub fn send_request(
        &self,
        request: AgentRequest,
    ) -> Result<mpsc::UnboundedReceiver<AgentEvent>, std::io::Error> {
        let socket_path = self.socket_path.clone();
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn blocking task to handle the Unix socket communication
        std::thread::spawn(move || {
            if let Err(e) = send_request_blocking(&socket_path, request, tx) {
                eprintln!("[gateway] Agent connection error: {}", e);
            }
        });

        Ok(rx)
    }

    /// Check if agent daemon is available
    pub fn is_available(&self) -> bool {
        Path::new(&self.socket_path).exists()
    }

    /// Send a ping to check agent health
    pub fn ping(&self) -> Result<bool, std::io::Error> {
        let request = AgentRequest::ping(&uuid::Uuid::new_v4().to_string());
        let mut stream = UnixStream::connect(&self.socket_path)?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

        // Send request
        let json = serde_json::to_string(&request)?;
        writeln!(stream, "{}", json)?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        // Parse response
        if let Ok(event) = serde_json::from_str::<AgentEvent>(&line) {
            Ok(matches!(event.event, crate::protocol::internal::AgentEventType::Pong))
        } else {
            Ok(false)
        }
    }
}

fn send_request_blocking(
    socket_path: &str,
    request: AgentRequest,
    tx: mpsc::UnboundedSender<AgentEvent>,
) -> Result<(), std::io::Error> {
    let stream = UnixStream::connect(socket_path)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Send request
    let json = serde_json::to_string(&request).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    })?;
    writeln!(writer, "{}", json)?;
    writer.flush()?;

    // Read streaming events
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            // Connection closed
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse event
        match serde_json::from_str::<AgentEvent>(trimmed) {
            Ok(event) => {
                let is_terminal = matches!(
                    &event.event,
                    crate::protocol::internal::AgentEventType::Done { .. }
                        | crate::protocol::internal::AgentEventType::Error { .. }
                        | crate::protocol::internal::AgentEventType::Yield { .. }
                );

                if tx.send(event).is_err() {
                    // Receiver dropped
                    break;
                }

                if is_terminal {
                    break;
                }
            }
            Err(e) => {
                eprintln!("[gateway] Failed to parse agent event: {}", e);
            }
        }
    }

    Ok(())
}

/// Async wrapper for AgentConnection using tokio
pub struct AsyncAgentConnection {
    inner: AgentConnection,
}

impl AsyncAgentConnection {
    pub fn new(socket_path: &str) -> Self {
        Self {
            inner: AgentConnection::new(socket_path),
        }
    }

    /// Send a request and get a stream of events
    pub async fn send_request(
        &self,
        request: AgentRequest,
    ) -> Result<mpsc::UnboundedReceiver<AgentEvent>, std::io::Error> {
        // Use spawn_blocking for the synchronous Unix socket work
        let socket_path = self.inner.socket_path.clone();
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::task::spawn_blocking(move || {
            if let Err(e) = send_request_blocking(&socket_path, request, tx) {
                eprintln!("[gateway] Agent connection error: {}", e);
            }
        });

        Ok(rx)
    }

    /// Check if agent is available
    pub fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    /// Ping the agent (blocking, use sparingly)
    pub async fn ping(&self) -> Result<bool, std::io::Error> {
        let socket_path = self.inner.socket_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = AgentConnection::new(&socket_path);
            conn.ping()
        })
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?
    }
}
