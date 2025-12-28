//! Hook system for executing shell commands at lifecycle points.
//!
//! Implements Claude Code-compatible hooks with JSON input/output via stdin/stdout.
//! Exit codes: 0 = allow, 2 = block, other = warn (continue with warning).

use crate::config::{HookConfig, HookEvent};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Base session info included in all hook inputs
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub cwd: PathBuf,
}

/// Input for PreToolUse hook
#[derive(Debug, Clone, Serialize)]
pub struct PreToolUseInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub tool_name: String,
    pub tool_args: Value,
}

/// Input for PostToolUse hook
#[derive(Debug, Clone, Serialize)]
pub struct PostToolUseInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub tool_name: String,
    pub tool_args: Value,
    pub tool_result: Value,
    pub duration_ms: u64,
}

/// Input for UserPromptSubmit hook
#[derive(Debug, Clone, Serialize)]
pub struct UserPromptSubmitInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub prompt: String,
}

/// Input for Stop hook
#[derive(Debug, Clone, Serialize)]
pub struct StopInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub stop_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<String>,
}

/// Input for SubagentStop hook
#[derive(Debug, Clone, Serialize)]
pub struct SubagentStopInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub agent_name: String,
    pub ok: bool,
    pub output_text: String,
    pub duration_ms: u64,
}

/// Input for SessionStart hook
#[derive(Debug, Clone, Serialize)]
pub struct SessionStartInput {
    #[serde(flatten)]
    pub session: SessionInfo,
    pub hook_event: &'static str,
    pub mode: String,
}

/// Output from PreToolUse hook (parsed from stdout JSON)
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PreToolUseOutput {
    #[serde(default)]
    pub permission_decision: Option<String>, // "allow" | "deny"
    #[serde(default)]
    pub updated_args: Option<Value>,
}

/// Output from UserPromptSubmit hook
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserPromptSubmitOutput {
    #[serde(default)]
    pub decision: Option<String>, // "allow" | "block"
    #[serde(default)]
    pub updated_prompt: Option<String>,
}

/// Output from Stop hook
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StopOutput {
    #[serde(default)]
    pub force_continue: Option<bool>,
    #[serde(default)]
    pub continue_prompt: Option<String>,
}

/// Result of hook execution
#[derive(Debug)]
pub struct HookResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl HookResult {
    /// Check if the hook blocked the action (exit code 2)
    pub fn is_blocked(&self) -> bool {
        self.exit_code == Some(2)
    }

