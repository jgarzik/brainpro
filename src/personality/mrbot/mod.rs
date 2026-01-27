//! MrBot personality - conversational bot with personality.
//!
//! MrBot is designed for:
//! - Gateway/daemon architecture (yield/resume)
//! - Full toolset including messaging/voice (future)
//! - SOUL.md persona file support
//! - Modular prompt builder (clawdbot-style)

mod loop_impl;
mod prompts;
mod soul;

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use crate::personality::{Personality, PromptContext};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

pub use soul::load_soul;

/// MrBot personality - conversational bot with SOUL
pub struct MrBot {
    /// Available tools for MrBot (full toolset)
    tools: Vec<&'static str>,
    /// Path to SOUL.md file
    soul_path: Option<PathBuf>,
    /// Cached SOUL content
    soul_content: Option<String>,
}

impl MrBot {
    /// Create a new MrBot personality
    pub fn new() -> Self {
        Self {
            tools: vec![
                "Read",
                "Write",
                "Edit",
                "Glob",
                "Grep",
                "Bash",
                "Search",
                "Task",
                "TodoWrite",
                "AskUserQuestion",
                "ActivateSkill",
                "EnterPlanMode",
                "ExitPlanMode",
            ],
            soul_path: None,
            soul_content: None,
        }
    }

    /// Create MrBot with a specific SOUL.md path
    pub fn with_soul_path(mut self, path: PathBuf) -> Self {
        self.soul_path = Some(path);
        self
    }

    /// Load SOUL content from default locations or specified path
    pub fn load_soul(&mut self, working_dir: &PathBuf) {
        if let Some(ref path) = self.soul_path {
            self.soul_content = soul::load_soul_from_path(path);
        } else {
            self.soul_content = soul::load_soul(working_dir);
        }
    }

    /// Get the SOUL content
    pub fn soul_content(&self) -> Option<&str> {
        self.soul_content.as_deref()
    }
}

impl Default for MrBot {
    fn default() -> Self {
        Self::new()
    }
}

impl Personality for MrBot {
    fn name(&self) -> &str {
        "MrBot"
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
        loop_impl::run_turn(ctx, user_input, messages, self.soul_content.as_deref())
    }

    fn available_tools(&self) -> &[&str] {
        &self.tools
    }

    fn permission_mode(&self) -> PermissionMode {
        // MrBot uses default mode by default (gateway handles approvals)
        PermissionMode::Default
    }
}
