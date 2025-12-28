//! Transport layer for MCP server communication.
//!
//! Supports three transport types:
//! - Stdio: Spawns MCP servers as subprocesses (newline-delimited JSON)
//! - HTTP: Communicates via HTTP POST requests
//! - SSE: Server-Sent Events for streaming responses

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Stdio transport for communicating with an MCP server subprocess
pub struct StdioTransport {
    pub child: Child,
    pub stdin: ChildStdin,
    pub response_rx: Receiver<Value>,
    reader_handle: Option<JoinHandle<()>>,
}

impl StdioTransport {
    /// Spawn an MCP server subprocess and set up communication channels
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: &Path,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(cwd)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Let server errors show in terminal

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {}", command))?;

        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = child.stdout.take().expect("Failed to get stdout");

        let (tx, rx) = mpsc::channel();

        // Spawn reader thread to process stdout
        let reader_handle = thread::spawn(move || {
            Self::reader_loop(stdout, tx);
        });

        Ok(Self {
            child,
            stdin,
            response_rx: rx,
            reader_handle: Some(reader_handle),
        })
    }

    /// Reader loop that processes newline-delimited JSON from stdout
    fn reader_loop(stdout: ChildStdout, tx: Sender<Value>) {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) if !line.is_empty() => {
                    match serde_json::from_str(&line) {
                        Ok(msg) => {
                            if tx.send(msg).is_err() {
                                // Receiver dropped, exit loop
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("MCP: Failed to parse JSON from server: {}", e);
                            eprintln!("MCP: Line was: {}", line);
                        }
                    }
                }
                Err(_) => break, // Pipe closed
                _ => {}
            }
        }
    }

    /// Send a JSON-RPC message to the MCP server
    pub fn send(&mut self, message: &Value) -> Result<()> {
        let json = serde_json::to_string(message)?;
        writeln!(self.stdin, "{}", json).context("Failed to write to MCP server stdin")?;
        self.stdin
            .flush()
            .context("Failed to flush MCP server stdin")?;
        Ok(())
    }

    /// Receive a response with timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Value> {
        self.response_rx
            .recv_timeout(timeout)
            .map_err(|e| anyhow::anyhow!("Receive timeout: {}", e))
    }

    /// Check if the server process is still alive
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // Process exited
            Ok(None) => true,     // Still running
            Err(_) => false,      // Error checking status
        }
    }

    /// Get the process ID of the child
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Get exit status if the process has exited
    pub fn exit_status(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        }
    }

    /// Kill the server process
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("Failed to kill MCP server")?;
        self.child.wait().context("Failed to wait for MCP server")?;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Attempt to kill the child process if still running
        let _ = self.child.kill();
        let _ = self.child.wait();

        // Wait for reader thread to finish
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

/// HTTP transport for communicating with an MCP server over HTTP
pub struct HttpTransport {
    url: String,
    agent: ureq::Agent,
    timeout: Duration,
}

