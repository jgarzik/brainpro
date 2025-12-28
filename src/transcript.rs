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

    /// Log skill index built
    pub fn skill_index_built(&mut self, count: usize) -> Result<()> {
        self.log(
            "skill_index_built",
            serde_json::json!({
                "count": count,
            }),
        )
    }

    /// Log skill activation
    pub fn skill_activate(
        &mut self,
        name: &str,
        reason: Option<&str>,
        allowed_tools: Option<&Vec<String>>,
    ) -> Result<()> {
        self.log(
            "skill_activate",
            serde_json::json!({
                "name": name,
                "reason": reason,
                "allowed_tools": allowed_tools,
            }),
        )
    }

    /// Log skill deactivation
    pub fn skill_deactivate(&mut self, name: &str) -> Result<()> {
        self.log("skill_deactivate", serde_json::json!({ "name": name }))
    }

    /// Log skill parse error
    pub fn skill_parse_error(&mut self, path: &std::path::Path, error: &str) -> Result<()> {
        self.log(
            "skill_parse_error",
            serde_json::json!({
                "path": path.display().to_string(),
                "error": error,
            }),
        )
    }

    /// Log plan mode start
    pub fn plan_mode_start(&mut self, goal: &str) -> Result<()> {
        self.log("plan_mode_start", serde_json::json!({ "goal": goal }))
    }

    /// Log plan created
    pub fn plan_created(&mut self, name: &str, step_count: usize) -> Result<()> {
        self.log(
            "plan_created",
            serde_json::json!({
                "name": name,
                "step_count": step_count,
            }),
        )
    }

    /// Log plan step start
    pub fn plan_step_start(&mut self, plan: &str, step: usize, title: &str) -> Result<()> {
        self.log(
            "plan_step_start",
            serde_json::json!({
                "plan": plan,
                "step": step,
                "title": title,
            }),
        )
    }

    /// Log plan step end
    pub fn plan_step_end(&mut self, plan: &str, step: usize, status: &str) -> Result<()> {
        self.log(
            "plan_step_end",
            serde_json::json!({
                "plan": plan,
                "step": step,
                "status": status,
            }),
        )
    }

    /// Log plan saved
    pub fn plan_saved(&mut self, name: &str, path: &Path) -> Result<()> {
        self.log(
            "plan_saved",
            serde_json::json!({
                "name": name,
                "path": path.display().to_string(),
            }),
        )
    }

    /// Log plan loaded
    pub fn plan_loaded(&mut self, name: &str) -> Result<()> {
        self.log("plan_loaded", serde_json::json!({ "name": name }))
    }

    /// Log plan execution complete
    pub fn plan_complete(
        &mut self,
        name: &str,
        steps_completed: usize,
        steps_failed: usize,
    ) -> Result<()> {
        self.log(
            "plan_complete",
            serde_json::json!({
                "name": name,
                "steps_completed": steps_completed,
                "steps_failed": steps_failed,
            }),
        )
    }

    /// Log token usage for an LLM call
    pub fn token_usage(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    ) -> Result<()> {
        self.log(
            "token_usage",
            serde_json::json!({
                "model": model,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "total_tokens": input_tokens + output_tokens,
                "cost_usd": cost_usd,
            }),
        )
    }
}
