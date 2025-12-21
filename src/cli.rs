use crate::{
    agent, backend::BackendRegistry, config::Config, config::PermissionMode,
    mcp::manager::McpManager, policy::PolicyEngine, transcript::Transcript, Args,
};
use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::path::PathBuf;

pub struct Context {
    pub args: Args,
    pub root: PathBuf,
    pub transcript: RefCell<Transcript>,
    pub session_id: String,
    pub tracing: RefCell<bool>,
    pub config: RefCell<Config>,
    pub backends: RefCell<BackendRegistry>,
    pub current_skill: RefCell<String>,
    pub policy: RefCell<PolicyEngine>,
    pub mcp_manager: RefCell<McpManager>,
}

pub fn run_once(ctx: &Context, prompt: &str) -> Result<()> {
    let mut messages = Vec::new();
    agent::run_turn(ctx, prompt, &mut messages)?;
    Ok(())
}

pub fn run_repl(ctx: Context) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut messages = Vec::new();

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

                if let Err(e) = agent::run_turn(&ctx, line, &mut messages) {
                    eprintln!("Error: {}", e);
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Input error: {}", e);
                break;
            }
        }
    }

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
            println!("  /skills         - show skill → target routing");
            println!("  /skill <name>   - set current skill");
            println!("  /target <t>     - override current target");
            println!("Permissions:");
            println!("  /mode [name]    - get/set permission mode (default|acceptEdits|bypassPermissions)");
            println!("  /permissions    - show permission rules");
            println!("  /permissions add allow|ask|deny \"pattern\"");
            println!("  /permissions rm allow|ask|deny <index>");
            println!("Context:");
            println!("  /context        - show context usage stats");
            println!("Subagents:");
            println!("  /agents                - list available subagents");
            println!("  /task <agent> <prompt> - run a subagent with the given prompt");
            println!("MCP (Model Context Protocol):");
            println!("  /mcp list              - list configured MCP servers");
            println!("  /mcp connect <name>    - connect to an MCP server");
            println!("  /mcp disconnect <name> - disconnect from an MCP server");
            println!("  /mcp tools <name>      - list tools from an MCP server");
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
        "/skills" => {
            let current = ctx.current_skill.borrow();
            let config = ctx.config.borrow();
            println!("Skills → Targets:");
            for (skill, target) in &config.skills {
                let marker = if skill == &*current { " *" } else { "" };
                println!("  {}: {}{}", skill, target, marker);
            }
        }
        "/skill" => {
            if parts.len() > 1 {
                let skill = parts[1].trim();
                let config = ctx.config.borrow();
                if config.skills.contains_key(skill) {
                    drop(config);
                    *ctx.current_skill.borrow_mut() = skill.to_string();
                    if let Some(target) = ctx.config.borrow().resolve_skill(skill) {
                        println!("Switched to skill: {} ({})", skill, target);
                    }
                } else {
                    println!(
                        "Unknown skill: {}. Available: {:?}",
                        skill,
                        config.skills.keys().collect::<Vec<_>>()
                    );
                }
            } else {
                let skill = ctx.current_skill.borrow();
                let config = ctx.config.borrow();
                if let Some(target) = config.resolve_skill(&skill) {
                    println!("Current skill: {} ({})", skill, target);
                } else {
                    println!("Current skill: {}", skill);
                }
            }
        }
        "/target" => {
            if parts.len() > 1 {
                let target_str = parts[1].trim();
                if let Some(target) = crate::config::Target::parse(target_str) {
                    if ctx.backends.borrow().has_backend(&target.backend) {
                        // Override the default skill's target
                        println!("Target override: {} (use /skill to switch skills)", target);
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
                let skill = ctx.current_skill.borrow();
                let config = ctx.config.borrow();
                if let Some(target) = config.resolve_skill(&skill) {
                    println!("Current target: {} (skill: {})", target, skill);
                } else {
                    println!("No target configured for skill: {}", skill);
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
        _ => println!("Unknown command: {}", parts[0]),
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
    match crate::subagent::run_subagent(ctx, &spec, prompt, None) {
        Ok(result) => {
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
        }
        Err(e) => {
            eprintln!("Failed to run subagent: {}", e);
        }
    }
}
