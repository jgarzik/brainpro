//! yo - MrCode CLI that spawns brainpro-agent on demand.
//!
//! This is a lightweight CLI that connects to a local agent via Unix socket.
//! If the agent isn't running, it spawns one automatically.
//!
//! Usage:
//!   yo -p "list files in src/"
//!   yo --yes -p "read src/lib.rs and summarize"
//!   yo  # interactive REPL mode

use brainpro::agent::CommandStats;
use brainpro::backend::BackendRegistry;
use brainpro::cli::Context;
use brainpro::commands::CommandIndex;
use brainpro::config::{Config, Target};
use brainpro::cost::{format_cost, PricingTable, SessionCosts};
use brainpro::hooks::HookManager;
use brainpro::model_routing::ModelRouter;
use brainpro::persona::mrcode::MrCode;
use brainpro::persona::Persona;
use brainpro::plan::PlanModeState;
use brainpro::policy::PolicyEngine;
use brainpro::skillpacks::{ActiveSkills, SkillIndex};
use brainpro::tools::{ask_user, todo::TodoState};
use brainpro::transcript::Transcript;

use anyhow::Result;
use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// MrCode - focused coding assistant
#[derive(Parser)]
#[command(name = "yo", about = "MrCode - focused coding assistant")]
struct Args {
    #[arg(short, long, help = "One-shot prompt mode")]
    prompt: Option<String>,

    #[arg(long, help = "Auto-approve mutations")]
    yes: bool,

    #[arg(long, help = "Override default target (e.g., gpt-4@chatgpt)")]
    target: Option<String>,

    #[arg(long, help = "Enable tracing of tool calls")]
    trace: bool,

    #[arg(long, help = "Verbose output")]
    verbose: bool,

    #[arg(long, help = "Debug output")]
    debug: bool,

    #[arg(
        short = 'O',
        long = "optimize",
        help = "Optimize output for token efficiency"
    )]
    optimize: bool,

    #[arg(
        long = "max-turns",
        value_name = "N",
        help = "Maximum agent iterations per turn"
    )]
    max_turns: Option<usize>,

    #[arg(long, help = "Dump assembled system prompt before LLM call")]
    dump_prompt: bool,
}

/// Get the path to the history file
fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".brainpro")
        .join("yo_history")
}

/// Print command stats to stderr
fn print_stats(duration: Duration, stats: &CommandStats, cost: Option<f64>) {
    let tokens = stats.total_tokens();
    let token_display = if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    };
    if let Some(cost_usd) = cost {
        eprintln!(
            "[Duration: {:.1}s | Tokens: {} | Cost: {} | Tools: {}]",
            duration.as_secs_f64(),
            token_display,
            format_cost(cost_usd),
            stats.tool_uses
        );
    } else {
        eprintln!(
            "[Duration: {:.1}s | Tokens: {} | Tools: {}]",
            duration.as_secs_f64(),
            token_display,
            stats.tool_uses
        );
    }
}

