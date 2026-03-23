//! Shared hooks for persona agent loops.
//!
//! This provides the common AgentHooks implementation used by
//! both MrCode and MrBot, differing only in tool filtering.

use crate::agent::core::AgentHooks;
use crate::cli::Context;
use crate::persona::loader::{self, PersonaConfig};
use crate::persona::PromptContext;
use crate::plan;
use chrono::Local;
use serde_json::Value;

/// Shared hooks implementation for persona-based agents.
///
/// The only difference between MrCode and MrBot is whether
/// Task tool is included - that's handled by AgentLoopConfig.
#[allow(dead_code)] // Used by library consumers (yo binary)
pub struct PersonaHooks<'a> {
    pub config: &'a PersonaConfig,
}

#[allow(dead_code)] // Used by library consumers (yo binary)
impl<'a> PersonaHooks<'a> {
    pub fn new(config: &'a PersonaConfig) -> Self {
        Self { config }
    }
}

impl<'a> AgentHooks for PersonaHooks<'a> {
    fn build_system_prompt(&self, ctx: &Context, in_planning_mode: bool) -> String {
        // Build the base system prompt from persona config
        let prompt_ctx = PromptContext::from_context(ctx);
        let mut system_prompt = if in_planning_mode {
            plan::PLAN_MODE_SYSTEM_PROMPT.to_string()
        } else {
            loader::build_system_prompt(self.config, &prompt_ctx)
        };

        // Add optimization mode instructions if -O flag is set
        if ctx.args.optimize {
            system_prompt.push_str(
                "\n\nAI-to-AI mode. Maximum information density. Structure over prose. No narration.",
            );
        }

        // Add dynamic environment section
        system_prompt.push_str(&build_environment_section(ctx));

        // Add skill pack index
        let skill_index = ctx.skill_index.borrow();
        let skill_prompt = skill_index.format_for_prompt(50);
        drop(skill_index);
        if !skill_prompt.is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&skill_prompt);
        }

        // Add active skill instructions
        let active_skills = ctx.active_skills.borrow();
        if !active_skills.is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&active_skills.format_for_conversation());
        }

        system_prompt
    }

    fn filter_tools(&self, schemas: Vec<Value>, in_planning_mode: bool) -> Vec<Value> {
        if in_planning_mode {
            // Only read-only tools in planning mode
            schemas
                .into_iter()
                .filter(|schema| {
                    schema
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|name| matches!(name, "Read" | "Glob" | "Search"))
                        .unwrap_or(false)
                })
                .collect()
        } else {
            schemas
        }
    }
}

/// Build a dynamic environment section for the system prompt.
///
/// Includes current date, platform, model name, and git repo info.
fn build_environment_section(ctx: &Context) -> String {
    let date = Local::now().format("%Y-%m-%d").to_string();
    let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

    let model_info = ctx
        .current_target
        .borrow()
        .as_ref()
        .map(|t| format!("{} (backend: {})", t.model, t.backend))
        .unwrap_or_else(|| "unknown".to_string());

    let git_head = ctx.root.join(".git").join("HEAD");
    let git_info = if git_head.exists() {
        let branch = std::fs::read_to_string(&git_head)
            .ok()
            .and_then(|contents| {
                let line = contents.lines().next()?.trim().to_string();
                if let Some(rest) = line.strip_prefix("ref:") {
                    // Normal branch: "ref: refs/heads/main"
                    let path = rest.trim();
                    Some(path.rsplit('/').next().unwrap_or(path).to_string())
                } else if !line.is_empty() {
                    // Detached HEAD: raw commit hash, shorten for display
                    Some(line.chars().take(7).collect())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());
        format!("yes, branch: {}", branch)
    } else {
        "no".to_string()
    };

    format!(
        "\n\n## Environment\n- Date: {}\n- Platform: {}\n- Model: {}\n- Git repo: {}",
        date, platform, model_info, git_info
    )
}
