//! MrCode personality - focused coding assistant.
//!
//! MrCode is a direct, terse coding assistant designed for:
//! - On-demand local agent via Unix socket
//! - Minimal toolset: Read, Write, Edit, Glob, Grep, Bash
//! - Simple system prompt (no SOUL.md complexity)

mod loop_impl;
mod prompts;

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use crate::personality::{Personality, PromptContext};
use anyhow::Result;
use serde_json::Value;

/// MrCode personality - focused coding assistant
pub struct MrCode {
    /// Available tools for MrCode
    tools: Vec<&'static str>,
}

impl MrCode {
    /// Create a new MrCode personality
    pub fn new() -> Self {
        Self {
            tools: vec!["Read", "Write", "Edit", "Glob", "Grep", "Bash", "Search"],
        }
    }
}

impl Default for MrCode {
    fn default() -> Self {
        Self::new()
    }
}

impl Personality for MrCode {
    fn name(&self) -> &str {
        "MrCode"
    }

    fn build_system_prompt(&self, ctx: &PromptContext) -> String {
        prompts::build_system_prompt(ctx)
    }

    fn run_turn(
        &self,
        ctx: &Context,
        user_input: &str,
        messages: &mut Vec<Value>,
    ) -> Result<TurnResult> {
        loop_impl::run_turn(ctx, user_input, messages)
    }

    fn available_tools(&self) -> &[&str] {
        &self.tools
    }

    fn permission_mode(&self) -> PermissionMode {
        // MrCode uses default mode - asks for mutations and execution
        PermissionMode::Default
    }
}
