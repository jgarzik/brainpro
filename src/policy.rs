//! Policy engine for permission decisions on tool calls.
//!
//! This module implements the rule-based permission system with allow/ask/deny rules
//! and three modes: Default, AcceptEdits, and BypassPermissions.

use crate::config::{PermissionMode, PermissionsConfig};
use serde_json::Value;
use std::io::{self, Write};

/// Permission decision result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    Ask,
}

/// Tool category for default behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// Read-only tools: Read, Grep, Glob
    ReadOnly,
    /// Mutation tools: Write, Edit
    Mutation,
    /// Execution tools: Bash
    Execution,
}

impl ToolCategory {
    /// Determine the category of a tool by name
    pub fn from_tool_name(name: &str) -> Self {
        match name {
            "Read" | "Grep" | "Glob" => ToolCategory::ReadOnly,
            "Write" | "Edit" => ToolCategory::Mutation,
            "Bash" => ToolCategory::Execution,
            _ if name.starts_with("mcp.") => ToolCategory::Execution, // MCP tools require permission
            _ => ToolCategory::Execution, // Unknown tools require permission
        }
    }
}

/// Default deny rules that are always applied
const DEFAULT_DENY_PATTERNS: &[&str] = &["Bash(curl:*)", "Bash(wget:*)"];

/// The policy engine that makes permission decisions
pub struct PolicyEngine {
    config: PermissionsConfig,
    print_mode: bool,
    auto_yes: bool,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new(config: PermissionsConfig, print_mode: bool, auto_yes: bool) -> Self {
        Self {
            config,
            print_mode,
            auto_yes,
        }
    }

    /// Get the current permission mode
    pub fn mode(&self) -> PermissionMode {
        self.config.mode
    }

