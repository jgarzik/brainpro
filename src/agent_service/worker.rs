//! Worker that wraps agent.rs to emit streaming NDJSON events.

use crate::protocol::internal::{AgentEvent, AgentMethod, AgentRequest, UsageStats};
use clap::Parser;
use std::sync::mpsc;

/// Result of running a worker task
pub struct WorkerHandle {
    /// Channel to receive streaming events
    pub events: mpsc::Receiver<AgentEvent>,
}

/// Run the agent in a blocking fashion, sending events to a channel.
/// This wraps the existing synchronous agent::run_turn function.
pub fn run_agent_task(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), String> {
    let id = &request.id;

    match request.method {
        AgentMethod::Ping => {
            let _ = event_tx.send(AgentEvent::pong(id));
            Ok(())
        }
        AgentMethod::Cancel => {
            // Cancel is handled at the server level
            let _ = event_tx.send(AgentEvent::error(
                id,
                "not_implemented",
                "Cancel should be handled by server",
            ));
            Ok(())
        }
        AgentMethod::RunTurn => run_turn_task(request, event_tx),
    }
}

fn run_turn_task(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), String> {
    use crate::backend::BackendRegistry;
    use crate::cli::Context;
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
    use crate::Args;
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use std::path::PathBuf;

    let id = &request.id;

    // Parse working directory
    let root = request
        .working_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Load config
    let cfg = Config::load().map_err(|e| format!("Config load failed: {}", e))?;

    // Parse target
    let target = request
        .target
        .as_ref()
        .and_then(|t| Target::parse(t))
        .or_else(|| cfg.get_default_target());

    if target.is_none() {
        let _ = event_tx.send(AgentEvent::error(
            id,
            "no_target",
            "No target configured and none provided",
        ));
        return Ok(());
    }

    // Create minimal Args for the context
    let args = Args::parse_from(["brainpro"]);

    // Create transcript directory if needed
    let transcripts_dir = root.join(".brainpro").join("sessions");
    let _ = std::fs::create_dir_all(&transcripts_dir);

    // Initialize transcript
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", request.session_id));
    let transcript = match Transcript::new(&transcript_path, &request.session_id, &root) {
        Ok(t) => t,
        Err(e) => {
            let _ = event_tx.send(AgentEvent::error(
                id,
                "transcript_error",
                &format!("Failed to create transcript: {}", e),
            ));
            return Ok(());
        }
    };

    // Initialize components
    let policy = PolicyEngine::new(cfg.permissions.clone(), false, false);
    let hooks = HookManager::new(cfg.hooks.clone(), request.session_id.clone(), root.clone());
    let skill_index = SkillIndex::build(&root);
    let model_router = ModelRouter::new(cfg.model_routing.clone());
    let command_index = CommandIndex::build(&root);
    let pricing = PricingTable::from_config(&cfg.model_pricing);
    let session_costs = SessionCosts::new(request.session_id.clone(), pricing);

    // Build context
    let ctx = Context {
        args,
        root: root.clone(),
        transcript: RefCell::new(transcript),
        session_id: request.session_id.clone(),
        tracing: RefCell::new(false),
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

    // Convert request messages to mutable vec
    let mut messages: Vec<Value> = request.messages;

    // Extract user message from the last message if it exists
    let user_input = messages
        .last()
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // If messages is empty or doesn't have a user message, we need input
    if user_input.is_empty() {
        let _ = event_tx.send(AgentEvent::error(id, "no_input", "No user message provided"));
        return Ok(());
    }

    // Remove the last user message since run_turn will add it
    if !messages.is_empty() {
        messages.pop();
    }

    // Run the agent turn
    match crate::agent::run_turn(&ctx, &user_input, &mut messages) {
        Ok(result) => {
            // Check for pending questions
            if let Some(pending) = result.pending_question {
                let questions: Vec<Value> = pending
                    .questions
                    .iter()
                    .map(|q| {
                        json!({
                            "question": q.question,
                            "header": q.header,
                            "options": q.options.iter().map(|o| {
                                json!({
                                    "label": o.label,
                                    "description": o.description
                                })
                            }).collect::<Vec<_>>(),
                            "multi_select": q.multi_select
                        })
                    })
                    .collect();

                let _ = event_tx.send(AgentEvent::awaiting_input(
                    id,
                    &pending.tool_call_id,
                    questions,
                ));
            }

            // Send done event with usage stats
            let usage = UsageStats {
                input_tokens: result.stats.input_tokens,
                output_tokens: result.stats.output_tokens,
                tool_uses: result.stats.tool_uses,
            };
            let _ = event_tx.send(AgentEvent::done(id, usage));
        }
        Err(e) => {
            let _ = event_tx.send(AgentEvent::error(id, "agent_error", &e.to_string()));
        }
    }

    Ok(())
}

/// Spawn a worker task in a new thread
pub fn spawn_worker(request: AgentRequest) -> WorkerHandle {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        if let Err(e) = run_agent_task(request, tx) {
            eprintln!("[worker] Error: {}", e);
        }
    });

    WorkerHandle { events: rx }
}
