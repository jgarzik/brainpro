//! MrBot modular prompt builder.
//!
//! Builds system prompts from composable sections following clawdbot pattern.

use crate::personality::PromptContext;

/// Build the identity section
fn build_identity_section() -> String {
    "You are MrBot, an agentic coding assistant with personality.
You can only access files via tools. All paths are relative to the project root.
Use Glob/Grep to find files before Read. Before Edit/Write, explain what you will change.
Use Bash for running builds, tests, formatters, and git operations.
Never use curl or wget - they are blocked by policy.
Keep edits minimal and precise.".to_string()
}

/// Build the SOUL section from loaded content
fn build_soul_section(soul_content: Option<&str>) -> Option<String> {
    soul_content.map(|content| {
        format!(
            "## Personality & Values\n\n{}",
            content.trim()
        )
    })
}

/// Build the tooling section
fn build_tooling_section() -> String {
    "## Available Tools
- Read: Read file contents
- Write: Create or overwrite files
- Edit: Make precise edits to files
- Glob: Find files by pattern
- Grep/Search: Search file contents
- Bash: Execute shell commands
- Task: Delegate to subagents
- AskUserQuestion: Ask the user for clarification".to_string()
}

/// Build the workspace section
fn build_workspace_section(ctx: &PromptContext) -> Option<String> {
    if ctx.working_dir.as_os_str().is_empty() {
        return None;
    }
    Some(format!(
        "## Workspace\nWorking directory: {}",
        ctx.working_dir.display()
    ))
}

/// Build the skills section
fn build_skills_section(ctx: &PromptContext) -> Option<String> {
    if ctx.active_skills.is_empty() {
        return None;
    }
    Some(format!(
        "## Active Skills\n{}",
        ctx.active_skills.join(", ")
    ))
}

/// Build the plan mode section
fn build_plan_mode_section() -> String {
    "## Plan Mode
You are in planning mode. Use only read-only tools: Read, Glob, Grep, Search.
Analyze the codebase and create a step-by-step plan.

Output format:
## Summary
Brief description of the approach.

## Steps
1. **Step Title** - Description of what to do
   Files: file1.rs, file2.rs
2. **Next Step** - ...

Keep plans concrete and actionable.".to_string()
}

/// Build the optimization mode section
fn build_optimize_section() -> String {
    "AI-to-AI mode. Maximum information density. Structure over prose. No narration.".to_string()
}

/// Build the full system prompt for MrBot
pub fn build_system_prompt(ctx: &PromptContext) -> String {
    let mut sections = vec![];

    // Identity section (always included)
    sections.push(build_identity_section());

    // SOUL section (if available)
    if let Some(soul_section) = build_soul_section(ctx.soul_content.as_deref()) {
        sections.push(soul_section);
    }

    // Tooling section
    sections.push(build_tooling_section());

    // Workspace section
    if let Some(workspace) = build_workspace_section(ctx) {
        sections.push(workspace);
    }

    // Skills section
    if let Some(skills) = build_skills_section(ctx) {
        sections.push(skills);
    }

    // Plan mode section (replaces other sections when active)
    if ctx.plan_mode {
        sections.push(build_plan_mode_section());
    }

    // Optimization mode
    if ctx.optimize_mode {
        sections.push(build_optimize_section());
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_basic_prompt() {
        let ctx = PromptContext::default();
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("MrBot"));
        assert!(prompt.contains("Available Tools"));
    }

    #[test]
    fn test_prompt_with_soul() {
        let ctx = PromptContext {
            soul_content: Some("Be helpful and direct.".to_string()),
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("Personality & Values"));
        assert!(prompt.contains("Be helpful and direct"));
    }

    #[test]
    fn test_plan_mode_prompt() {
        let ctx = PromptContext {
            plan_mode: true,
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("Plan Mode"));
        assert!(prompt.contains("read-only tools"));
    }

    #[test]
    fn test_optimize_mode() {
        let ctx = PromptContext {
            optimize_mode: true,
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("AI-to-AI mode"));
    }

    #[test]
    fn test_working_dir() {
        let ctx = PromptContext {
            working_dir: PathBuf::from("/home/user/project"),
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("Workspace"));
        assert!(prompt.contains("/home/user/project"));
    }

    #[test]
    fn test_active_skills() {
        let ctx = PromptContext {
            active_skills: vec!["rust".to_string(), "testing".to_string()],
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("Active Skills"));
        assert!(prompt.contains("rust, testing"));
    }
}
