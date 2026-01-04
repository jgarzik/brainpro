//! Tool display formatting for nice CLI output.
//!
//! Formats tool calls and results similar to Claude Code's output style:
//! ```
//! ⏺ Read(path: "src/main.rs")
//!   ⎿  Read 40 lines
//! ```

use serde_json::Value;

/// Format a tool call for display.
/// Returns something like: `⏺ Read(path: "src/main.rs")`
pub fn format_tool_call(name: &str, args: &Value) -> String {
    let params = format_args_inline(name, args);
    if params.is_empty() {
        format!("⏺ {}", name)
    } else {
        format!("⏺ {}({})", name, params)
    }
}

/// Format tool arguments inline for display.
fn format_args_inline(name: &str, args: &Value) -> String {
    match name {
        "Read" => {
            let mut parts = Vec::new();
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                parts.push(format!("path: \"{}\"", path));
            }
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                if offset > 0 {
                    parts.push(format!("offset: {}", offset));
                }
            }
            if let Some(max_bytes) = args.get("max_bytes").and_then(|v| v.as_u64()) {
                parts.push(format!("max_bytes: {}", max_bytes));
            }
            parts.join(", ")
        }
        "Write" => {
            let mut parts = Vec::new();
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                parts.push(format!("path: \"{}\"", path));
            }
            parts.join(", ")
        }
        "Edit" => {
            let mut parts = Vec::new();
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                parts.push(format!("path: \"{}\"", path));
            }
            if let Some(edits) = args.get("edits").and_then(|v| v.as_array()) {
                parts.push(format!("edits: {}", edits.len()));
            }
            parts.join(", ")
        }
        "Bash" => {
            let mut parts = Vec::new();
            if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                // Truncate long commands
                let display_cmd = if cmd.len() > 60 {
                    format!("{}...", &cmd[..57])
                } else {
                    cmd.to_string()
                };
                parts.push(format!("command: \"{}\"", display_cmd));
            }
            parts.join(", ")
        }
        "Glob" => {
            let mut parts = Vec::new();
            if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                parts.push(format!("pattern: \"{}\"", pattern));
            }
            parts.join(", ")
        }
        "Grep" => {
            let mut parts = Vec::new();
            if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                parts.push(format!("pattern: \"{}\"", pattern));
            }
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                parts.push(format!("path: \"{}\"", path));
            }
            parts.join(", ")
        }
        "Search" => {
            let mut parts = Vec::new();
            if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                parts.push(format!("pattern: \"{}\"", pattern));
            }
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                parts.push(format!("path: \"{}\"", path));
            }
            if let Some(mode) = args.get("output_mode").and_then(|v| v.as_str()) {
                if mode != "files_with_matches" {
                    parts.push(format!("output_mode: \"{}\"", mode));
                }
            }
            parts.join(", ")
        }
        "Task" => {
            let mut parts = Vec::new();
            if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                parts.push(format!("description: \"{}\"", desc));
            }
            parts.join(", ")
        }
        "ActivateSkill" => {
            let mut parts = Vec::new();
            if let Some(skill) = args.get("name").and_then(|v| v.as_str()) {
                parts.push(format!("name: \"{}\"", skill));
            }
            parts.join(", ")
        }
        _ => {
            // For unknown tools or MCP tools, show first few key params
            if let Some(obj) = args.as_object() {
                let parts: Vec<String> = obj
                    .iter()
                    .take(3)
                    .filter_map(|(k, v)| {
                        if let Some(s) = v.as_str() {
                            let display = if s.len() > 40 {
                                format!("{}...", &s[..37])
                            } else {
                                s.to_string()
                            };
                            Some(format!("{}: \"{}\"", k, display))
                        } else if let Some(n) = v.as_i64() {
                            Some(format!("{}: {}", k, n))
                        } else {
                            v.as_bool().map(|b| format!("{}: {}", k, b))
                        }
                    })
                    .collect();
                parts.join(", ")
            } else {
                String::new()
            }
        }
    }
}

