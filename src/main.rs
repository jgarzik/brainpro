mod agent;
mod backend;
mod cli;
mod config;
mod context;
mod llm;
mod mcp;
mod permissions;
mod policy;
mod subagent;
mod tools;
mod transcript;

use anyhow::Result;
use clap::Parser;
use std::cell::RefCell;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "yo", about = "An agentic coding assistant")]
pub struct Args {
    #[arg(short, long, help = "One-shot prompt mode")]
    pub prompt: Option<String>,

    #[arg(long, help = "API key (overrides env vars)")]
    pub api_key: Option<String>,

    #[arg(
        long,
        env = "OPENAI_BASE_URL",
        default_value = "https://api.venice.ai/api/v1"
    )]
    pub base_url: String,

    #[arg(
        long,
        env = "OPENAI_MODEL",
        default_value = "qwen3-235b-a22b-instruct-2507"
    )]
    pub model: String,

    #[arg(long, help = "Auto-approve mutations in -p mode")]
    pub yes: bool,

    #[arg(long, help = "Session transcripts directory")]
    pub transcripts_dir: Option<PathBuf>,

    #[arg(long, help = "Enable tracing of tool calls and thinking")]
    pub trace: bool,

    #[arg(long, help = "Config file path")]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Override default target (e.g., gpt-4@chatgpt)")]
    pub target: Option<String>,

    #[arg(long, help = "List all configured targets and exit")]
    pub list_targets: bool,

    #[arg(
        long,
        value_name = "MODE",
        help = "Permission mode: default, acceptEdits, bypassPermissions"
    )]
    pub mode: Option<String>,

    #[arg(long = "allowed-tools", value_name = "RULE", action = clap::ArgAction::Append, help = "Allow tool pattern (e.g., 'Bash(cargo test:*)')")]
    pub allowed_tools: Vec<String>,

    #[arg(long = "disallowed-tools", value_name = "RULE", action = clap::ArgAction::Append, help = "Deny tool pattern")]
    pub disallowed_tools: Vec<String>,

    #[arg(long = "ask-tools", value_name = "RULE", action = clap::ArgAction::Append, help = "Always prompt for tool pattern")]
    pub ask_tools: Vec<String>,

    #[arg(
        long = "max-turns",
        value_name = "N",
        help = "Maximum agent iterations per turn (default: 12)"
    )]
    pub max_turns: Option<usize>,

    #[arg(long, help = "Verbose output (print tool calls)")]
    pub verbose: bool,

    #[arg(long, help = "Debug output (print HTTP details and settings)")]
    pub debug: bool,
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();

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
        cfg.skills.insert("default".to_string(), target_str.clone());
    }

    // If no default skill is set, try to set one based on available API keys
    // Priority: Venice (our default) > ChatGPT > Claude > Ollama
    if cfg.resolve_skill("default").is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.skills
                .insert("default".to_string(), format!("{}@venice", args.model));
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.skills
                .insert("default".to_string(), "gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.skills.insert(
                "default".to_string(),
                "claude-3-5-sonnet-latest@claude".to_string(),
            );
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
        println!("\nSkills → Targets:");
        for (skill, target) in &cfg.skills {
            println!("  {}: {}", skill, target);
        }
        return Ok(());
    }

    // Ensure we have at least one backend configured
    if !cfg.has_backends() {
        return Err(anyhow::anyhow!(
            "No backends configured. Use --api-key and --base-url, or create a config file with named backends (venice, chatgpt, claude, ollama, etc.)."
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
        .unwrap_or_else(|| root.join(".yo").join("sessions"));
    std::fs::create_dir_all(&transcripts_dir)?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", session_id));
    let transcript = transcript::Transcript::new(&transcript_path, &session_id, &root)?;

    let trace = args.trace;
    let backends = backend::BackendRegistry::new(&cfg);

    // Create policy engine from config
    let print_mode = args.prompt.is_some();
    let auto_yes = args.yes;
    let policy_engine = policy::PolicyEngine::new(cfg.permissions.clone(), print_mode, auto_yes);

    // Create MCP manager from config
    let mcp_manager = mcp::manager::McpManager::new(cfg.mcp.servers.clone());

    let ctx = cli::Context {
        args,
        root,
        transcript: RefCell::new(transcript),
        session_id,
        tracing: RefCell::new(trace),
        config: RefCell::new(cfg),
        backends: RefCell::new(backends),
        current_skill: RefCell::new("default".to_string()),
        policy: RefCell::new(policy_engine),
        mcp_manager: RefCell::new(mcp_manager),
    };

    if let Some(prompt) = &ctx.args.prompt {
        cli::run_once(&ctx, prompt)
    } else {
        cli::run_repl(ctx)
    }
}