fn main() -> Result<()> {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    let args = Args::parse();

    // Get working directory
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Load config
    let mut cfg = Config::load()?;

    // Apply default target based on available API keys
    if cfg.default_target.is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.default_target = Some("qwen3-235b-a22b-instruct-2507@venice".to_string());
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
        }
    }

    // Parse target override
    let target = args
        .target
        .as_ref()
        .and_then(|t| Target::parse(t))
        .or_else(|| cfg.get_default_target());

    if target.is_none() && args.prompt.is_some() {
        eprintln!("Error: No target configured. Set VENICE_API_KEY, OPENAI_API_KEY, or ANTHROPIC_API_KEY.");
        std::process::exit(1);
    }

    // Generate session ID
    let session_id = uuid::Uuid::new_v4().to_string();

    // Create transcript directory if needed
    let transcripts_dir = root.join(".brainpro").join("sessions");
    let _ = std::fs::create_dir_all(&transcripts_dir);

    // Initialize transcript
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", session_id));
    let transcript = Transcript::new(&transcript_path, &session_id, &root)?;

    // Create brainpro Args for compatibility
    let brainpro_args = brainpro::Args {
        prompt: args.prompt.clone(),
        api_key: None,
        base_url: "https://api.venice.ai/api/v1".to_string(),
        model: "qwen3-235b-a22b-instruct-2507".to_string(),
        yes: args.yes,
        transcripts_dir: None,
        trace: args.trace,
        config: None,
        target: args.target.clone(),
        list_targets: false,
        mode: None,
        allowed_tools: vec![],
        disallowed_tools: vec![],
        ask_tools: vec![],
        max_turns: args.max_turns,
        verbose: args.verbose,
        debug: args.debug,
        optimize: args.optimize,
        resume: None,
        gateway: None,
        dump_prompt: args.dump_prompt,
    };

    // Initialize components
    let print_mode = args.prompt.is_some();
    let policy = PolicyEngine::new(cfg.permissions.clone(), print_mode, args.yes);
    let hooks = HookManager::new(cfg.hooks.clone(), session_id.clone(), root.clone());
    let skill_index = SkillIndex::build(&root);
    let model_router = ModelRouter::new(cfg.model_routing.clone());
    let command_index = CommandIndex::build(&root);
    let pricing = PricingTable::from_config(&cfg.model_pricing);
    let session_costs = SessionCosts::new(session_id.clone(), pricing);

    // Build context
    let ctx = Context {
        args: brainpro_args,
        root: root.clone(),
        transcript: RefCell::new(transcript),
        session_id: session_id.clone(),
        tracing: RefCell::new(args.trace),
        config: RefCell::new(cfg.clone()),
        backends: RefCell::new(BackendRegistry::new(&cfg)),
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
    };

    // Get MrCode persona
    let persona = MrCode::new();

    if let Some(prompt) = &args.prompt {
        // One-shot mode
        run_once(&ctx, &persona, prompt)?;
    } else {
        // Interactive REPL mode
        run_repl(ctx, &persona)?;
    }

    Ok(())
}

fn run_once(ctx: &Context, persona: &MrCode, prompt: &str) -> Result<()> {
    // Run UserPromptSubmit hooks
    let (proceed, updated_prompt) = ctx.hooks.borrow().user_prompt_submit(prompt);
    if !proceed {
        eprintln!("Prompt blocked by hook");
        return Ok(());
    }
    let prompt = updated_prompt.as_deref().unwrap_or(prompt);

    // Increment turn counter
    let turn_number = {
        let mut counter = ctx.turn_counter.borrow_mut();
        *counter += 1;
        *counter
    };

    let start = Instant::now();
    let mut messages = Vec::new();
    let mut rl = DefaultEditor::new()?;
    let result = persona.run_turn(ctx, prompt, &mut messages)?;

    let mut total_stats = result.stats.clone();
    let mut current_result = result;

    // Handle pending questions
    while let Some(pending) = current_result.pending_question.take() {
        match ask_user::display_and_collect(&pending.questions, &mut rl) {
            Ok(answers) => {
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": pending.tool_call_id,
                    "content": serde_json::to_string(&answers).unwrap_or_default()
                }));

                current_result =
                    persona.run_turn(ctx, "[User answered questions above]", &mut messages)?;
                total_stats.merge(&current_result.stats);
            }
            Err(e) => {
                eprintln!("Input error: {}", e);
                break;
            }
        }
    }

    // Handle force_continue
    while current_result.force_continue {
        if let Some(continue_prompt) = current_result.continue_prompt.take() {
            println!("[Continuing...]");
            current_result = persona.run_turn(ctx, &continue_prompt, &mut messages)?;
            total_stats.merge(&current_result.stats);
        } else {
            break;
        }
    }

    // Get cost for this turn
    let cost = if ctx.config.borrow().cost_tracking.display_in_stats {
        let costs = ctx.session_costs.borrow();
        costs
            .turns()
            .iter()
            .find(|t| t.turn_number == turn_number)
            .map(|t| t.total_cost())
    } else {
        None
    };

    print_stats(start.elapsed(), &total_stats, cost);
    Ok(())
}