    /// Set the permission mode
    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.config.mode = mode;
    }

    /// Get a reference to the config
    pub fn config(&self) -> &PermissionsConfig {
        &self.config
    }

    /// Get a mutable reference to the config
    pub fn config_mut(&mut self) -> &mut PermissionsConfig {
        &mut self.config
    }

    /// Extract the primary argument for rule matching from tool args
    /// For Bash: the command string
    /// For Write/Edit/Read: the path
    /// For Grep/Glob: the pattern
    fn extract_tool_arg(tool: &str, args: &Value) -> Option<String> {
        match tool {
            "Bash" => args
                .get("command")
                .and_then(|v| v.as_str())
                .map(String::from),
            "Write" | "Edit" | "Read" => {
                args.get("path").and_then(|v| v.as_str()).map(String::from)
            }
            "Grep" | "Glob" => args
                .get("pattern")
                .and_then(|v| v.as_str())
                .map(String::from),
            _ => None,
        }
    }

    /// Check if a rule pattern matches a tool call
    /// Pattern format: "ToolName" or "ToolName(prefix:*)" or "mcp.*" or "mcp.server.*"
    fn rule_matches(pattern: &str, tool: &str, arg: Option<&str>) -> bool {
        crate::tool_filter::tool_matches(tool, pattern, arg)
    }

    /// Determine the permission decision for a tool call
    /// Returns (Decision, Option<matched_rule>)
    pub fn decide(&self, tool: &str, args: &Value) -> (Decision, Option<String>) {
        let arg = Self::extract_tool_arg(tool, args);
        let arg_ref = arg.as_deref();

        // 1. Check default deny rules first (highest priority)
        for pattern in DEFAULT_DENY_PATTERNS {
            if Self::rule_matches(pattern, tool, arg_ref) {
                return (Decision::Deny, Some(pattern.to_string()));
            }
        }

        // 2. Check user deny rules
        for rule in &self.config.deny {
            if Self::rule_matches(rule, tool, arg_ref) {
                return (Decision::Deny, Some(rule.clone()));
            }
        }

        // 3. Check ask rules
        for rule in &self.config.ask {
            if Self::rule_matches(rule, tool, arg_ref) {
                return (Decision::Ask, Some(rule.clone()));
            }
        }

        // 4. Check allow rules
        for rule in &self.config.allow {
            if Self::rule_matches(rule, tool, arg_ref) {
                return (Decision::Allow, Some(rule.clone()));
            }
        }

        // 5. Apply mode-based defaults
        let decision = match self.config.mode {
            PermissionMode::BypassPermissions => Decision::Allow,
            PermissionMode::AcceptEdits => match ToolCategory::from_tool_name(tool) {
                ToolCategory::ReadOnly => Decision::Allow,
                ToolCategory::Mutation => Decision::Allow,
                ToolCategory::Execution => Decision::Ask,
            },
            PermissionMode::Default => match ToolCategory::from_tool_name(tool) {
                ToolCategory::ReadOnly => Decision::Allow,
                ToolCategory::Mutation => Decision::Ask,
                ToolCategory::Execution => Decision::Ask,
            },
        };

        (decision, None)
    }

    /// Check permission and prompt if needed
    /// Returns true if the action is allowed
    pub fn check_permission(&self, tool: &str, args: &Value) -> (bool, Decision, Option<String>) {
        let (decision, rule) = self.decide(tool, args);

        let allowed = match decision {
            Decision::Allow => true,
            Decision::Deny => {
                let arg = Self::extract_tool_arg(tool, args).unwrap_or_default();
                eprintln!(
                    "Permission denied: {}({}) - denied by policy{}",
                    tool,
                    arg,
                    rule.as_ref()
                        .map(|r| format!(" (rule: {})", r))
                        .unwrap_or_default()
                );
                false
            }
            Decision::Ask => self.prompt_user(tool, args),
        };

        (allowed, decision, rule)
    }

    /// Prompt the user for permission
    fn prompt_user(&self, tool: &str, args: &Value) -> bool {
        let arg = Self::extract_tool_arg(tool, args).unwrap_or_else(|| "?".to_string());

        // In print mode without --yes, deny
        if self.print_mode && !self.auto_yes {
            eprintln!(
                "Permission denied: {}({}) - use --yes in -p mode",
                tool, arg
            );
            return false;
        }

        // With --yes, auto-approve
        if self.auto_yes {
            return true;
        }

        // Interactive prompt
        println!("Permission required: {}(\"{}\")", tool, arg);

        // Show summary for specific tools
        match tool {
            "Edit" => {
                if let Some(old) = args.get("old_string").and_then(|v| v.as_str()) {
                    let preview = if old.len() > 60 {
                        format!("{}...", &old[..60])
                    } else {
                        old.to_string()
                    };
                    println!("  Replacing: \"{}\"", preview);
                }
            }
            "Bash" => {
                println!("  Command: {}", arg);
            }
            "Write" => {
                println!("  Action: create/overwrite file");
            }
            _ => {}
        }

        print!("Allow? [y/N]: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim().to_lowercase();
            input == "y" || input == "yes"
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn default_engine() -> PolicyEngine {
        PolicyEngine::new(PermissionsConfig::default(), false, false)
    }

    #[test]
    fn test_tool_category() {
        assert_eq!(ToolCategory::from_tool_name("Read"), ToolCategory::ReadOnly);
        assert_eq!(ToolCategory::from_tool_name("Grep"), ToolCategory::ReadOnly);
        assert_eq!(
            ToolCategory::from_tool_name("Write"),
            ToolCategory::Mutation
        );
        assert_eq!(ToolCategory::from_tool_name("Edit"), ToolCategory::Mutation);
        assert_eq!(
            ToolCategory::from_tool_name("Bash"),
            ToolCategory::Execution
        );
    }

    #[test]
    fn test_rule_matching_simple() {
        assert!(PolicyEngine::rule_matches("Write", "Write", None));
        assert!(PolicyEngine::rule_matches(
            "Write",
            "Write",
            Some("foo.txt")
        ));
        assert!(!PolicyEngine::rule_matches("Write", "Read", None));
    }

    #[test]
    fn test_rule_matching_prefix() {
        assert!(PolicyEngine::rule_matches(
            "Bash(git diff:*)",
            "Bash",
            Some("git diff HEAD")
        ));
        assert!(PolicyEngine::rule_matches(
            "Bash(git diff:*)",
            "Bash",
            Some("git diff --stat")
        ));
        assert!(!PolicyEngine::rule_matches(
            "Bash(git diff:*)",
            "Bash",
            Some("git status")
        ));
        assert!(!PolicyEngine::rule_matches(
            "Bash(git diff:*)",
            "Bash",
            None
        ));
    }

    #[test]
    fn test_rule_matching_exact() {
        assert!(PolicyEngine::rule_matches(
            "Edit(src/main.rs)",
            "Edit",
            Some("src/main.rs")
        ));
        assert!(!PolicyEngine::rule_matches(
            "Edit(src/main.rs)",
            "Edit",
            Some("src/lib.rs")
        ));
    }

    #[test]
    fn test_default_deny_curl() {
        let engine = default_engine();
        let (decision, rule) =
            engine.decide("Bash", &json!({"command": "curl https://example.com"}));
        assert_eq!(decision, Decision::Deny);
        assert!(rule.is_some());
    }

    #[test]
    fn test_default_deny_wget() {
        let engine = default_engine();
        let (decision, _) = engine.decide("Bash", &json!({"command": "wget https://example.com"}));
        assert_eq!(decision, Decision::Deny);
    }

    #[test]
    fn test_default_mode_read_allowed() {
        let engine = default_engine();
        let (decision, _) = engine.decide("Read", &json!({"path": "src/main.rs"}));
        assert_eq!(decision, Decision::Allow);
    }

    #[test]
    fn test_default_mode_write_asks() {
        let engine = default_engine();
        let (decision, _) = engine.decide("Write", &json!({"path": "foo.txt"}));
        assert_eq!(decision, Decision::Ask);
    }

    #[test]
    fn test_accept_edits_mode() {
        let mut config = PermissionsConfig::default();
        config.mode = PermissionMode::AcceptEdits;
        let engine = PolicyEngine::new(config, false, false);

        let (decision, _) = engine.decide("Write", &json!({"path": "foo.txt"}));
        assert_eq!(decision, Decision::Allow);

        let (decision, _) = engine.decide("Edit", &json!({"path": "foo.txt"}));
        assert_eq!(decision, Decision::Allow);

        // Bash still asks in AcceptEdits mode
        let (decision, _) = engine.decide("Bash", &json!({"command": "cargo test"}));
        assert_eq!(decision, Decision::Ask);
    }

    #[test]
    fn test_allow_rule_overrides_default() {
        let mut config = PermissionsConfig::default();
        config.allow.push("Bash(cargo test:*)".to_string());
        let engine = PolicyEngine::new(config, false, false);

        let (decision, rule) = engine.decide("Bash", &json!({"command": "cargo test"}));
        assert_eq!(decision, Decision::Allow);
        assert_eq!(rule.as_deref(), Some("Bash(cargo test:*)"));
    }

    #[test]
    fn test_ask_rule_overrides_allow() {
        let mut config = PermissionsConfig::default();
        config.allow.push("Bash(git:*)".to_string());
        config.ask.push("Bash(git push:*)".to_string());
        let engine = PolicyEngine::new(config, false, false);

        // git diff should be allowed
        let (decision, _) = engine.decide("Bash", &json!({"command": "git diff"}));
        assert_eq!(decision, Decision::Allow);

        // git push should ask (ask overrides allow)
        let (decision, _) = engine.decide("Bash", &json!({"command": "git push origin main"}));
        assert_eq!(decision, Decision::Ask);
    }

    #[test]
    fn test_deny_rule_highest_priority() {
        let mut config = PermissionsConfig::default();
        config.allow.push("Bash(rm:*)".to_string());
        config.deny.push("Bash(rm -rf:*)".to_string());
        let engine = PolicyEngine::new(config, false, false);

        // rm should be allowed
        let (decision, _) = engine.decide("Bash", &json!({"command": "rm foo.txt"}));
        assert_eq!(decision, Decision::Allow);

        // rm -rf should be denied
        let (decision, _) = engine.decide("Bash", &json!({"command": "rm -rf /"}));
        assert_eq!(decision, Decision::Deny);
    }

    #[test]
    fn test_mcp_wildcard_all() {
        // Pattern "mcp.*" should match any MCP tool
        assert!(PolicyEngine::rule_matches("mcp.*", "mcp.echo.add", None));
        assert!(PolicyEngine::rule_matches("mcp.*", "mcp.echo.echo", None));
        assert!(PolicyEngine::rule_matches("mcp.*", "mcp.git.status", None));
        assert!(!PolicyEngine::rule_matches("mcp.*", "Write", None));
        assert!(!PolicyEngine::rule_matches("mcp.*", "Bash", None));
    }

    #[test]
    fn test_mcp_wildcard_server() {
        // Pattern "mcp.echo.*" should match any tool from echo server
        assert!(PolicyEngine::rule_matches(
            "mcp.echo.*",
            "mcp.echo.add",
            None
        ));
        assert!(PolicyEngine::rule_matches(
            "mcp.echo.*",
            "mcp.echo.echo",
            None
        ));
        assert!(!PolicyEngine::rule_matches(
            "mcp.echo.*",
            "mcp.git.status",
            None
        ));
        assert!(!PolicyEngine::rule_matches(
            "mcp.echo.*",
            "mcp.echoserver.add",
            None
        )); // Partial match should fail
    }

    #[test]
    fn test_mcp_tool_category() {
        assert_eq!(
            ToolCategory::from_tool_name("mcp.echo.add"),
            ToolCategory::Execution
        );
        assert_eq!(
            ToolCategory::from_tool_name("mcp.git.status"),
            ToolCategory::Execution
        );
    }

    #[test]
    fn test_mcp_default_asks() {
        let engine = default_engine();
        // MCP tools should ask by default (since they're Execution category)
        let (decision, _) = engine.decide("mcp.echo.add", &json!({}));
        assert_eq!(decision, Decision::Ask);
    }

    #[test]
    fn test_mcp_allow_wildcard() {
        let mut config = PermissionsConfig::default();
        config.allow.push("mcp.echo.*".to_string());
        let engine = PolicyEngine::new(config, false, false);

        // Tools from echo server should be allowed
        let (decision, rule) = engine.decide("mcp.echo.add", &json!({}));
        assert_eq!(decision, Decision::Allow);
        assert_eq!(rule.as_deref(), Some("mcp.echo.*"));

        // Tools from other servers should still ask
        let (decision, _) = engine.decide("mcp.git.status", &json!({}));
        assert_eq!(decision, Decision::Ask);
    }
}
