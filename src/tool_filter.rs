//! Unified tool name pattern matching.
//!
//! Consolidates tool filtering logic used by policy.rs and subagent.rs.

/// Check if a tool name matches a pattern.
///
/// # Pattern formats:
/// - `"Read"` - exact match
/// - `"mcp.*"` - matches all MCP tools (e.g., "mcp.echo.add")
/// - `"mcp.server.*"` - matches tools from specific MCP server
/// - `"Bash(git:*)"` - matches Bash with args starting with "git"
/// - `"Edit(src/lib.rs)"` - matches Edit with exact file path
///
/// # Arguments
/// * `tool` - The tool name being checked
/// * `pattern` - The pattern to match against
/// * `arg` - Optional argument for tools that support arg matching (Bash, Edit, etc.)
pub fn tool_matches(tool: &str, pattern: &str, arg: Option<&str>) -> bool {
    // Exact match
    if pattern == tool {
        return true;
    }

    // MCP wildcard matching: "mcp.*" or "mcp.server.*"
    // Pattern "mcp.*" matches any MCP tool (e.g., "mcp.echo.add")
    // Pattern "mcp.echo.*" matches any tool from echo server
    if let Some(prefix) = pattern.strip_suffix(".*") {
        if let Some(remaining) = tool.strip_prefix(prefix) {
            // Match at dot boundary: remaining must be empty or start with '.'
            if remaining.is_empty() || remaining.starts_with('.') {
                return true;
            }
        }
    }

    // Pattern with argument: "Bash(git diff:*)" or "Edit(src/lib.rs)"
    if let Some(open_paren) = pattern.find('(') {
        let pattern_tool = &pattern[..open_paren];
        if pattern_tool != tool {
            return false;
        }

        // Extract the argument pattern
        let close_paren = pattern.rfind(')').unwrap_or(pattern.len());
        let arg_pattern = &pattern[open_paren + 1..close_paren];

        let Some(actual_arg) = arg else {
            return false;
        };

        // Check for prefix match: "git diff:*"
        if let Some(prefix) = arg_pattern.strip_suffix(":*") {
            return actual_arg.starts_with(prefix);
        }

        // Exact argument match
        return arg_pattern == actual_arg;
    }

    false
}

/// Check if a tool matches any pattern in a list
#[allow(dead_code)] // For future use with argument matching
pub fn tool_matches_any(tool: &str, patterns: &[String], arg: Option<&str>) -> bool {
    patterns.iter().any(|p| tool_matches(tool, p, arg))
}

/// Check if a tool matches any pattern (no argument version)
/// Convenience for subagent tool filtering where arguments aren't used
pub fn tool_matches_any_simple(tool: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| tool_matches(tool, p, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(tool_matches("Read", "Read", None));
        assert!(tool_matches("Write", "Write", None));
        assert!(!tool_matches("Read", "Write", None));
    }

    #[test]
    fn test_mcp_wildcard() {
        // mcp.* matches all mcp tools
        assert!(tool_matches("mcp.echo.add", "mcp.*", None));
        assert!(tool_matches("mcp.github.list_prs", "mcp.*", None));

        // mcp.server.* matches tools from that server only
        assert!(tool_matches("mcp.echo.add", "mcp.echo.*", None));
        assert!(tool_matches("mcp.echo.multiply", "mcp.echo.*", None));
        assert!(!tool_matches("mcp.github.list_prs", "mcp.echo.*", None));

        // Should not match non-dot boundary
        assert!(!tool_matches("mcpfake.tool", "mcp.*", None));
    }

    #[test]
    fn test_arg_prefix_match() {
        assert!(tool_matches("Bash", "Bash(git:*)", Some("git status")));
        assert!(tool_matches(
            "Bash",
            "Bash(git diff:*)",
            Some("git diff HEAD")
        ));
        assert!(!tool_matches("Bash", "Bash(git:*)", Some("npm install")));
        assert!(!tool_matches("Bash", "Bash(git:*)", None));
    }

    #[test]
    fn test_arg_exact_match() {
        assert!(tool_matches("Edit", "Edit(src/lib.rs)", Some("src/lib.rs")));
        assert!(!tool_matches(
            "Edit",
            "Edit(src/lib.rs)",
            Some("src/main.rs")
        ));
    }

    #[test]
    fn test_matches_any() {
        let patterns = vec![
            "Read".to_string(),
            "Grep".to_string(),
            "mcp.echo.*".to_string(),
        ];

        assert!(tool_matches_any("Read", &patterns, None));
        assert!(tool_matches_any("Grep", &patterns, None));
        assert!(tool_matches_any("mcp.echo.add", &patterns, None));
        assert!(!tool_matches_any("Write", &patterns, None));
    }

    #[test]
    fn test_matches_any_simple() {
        let patterns = vec!["Read".to_string(), "Glob".to_string()];

        assert!(tool_matches_any_simple("Read", &patterns));
        assert!(tool_matches_any_simple("Glob", &patterns));
        assert!(!tool_matches_any_simple("Write", &patterns));
    }
}