fn run_repl(ctx: Context, persona: &MrCode) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut messages = Vec::new();

    // Load command history
    let history_file = history_path();
    let _ = rl.load_history(&history_file);

    println!("yo - MrCode coding assistant. /help for commands, /exit to quit");

    loop {
        match rl.readline("yo> ") {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(line)?;

                if line.starts_with('/') {
                    if handle_command(&ctx, line, &mut messages) {
                        break;
                    }
                    continue;
                }

                // Run UserPromptSubmit hooks
                let (proceed, updated_prompt) = ctx.hooks.borrow().user_prompt_submit(line);
                if !proceed {
                    eprintln!("Prompt blocked by hook");
                    continue;
                }
                let line = updated_prompt.unwrap_or_else(|| line.to_string());

                // Increment turn counter
                let turn_number = {
                    let mut counter = ctx.turn_counter.borrow_mut();
                    *counter += 1;
                    *counter
                };

                let start = Instant::now();
                match persona.run_turn(&ctx, &line, &mut messages) {
                    Ok(result) => {
                        let mut total_stats = result.stats.clone();
                        let mut current_result = result;

                        // Handle pending questions
                        while let Some(pending) = current_result.pending_question.take() {
                            match ask_user::display_and_collect(&pending.questions, &mut rl) {
                                Ok(answers) => {
                                    messages.push(serde_json::json!({
                                        "role": "tool",
                                        "tool_call_id": pending.tool_call_id,
                                        "content": serde_json::to_string(&answers).unwrap_or_default()
                                    }));

                                    match persona.run_turn(
                                        &ctx,
                                        "[User answered questions above]",
                                        &mut messages,
                                    ) {
                                        Ok(continuation) => {
                                            total_stats.merge(&continuation.stats);
                                            current_result = continuation;
                                        }
                                        Err(e) => {
                                            eprintln!("Continuation error: {}", e);
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Input error: {}", e);
                                    break;
                                }
                            }
                        }

                        // Handle force_continue
                        while current_result.force_continue {
                            if let Some(continue_prompt) = current_result.continue_prompt.take() {
                                println!("[Continuing...]");
                                match persona.run_turn(&ctx, &continue_prompt, &mut messages) {
                                    Ok(continuation) => {
                                        total_stats.merge(&continuation.stats);
                                        current_result = continuation;
                                    }
                                    Err(e) => {
                                        eprintln!("Continuation error: {}", e);
                                        break;
                                    }
                                }
                            } else {
                                break;
                            }
                        }

                        // Get cost
                        let cost = if ctx.config.borrow().cost_tracking.display_in_stats {
                            let costs = ctx.session_costs.borrow();
                            costs
                                .turns()
                                .iter()
                                .find(|t| t.turn_number == turn_number)
                                .map(|t| t.total_cost())
                        } else {
                            None
                        };
                        print_stats(start.elapsed(), &total_stats, cost);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Input error: {}", e);
                break;
            }
        }
    }

    // Save command history
    if let Some(parent) = history_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.save_history(&history_file);

    Ok(())
}

fn handle_command(ctx: &Context, cmd: &str, messages: &mut Vec<serde_json::Value>) -> bool {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    match parts[0] {
        "/exit" | "/quit" => return true,
        "/help" => {
            println!("Commands:");
            println!("  /exit           - quit");
            println!("  /help           - show commands");
            println!("  /clear          - clear conversation");
            println!("  /trace          - toggle tracing");
            println!("  /target [t]     - show/set current target");
        }
        "/clear" => {
            messages.clear();
            println!("Conversation cleared");
        }
        "/trace" => {
            let mut t = ctx.tracing.borrow_mut();
            *t = !*t;
            println!("Tracing: {}", if *t { "on" } else { "off" });
        }
        "/target" => {
            if parts.len() > 1 {
                let target_str = parts[1].trim();
                if let Some(target) = brainpro::config::Target::parse(target_str) {
                    if ctx.backends.borrow().has_backend(&target.backend) {
                        *ctx.current_target.borrow_mut() = Some(target.clone());
                        println!("Target set: {}", target);
                    } else {
                        println!("Unknown backend: {}", target.backend);
                    }
                } else {
                    println!("Invalid target format. Use: model@backend");
                }
            } else {
                let current = ctx.current_target.borrow();
                if let Some(t) = current.as_ref() {
                    println!("Current target: {}", t);
                } else {
                    println!("No target configured");
                }
            }
        }
        _ => {
            println!("Unknown command: {}", parts[0]);
        }
    }
    false
}
