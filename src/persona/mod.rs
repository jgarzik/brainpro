//! Agent persona system.
//!
//! This module provides distinct agent personas with separate behaviors:
//! - MrCode: Direct CLI, focused coding assistant with minimal toolset
//! - MrBot: Gateway/Docker path, conversational bot with SOUL.md support

pub mod loader;
pub mod mrbot;
pub mod mrcode;

pub use loader::{load_persona, PersonaConfig};

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// Context for building system prompts
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    /// Working directory for the agent
    pub working_dir: PathBuf,
    /// Currently active skill packs
    pub active_skills: Vec<String>,
    /// Whether in plan mode
    pub plan_mode: bool,
    /// Whether optimize mode is enabled (-O flag)
    pub optimize_mode: bool,
    /// SOUL.md content for MrBot persona
    pub soul_content: Option<String>,
}

impl PromptContext {
    /// Create a new prompt context from the CLI context
    pub fn from_context(ctx: &Context) -> Self {
        let active_skills = ctx
            .active_skills
            .borrow()
            .list()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let plan_mode = ctx.plan_mode.borrow().phase == crate::plan::PlanPhase::Planning;

        Self {
            working_dir: ctx.root.clone(),
            active_skills,
            plan_mode,
            optimize_mode: ctx.args.optimize,
            soul_content: None,
        }
    }

    /// Set the SOUL content
    pub fn with_soul(mut self, soul: Option<String>) -> Self {
        self.soul_content = soul;
        self
    }
}

/// Trait defining an agent persona
pub trait Persona: Send + Sync {
    /// Persona identifier
    fn name(&self) -> &str;

    /// Get the persona configuration (loaded from files)
    fn config(&self) -> &PersonaConfig;

    /// Build the system prompt for this persona
    fn build_system_prompt(&self, ctx: &PromptContext) -> String;

    /// Run the agent loop (each persona has its own implementation)
    fn run_turn(
        &self,
        ctx: &Context,
        user_input: &str,
        messages: &mut Vec<Value>,
    ) -> Result<TurnResult>;

    /// Get available tools for this persona
    fn available_tools(&self) -> &[&str];

    /// Default permission mode for this persona
    fn permission_mode(&self) -> PermissionMode;
}

/// Get persona by name
pub fn get_persona(name: &str) -> Option<Box<dyn Persona>> {
    match name.to_lowercase().as_str() {
        "mrcode" => Some(Box::new(mrcode::MrCode::new())),
        "mrbot" => Some(Box::new(mrbot::MrBot::new())),
        _ => None,
    }
}

/// Get the default MrCode persona
pub fn mrcode() -> mrcode::MrCode {
    mrcode::MrCode::new()
}

/// Get the default MrBot persona
pub fn mrbot() -> mrbot::MrBot {
    mrbot::MrBot::new()
}
