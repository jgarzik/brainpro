//! MrCode system prompts.
//!
//! Simple, focused prompts for coding assistance.

use crate::personality::PromptContext;

/// Core system prompt for MrCode
const MRCODE_CORE: &str = r#"You are MrCode, a focused coding assistant.
Access files via tools. Paths relative to project root.
Use Glob/Grep to find files before Read.
Before Edit/Write, explain what you will change.
Use Bash for builds, tests, git operations.
Keep edits minimal and precise."#;

/// Plan mode system prompt
const MRCODE_PLAN_MODE: &str = r#"You are MrCode in planning mode.
Access files via tools. Paths relative to project root.
Use only read-only tools: Read, Glob, Grep, Search.
Analyze the codebase and create a step-by-step plan.

Output format:
## Summary
Brief description of the approach.

## Steps
1. **Step Title** - Description of what to do
   Files: file1.rs, file2.rs
2. **Next Step** - ...

Keep plans concrete and actionable."#;

/// Build the full system prompt for MrCode
pub fn build_system_prompt(ctx: &PromptContext) -> String {
    let mut prompt = if ctx.plan_mode {
        MRCODE_PLAN_MODE.to_string()
    } else {
        MRCODE_CORE.to_string()
    };

    // Add optimization mode instructions if enabled
    if ctx.optimize_mode {
        prompt.push_str("\n\nAI-to-AI mode. Maximum information density. Structure over prose. No narration.");
    }

    // Add working directory context
    if !ctx.working_dir.as_os_str().is_empty() {
        prompt.push_str(&format!(
            "\n\nWorking directory: {}",
            ctx.working_dir.display()
        ));
    }

    // Add active skills if any
    if !ctx.active_skills.is_empty() {
        prompt.push_str(&format!(
            "\n\nActive skills: {}",
            ctx.active_skills.join(", ")
        ));
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_basic_prompt() {
        let ctx = PromptContext::default();
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("MrCode"));
        assert!(prompt.contains("focused coding assistant"));
    }

    #[test]
    fn test_plan_mode_prompt() {
        let ctx = PromptContext {
            plan_mode: true,
            ..Default::default()
        };
        let prompt = build_system_prompt(&ctx);
        assert!(prompt.contains("planning mode"));
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
        assert!(prompt.contains("/home/user/project"));
    }
}
