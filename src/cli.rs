use crate::{
    agent::{self, CommandStats},
    backend::BackendRegistry,
    commands::CommandIndex,
    compact,
    config::Config,
    config::PermissionMode,
    config::Target,
    cost::{format_cost, SessionCosts},
    hooks::HookManager,
    mcp::manager::McpManager,
    model_routing::ModelRouter,
    plan::{self, PlanModeState},
    policy::PolicyEngine,
    skillpacks::{ActiveSkills, SkillIndex},
    transcript::Transcript,
    Args,
};
use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Get the path to the history file
fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".yo")
        .join("history")
}

pub struct Context {
    pub args: Args,
    pub root: PathBuf,
    pub transcript: RefCell<Transcript>,
    pub session_id: String,
    pub tracing: RefCell<bool>,
    pub config: RefCell<Config>,
    pub backends: RefCell<BackendRegistry>,
    pub current_target: RefCell<Option<Target>>,
    pub policy: RefCell<PolicyEngine>,
    pub mcp_manager: RefCell<McpManager>,
    pub skill_index: RefCell<SkillIndex>,
    pub active_skills: RefCell<ActiveSkills>,
    pub model_router: RefCell<ModelRouter>,
    pub plan_mode: RefCell<PlanModeState>,
    pub hooks: RefCell<HookManager>,
    // Cost tracking
    pub session_costs: RefCell<SessionCosts>,
    pub turn_counter: RefCell<u32>,
    // Slash commands
    pub command_index: RefCell<CommandIndex>,
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

pub fn run_once(ctx: &Context, prompt: &str) -> Result<()> {
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
    let result = agent::run_turn(ctx, prompt, &mut messages)?;

    // Handle force_continue - run another turn if requested
    let mut total_stats = result.stats.clone();
    if result.force_continue {
        if let Some(continue_prompt) = result.continue_prompt {
            println!("[Continuing due to Stop hook...]");
            let continuation = agent::run_turn(ctx, &continue_prompt, &mut messages)?;
            total_stats.merge(&continuation.stats);
        }
    }

    // Get cost for this turn if cost tracking is enabled
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

pub fn run_repl(ctx: Context) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut messages = Vec::new();

    // Load command history
    let history_file = history_path();
    let _ = rl.load_history(&history_file);

    println!("yo - type /help for commands, /exit to quit");

    loop {
        match rl.readline(">>> ") {
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
                match agent::run_turn(&ctx, &line, &mut messages) {
                    Ok(result) => {
                        // Handle force_continue - run another turn if requested
                        let mut total_stats = result.stats.clone();
                        if result.force_continue {
                            if let Some(continue_prompt) = result.continue_prompt {
                                println!("[Continuing due to Stop hook...]");
                                if let Ok(continuation) =
                                    agent::run_turn(&ctx, &continue_prompt, &mut messages)
                                {
                                    total_stats.merge(&continuation.stats);
                                }
                            }
                        }

                        // Get cost for this turn if cost tracking is enabled
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

    // Save command history (create parent directory if needed)
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
            println!("  /session        - show session info");
            println!("  /clear          - clear conversation");
            println!("  /trace          - toggle tracing");
            println!("  /backends       - list configured backends");
            println!("  /target [t]     - show/set current target (model@backend)");
            println!("Permissions:");
            println!("  /mode [name]    - get/set permission mode (default|acceptEdits|bypassPermissions)");
            println!("  /permissions    - show permission rules");
            println!("  /permissions add allow|ask|deny \"pattern\"");
            println!("  /permissions rm allow|ask|deny <index>");
            println!("Context:");
            println!("  /context        - show context usage stats");
            println!("  /compact        - compact conversation history");
            println!("  /cost           - show session cost breakdown");
            println!("Subagents:");
            println!("  /agents                - list available subagents");
            println!("  /task <agent> <prompt> - run a subagent with the given prompt");
            println!("MCP (Model Context Protocol):");
            println!("  /mcp list              - list configured MCP servers");
            println!("  /mcp connect <name>    - connect to an MCP server");
            println!("  /mcp disconnect <name> - disconnect from an MCP server");
            println!("  /mcp tools <name>      - list tools from an MCP server");
            println!("Skill Packs:");
            println!("  /skillpacks            - list all skill packs");
            println!("  /skillpack show <name> - show skill details");
            println!("  /skillpack use <name>  - activate skill");
            println!("  /skillpack drop <name> - deactivate skill");
            println!("  /skillpack active      - list active skills");
            println!("Slash Commands:");
            println!("  /commands              - list user-defined commands");
            println!("  /<command> [args]      - run a user-defined command");
            println!("Plan Mode:");
            println!("  /plan <task>           - enter plan mode with a task");
            println!("  /plan                  - show current plan or help");
            println!("  /plan execute          - execute current plan step-by-step");
            println!("  /plan save [name]      - save plan to .yo/plans/");
            println!("  /plan cancel           - discard current plan");
            println!("  /plan list             - list saved plans");
            println!("  /plan load <name>      - load a saved plan");
            println!("  /plan run <name>       - load and execute a saved plan");
            println!("  /plan delete <name>    - delete a saved plan");
        }
        "/session" => {
            println!("Session: {}", ctx.session_id);
            println!("Transcript: {:?}", ctx.transcript.borrow().path);
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
        "/backends" => {
            println!("Configured backends:");
            for (name, backend) in ctx.backends.borrow().list_backends() {
                println!("  {}: {}", name, backend.base_url);
            }
        }
        "/target" => {
            if parts.len() > 1 {
                let target_str = parts[1].trim();
                if let Some(target) = Target::parse(target_str) {
                    if ctx.backends.borrow().has_backend(&target.backend) {
                        *ctx.current_target.borrow_mut() = Some(target.clone());
                        println!("Target set: {}", target);
                    } else {
                        println!(
                            "Unknown backend: {}. Use /backends to list.",
                            target.backend
                        );
                    }
                } else {
                    println!("Invalid target format. Use: model@backend");
                }
            } else {
                let current = ctx.current_target.borrow();
                if let Some(t) = current.as_ref() {
                    println!("Current target: {}", t);
                } else {
                    let config = ctx.config.borrow();
                    if let Some(t) = config.get_default_target() {
                        println!("Current target: {} (default)", t);
                    } else {
                        println!("No target configured. Use /target model@backend");
                    }
                }
            }
        }
        "/mode" => {
            if parts.len() > 1 {
                let mode_str = parts[1].trim();
                if let Some(mode) = PermissionMode::from_str(mode_str) {
                    ctx.policy.borrow_mut().set_mode(mode);
                    println!("Permission mode: {}", mode.as_str());
                } else {
                    println!("Unknown mode. Valid: default, acceptEdits, bypassPermissions");
                }
            } else {
                let mode = ctx.policy.borrow().mode();
                println!("Current mode: {}", mode.as_str());
            }
        }
        "/permissions" => {
            handle_permissions_command(ctx, if parts.len() > 1 { parts[1] } else { "" });
        }
        "/context" => {
            let total_chars: usize = messages
                .iter()
                .map(|m| serde_json::to_string(m).map(|s| s.len()).unwrap_or(0))
                .sum();
            let max_chars = ctx.config.borrow().context.max_chars;
            let usage_pct = (total_chars as f64 / max_chars as f64) * 100.0;
            println!("Context usage:");
            println!("  Messages: {} ({} chars)", messages.len(), total_chars);
            println!("  Max: {} chars", max_chars);
            println!("  Usage: {:.1}%", usage_pct);
            if compact::needs_compaction(messages, &ctx.config.borrow().context) {
                println!("  ⚠️  Compaction recommended. Run /compact");
            }
        }
        "/compact" => {
            handle_compact_command(ctx, messages);
        }
        "/cost" => {
            handle_cost_command(ctx);
        }
        "/commands" => {
            handle_commands_list(ctx);
        }
        "/mcp" => {
            handle_mcp_command(ctx, if parts.len() > 1 { parts[1] } else { "" });
        }
        "/agents" => {
            handle_agents_command(ctx);
        }
        "/task" => {
            handle_task_command(ctx, if parts.len() > 1 { parts[1] } else { "" });
        }
        "/skillpacks" => {
            handle_skillpacks_command(ctx);
        }
        "/skillpack" => {
            handle_skillpack_command(ctx, if parts.len() > 1 { parts[1] } else { "" });
        }
        "/plan" => {
            handle_plan_command(ctx, if parts.len() > 1 { parts[1] } else { "" }, messages);
        }
        _ => {
            // Check for user-defined slash commands
            let cmd_name = &parts[0][1..]; // Remove leading /
            let args = if parts.len() > 1 { parts[1] } else { "" };
            if !try_run_slash_command(ctx, cmd_name, args, messages) {
                println!("Unknown command: {}", parts[0]);
            }
        }
    }
    false
}

fn handle_permissions_command(ctx: &Context, args: &str) {
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.is_empty() {
        // Show current permissions
        let policy = ctx.policy.borrow();
        let config = policy.config();
        println!("Mode: {}", config.mode.as_str());
        println!("\nAllow rules:");
        for (i, rule) in config.allow.iter().enumerate() {
            println!("  [{}] {}", i, rule);
        }
        println!("\nAsk rules:");
        for (i, rule) in config.ask.iter().enumerate() {
            println!("  [{}] {}", i, rule);
        }
        println!("\nDeny rules:");
        for (i, rule) in config.deny.iter().enumerate() {
            println!("  [{}] {}", i, rule);
        }
        return;
    }

    match parts[0] {
        "add" if parts.len() >= 3 => {
            let decision_type = parts[1];
            // Join remaining parts and strip quotes
            let pattern = parts[2..].join(" ");
            let pattern = pattern.trim_matches('"').to_string();

            let mut policy = ctx.policy.borrow_mut();
            let config = policy.config_mut();

            match decision_type {
                "allow" => {
                    config.allow.push(pattern.clone());
                    println!("Added allow rule: {}", pattern);
                }
                "ask" => {
                    config.ask.push(pattern.clone());
                    println!("Added ask rule: {}", pattern);
                }
                "deny" => {
                    config.deny.push(pattern.clone());
                    println!("Added deny rule: {}", pattern);
                }
                _ => {
                    println!("Invalid decision type. Use: allow, ask, deny");
                    return;
                }
            }
            drop(policy);

            // Save to local config
            if let Err(e) = ctx.config.borrow().save_local_permissions() {
                eprintln!("Warning: failed to save permissions: {}", e);
            }
        }
        "rm" if parts.len() >= 3 => {
            let decision_type = parts[1];
            if let Ok(idx) = parts[2].parse::<usize>() {
                let mut policy = ctx.policy.borrow_mut();
                let config = policy.config_mut();

                let removed = match decision_type {
                    "allow" if idx < config.allow.len() => Some(config.allow.remove(idx)),
                    "ask" if idx < config.ask.len() => Some(config.ask.remove(idx)),
                    "deny" if idx < config.deny.len() => Some(config.deny.remove(idx)),
                    _ => None,
                };

                if let Some(rule) = removed {
                    println!("Removed {} rule: {}", decision_type, rule);
                    drop(policy);
                    if let Err(e) = ctx.config.borrow().save_local_permissions() {
                        eprintln!("Warning: failed to save permissions: {}", e);
                    }
                } else {
                    println!("Rule not found at index {}", idx);
                }
            } else {
                println!("Invalid index: {}", parts[2]);
            }
        }
        _ => {
            println!("Usage:");
            println!("  /permissions                    - show current rules");
            println!("  /permissions add allow|ask|deny \"pattern\"");
            println!("  /permissions rm allow|ask|deny <index>");
        }
    }
}

fn handle_mcp_command(ctx: &Context, args: &str) {
    let parts: Vec<&str> = args.split_whitespace().collect();

    match parts.first().copied() {
        Some("list") | None => {
            let manager = ctx.mcp_manager.borrow();
            let servers = manager.list_servers();
            if servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Add servers to .yo/config.toml under [mcp.servers.<name>]");
            } else {
                println!("MCP Servers:");
                for (name, config, connected) in servers {
                    let status = if connected { "[connected]" } else { "" };
                    let enabled = if config.enabled { "" } else { " (disabled)" };
                    println!("  {} - {}{} {}", name, config.command, enabled, status);
                }
            }
        }
        Some("connect") if parts.len() >= 2 => {
            let name = parts[1];
            let mut manager = ctx.mcp_manager.borrow_mut();
            match manager.connect(name, &ctx.root) {
                Ok((pid, tool_count)) => {
                    println!("Connected to MCP server: {}", name);
                    println!("  PID: {}", pid);
                    println!("  Tools discovered: {}", tool_count);
                    // Log to transcript
                    let config = ctx.config.borrow();
                    if let Some(server_config) = config.mcp.servers.get(name) {
                        let _ = ctx.transcript.borrow_mut().mcp_server_start(
                            name,
                            &server_config.command,
                            pid,
                        );
                    }
                    let _ = ctx.transcript.borrow_mut().mcp_initialize_ok(name);
                    let _ = ctx.transcript.borrow_mut().mcp_tools_list(name, tool_count);
                }
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", name, e);
                    let _ = ctx
                        .transcript
                        .borrow_mut()
                        .mcp_initialize_err(name, &e.to_string());
                }
            }
        }
        Some("disconnect") if parts.len() >= 2 => {
            let name = parts[1];
            let mut manager = ctx.mcp_manager.borrow_mut();
            match manager.disconnect(name) {
                Ok(()) => {
                    println!("Disconnected from MCP server: {}", name);
                    let _ = ctx.transcript.borrow_mut().mcp_server_stop(name);
                }
                Err(e) => {
                    eprintln!("Failed to disconnect from {}: {}", name, e);
                }
            }
        }
        Some("tools") if parts.len() >= 2 => {
            let name = parts[1];
            let manager = ctx.mcp_manager.borrow();
            let tools = manager.get_server_tools(name);
            if tools.is_empty() {
                if manager.is_connected(name) {
                    println!("Server {} has no tools.", name);
                } else {
                    println!(
                        "Server {} is not connected. Use '/mcp connect {}'",
                        name, name
                    );
                }
            } else {
                println!("Tools from {}:", name);
                for tool in tools {
                    println!("  {} - {}", tool.full_name, tool.description);
                }
            }
        }
        _ => {
            println!("MCP commands:");
            println!("  /mcp list              - list configured MCP servers");
            println!("  /mcp connect <name>    - connect to an MCP server");
            println!("  /mcp disconnect <name> - disconnect from an MCP server");
            println!("  /mcp tools <name>      - list tools from an MCP server");
        }
    }
}

fn handle_cost_command(ctx: &Context) {
    use crate::cost::format_tokens;

    let costs = ctx.session_costs.borrow();
    let total_cost = costs.total_cost();
    let total_tokens = costs.total_tokens();

    println!("Session Cost Summary");
    println!("────────────────────");
    println!(
        "Total: {} ({} tokens)",
        format_cost(total_cost),
        format_tokens(total_tokens)
    );

    // Breakdown by model
    let by_model = costs.cost_by_model();
    if !by_model.is_empty() {
        println!("\nBy Model:");
        let mut models: Vec<_> = by_model.iter().collect();
        models.sort_by(|a, b| {
            b.1 .1
                .partial_cmp(&a.1 .1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (model, (tokens, cost)) in models {
            println!(
                "  {}: {} ({} tokens)",
                model,
                format_cost(*cost),
                format_tokens(*tokens)
            );
        }
    }

    // Breakdown by turn
    let turns = costs.turns();
    if !turns.is_empty() {
        println!("\nBy Turn:");
        for turn in turns {
            println!(
                "  Turn {}: {} ({} tokens)",
                turn.turn_number,
                format_cost(turn.total_cost()),
                format_tokens(turn.total_tokens())
            );
        }
    }

    // Check for warning threshold
    if let Some(threshold) = ctx.config.borrow().cost_tracking.warn_threshold_usd {
        if total_cost > threshold {
            println!(
                "\n⚠️  Session cost exceeds threshold of {}",
                format_cost(threshold)
            );
        }
    }
}

fn handle_agents_command(ctx: &Context) {
    let config = ctx.config.borrow();
    if config.agents.is_empty() {
        println!("No subagents configured.");
        println!("Add agent definitions to .yo/agents/<name>.toml");
    } else {
        println!("Available subagents:");
        for (name, spec) in &config.agents {
            println!(
                "  {} - {} [tools: {}]",
                name,
                spec.description,
                spec.allowed_tools.join(", ")
            );
        }
    }
}

fn handle_task_command(ctx: &Context, args: &str) {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();

    if parts.is_empty() || parts[0].is_empty() {
        println!("Usage: /task <agent> <prompt>");
        println!("Run '/agents' to see available subagents.");
        return;
    }

    let agent_name = parts[0];
    let prompt = if parts.len() > 1 { parts[1] } else { "" };

    if prompt.is_empty() {
        println!("Error: prompt is required");
        println!("Usage: /task <agent> <prompt>");
        return;
    }

    // Get agent spec
    let config = ctx.config.borrow();
    let spec = match config.agents.get(agent_name) {
        Some(s) => s.clone(),
        None => {
            let available: Vec<&String> = config.agents.keys().collect();
            println!(
                "Agent '{}' not found. Available agents: {:?}",
                agent_name, available
            );
            return;
        }
    };
    drop(config);

    println!("Running subagent '{}'...", agent_name);

    // Run the subagent
    let start = Instant::now();
    match crate::subagent::run_subagent(ctx, &spec, prompt, None) {
        Ok((result, stats)) => {
            if result.ok {
                println!("\n--- Subagent Output ---");
                println!("{}", result.output.text);
                if !result.output.files_referenced.is_empty() {
                    println!("\nFiles referenced: {:?}", result.output.files_referenced);
                }
                if !result.output.proposed_edits.is_empty() {
                    println!("\nProposed edits: {}", result.output.proposed_edits.len());
                }
            } else if let Some(error) = &result.error {
                println!("Subagent error: {} - {}", error.code, error.message);
            }
            // TODO: Add cost tracking for explicit /task commands
            print_stats(start.elapsed(), &stats, None);
        }
        Err(e) => {
            eprintln!("Failed to run subagent: {}", e);
        }
    }
}

fn handle_skillpacks_command(ctx: &Context) {
    use crate::skillpacks::index::SkillSource;

    let index = ctx.skill_index.borrow();
    if index.count() == 0 {
        println!("No skill packs found.");
        println!("Add skills to .yo/skills/<name>/SKILL.md");
    } else {
        println!("Skill Packs ({}):", index.count());
        for meta in index.all() {
            let source = match meta.source {
                SkillSource::Project => "[project]",
                SkillSource::User => "[user]",
            };
            println!("  {} {} - {}", meta.name, source, meta.description);
        }
    }

    // Show parse errors if any
    for (path, error) in index.errors() {
        eprintln!("  [error] {}: {}", path.display(), error);
    }
}

fn handle_skillpack_command(ctx: &Context, args: &str) {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();

    if parts.is_empty() || parts[0].is_empty() {
        println!("Usage:");
        println!("  /skillpack show <name>  - show skill details");
        println!("  /skillpack use <name>   - activate skill");
        println!("  /skillpack drop <name>  - deactivate skill");
        println!("  /skillpack active       - list active skills");
        return;
    }

    match parts[0] {
        "show" if parts.len() > 1 => {
            let name = parts[1].trim();
            let index = ctx.skill_index.borrow();
            if let Some(meta) = index.get(name) {
                println!("Skill: {}", meta.name);
                println!("Description: {}", meta.description);
                if let Some(tools) = &meta.allowed_tools {
                    println!("Allowed tools: {}", tools.join(", "));
                }
                println!("Path: {}", meta.path.display());
            } else {
                println!("Skill '{}' not found", name);
            }
        }
        "use" if parts.len() > 1 => {
            let name = parts[1].trim();
            let index = ctx.skill_index.borrow();
            let mut active = ctx.active_skills.borrow_mut();
            match active.activate(name, &index) {
                Ok(activation) => {
                    println!("Activated skill: {}", activation.name);
                    let _ = ctx.transcript.borrow_mut().skill_activate(
                        &activation.name,
                        None,
                        activation.allowed_tools.as_ref(),
                    );
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        "drop" if parts.len() > 1 => {
            let name = parts[1].trim();
            let mut active = ctx.active_skills.borrow_mut();
            match active.deactivate(name) {
                Ok(()) => {
                    println!("Deactivated skill: {}", name);
                    let _ = ctx.transcript.borrow_mut().skill_deactivate(name);
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        "active" => {
            let active = ctx.active_skills.borrow();
            let names = active.list();
            if names.is_empty() {
                println!("No active skills");
            } else {
                println!("Active skills: {}", names.join(", "));
            }
        }
        _ => {
            println!("Unknown subcommand. Use: show, use, drop, active");
        }
    }
}

fn handle_plan_command(ctx: &Context, args: &str, messages: &mut Vec<serde_json::Value>) {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let subcommand = parts.first().copied().unwrap_or("");

    match subcommand {
        "" => {
            // Show current plan status or help
            let state = ctx.plan_mode.borrow();
            if state.active {
                if let Some(plan) = &state.current_plan {
                    println!("{}", plan.format_display());
                    println!("Options:");
                    println!("  /plan execute  - execute the plan step-by-step");
                    println!("  /plan save     - save plan to .yo/plans/");
                    println!("  /plan cancel   - discard current plan");
                }
            } else {
                println!("Plan Mode Commands:");
                println!("  /plan <task>        - enter plan mode with a task");
                println!("  /plan execute       - execute the current plan");
                println!("  /plan save [name]   - save plan to .yo/plans/");
                println!("  /plan cancel        - discard current plan");
                println!("  /plan list          - list saved plans");
                println!("  /plan load <name>   - load a saved plan");
                println!("  /plan run <name>    - load and execute a plan");
                println!("  /plan delete <name> - delete a saved plan");
            }
        }

        "execute" => {
            handle_plan_execute(ctx, messages);
        }

        "save" => {
            let name = parts.get(1).map(|s| s.trim());
            handle_plan_save(ctx, name);
        }

        "cancel" => {
            handle_plan_cancel(ctx);
        }

        "list" => {
            handle_plan_list(ctx);
        }

        "load" => {
            if let Some(name) = parts.get(1) {
                handle_plan_load(ctx, name.trim());
            } else {
                println!("Usage: /plan load <name>");
            }
        }

        "run" => {
            if let Some(name) = parts.get(1) {
                handle_plan_run(ctx, name.trim(), messages);
            } else {
                println!("Usage: /plan run <name>");
            }
        }

        "delete" => {
            if let Some(name) = parts.get(1) {
                handle_plan_delete(ctx, name.trim());
            } else {
                println!("Usage: /plan delete <name>");
            }
        }

        _ => {
            // Treat as a task description - enter planning mode
            let goal = args.to_string();
            handle_plan_start(ctx, goal, messages);
        }
    }
}

fn handle_plan_start(ctx: &Context, goal: String, messages: &mut Vec<serde_json::Value>) {
    if goal.is_empty() {
        println!("Usage: /plan <task description>");
        return;
    }

    // Check if already in plan mode
    {
        let state = ctx.plan_mode.borrow();
        if state.active {
            println!("Already in plan mode. Use /plan cancel to exit first.");
            return;
        }
    }

    // Enter planning mode
    ctx.plan_mode.borrow_mut().enter_planning(goal.clone());
    println!("[Plan Mode] Entering planning mode...");
    println!("[Plan Mode] Goal: {}", goal);
    println!("[Plan Mode] Using read-only tools to explore the codebase...\n");

    // Log to transcript
    let _ = ctx.transcript.borrow_mut().plan_mode_start(&goal);

    // Increment turn counter for plan mode
    let turn_number = {
        let mut counter = ctx.turn_counter.borrow_mut();
        *counter += 1;
        *counter
    };

    // Run the planning turn
    let start = Instant::now();
    match agent::run_turn(ctx, &goal, messages) {
        Ok(result) => {
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
            print_stats(start.elapsed(), &result.stats, cost);

            // Check if we got a plan
            let state = ctx.plan_mode.borrow();
            if let Some(plan) = &state.current_plan {
                if !plan.steps.is_empty() {
                    println!("\n{}", plan.format_display());
                    println!("Options:");
                    println!("  /plan execute  - execute the plan step-by-step");
                    println!("  /plan save     - save plan to .yo/plans/");
                    println!("  /plan cancel   - discard current plan");
                }
            }
        }
        Err(e) => {
            eprintln!("Planning error: {}", e);
            ctx.plan_mode.borrow_mut().exit();
        }
    }
}

fn handle_plan_execute(ctx: &Context, messages: &mut Vec<serde_json::Value>) {
    // Check if we have a plan to execute
    {
        let state = ctx.plan_mode.borrow();
        if !state.active || state.current_plan.is_none() {
            println!("No plan to execute. Use /plan <task> to create one, or /plan load <name> to load a saved plan.");
            return;
        }
    }

    // Enter executing phase
    ctx.plan_mode.borrow_mut().enter_executing();
    println!("[Plan Mode] Starting plan execution...\n");

    // Execute steps one at a time
    loop {
        // Get next step
        let next_step = {
            let state = ctx.plan_mode.borrow();
            state
                .current_plan
                .as_ref()
                .and_then(|p| p.next_step())
                .cloned()
        };

        let Some(step) = next_step else {
            // All steps done
            let state = ctx.plan_mode.borrow();
            if let Some(plan) = &state.current_plan {
                println!("\n[Plan Mode] Plan execution complete!");
                println!(
                    "  Completed: {}, Failed: {}",
                    plan.completed_count(),
                    plan.failed_count()
                );

                // Log completion
                let _ = ctx.transcript.borrow_mut().plan_complete(
                    &plan.name,
                    plan.completed_count(),
                    plan.failed_count(),
                );
            }
            drop(state);
            ctx.plan_mode.borrow_mut().enter_review();
            return;
        };

        println!(
            "=== Step {}: {} ===\n{}",
            step.number, step.title, step.description
        );
        if !step.files.is_empty() {
            println!("Files: {}", step.files.join(", "));
        }
        println!();

        // Mark step as in progress
        {
            let mut state = ctx.plan_mode.borrow_mut();
            if let Some(plan) = &mut state.current_plan {
                if let Some(s) = plan.step_mut(step.number) {
                    s.status = plan::PlanStepStatus::InProgress;
                }
            }
        }

        // Log step start
        let plan_name = ctx
            .plan_mode
            .borrow()
            .current_plan
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let _ = ctx
            .transcript
            .borrow_mut()
            .plan_step_start(&plan_name, step.number, &step.title);

        // Build prompt for this step
        let prompt = format!(
            "Execute Step {}: {}\n\n{}\n\nFiles to work with: {}",
            step.number,
            step.title,
            step.description,
            if step.files.is_empty() {
                "(none specified)".to_string()
            } else {
                step.files.join(", ")
            }
        );

        // Increment turn counter for plan step
        let turn_number = {
            let mut counter = ctx.turn_counter.borrow_mut();
            *counter += 1;
            *counter
        };

        // Execute the step
        let start = Instant::now();
        let turn_result = agent::run_turn(ctx, &prompt, messages);

        // Update step status based on result
        let step_status = if turn_result.is_ok() {
            plan::PlanStepStatus::Completed
        } else {
            plan::PlanStepStatus::Failed
        };

        {
            let mut state = ctx.plan_mode.borrow_mut();
            if let Some(plan) = &mut state.current_plan {
                if let Some(s) = plan.step_mut(step.number) {
                    s.status = step_status;
                }
            }
        }

        // Log step end
        let _ = ctx.transcript.borrow_mut().plan_step_end(
            &plan_name,
            step.number,
            if turn_result.is_ok() {
                "completed"
            } else {
                "failed"
            },
        );

        match turn_result {
            Ok(result) => {
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
                print_stats(start.elapsed(), &result.stats, cost);
                println!("\nStep {} complete.", step.number);
            }
            Err(e) => {
                eprintln!("Step {} failed: {}", step.number, e);
            }
        }

        // Ask user to continue
        print!("Continue with next step? [Y/n]: ");
        use std::io::{self, Write};
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Stopping execution.");
            break;
        }

        let input = input.trim().to_lowercase();
        if input == "n" || input == "no" {
            println!("Stopping execution. Use /plan execute to continue later.");
            ctx.plan_mode.borrow_mut().enter_review();
            return;
        }
    }
}

fn handle_plan_save(ctx: &Context, name: Option<&str>) {
    let state = ctx.plan_mode.borrow();
    let Some(plan) = &state.current_plan else {
        println!("No plan to save.");
        return;
    };

    // Clone the plan so we can modify it if needed
    let mut plan_to_save = plan.clone();
    drop(state);

    // Use provided name or the auto-generated one
    if let Some(n) = name {
        if !n.is_empty() {
            plan_to_save.name = n.to_string();
        }
    }

    match plan::save_plan(&plan_to_save, &ctx.root) {
        Ok(path) => {
            println!("Plan saved to: {}", path.display());
            let _ = ctx
                .transcript
                .borrow_mut()
                .plan_saved(&plan_to_save.name, &path);
        }
        Err(e) => {
            eprintln!("Failed to save plan: {}", e);
        }
    }
}

fn handle_plan_cancel(ctx: &Context) {
    let was_active = ctx.plan_mode.borrow().active;
    ctx.plan_mode.borrow_mut().exit();

    if was_active {
        println!("Plan cancelled.");
    } else {
        println!("No active plan.");
    }
}

fn handle_plan_list(ctx: &Context) {
    match plan::list_plans(&ctx.root) {
        Ok(plans) => {
            if plans.is_empty() {
                println!("No saved plans.");
                println!("Use /plan <task> to create a new plan.");
            } else {
                println!("Saved Plans:");
                for p in plans {
                    let goal_preview: String = p.goal.chars().take(50).collect();
                    let ellipsis = if p.goal.len() > 50 { "..." } else { "" };
                    println!(
                        "  {} [{}] \"{}{}\" ({} steps)",
                        p.name,
                        p.status.as_str(),
                        goal_preview,
                        ellipsis,
                        p.step_count
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to list plans: {}", e);
        }
    }
}

fn handle_plan_load(ctx: &Context, name: &str) {
    // Check if already in plan mode
    {
        let state = ctx.plan_mode.borrow();
        if state.active {
            println!("Already in plan mode. Use /plan cancel to exit first.");
            return;
        }
    }

    match plan::load_plan(name, &ctx.root) {
        Ok(plan) => {
            println!("Loaded plan: {}", plan.name);
            println!("{}", plan.format_display());

            let _ = ctx.transcript.borrow_mut().plan_loaded(&plan.name);
            ctx.plan_mode.borrow_mut().load_plan(plan);

            println!("Options:");
            println!("  /plan execute  - execute the plan step-by-step");
            println!("  /plan cancel   - discard current plan");
        }
        Err(e) => {
            eprintln!("Failed to load plan: {}", e);
        }
    }
}

fn handle_plan_run(ctx: &Context, name: &str, messages: &mut Vec<serde_json::Value>) {
    // Load the plan first
    handle_plan_load(ctx, name);

    // If loaded successfully, execute it
    if ctx.plan_mode.borrow().active {
        handle_plan_execute(ctx, messages);
    }
}

fn handle_plan_delete(ctx: &Context, name: &str) {
    match plan::delete_plan(name, &ctx.root) {
        Ok(()) => {
            println!("Deleted plan: {}", name);
        }
        Err(e) => {
            eprintln!("Failed to delete plan: {}", e);
        }
    }
}

fn handle_compact_command(ctx: &Context, messages: &mut Vec<serde_json::Value>) {
    if messages.is_empty() {
        println!("No messages to compact.");
        return;
    }

    // Get target and client
    let target = {
        let current = ctx.current_target.borrow();
        if let Some(t) = current.as_ref() {
            t.clone()
        } else {
            match ctx.config.borrow().get_default_target() {
                Some(t) => t,
                None => {
                    println!("No target configured. Use /target to set one.");
                    return;
                }
            }
        }
    };

    println!("Compacting conversation...");

    // Get context config before borrowing backends
    let context_config = ctx.config.borrow().context.clone();

    // Get client and perform compaction - capture result and release borrow
    let compact_result = {
        let mut backends = ctx.backends.borrow_mut();
        let client = match backends.get_client(&target.backend) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to get client: {}", e);
                return;
            }
        };

        compact::compact_messages(messages, &context_config, client, &target.model)
    };

    match compact_result {
        Ok((compacted, result)) => {
            *messages = compacted;
            println!("{}", compact::format_result(&result));
            if !result.summary.is_empty() {
                println!("\nSummary:\n{}", result.summary);
            }
        }
        Err(e) => {
            eprintln!("Compaction failed: {}", e);
        }
    }
}

fn handle_commands_list(ctx: &Context) {
    use crate::commands::CommandSource;

    let index = ctx.command_index.borrow();
    let commands = index.list();

    if commands.is_empty() {
        println!("No slash commands defined.");
        println!("Add commands to .yo/commands/<name>.md");
    } else {
        println!("Slash Commands ({}):", commands.len());
        for cmd in commands {
            let source = match cmd.source {
                CommandSource::Project => "[project]",
                CommandSource::User => "[user]",
            };
            let desc = cmd
                .meta
                .description
                .as_deref()
                .unwrap_or("(no description)");
            println!("  /{} {} - {}", cmd.name, source, desc);
        }
    }

    // Show errors if any
    for (path, error) in index.errors() {
        eprintln!("  [error] {}: {}", path.display(), error);
    }
}

/// Try to run a user-defined slash command
/// Returns true if a command was found and executed
fn try_run_slash_command(
    ctx: &Context,
    cmd_name: &str,
    args: &str,
    messages: &mut Vec<serde_json::Value>,
) -> bool {
    let command = {
        let index = ctx.command_index.borrow();
        index.get(cmd_name).cloned()
    };

    let Some(command) = command else {
        return false;
    };

    // Expand the command with arguments
    let prompt = command.expand(args);

    println!("Running command: /{}", cmd_name);
    if ctx.args.verbose {
        println!("Expanded prompt: {}", prompt);
    }

    // Run UserPromptSubmit hooks
    let (proceed, updated_prompt) = ctx.hooks.borrow().user_prompt_submit(&prompt);
    if !proceed {
        eprintln!("Command blocked by hook");
        return true;
    }
    let prompt = updated_prompt.unwrap_or(prompt);

    // Increment turn counter
    let turn_number = {
        let mut counter = ctx.turn_counter.borrow_mut();
        *counter += 1;
        *counter
    };

    // Run the command as a regular prompt
    let start = Instant::now();
    match agent::run_turn(ctx, &prompt, messages) {
        Ok(result) => {
            // Handle force_continue
            let mut total_stats = result.stats.clone();
            if result.force_continue {
                if let Some(continue_prompt) = result.continue_prompt {
                    println!("[Continuing due to Stop hook...]");
                    if let Ok(continuation) = agent::run_turn(ctx, &continue_prompt, messages) {
                        total_stats.merge(&continuation.stats);
                    }
                }
            }

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
            eprintln!("Command error: {}", e);
        }
    }

    true
}