impl HttpTransport {
    /// Create a new HTTP transport
    pub fn new(url: &str, timeout_ms: u64) -> Self {
        Self {
            url: url.to_string(),
            agent: ureq::Agent::new(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Send a JSON-RPC message and receive response
    pub fn send(&self, message: &Value) -> Result<Value> {
        let resp = self
            .agent
            .post(&self.url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .send_json(message.clone());

        match resp {
            Ok(r) => {
                let body: Value = r.into_json()?;
                Ok(body)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(anyhow::anyhow!("HTTP error {}: {}", code, body))
            }
            Err(e) => Err(anyhow::anyhow!("HTTP request failed: {}", e)),
        }
    }

    /// HTTP transport is always "alive" since it's stateless
    pub fn is_alive(&self) -> bool {
        true
    }
}

/// SSE (Server-Sent Events) transport for MCP servers
/// Uses HTTP POST for requests and SSE for streaming responses
pub struct SseTransport {
    url: String,
    agent: ureq::Agent,
    timeout: Duration,
}

impl SseTransport {
    /// Create a new SSE transport
    pub fn new(url: &str, timeout_ms: u64) -> Self {
        Self {
            url: url.to_string(),
            agent: ureq::Agent::new(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Send a JSON-RPC message and wait for response via SSE
    pub fn send(&self, message: &Value) -> Result<Value> {
        // For SSE, we send the request and then listen for events
        // The request includes a unique ID that we match in the response
        let request_id = message.get("id").and_then(|v| v.as_u64());

        // Send the request via POST
        let resp = self
            .agent
            .post(&self.url)
            .timeout(self.timeout)
            .set("Content-Type", "application/json")
            .send_json(message.clone());

        match resp {
            Ok(r) => {
                // For simple SSE implementations, the response comes back directly
                let body: Value = r.into_json()?;
                Ok(body)
            }
            Err(ureq::Error::Status(code, resp)) => {
                // Try to get SSE response from the event endpoint
                // Some servers send the response on a separate event stream
                self.try_sse_fallback(request_id, code, resp)
            }
            Err(e) => Err(anyhow::anyhow!("SSE request failed: {}", e)),
        }
    }

    fn try_sse_fallback(
        &self,
        request_id: Option<u64>,
        http_code: u16,
        resp: ureq::Response,
    ) -> Result<Value> {
        // If the POST returned a redirect or the response is chunked,
        // try to read it as SSE
        let content_type = resp.header("content-type").unwrap_or("").to_lowercase();

        if content_type.contains("text/event-stream") {
            // Parse SSE events
            let mut reader = BufReader::new(resp.into_reader());
            let mut line = String::new();
            let mut data = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let line = line.trim();
                        if let Some(stripped) = line.strip_prefix("data:") {
                            data = stripped.trim().to_string();
                        } else if line.is_empty() && !data.is_empty() {
                            // End of event, parse the data
                            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                                // Check if this is the response we're waiting for
                                if let Some(id) = request_id {
                                    if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                                        return Ok(value);
                                    }
                                } else {
                                    return Ok(value);
                                }
                            }
                            data.clear();
                        }
                    }
                    Err(e) => return Err(anyhow::anyhow!("SSE read error: {}", e)),
                }
            }
        }

        Err(anyhow::anyhow!(
            "HTTP error {}: SSE fallback failed",
            http_code
        ))
    }

    /// SSE transport is always "alive" since it's stateless
    pub fn is_alive(&self) -> bool {
        true
    }
}

/// Unified transport enum for MCP communication
pub enum McpTransportImpl {
    Stdio(StdioTransport),
    Http(HttpTransport),
    Sse(SseTransport),
}

impl McpTransportImpl {
    /// Send a message and receive response
    pub fn send(&mut self, message: &Value) -> Result<Value> {
        match self {
            McpTransportImpl::Stdio(t) => {
                t.send(message)?;
                t.recv_timeout(Duration::from_secs(30))
            }
            McpTransportImpl::Http(t) => t.send(message),
            McpTransportImpl::Sse(t) => t.send(message),
        }
    }

    /// Check if the transport is alive
    pub fn is_alive(&mut self) -> bool {
        match self {
            McpTransportImpl::Stdio(t) => t.is_alive(),
            McpTransportImpl::Http(t) => t.is_alive(),
            McpTransportImpl::Sse(t) => t.is_alive(),
        }
    }

    /// Get exit status (only for stdio)
    pub fn exit_status(&mut self) -> Option<i32> {
        match self {
            McpTransportImpl::Stdio(t) => t.exit_status(),
            _ => None,
        }
    }

    /// Kill the transport (only affects stdio)
    pub fn kill(&mut self) -> Result<()> {
        match self {
            McpTransportImpl::Stdio(t) => t.kill(),
            _ => Ok(()), // HTTP/SSE are stateless
        }
    }
}
