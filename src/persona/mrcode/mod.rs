//! MrCode persona - focused coding assistant.
//!
//! MrCode is a direct, terse coding assistant designed for:
//! - On-demand local agent via Unix socket
//! - Minimal toolset: Read, Write, Edit, Glob, Grep, Bash
//! - Simple system prompt loaded from config/persona/mrcode/

mod loop_impl;

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use crate::persona::loader::{self, PersonaConfig};
use crate::persona::{Persona, PromptContext};
use anyhow::Result;
use serde_json::Value;

/// MrCode persona - focused coding assistant
pub struct MrCode {
    /// Loaded configuration from files
    config: PersonaConfig,
    /// Cached tools as static refs
    tools: Vec<&'static str>,
}

impl MrCode {
    /// Create a new MrCode persona
    pub fn new() -> Self {
        let config =
            loader::load_persona("mrcode").expect("Failed to load mrcode persona config");
        let tools = config.tools_as_static();
        Self { config, tools }
    }
}

impl Default for MrCode {
    fn default() -> Self {
        Self::new()
    }
}

impl Persona for MrCode {
    fn name(&self) -> &str {
        "MrCode"
    }

    fn config(&self) -> &PersonaConfig {
        &self.config
    }

    fn build_system_prompt(&self, ctx: &PromptContext) -> String {
        loader::build_system_prompt(&self.config, ctx)
    }

    fn run_turn(
        &self,
        ctx: &Context,
        user_input: &str,
        messages: &mut Vec<Value>,
    ) -> Result<TurnResult> {
        loop_impl::run_turn(&self.config, ctx, user_input, messages)
    }

    fn available_tools(&self) -> &[&str] {
        &self.tools
    }

    fn permission_mode(&self) -> PermissionMode {
        self.config.permission_mode
    }
}