    /// Check if the hook allowed the action (exit code 0)
    pub fn is_allowed(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Manager for executing hooks at lifecycle points
pub struct HookManager {
    hooks: Vec<HookConfig>,
    session_info: SessionInfo,
}

impl HookManager {
    /// Create a new HookManager with the given configuration
    pub fn new(hooks: Vec<HookConfig>, session_id: String, cwd: PathBuf) -> Self {
        Self {
            hooks,
            session_info: SessionInfo { session_id, cwd },
        }
    }

    /// Get hooks for a specific event
    fn get_hooks(&self, event: HookEvent) -> Vec<&HookConfig> {
        self.hooks.iter().filter(|h| h.event == event).collect()
    }

    /// Check if a hook's matcher applies to a tool name
    fn matches_tool(&self, hook: &HookConfig, tool_name: &str) -> bool {
        match &hook.matcher {
            None => true, // No matcher = match all tools
            Some(pattern) => {
                // Try to compile as regex and match
                Regex::new(pattern)
                    .map(|re| re.is_match(tool_name))
                    .unwrap_or(false)
            }
        }
    }

    /// Execute a single hook with the given JSON input
    fn execute_hook<T: Serialize>(&self, hook: &HookConfig, input: &T) -> HookResult {
        let input_json = match serde_json::to_string(input) {
            Ok(json) => json,
            Err(e) => {
                return HookResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Failed to serialize input: {}", e),
                };
            }
        };

        // Build command
        let (cmd, args) = match hook.command.split_first() {
            Some((c, a)) => (c, a),
            None => {
                return HookResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: "Empty command".to_string(),
                };
            }
        };

        // Spawn process
        let mut child = match Command::new(cmd)
            .args(args)
            .current_dir(&self.session_info.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return HookResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Failed to spawn hook: {}", e),
                };
            }
        };

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input_json.as_bytes());
        }

        // Wait with timeout
        let timeout = Duration::from_millis(hook.timeout_ms);
        let status = match child.wait_timeout(timeout) {
            Ok(Some(s)) => s,
            Ok(None) => {
                // Timeout - kill the process
                let _ = child.kill();
                let _ = child.wait();
                return HookResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Hook timed out after {}ms", hook.timeout_ms),
                };
            }
            Err(e) => {
                return HookResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Wait error: {}", e),
                };
            }
        };

        // Read outputs
        let stdout = child
            .stdout
            .map(|mut out| {
                let mut buf = String::new();
                use std::io::Read;
                let _ = out.read_to_string(&mut buf);
                buf
            })
            .unwrap_or_default();

        let stderr = child
            .stderr
            .map(|mut err| {
                let mut buf = String::new();
                use std::io::Read;
                let _ = err.read_to_string(&mut buf);
                buf
            })
            .unwrap_or_default();

        HookResult {
            exit_code: status.code(),
            stdout,
            stderr,
        }
    }

    /// Run PreToolUse hooks for a tool call
    /// Returns (should_proceed, updated_args)
    pub fn pre_tool_use(&self, tool_name: &str, tool_args: &Value) -> (bool, Option<Value>) {
        let hooks: Vec<_> = self
            .get_hooks(HookEvent::PreToolUse)
            .into_iter()
            .filter(|h| self.matches_tool(h, tool_name))
            .collect();

        if hooks.is_empty() {
            return (true, None);
        }

        let input = PreToolUseInput {
            session: self.session_info.clone(),
            hook_event: "PreToolUse",
            tool_name: tool_name.to_string(),
            tool_args: tool_args.clone(),
        };

        let mut updated_args: Option<Value> = None;

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log hook execution (stderr goes to our stderr as warning)
            if !result.stderr.is_empty() {
                eprintln!("[Hook:PreToolUse] {}", result.stderr.trim());
            }

            // Exit code 2 = block
            if result.is_blocked() {
                return (false, None);
            }

            // Parse output for permission decision and updated args
            if result.is_allowed() && !result.stdout.is_empty() {
                if let Ok(output) = serde_json::from_str::<PreToolUseOutput>(&result.stdout) {
                    // Check permission decision
                    if let Some(decision) = &output.permission_decision {
                        if decision == "deny" {
                            return (false, None);
                        }
                    }
                    // Capture updated args (first one wins)
                    if updated_args.is_none() && output.updated_args.is_some() {
                        updated_args = output.updated_args;
                    }
                }
            }
        }

        (true, updated_args)
    }

    /// Run PostToolUse hooks for a completed tool call
    pub fn post_tool_use(
        &self,
        tool_name: &str,
        tool_args: &Value,
        tool_result: &Value,
        duration_ms: u64,
    ) {
        let hooks: Vec<_> = self
            .get_hooks(HookEvent::PostToolUse)
            .into_iter()
            .filter(|h| self.matches_tool(h, tool_name))
            .collect();

        if hooks.is_empty() {
            return;
        }

        let input = PostToolUseInput {
            session: self.session_info.clone(),
            hook_event: "PostToolUse",
            tool_name: tool_name.to_string(),
            tool_args: tool_args.clone(),
            tool_result: tool_result.clone(),
            duration_ms,
        };

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log any stderr output
            if !result.stderr.is_empty() {
                eprintln!("[Hook:PostToolUse] {}", result.stderr.trim());
            }
        }
    }

    /// Run UserPromptSubmit hooks
    /// Returns (should_proceed, updated_prompt)
    pub fn user_prompt_submit(&self, prompt: &str) -> (bool, Option<String>) {
        let hooks = self.get_hooks(HookEvent::UserPromptSubmit);

        if hooks.is_empty() {
            return (true, None);
        }

        let input = UserPromptSubmitInput {
            session: self.session_info.clone(),
            hook_event: "UserPromptSubmit",
            prompt: prompt.to_string(),
        };

        let mut updated_prompt: Option<String> = None;

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log any stderr output
            if !result.stderr.is_empty() {
                eprintln!("[Hook:UserPromptSubmit] {}", result.stderr.trim());
            }

            // Exit code 2 = block
            if result.is_blocked() {
                return (false, None);
            }

            // Parse output
            if result.is_allowed() && !result.stdout.is_empty() {
                if let Ok(output) = serde_json::from_str::<UserPromptSubmitOutput>(&result.stdout) {
                    if let Some(decision) = &output.decision {
                        if decision == "block" {
                            return (false, None);
                        }
                    }
                    if updated_prompt.is_none() && output.updated_prompt.is_some() {
                        updated_prompt = output.updated_prompt;
                    }
                }
            }
        }

        (true, updated_prompt)
    }

    /// Run Stop hooks
    /// Returns (force_continue, continue_prompt)
    pub fn on_stop(
        &self,
        stop_reason: &str,
        assistant_message: Option<&str>,
    ) -> (bool, Option<String>) {
        let hooks = self.get_hooks(HookEvent::Stop);

        if hooks.is_empty() {
            return (false, None);
        }

        let input = StopInput {
            session: self.session_info.clone(),
            hook_event: "Stop",
            stop_reason: stop_reason.to_string(),
            assistant_message: assistant_message.map(|s| s.to_string()),
        };

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log any stderr output
            if !result.stderr.is_empty() {
                eprintln!("[Hook:Stop] {}", result.stderr.trim());
            }

            // Parse output
            if result.is_allowed() && !result.stdout.is_empty() {
                if let Ok(output) = serde_json::from_str::<StopOutput>(&result.stdout) {
                    if output.force_continue == Some(true) {
                        return (true, output.continue_prompt);
                    }
                }
            }
        }

        (false, None)
    }

    /// Run SubagentStop hooks
    pub fn on_subagent_stop(
        &self,
        agent_name: &str,
        ok: bool,
        output_text: &str,
        duration_ms: u64,
    ) {
        let hooks = self.get_hooks(HookEvent::SubagentStop);

        if hooks.is_empty() {
            return;
        }

        let input = SubagentStopInput {
            session: self.session_info.clone(),
            hook_event: "SubagentStop",
            agent_name: agent_name.to_string(),
            ok,
            output_text: output_text.to_string(),
            duration_ms,
        };

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log any stderr output
            if !result.stderr.is_empty() {
                eprintln!("[Hook:SubagentStop] {}", result.stderr.trim());
            }
        }
    }

    /// Run SessionStart hooks
    pub fn on_session_start(&self, mode: &str) {
        let hooks = self.get_hooks(HookEvent::SessionStart);

        if hooks.is_empty() {
            return;
        }

        let input = SessionStartInput {
            session: self.session_info.clone(),
            hook_event: "SessionStart",
            mode: mode.to_string(),
        };

        for hook in hooks {
            let result = self.execute_hook(hook, &input);

            // Log any stderr output
            if !result.stderr.is_empty() {
                eprintln!("[Hook:SessionStart] {}", result.stderr.trim());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_manager_no_hooks() {
        let manager = HookManager::new(Vec::new(), "test-session".to_string(), PathBuf::from("."));

        // With no hooks, should always proceed
        let (proceed, updated) = manager.pre_tool_use("Bash", &serde_json::json!({}));
        assert!(proceed);
        assert!(updated.is_none());

        let (proceed, updated) = manager.user_prompt_submit("test prompt");
        assert!(proceed);
        assert!(updated.is_none());
    }

    #[test]
    fn test_matcher_none_matches_all() {
        let hook = HookConfig {
            event: HookEvent::PreToolUse,
            command: vec!["true".to_string()],
            matcher: None,
            timeout_ms: 1000,
        };

        let manager = HookManager::new(
            vec![hook.clone()],
            "test-session".to_string(),
            PathBuf::from("."),
        );

        assert!(manager.matches_tool(&hook, "Bash"));
        assert!(manager.matches_tool(&hook, "Read"));
        assert!(manager.matches_tool(&hook, "Write"));
    }

    #[test]
    fn test_matcher_exact() {
        let hook = HookConfig {
            event: HookEvent::PreToolUse,
            command: vec!["true".to_string()],
            matcher: Some("^Bash$".to_string()),
            timeout_ms: 1000,
        };

        let manager = HookManager::new(
            vec![hook.clone()],
            "test-session".to_string(),
            PathBuf::from("."),
        );

        assert!(manager.matches_tool(&hook, "Bash"));
        assert!(!manager.matches_tool(&hook, "Read"));
        assert!(!manager.matches_tool(&hook, "BashStuff"));
    }

    #[test]
    fn test_matcher_regex() {
        let hook = HookConfig {
            event: HookEvent::PreToolUse,
            command: vec!["true".to_string()],
            matcher: Some("^(Read|Write|Edit)$".to_string()),
            timeout_ms: 1000,
        };

        let manager = HookManager::new(
            vec![hook.clone()],
            "test-session".to_string(),
            PathBuf::from("."),
        );

        assert!(manager.matches_tool(&hook, "Read"));
        assert!(manager.matches_tool(&hook, "Write"));
        assert!(manager.matches_tool(&hook, "Edit"));
        assert!(!manager.matches_tool(&hook, "Bash"));
        assert!(!manager.matches_tool(&hook, "Grep"));
    }
}