/// Format a tool result for display.
/// Returns something like: `  ⎿  Read 40 lines`
pub fn format_tool_result(name: &str, result: &Value) -> String {
    // Check for errors first
    if let Some(error) = result.get("error") {
        let msg = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        let code = error
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("error");
        return format!("  ⎿  Error [{}]: {}", code, truncate_str(msg, 60));
    }

    match name {
        "Read" => {
            let lines = result.get("lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let truncated = result
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if truncated {
                format!("  ⎿  Read {} lines (truncated)", lines)
            } else {
                format!("  ⎿  Read {} lines", lines)
            }
        }
        "Write" => {
            let lines = result.get("lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let bytes = result
                .get("bytes_written")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            format!("  ⎿  Wrote {} lines ({} bytes)", lines, bytes)
        }
        "Edit" => {
            let applied = result.get("applied").and_then(|v| v.as_u64()).unwrap_or(0);
            if applied == 1 {
                "  ⎿  Applied 1 edit".to_string()
            } else {
                format!("  ⎿  Applied {} edits", applied)
            }
        }
        "Bash" => {
            let exit_code = result.get("exit_code").and_then(|v| v.as_i64());
            let duration = result
                .get("duration_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let truncated = result
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let status = match exit_code {
                Some(0) => "✓".to_string(),
                Some(code) => format!("exit {}", code),
                None => "killed".to_string(),
            };

            let duration_str = if duration >= 1000 {
                format!("{:.1}s", duration as f64 / 1000.0)
            } else {
                format!("{}ms", duration)
            };

            if truncated {
                format!("  ⎿  {} in {} (output truncated)", status, duration_str)
            } else {
                format!("  ⎿  {} in {}", status, duration_str)
            }
        }
        "Glob" => {
            let paths = result
                .get("paths")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let truncated = result
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if truncated {
                format!("  ⎿  Found {} files (truncated)", paths)
            } else if paths == 1 {
                "  ⎿  Found 1 file".to_string()
            } else {
                format!("  ⎿  Found {} files", paths)
            }
        }
        "Grep" => {
            let count = result
                .get("matches_found")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let truncated = result
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if truncated {
                format!("  ⎿  Found {} matches (truncated)", count)
            } else if count == 1 {
                "  ⎿  Found 1 match".to_string()
            } else {
                format!("  ⎿  Found {} matches", count)
            }
        }
        "Search" => {
            // Search has different output modes
            if let Some(count) = result.get("count").and_then(|v| v.as_u64()) {
                let truncated = result
                    .get("truncated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // Could be files_with_matches (paths) or content mode (matches)
                if result.get("paths").is_some() {
                    if truncated {
                        format!("  ⎿  Found {} files (truncated)", count)
                    } else if count == 1 {
                        "  ⎿  Found 1 file".to_string()
                    } else {
                        format!("  ⎿  Found {} files", count)
                    }
                } else if result.get("matches").is_some() {
                    if truncated {
                        format!("  ⎿  Found {} lines (truncated)", count)
                    } else if count == 1 {
                        "  ⎿  Found 1 line".to_string()
                    } else {
                        format!("  ⎿  Found {} lines", count)
                    }
                } else if result.get("by_file").is_some() {
                    // Count mode
                    let files = result
                        .get("files_searched")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    format!("  ⎿  Found {} matches in {} files", count, files)
                } else {
                    format!("  ⎿  Found {} matches", count)
                }
            } else {
                "  ⎿  Search complete".to_string()
            }
        }
        "Task" => {
            if let Some(ok) = result.get("ok").and_then(|v| v.as_bool()) {
                if ok {
                    "  ⎿  Task completed".to_string()
                } else {
                    "  ⎿  Task failed".to_string()
                }
            } else if result.get("result").is_some() {
                "  ⎿  Task completed".to_string()
            } else {
                "  ⎿  Task complete".to_string()
            }
        }
        "ActivateSkill" => {
            if let Some(ok) = result.get("ok").and_then(|v| v.as_bool()) {
                if ok {
                    let skill_name = result
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("skill");
                    format!("  ⎿  Activated '{}'", skill_name)
                } else {
                    "  ⎿  Activation failed".to_string()
                }
            } else {
                "  ⎿  Skill activated".to_string()
            }
        }
        _ => {
            // For MCP tools or unknown tools
            if name.starts_with("mcp.") {
                let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
                let duration = result.get("duration_ms").and_then(|v| v.as_u64());
                let truncated = result
                    .get("truncated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let status = if ok { "✓" } else { "✗" };
                let mut msg = format!("  ⎿  {}", status);

                if let Some(d) = duration {
                    if d >= 1000 {
                        msg.push_str(&format!(" in {:.1}s", d as f64 / 1000.0));
                    } else {
                        msg.push_str(&format!(" in {}ms", d));
                    }
                }

                if truncated {
                    msg.push_str(" (truncated)");
                }

                msg
            } else {
                // Generic success/failure
                if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true) {
                    "  ⎿  Done".to_string()
                } else {
                    "  ⎿  Failed".to_string()
                }
            }
        }
    }
}

/// Truncate a string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_read_call() {
        let args = json!({"path": "src/main.rs"});
        let result = format_tool_call("Read", &args);
        assert_eq!(result, "⏺ Read(path: \"src/main.rs\")");
    }

    #[test]
    fn test_format_read_result() {
        let result = json!({"lines": 42, "truncated": false});
        let display = format_tool_result("Read", &result);
        assert_eq!(display, "  ⎿  Read 42 lines");
    }

    #[test]
    fn test_format_bash_call() {
        let args = json!({"command": "cargo build"});
        let result = format_tool_call("Bash", &args);
        assert_eq!(result, "⏺ Bash(command: \"cargo build\")");
    }

    #[test]
    fn test_format_bash_result_success() {
        let result = json!({"exit_code": 0, "duration_ms": 1500});
        let display = format_tool_result("Bash", &result);
        assert_eq!(display, "  ⎿  ✓ in 1.5s");
    }

    #[test]
    fn test_format_bash_result_failure() {
        let result = json!({"exit_code": 1, "duration_ms": 250});
        let display = format_tool_result("Bash", &result);
        assert_eq!(display, "  ⎿  exit 1 in 250ms");
    }

    #[test]
    fn test_format_error() {
        let result = json!({"error": {"code": "read_error", "message": "File not found"}});
        let display = format_tool_result("Read", &result);
        assert_eq!(display, "  ⎿  Error [read_error]: File not found");
    }

    #[test]
    fn test_format_search_call() {
        let args = json!({"pattern": "fn main", "path": "src/", "output_mode": "content"});
        let result = format_tool_call("Search", &args);
        assert_eq!(
            result,
            "⏺ Search(pattern: \"fn main\", path: \"src/\", output_mode: \"content\")"
        );
    }

    #[test]
    fn test_format_glob_result() {
        let result = json!({"paths": ["a.rs", "b.rs", "c.rs"], "truncated": false});
        let display = format_tool_result("Glob", &result);
        assert_eq!(display, "  ⎿  Found 3 files");
    }
}
