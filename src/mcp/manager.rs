//! MCP server lifecycle manager.

use super::client::McpClient;
use super::transport::{HttpTransport, McpTransportImpl, SseTransport, StdioTransport};
use super::McpToolDef;
use crate::config::{McpServerConfig, McpTransport};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

/// Result of an MCP tool call
pub struct McpToolResult {
    pub server: String,
    pub tool: String,
    pub ok: bool,
    pub duration_ms: u64,
    pub truncated: bool,
    pub data: Value,
}

/// MCP server manager - handles server lifecycle and tool dispatch
pub struct McpManager {
    configs: HashMap<String, McpServerConfig>,
    clients: HashMap<String, McpClient>,
    tools: HashMap<String, Vec<McpToolDef>>,
    connected_servers: Vec<String>,
}

impl McpManager {
    /// Create a new MCP manager with server configurations
    pub fn new(configs: HashMap<String, McpServerConfig>) -> Self {
        Self {
            configs,
            clients: HashMap::new(),
            tools: HashMap::new(),
            connected_servers: Vec::new(),
        }
    }

    /// List all configured servers with their status
    pub fn list_servers(&self) -> Vec<(&String, &McpServerConfig, bool)> {
        self.configs
            .iter()
            .map(|(name, config)| (name, config, self.clients.contains_key(name)))
            .collect()
    }

    /// Check if a server is connected
    pub fn is_connected(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    /// Connect to an MCP server by name
    /// Returns (pid_or_0, tool_count) - pid is 0 for HTTP/SSE transports
    pub fn connect(&mut self, name: &str, root: &Path) -> Result<(u32, usize)> {
        // Check if already connected
        if self.clients.contains_key(name) {
            return Err(anyhow::anyhow!("Server {} is already connected", name));
        }

        let config = self
            .configs
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown MCP server: {}", name))?
            .clone();

        if !config.enabled {
            return Err(anyhow::anyhow!("Server {} is disabled", name));
        }

        // Create appropriate transport based on config
        let (transport, pid) = match config.transport {
            McpTransport::Stdio => {
                // Resolve cwd relative to project root
                let cwd = root.join(&config.cwd);
                let t = StdioTransport::spawn(&config.command, &config.args, &config.env, &cwd)?;
                let pid = t.pid();
                (McpTransportImpl::Stdio(t), pid)
            }
            McpTransport::Http => {
                let url = config
                    .url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("HTTP transport requires 'url' in config"))?;
                let t = HttpTransport::new(url, config.timeout_ms);
                (McpTransportImpl::Http(t), 0)
            }
            McpTransport::Sse => {
                let url = config
                    .url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("SSE transport requires 'url' in config"))?;
                let t = SseTransport::new(url, config.timeout_ms);
                (McpTransportImpl::Sse(t), 0)
            }
        };

        let mut client = McpClient::with_transport(transport);

        // Initialize connection
        client.initialize()?;

        // List tools
        let tools = client.list_tools(name)?;
        let tool_count = tools.len();

        self.tools.insert(name.to_string(), tools);
        self.clients.insert(name.to_string(), client);
        self.connected_servers.push(name.to_string());

        Ok((pid, tool_count))
    }

    /// Disconnect from an MCP server
    pub fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(mut client) = self.clients.remove(name) {
            client.shutdown()?;
        }
        self.tools.remove(name);
        self.connected_servers.retain(|s| s != name);
        Ok(())
    }

    /// Get tools for a specific server
    pub fn get_server_tools(&self, name: &str) -> Vec<&McpToolDef> {
        self.tools
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Get all tools from all connected servers
    pub fn get_all_tools(&self) -> Vec<&McpToolDef> {
        self.tools.values().flatten().collect()
    }

    /// Check if any servers are connected
    pub fn has_connected_servers(&self) -> bool {
        !self.connected_servers.is_empty()
    }

    /// Check health of a specific server, returning exit status if dead
    pub fn check_server_health(&mut self, name: &str) -> Option<i32> {
        if let Some(client) = self.clients.get_mut(name) {
            if !client.is_alive() {
                let exit_status = client.exit_status();
                // Clean up dead server
                self.clients.remove(name);
                self.tools.remove(name);
                self.connected_servers.retain(|s| s != name);
                return exit_status;
            }
        }
        None
    }

    /// Call an MCP tool by its full namespaced name (e.g., "mcp.echo.add")
    pub fn call_tool(&mut self, full_name: &str, args: Value) -> Result<McpToolResult> {
        // Parse "mcp.server.tool" format
        let parts: Vec<&str> = full_name.splitn(3, '.').collect();
        if parts.len() != 3 || parts[0] != "mcp" {
            return Err(anyhow::anyhow!("Invalid MCP tool name: {}", full_name));
        }

        let server = parts[1];
        let tool = parts[2];

        let client = self
            .clients
            .get_mut(server)
            .ok_or_else(|| anyhow::anyhow!("MCP server not connected: {}", server))?;

        // Check if server is still alive
        if !client.is_alive() {
            let exit_status = client.exit_status();
            self.clients.remove(server);
            self.tools.remove(server);
            self.connected_servers.retain(|s| s != server);
            return Err(anyhow::anyhow!(
                "MCP server {} has died (exit status: {:?})",
                server,
                exit_status
            ));
        }

        let start = Instant::now();
        let result = client.call_tool(tool, args);
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(data) => {
                // Check if result needs truncation (200KB limit)
                let data_str = serde_json::to_string(&data).unwrap_or_default();
                let truncated = data_str.len() > 200_000;
                let final_data = if truncated {
                    serde_json::json!({
                        "result": format!("{}... [truncated]", &data_str[..200_000]),
                        "truncated": true
                    })
                } else {
                    data
                };

                Ok(McpToolResult {
                    server: server.to_string(),
                    tool: tool.to_string(),
                    ok: true,
                    duration_ms,
                    truncated,
                    data: final_data,
                })
            }
            Err(e) => Ok(McpToolResult {
                server: server.to_string(),
                tool: tool.to_string(),
                ok: false,
                duration_ms,
                truncated: false,
                data: serde_json::json!({
                    "error": {
                        "code": "mcp_error",
                        "message": e.to_string()
                    }
                }),
            }),
        }
    }
}
