use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Transcript {
    pub path: PathBuf,
    session_id: String,
    cwd: PathBuf,
    file: File,
}

#[derive(Serialize)]
struct Event<'a> {
    ts: DateTime<Utc>,
    session_id: &'a str,
    cwd: &'a Path,
    #[serde(rename = "type")]
    event_type: &'a str,
    #[serde(flatten)]
    data: serde_json::Value,
}

impl Transcript {
    pub fn new(path: &Path, session_id: &str, cwd: &Path) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            session_id: session_id.to_string(),
            cwd: cwd.to_path_buf(),
            file,
        })
    }

    pub fn log(&mut self, event_type: &str, data: serde_json::Value) -> Result<()> {
        let event = Event {
            ts: Utc::now(),
            session_id: &self.session_id,
            cwd: &self.cwd,
            event_type,
            data,
        };
        let line = serde_json::to_string(&event)?;
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        Ok(())
    }

    pub fn user_message(&mut self, content: &str) -> Result<()> {
        self.log("user_message", serde_json::json!({ "content": content }))
    }

    pub fn assistant_message(&mut self, content: &str) -> Result<()> {
        self.log(
            "assistant_message",
            serde_json::json!({ "content": content }),
        )
    }

    pub fn tool_call(&mut self, tool: &str, args: &serde_json::Value) -> Result<()> {
        self.log(
            "tool_call",
            serde_json::json!({ "tool": tool, "args": args }),
        )
    }

    pub fn tool_result(&mut self, tool: &str, ok: bool, result: &serde_json::Value) -> Result<()> {
        self.log(
            "tool_result",
            serde_json::json!({ "tool": tool, "ok": ok, "result": result }),
        )
    }

    #[allow(dead_code)]
    pub fn permission(&mut self, tool: &str, allowed: bool) -> Result<()> {
        self.log(
            "permission",
            serde_json::json!({ "tool": tool, "allowed": allowed }),
        )
    }

    /// Log a policy decision for a tool call
    pub fn policy_decision(
        &mut self,
        tool: &str,
        decision: &str,
        rule_matched: Option<&str>,
    ) -> Result<()> {
        self.log(
            "policy_decision",
            serde_json::json!({
                "tool": tool,
                "decision": decision,
                "rule_matched": rule_matched,
            }),
        )
    }

    /// Log a context compaction event
    #[allow(dead_code)]
    pub fn compact_event(
        &mut self,
        chars_before: usize,
        chars_after: usize,
        messages_before: usize,
        messages_after: usize,
    ) -> Result<()> {
        self.log(
            "compact_event",
            serde_json::json!({
                "chars_before": chars_before,
                "chars_after": chars_after,
                "messages_before": messages_before,
                "messages_after": messages_after,
            }),
        )
    }

    /// Log settings loaded at startup
    #[allow(dead_code)]
    pub fn settings_loaded(
        &mut self,
        config_files: &[String],
        permission_mode: &str,
        allow_rules: usize,
        ask_rules: usize,
        deny_rules: usize,
    ) -> Result<()> {
        self.log(
            "settings_loaded",
            serde_json::json!({
                "config_files": config_files,
                "permission_mode": permission_mode,
                "allow_rules": allow_rules,
                "ask_rules": ask_rules,
                "deny_rules": deny_rules,
            }),
        )
    }

    #[allow(dead_code)]
    pub fn error(&mut self, message: &str) -> Result<()> {
        self.log("error", serde_json::json!({ "message": message }))
    }

    /// Log MCP server start
    pub fn mcp_server_start(&mut self, name: &str, command: &str, pid: u32) -> Result<()> {
        self.log(
            "mcp_server_start",
            serde_json::json!({
                "name": name,
                "command": command,
                "pid": pid,
            }),
        )
    }

    /// Log MCP initialize success
    pub fn mcp_initialize_ok(&mut self, name: &str) -> Result<()> {
        self.log("mcp_initialize_ok", serde_json::json!({ "name": name }))
    }

    /// Log MCP initialize error
    pub fn mcp_initialize_err(&mut self, name: &str, error: &str) -> Result<()> {
        self.log(
            "mcp_initialize_err",
            serde_json::json!({
                "name": name,
                "error": error,
            }),
        )
    }

    /// Log MCP tools list discovery
    pub fn mcp_tools_list(&mut self, name: &str, count: usize) -> Result<()> {
        self.log(
            "mcp_tools_list",
            serde_json::json!({
                "name": name,
                "count": count,
            }),
        )
    }

    /// Log MCP tool call
    pub fn mcp_tool_call(
        &mut self,
        server: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> Result<()> {
        self.log(
            "mcp_tool_call",
            serde_json::json!({
                "name": server,
                "tool": tool,
                "args": args,
            }),
        )
    }

    /// Log MCP tool result
    pub fn mcp_tool_result(
        &mut self,
        server: &str,
        tool: &str,
        ok: bool,
        duration_ms: u64,
        truncated: bool,
    ) -> Result<()> {
        self.log(
            "mcp_tool_result",
            serde_json::json!({
                "name": server,
                "tool": tool,
                "ok": ok,
                "duration_ms": duration_ms,
                "truncated": truncated,
            }),
        )
    }

    /// Log MCP server stop
    pub fn mcp_server_stop(&mut self, name: &str) -> Result<()> {
        self.log("mcp_server_stop", serde_json::json!({ "name": name }))
    }

    /// Log MCP server died unexpectedly
    pub fn mcp_server_died(&mut self, name: &str, exit_status: Option<i32>) -> Result<()> {
        self.log(
            "mcp_server_died",
            serde_json::json!({
                "name": name,
                "exit_status": exit_status,
            }),
        )
    }

    /// Log subagent start
    pub fn subagent_start(
        &mut self,
        name: &str,
        effective_mode: &str,
        allowed_tools: &[String],
    ) -> Result<()> {
        self.log(
            "subagent_start",
            serde_json::json!({
                "name": name,
                "effective_mode": effective_mode,
                "allowed_tools": allowed_tools,
            }),
        )
    }

    /// Log subagent end
    pub fn subagent_end(&mut self, name: &str, ok: bool, duration_ms: u64) -> Result<()> {
        self.log(
            "subagent_end",
            serde_json::json!({
                "name": name,
                "ok": ok,
                "duration_ms": duration_ms,
            }),
        )
    }

    /// Log subagent tool call
    pub fn subagent_tool_call(
        &mut self,
        agent: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> Result<()> {
        self.log(
            "subagent_tool_call",
            serde_json::json!({
                "agent": agent,
                "tool": tool,
                "args": args,
            }),
        )
    }
}
