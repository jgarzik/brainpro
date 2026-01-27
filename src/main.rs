mod agent;
mod agent_service;
mod backend;
mod cli;
mod commands;
mod compact;
mod config;
mod cost;
mod gateway;
mod gateway_client;
mod hooks;
mod llm;
mod model_routing;
mod plan;
mod policy;
mod protocol;
mod session;
mod skillpacks;
mod subagent;
mod tool_display;
mod tool_filter;
mod tools;
mod transcript;
mod vendors;

use anyhow::Result;
use clap::Parser;
use cli::Args;
use std::cell::RefCell;
use std::path::PathBuf;

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    // Gateway client mode: connect to remote gateway instead of running locally
    if let Some(gateway_url) = &args.gateway {
        return gateway_client::run_gateway_mode(gateway_url, args.prompt.as_deref());
    }

    // Load configuration (includes built-in backends)
    let mut cfg = if let Some(config_path) = &args.config {
        config::Config::load_from(config_path)?
    } else {
        config::Config::load().unwrap_or_else(|_| config::Config::with_builtin_backends())
    };

    // If CLI provides an API key, apply it to the appropriate backend
    if let Some(api_key) = &args.api_key {
        cfg = config::Config::from_cli_args(&args.model, &args.base_url, api_key);
    }

    // Apply --target override if provided
    if let Some(target_str) = &args.target {
        cfg.default_target = Some(target_str.clone());
    }

    // If no default target is set, try to set one based on available API keys
    // Priority: Venice (our default) > ChatGPT > Claude > Ollama
    if cfg.default_target.is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.default_target = Some(format!("{}@venice", args.model));
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
        }
        // Ollama doesn't need a key, but user should explicitly set --target
    }

    // Handle --list-targets: dump config and exit
    if args.list_targets {
        println!("Backends:");
        for (name, backend) in &cfg.backends {
            println!(
                "  {}: {} (key: {})",
                name,
                backend.base_url,
                backend
                    .api_key_env
                    .as_deref()
                    .unwrap_or(if backend.api_key.is_some() {
                        "<direct>"
                    } else {
                        "<none>"
                    })
            );
        }
        if let Some(target) = &cfg.default_target {
            println!("\nDefault target: {}", target);
        } else {
            println!("\nNo default target configured.");
        }
        return Ok(());
    }

    // Ensure we have at least one backend configured
    if !cfg.has_backends() {
        return Err(anyhow::anyhow!(
            "No backends configured. Use --api-key and --base-url, or create a config file with named backends (venice, chatgpt, claude, ollama, etc.)."
        ));
    }

    // Validate configuration
    if let Err(errors) = cfg.validate() {
        for err in &errors {
            eprintln!("Config error {}", err);
        }
        return Err(anyhow::anyhow!(
            "Configuration has {} validation error(s)",
            errors.len()
        ));
    }

    // Apply CLI permission overrides
    if let Some(mode_str) = &args.mode {
        if let Some(mode) = config::PermissionMode::from_str(mode_str) {
            cfg.permissions.mode = mode;
        } else {
            return Err(anyhow::anyhow!(
                "Invalid permission mode: {}. Use: default, acceptEdits, bypassPermissions",
                mode_str
            ));
        }
    }

    // Add CLI permission rules
    cfg.permissions.allow.extend(args.allowed_tools.clone());
    cfg.permissions.deny.extend(args.disallowed_tools.clone());
    cfg.permissions.ask.extend(args.ask_tools.clone());

    // Debug output if requested
    if args.debug {
        eprintln!("[DEBUG] Permission mode: {}", cfg.permissions.mode.as_str());
        eprintln!("[DEBUG] Allow rules: {:?}", cfg.permissions.allow);
        eprintln!("[DEBUG] Ask rules: {:?}", cfg.permissions.ask);
        eprintln!("[DEBUG] Deny rules: {:?}", cfg.permissions.deny);
    }

    let root = std::env::current_dir()?;
    let transcripts_dir = args
        .transcripts_dir
        .clone()
        .unwrap_or_else(|| root.join(".brainpro").join("sessions"));
    std::fs::create_dir_all(&transcripts_dir)?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", session_id));
    let mut transcript = transcript::Transcript::new(&transcript_path, &session_id, &root)?;

    let trace = args.trace;
    let backends = backend::BackendRegistry::new(&cfg);

    // Create policy engine from config
    let print_mode = args.prompt.is_some();
    let auto_yes = args.yes;
    let policy_engine = policy::PolicyEngine::new(cfg.permissions.clone(), print_mode, auto_yes);

    // Build skill pack index
    let skill_index = skillpacks::SkillIndex::build(&root);

    // Log skill index built
    let _ = transcript.skill_index_built(skill_index.count());

    // Log parse errors
    for (path, error) in skill_index.errors() {
        let _ = transcript.skill_parse_error(path, error);
    }

    // Create model router
    let model_router = model_routing::ModelRouter::new(cfg.model_routing.clone());

    // Create hook manager
    let hook_manager = hooks::HookManager::new(cfg.hooks.clone(), session_id.clone(), root.clone());

    // Create cost tracker with pricing from config + Venice API cache
    let mut pricing_table = cost::PricingTable::from_config(&cfg.model_pricing);
    if let Some(venice_pricing) = vendors::venice::get_venice_pricing() {
        pricing_table.merge_venice_pricing(venice_pricing);
    }
    let session_costs = cost::SessionCosts::new(session_id.clone(), pricing_table);

    // Build command index
    let command_index = commands::CommandIndex::build(&root);

    let ctx = cli::Context {
        args,
        root,
        transcript: RefCell::new(transcript),
        session_id,
        tracing: RefCell::new(trace),
        config: RefCell::new(cfg),
        backends: RefCell::new(backends),
        current_target: RefCell::new(None),
        policy: RefCell::new(policy_engine),
        skill_index: RefCell::new(skill_index),
        active_skills: RefCell::new(skillpacks::ActiveSkills::new()),
        model_router: RefCell::new(model_router),
        plan_mode: RefCell::new(plan::PlanModeState::new()),
        hooks: RefCell::new(hook_manager),
        session_costs: RefCell::new(session_costs),
        turn_counter: RefCell::new(0),
        command_index: RefCell::new(command_index),
        todo_state: RefCell::new(tools::todo::TodoState::new()),
    };

    // Fire SessionStart hook
    let session_mode = if ctx.args.prompt.is_some() {
        "one-shot"
    } else {
        "repl"
    };
    ctx.hooks.borrow().on_session_start(session_mode);

    if let Some(prompt) = &ctx.args.prompt {
        cli::run_once(&ctx, prompt)
    } else {
        // Handle session resume
        let initial_messages = if let Some(resume_id) = &ctx.args.resume {
            match session::load_session(resume_id) {
                Ok(saved) => {
                    // Update turn counter from saved session
                    *ctx.turn_counter.borrow_mut() = saved.turn_count;
                    Some(saved.messages)
                }
                Err(e) => {
                    eprintln!("Failed to load session '{}': {}", resume_id, e);
                    None
                }
            }
        } else {
            None
        };
        cli::run_repl(ctx, initial_messages)
    }
}
