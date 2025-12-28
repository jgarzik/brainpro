//! MCP JSON-RPC client for protocol communication.

use super::transport::McpTransportImpl;
use super::McpToolDef;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// JSON-RPC request structure
#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC response structure
#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// MCP client for communicating with an MCP server
pub struct McpClient {
    transport: McpTransportImpl,
    request_id: AtomicU64,
}

impl McpClient {
    /// Create a new MCP client with any transport type
    pub fn with_transport(transport: McpTransportImpl) -> Self {
        Self {
            transport,
            request_id: AtomicU64::new(1),
        }
    }

    /// Generate next request ID
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Send a JSON-RPC request and wait for response
    fn call(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let response_value = self.transport.send(&serde_json::to_value(&request)?)?;
        let response: JsonRpcResponse = serde_json::from_value(response_value)?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!(
                "MCP error {}: {}",
                error.code,
                error.message
            ));
        }
        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Perform MCP initialize handshake
    pub fn initialize(&mut self) -> Result<Value> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "yo",
                "version": "0.1.0"
            }
        });

        let result = self.call("initialize", Some(params))?;

        // Send initialized notification (no response expected)
        // For HTTP/SSE this is a fire-and-forget, result is ignored
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let _ = self.transport.send(&notification);

        Ok(result)
    }

    /// List available tools from the MCP server
    pub fn list_tools(&mut self, server_name: &str) -> Result<Vec<McpToolDef>> {
        let result = self.call("tools/list", None)?;

        let tools = result["tools"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid tools response: missing tools array"))?;

        let mcp_tools: Vec<McpToolDef> = tools
            .iter()
            .filter_map(|t| {
                let name = t["name"].as_str()?.to_string();
                Some(McpToolDef {
                    server: server_name.to_string(),
                    full_name: format!("mcp.{}.{}", server_name, name),
                    name,
                    description: t["description"].as_str().unwrap_or("").to_string(),
                    input_schema: t["inputSchema"].clone(),
                })
            })
            .collect();

        Ok(mcp_tools)
    }

    /// Call a tool on the MCP server
    pub fn call_tool(&mut self, tool_name: &str, args: Value) -> Result<Value> {
        let params = json!({
            "name": tool_name,
            "arguments": args
        });

        let result = self.call("tools/call", Some(params))?;

        // Extract content from MCP tool response
        // MCP returns: { "content": [{ "type": "text", "text": "..." }] }
        if let Some(content) = result.get("content") {
            if let Some(array) = content.as_array() {
                if let Some(first) = array.first() {
                    if first.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(text) = first.get("text") {
                            return Ok(json!({
                                "result": text
                            }));
                        }
                    }
                }
            }
        }

        // Return raw result if not in expected format
        Ok(result)
    }

    /// Check if the server process is still alive
    pub fn is_alive(&mut self) -> bool {
        self.transport.is_alive()
    }

    /// Get exit status if the process has exited
    pub fn exit_status(&mut self) -> Option<i32> {
        self.transport.exit_status()
    }

    /// Shutdown the server
    pub fn shutdown(&mut self) -> Result<()> {
        self.transport.kill()
    }
}
