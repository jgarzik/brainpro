//! Factory for building Context instances.
//!
//! Consolidates the duplicated Context construction code from:
//! - main.rs
//! - worker.rs (3 locations)

use crate::backend::BackendRegistry;
use crate::cli::{Args, Context};
use crate::commands::CommandIndex;
use crate::config::{Config, Target};
use crate::cost::{PricingTable, SessionCosts};
use crate::hooks::HookManager;
use crate::model_routing::ModelRouter;
use crate::plan::PlanModeState;
use crate::policy::PolicyEngine;
use crate::skillpacks::{ActiveSkills, SkillIndex};
use crate::tools::todo::TodoState;
use crate::transcript::Transcript;
use anyhow::Result;
use std::cell::RefCell;
use std::path::PathBuf;

/// Apply default target based on available API keys.
///
/// Priority: Venice > OpenAI > Anthropic
pub fn apply_default_target(cfg: &mut Config) {
    if cfg.default_target.is_some() {
        return;
    }

    if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
        cfg.default_target = Some("qwen3-235b-a22b-instruct-2507@venice".to_string());
    } else if std::env::var("OPENAI_API_KEY").is_ok() {
        cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
    }
}

/// Load config with default target applied.
pub fn load_config_with_defaults() -> Result<Config, String> {
    let mut cfg = Config::load().map_err(|e| format!("Config load failed: {}", e))?;
    apply_default_target(&mut cfg);
    Ok(cfg)
}

/// Parse working directory from optional string.
pub fn parse_working_dir(working_dir: Option<&String>) -> PathBuf {
    working_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Resolve target from request override or config default.
pub fn resolve_target(target_str: Option<&String>, cfg: &Config) -> Option<Target> {
    target_str
        .and_then(|t| Target::parse(t))
        .or_else(|| cfg.get_default_target())
}

/// Build a Context from a loaded Config.
///
/// This consolidates the duplicated Context construction logic.
pub fn build_context(
    cfg: &Config,
    root: PathBuf,
    session_id: String,
    target: Option<Target>,
) -> Result<Context, String> {
    // Create transcript directory if needed
    let transcripts_dir = root.join(".brainpro").join("sessions");
    std::fs::create_dir_all(&transcripts_dir)
        .map_err(|e| format!("Failed to create transcripts dir: {}", e))?;

    // Initialize transcript
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", session_id));
    let transcript = Transcript::new(&transcript_path, &session_id, &root)
        .map_err(|e| format!("Failed to create transcript: {}", e))?;

    // Initialize components
    let policy = PolicyEngine::new(cfg.permissions.clone(), false, false);
    let hooks = HookManager::new(cfg.hooks.clone(), session_id.clone(), root.clone());
    let skill_index = SkillIndex::build(&root);
    let model_router = ModelRouter::new(cfg.model_routing.clone());
    let command_index = CommandIndex::build(&root);
    let pricing = PricingTable::from_config(&cfg.model_pricing);
    let session_costs = SessionCosts::new(session_id.clone(), pricing);

    // Build context with default Args
    Ok(Context {
        args: Args::default(),
        root: root.clone(),
        transcript: RefCell::new(transcript),
        session_id,
        tracing: RefCell::new(false),
        config: RefCell::new(cfg.clone()),
        backends: RefCell::new(BackendRegistry::new(cfg)),
        current_target: RefCell::new(target),
        policy: RefCell::new(policy),
        skill_index: RefCell::new(skill_index),
        active_skills: RefCell::new(ActiveSkills::new()),
        model_router: RefCell::new(model_router),
        plan_mode: RefCell::new(PlanModeState::new()),
        hooks: RefCell::new(hooks),
        session_costs: RefCell::new(session_costs),
        turn_counter: RefCell::new(0),
        command_index: RefCell::new(command_index),
        todo_state: RefCell::new(TodoState::new()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_apply_default_target_venice() {
        // Save current env state
        let original = env::var("VENICE_API_KEY").ok();

        // Temporarily set env var
        env::set_var("VENICE_API_KEY", "test");
        let mut cfg = Config::default();
        apply_default_target(&mut cfg);

        // Restore original state
        if let Some(k) = original {
            env::set_var("VENICE_API_KEY", k);
        } else {
            env::remove_var("VENICE_API_KEY");
        }

        assert!(cfg.default_target.is_some());
        assert!(cfg.default_target.unwrap().contains("@venice"));
    }

    #[test]
    #[serial]
    fn test_apply_default_target_no_keys() {
        // Save current env state
        let venice_key = env::var("VENICE_API_KEY").ok();
        let venice_key_lc = env::var("venice_api_key").ok();
        let openai_key = env::var("OPENAI_API_KEY").ok();
        let anthropic_key = env::var("ANTHROPIC_API_KEY").ok();

        // Remove all keys
        env::remove_var("VENICE_API_KEY");
        env::remove_var("venice_api_key");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("ANTHROPIC_API_KEY");

        let mut cfg = Config::default();
        apply_default_target(&mut cfg);

        // Restore original env state before asserting
        if let Some(k) = venice_key {
            env::set_var("VENICE_API_KEY", k);
        }
        if let Some(k) = venice_key_lc {
            env::set_var("venice_api_key", k);
        }
        if let Some(k) = openai_key {
            env::set_var("OPENAI_API_KEY", k);
        }
        if let Some(k) = anthropic_key {
            env::set_var("ANTHROPIC_API_KEY", k);
        }

        assert!(cfg.default_target.is_none());
    }

    #[test]
    fn test_parse_working_dir_some() {
        let wd = parse_working_dir(Some(&"/tmp/test".to_string()));
        assert_eq!(wd, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_parse_working_dir_none() {
        let wd = parse_working_dir(None);
        // Should return current dir or fallback
        assert!(!wd.as_os_str().is_empty());
    }

    #[test]
    fn test_resolve_target_override() {
        let cfg = Config::default();
        let target = resolve_target(Some(&"gpt-4@chatgpt".to_string()), &cfg);
        assert!(target.is_some());
        let t = target.unwrap();
        assert_eq!(t.model, "gpt-4");
        assert_eq!(t.backend, "chatgpt");
    }
}
