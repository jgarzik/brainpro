//! Worker that wraps agent.rs to emit streaming NDJSON events.
//!
//! In gateway mode, the worker yields when tool approval is needed,
//! saving state for later resumption.

use crate::agent_service::turn_state::{PendingToolCall, TurnState, TurnStateStore};
use crate::protocol::internal::{
    AgentEvent, AgentMethod, AgentRequest, UsageStats, YieldReason,
};
use clap::Parser;
use serde_json::{json, Value};
use std::sync::{mpsc, Arc};

/// Result of running a worker task
pub struct WorkerHandle {
    /// Channel to receive streaming events
    pub events: mpsc::Receiver<AgentEvent>,
}

/// Configuration for the worker
pub struct WorkerConfig {
    /// Whether running in gateway mode (yields on ask decisions)
    pub gateway_mode: bool,
    /// Turn state store for persistence
    pub turn_store: Arc<TurnStateStore>,
    /// Personality to use (mrcode or mrbot)
    pub personality: String,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            gateway_mode: false,
            turn_store: Arc::new(TurnStateStore::default()),
            personality: "mrbot".to_string(),
        }
    }
}

/// Run the agent in a blocking fashion, sending events to a channel.
/// This wraps the existing synchronous agent::run_turn function.
pub fn run_agent_task(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), String> {
    run_agent_task_with_config(request, event_tx, &WorkerConfig::default())
}

/// Run agent task with explicit configuration
pub fn run_agent_task_with_config(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
    config: &WorkerConfig,
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
        AgentMethod::RunTurn => {
            if config.gateway_mode {
                run_turn_gateway_mode(request, event_tx, config)
            } else {
                run_turn_task(request, event_tx)
            }
        }
        AgentMethod::ResumeTurn => {
            run_resume_task(request, event_tx, config)
        }
    }
}

/// Run a turn in non-gateway mode (original behavior)
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
    let mut cfg = Config::load().map_err(|e| format!("Config load failed: {}", e))?;

    // Apply default target based on available API keys (same logic as main.rs)
    if cfg.default_target.is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.default_target = Some("qwen3-235b-a22b-instruct-2507@venice".to_string());
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
        }
    }

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
            "No target configured and none provided. Set VENICE_API_KEY, OPENAI_API_KEY, or ANTHROPIC_API_KEY.",
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
            // Send content event if there's response text
            if let Some(text) = &result.response_text {
                let _ = event_tx.send(AgentEvent::content(id, text));
            }

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

/// Run a turn in gateway mode with yield/resume semantics
fn run_turn_gateway_mode(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
    config: &WorkerConfig,
) -> Result<(), String> {
    use crate::backend::BackendRegistry;
    use crate::cli::Context;
    use crate::commands::CommandIndex;
    use crate::config::{Config, Target};
    use crate::cost::{PricingTable, SessionCosts};
    use crate::hooks::HookManager;
    use crate::llm::{self, LlmClient};
    use crate::model_routing::ModelRouter;
    use crate::plan::PlanModeState;
    use crate::policy::{Decision, PolicyEngine};
    use crate::skillpacks::{ActiveSkills, SkillIndex};
    use crate::tools::{self, todo::TodoState};
    use crate::transcript::Transcript;
    use crate::Args;
    use std::cell::RefCell;
    use std::path::PathBuf;

    let id = &request.id;
    let turn_id = uuid::Uuid::new_v4().to_string();

    // Parse working directory
    let root = request
        .working_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Load config
    let mut cfg = Config::load().map_err(|e| format!("Config load failed: {}", e))?;

    // Apply default target
    if cfg.default_target.is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.default_target = Some("qwen3-235b-a22b-instruct-2507@venice".to_string());
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
        }
    }

    let target = request
        .target
        .as_ref()
        .and_then(|t| Target::parse(t))
        .or_else(|| cfg.get_default_target());

    if target.is_none() {
        let _ = event_tx.send(AgentEvent::error(
            id,
            "no_target",
            "No target configured",
        ));
        return Ok(());
    }
    let target = target.unwrap();

    // Extract user message
    let mut messages: Vec<Value> = request.messages.clone();
    let user_input = messages
        .last()
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    if user_input.is_empty() {
        let _ = event_tx.send(AgentEvent::error(id, "no_input", "No user message provided"));
        return Ok(());
    }

    // Build system prompt
    let system_prompt = r#"You are an agentic coding assistant running locally.
You can only access files via tools. All paths are relative to the project root.
Use Glob/Grep to find files before Read. Before Edit/Write, explain what you will change.
Use Bash for running builds, tests, formatters, and git operations.
Never use curl or wget - they are blocked by policy.
Keep edits minimal and precise."#;

    // Get tool schemas
    let schema_opts = tools::SchemaOptions::new(false);
    let tool_schemas = tools::schemas_with_task(&schema_opts);

    // Create transcript directory if needed
    let transcripts_dir = root.join(".brainpro").join("sessions");
    let _ = std::fs::create_dir_all(&transcripts_dir);

    // Initialize transcript
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", request.session_id));
    let transcript = Transcript::new(&transcript_path, &request.session_id, &root)
        .map_err(|e| format!("Transcript error: {}", e))?;

    // Policy engine - gateway mode doesn't auto-approve
    let policy = PolicyEngine::new(cfg.permissions.clone(), false, false);
    let bash_config = cfg.bash.clone();

    // Initialize other components
    let hooks = HookManager::new(cfg.hooks.clone(), request.session_id.clone(), root.clone());
    let skill_index = SkillIndex::build(&root);
    let model_router = ModelRouter::new(cfg.model_routing.clone());
    let command_index = CommandIndex::build(&root);
    let pricing = PricingTable::from_config(&cfg.model_pricing);
    let session_costs = SessionCosts::new(request.session_id.clone(), pricing);
    let args = Args::parse_from(["brainpro"]);

    // Build context for tool execution
    let ctx = Context {
        args,
        root: root.clone(),
        transcript: RefCell::new(transcript),
        session_id: request.session_id.clone(),
        tracing: RefCell::new(false),
        config: RefCell::new(cfg.clone()),
        backends: RefCell::new(BackendRegistry::new(&cfg)),
        current_target: RefCell::new(Some(target.clone())),
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

    // Build messages for LLM
    let mut req_messages = vec![json!({
        "role": "system",
        "content": system_prompt
    })];
    req_messages.extend(messages.clone());

    // Token stats
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut tool_uses = 0u64;

    const MAX_ITERATIONS: usize = 12;

    for _iteration in 1..=MAX_ITERATIONS {
        // Get client for target's backend
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)
                .map_err(|e| format!("Backend error: {}", e))?;

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages.clone(),
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request).map_err(|e| format!("LLM error: {}", e))?
        };

        // Track usage
        if let Some(usage) = &response.usage {
            input_tokens += usage.prompt_tokens;
            output_tokens += usage.completion_tokens;
        }

        if response.choices.is_empty() {
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        // Send thinking/content event
        if let Some(content) = &msg.content {
            if !content.is_empty() {
                let _ = event_tx.send(AgentEvent::content(id, content));
            }
        }

        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => tc,
            _ => {
                // No tool calls, we're done
                break;
            }
        };

        // Add assistant message to conversation
        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        req_messages.push(assistant_msg.clone());
        messages.push(assistant_msg);

        // Process each tool call
        for tc in tool_calls {
            let name = &tc.function.name;
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            tool_uses += 1;

            // Send tool call event
            let _ = event_tx.send(AgentEvent::tool_call(id, name, args.clone(), &tc.id));

            // Check policy decision
            let (decision, matched_rule) = ctx.policy.borrow().decide(name, &args);

            match decision {
                Decision::Allow => {
                    // Execute the tool
                    let tool_start = std::time::Instant::now();
                    let result = execute_tool(&ctx, name, args.clone(), &bash_config)?;
                    let duration_ms = tool_start.elapsed().as_millis() as u64;

                    let ok = result.get("error").is_none();
                    let _ = event_tx.send(AgentEvent::tool_result(
                        id, name, &tc.id, result.clone(), ok, duration_ms,
                    ));

                    // Add tool result to conversation
                    let tool_msg = json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&result).unwrap_or_default()
                    });
                    req_messages.push(tool_msg.clone());
                    messages.push(tool_msg);
                }
                Decision::Deny => {
                    // Tool is denied, add error result
                    let result = json!({
                        "error": {
                            "code": "permission_denied",
                            "message": format!("Denied by policy{}",
                                matched_rule.as_ref().map(|r| format!(" (rule: {})", r)).unwrap_or_default())
                        }
                    });
                    let _ = event_tx.send(AgentEvent::tool_result(
                        id, name, &tc.id, result.clone(), false, 0,
                    ));

                    let tool_msg = json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&result).unwrap_or_default()
                    });
                    req_messages.push(tool_msg.clone());
                    messages.push(tool_msg);
                }
                Decision::Ask => {
                    // Need to yield for approval
                    // Check if this is AskUserQuestion (special case)
                    if name == "AskUserQuestion" {
                        match crate::tools::ask_user::validate(&args) {
                            Ok(questions) => {
                                let q_json: Vec<Value> = questions
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

                                // Save state
                                let state = TurnState::new(
                                    turn_id.clone(),
                                    request.session_id.clone(),
                                    id.to_string(),
                                    messages.clone(),
                                    PendingToolCall {
                                        tool_call_id: tc.id.clone(),
                                        tool_name: name.clone(),
                                        tool_args: args.clone(),
                                        policy_rule: None,
                                        questions: Some(q_json.clone()),
                                    },
                                    YieldReason::AwaitingInput,
                                    Some(target.to_string()),
                                    request.working_dir.clone(),
                                );
                                let _ = config.turn_store.save(state);

                                // Send yield event
                                let _ = event_tx.send(AgentEvent::yield_input(
                                    id,
                                    &turn_id,
                                    &tc.id,
                                    q_json,
                                ));
                                return Ok(());
                            }
                            Err(error) => {
                                // Invalid questions, return error
                                let _ = event_tx.send(AgentEvent::tool_result(
                                    id, name, &tc.id, error.clone(), false, 0,
                                ));
                                let tool_msg = json!({
                                    "role": "tool",
                                    "tool_call_id": tc.id,
                                    "content": serde_json::to_string(&error).unwrap_or_default()
                                });
                                req_messages.push(tool_msg.clone());
                                messages.push(tool_msg);
                            }
                        }
                    } else {
                        // Regular tool needing approval - save state and yield
                        let state = TurnState::new(
                            turn_id.clone(),
                            request.session_id.clone(),
                            id.to_string(),
                            messages.clone(),
                            PendingToolCall {
                                tool_call_id: tc.id.clone(),
                                tool_name: name.clone(),
                                tool_args: args.clone(),
                                policy_rule: matched_rule.clone(),
                                questions: None,
                            },
                            YieldReason::AwaitingApproval,
                            Some(target.to_string()),
                            request.working_dir.clone(),
                        );
                        let _ = config.turn_store.save(state);

                        // Send yield event for approval
                        let _ = event_tx.send(AgentEvent::yield_approval(
                            id,
                            &turn_id,
                            &tc.id,
                            name,
                            args.clone(),
                            matched_rule,
                        ));
                        return Ok(());
                    }
                }
            }
        }
    }

    // Send done event
    let usage = UsageStats {
        input_tokens,
        output_tokens,
        tool_uses,
    };
    let _ = event_tx.send(AgentEvent::done(id, usage));

    Ok(())
}

/// Resume a yielded turn
fn run_resume_task(
    request: AgentRequest,
    event_tx: mpsc::Sender<AgentEvent>,
    config: &WorkerConfig,
) -> Result<(), String> {
    use crate::backend::BackendRegistry;
    use crate::cli::Context;
    use crate::commands::CommandIndex;
    use crate::config::{Config, Target};
    use crate::cost::{PricingTable, SessionCosts};
    use crate::hooks::HookManager;
    use crate::llm::{self, LlmClient};
    use crate::model_routing::ModelRouter;
    use crate::plan::PlanModeState;
    use crate::policy::{Decision, PolicyEngine};
    use crate::skillpacks::{ActiveSkills, SkillIndex};
    use crate::tools::{self, todo::TodoState};
    use crate::transcript::Transcript;
    use crate::Args;
    use std::cell::RefCell;
    use std::path::PathBuf;

    let id = &request.id;

    let resume_data = match &request.resume_data {
        Some(d) => d,
        None => {
            let _ = event_tx.send(AgentEvent::error(
                id,
                "missing_resume_data",
                "ResumeTurn requires resume_data",
            ));
            return Ok(());
        }
    };

    // Get saved state
    let state = match config.turn_store.get(&resume_data.turn_id) {
        Some(s) => s,
        None => {
            let _ = event_tx.send(AgentEvent::error(
                id,
                "turn_not_found",
                &format!("Turn {} not found or expired", resume_data.turn_id),
            ));
            return Ok(());
        }
    };

    // Remove state from store (we're consuming it)
    config.turn_store.remove(&resume_data.turn_id);

    let _turn_id = state.turn_id.clone();

    // Parse working directory
    let root = state
        .working_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Load config
    let mut cfg = Config::load().map_err(|e| format!("Config load failed: {}", e))?;

    // Apply default target
    if cfg.default_target.is_none() {
        if std::env::var("VENICE_API_KEY").is_ok() || std::env::var("venice_api_key").is_ok() {
            cfg.default_target = Some("qwen3-235b-a22b-instruct-2507@venice".to_string());
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            cfg.default_target = Some("gpt-4o-mini@chatgpt".to_string());
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            cfg.default_target = Some("claude-3-5-sonnet-latest@claude".to_string());
        }
    }

    let target = state
        .target
        .as_ref()
        .and_then(|t| Target::parse(t))
        .or_else(|| cfg.get_default_target());

    if target.is_none() {
        let _ = event_tx.send(AgentEvent::error(id, "no_target", "No target configured"));
        return Ok(());
    }
    let target = target.unwrap();

    let bash_config = cfg.bash.clone();

    // Build system prompt
    let system_prompt = r#"You are an agentic coding assistant running locally.
You can only access files via tools. All paths are relative to the project root.
Use Glob/Grep to find files before Read. Before Edit/Write, explain what you will change.
Use Bash for running builds, tests, formatters, and git operations.
Never use curl or wget - they are blocked by policy.
Keep edits minimal and precise."#;

    // Get tool schemas
    let schema_opts = tools::SchemaOptions::new(false);
    let tool_schemas = tools::schemas_with_task(&schema_opts);

    // Create transcript directory if needed
    let transcripts_dir = root.join(".brainpro").join("sessions");
    let _ = std::fs::create_dir_all(&transcripts_dir);

    // Initialize transcript
    let transcript_path = transcripts_dir.join(format!("{}.jsonl", state.session_id));
    let transcript = Transcript::new(&transcript_path, &state.session_id, &root)
        .map_err(|e| format!("Transcript error: {}", e))?;

    // Policy engine
    let policy = PolicyEngine::new(cfg.permissions.clone(), false, false);

    // Initialize other components
    let hooks = HookManager::new(cfg.hooks.clone(), state.session_id.clone(), root.clone());
    let skill_index = SkillIndex::build(&root);
    let model_router = ModelRouter::new(cfg.model_routing.clone());
    let command_index = CommandIndex::build(&root);
    let pricing = PricingTable::from_config(&cfg.model_pricing);
    let session_costs = SessionCosts::new(state.session_id.clone(), pricing);
    let args = Args::parse_from(["brainpro"]);

    // Build context
    let ctx = Context {
        args,
        root: root.clone(),
        transcript: RefCell::new(transcript),
        session_id: state.session_id.clone(),
        tracing: RefCell::new(false),
        config: RefCell::new(cfg.clone()),
        backends: RefCell::new(BackendRegistry::new(&cfg)),
        current_target: RefCell::new(Some(target.clone())),
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

    // Restore messages
    let mut messages = state.messages.clone();
    let pending = &state.pending_tool_call;

    // Process the response based on yield reason
    let tool_result = match state.yield_reason {
        YieldReason::AwaitingApproval => {
            if resume_data.approved == Some(true) {
                // Execute the tool
                let tool_start = std::time::Instant::now();
                let result = execute_tool(&ctx, &pending.tool_name, pending.tool_args.clone(), &bash_config)?;
                let duration_ms = tool_start.elapsed().as_millis() as u64;

                let ok = result.get("error").is_none();
                let _ = event_tx.send(AgentEvent::tool_result(
                    id,
                    &pending.tool_name,
                    &pending.tool_call_id,
                    result.clone(),
                    ok,
                    duration_ms,
                ));
                result
            } else {
                // User denied
                json!({
                    "error": {
                        "code": "permission_denied",
                        "message": "User denied permission"
                    }
                })
            }
        }
        YieldReason::AwaitingInput => {
            // Process user's answers
            let answers = resume_data.answers.clone().unwrap_or(json!({}));
            json!({
                "ok": true,
                "answers": answers
            })
        }
    };

    // Add tool result to messages
    let tool_msg = json!({
        "role": "tool",
        "tool_call_id": pending.tool_call_id,
        "content": serde_json::to_string(&tool_result).unwrap_or_default()
    });
    messages.push(tool_msg);

    // Build messages for LLM with system prompt
    let mut req_messages = vec![json!({
        "role": "system",
        "content": system_prompt
    })];
    req_messages.extend(messages.clone());

    // Token stats
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut tool_uses = 0u64;

    const MAX_ITERATIONS: usize = 12;

    // Continue the agent loop
    for _iteration in 1..=MAX_ITERATIONS {
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)
                .map_err(|e| format!("Backend error: {}", e))?;

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages.clone(),
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request).map_err(|e| format!("LLM error: {}", e))?
        };

        if let Some(usage) = &response.usage {
            input_tokens += usage.prompt_tokens;
            output_tokens += usage.completion_tokens;
        }

        if response.choices.is_empty() {
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        if let Some(content) = &msg.content {
            if !content.is_empty() {
                let _ = event_tx.send(AgentEvent::content(id, content));
            }
        }

        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => tc,
            _ => break,
        };

        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        req_messages.push(assistant_msg.clone());
        messages.push(assistant_msg);

        for tc in tool_calls {
            let name = &tc.function.name;
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            tool_uses += 1;

            let _ = event_tx.send(AgentEvent::tool_call(id, name, args.clone(), &tc.id));

            let (decision, matched_rule) = ctx.policy.borrow().decide(name, &args);

            match decision {
                Decision::Allow => {
                    let tool_start = std::time::Instant::now();
                    let result = execute_tool(&ctx, name, args.clone(), &bash_config)?;
                    let duration_ms = tool_start.elapsed().as_millis() as u64;

                    let ok = result.get("error").is_none();
                    let _ = event_tx.send(AgentEvent::tool_result(
                        id, name, &tc.id, result.clone(), ok, duration_ms,
                    ));

                    let tool_msg = json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&result).unwrap_or_default()
                    });
                    req_messages.push(tool_msg.clone());
                    messages.push(tool_msg);
                }
                Decision::Deny => {
                    let result = json!({
                        "error": {
                            "code": "permission_denied",
                            "message": format!("Denied by policy{}",
                                matched_rule.as_ref().map(|r| format!(" (rule: {})", r)).unwrap_or_default())
                        }
                    });
                    let _ = event_tx.send(AgentEvent::tool_result(
                        id, name, &tc.id, result.clone(), false, 0,
                    ));

                    let tool_msg = json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&result).unwrap_or_default()
                    });
                    req_messages.push(tool_msg.clone());
                    messages.push(tool_msg);
                }
                Decision::Ask => {
                    // Handle AskUserQuestion specially
                    if name == "AskUserQuestion" {
                        match crate::tools::ask_user::validate(&args) {
                            Ok(questions) => {
                                let q_json: Vec<Value> = questions
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

                                let new_turn_id = uuid::Uuid::new_v4().to_string();
                                let state = TurnState::new(
                                    new_turn_id.clone(),
                                    ctx.session_id.clone(),
                                    id.to_string(),
                                    messages.clone(),
                                    PendingToolCall {
                                        tool_call_id: tc.id.clone(),
                                        tool_name: name.clone(),
                                        tool_args: args.clone(),
                                        policy_rule: None,
                                        questions: Some(q_json.clone()),
                                    },
                                    YieldReason::AwaitingInput,
                                    Some(target.to_string()),
                                    ctx.root.to_string_lossy().to_string().into(),
                                );
                                let _ = config.turn_store.save(state);

                                let _ = event_tx.send(AgentEvent::yield_input(
                                    id,
                                    &new_turn_id,
                                    &tc.id,
                                    q_json,
                                ));
                                return Ok(());
                            }
                            Err(error) => {
                                let _ = event_tx.send(AgentEvent::tool_result(
                                    id, name, &tc.id, error.clone(), false, 0,
                                ));
                                let tool_msg = json!({
                                    "role": "tool",
                                    "tool_call_id": tc.id,
                                    "content": serde_json::to_string(&error).unwrap_or_default()
                                });
                                req_messages.push(tool_msg.clone());
                                messages.push(tool_msg);
                            }
                        }
                    } else {
                        // Regular tool needing approval
                        let new_turn_id = uuid::Uuid::new_v4().to_string();
                        let state = TurnState::new(
                            new_turn_id.clone(),
                            ctx.session_id.clone(),
                            id.to_string(),
                            messages.clone(),
                            PendingToolCall {
                                tool_call_id: tc.id.clone(),
                                tool_name: name.clone(),
                                tool_args: args.clone(),
                                policy_rule: matched_rule.clone(),
                                questions: None,
                            },
                            YieldReason::AwaitingApproval,
                            Some(target.to_string()),
                            ctx.root.to_string_lossy().to_string().into(),
                        );
                        let _ = config.turn_store.save(state);

                        let _ = event_tx.send(AgentEvent::yield_approval(
                            id,
                            &new_turn_id,
                            &tc.id,
                            name,
                            args.clone(),
                            matched_rule,
                        ));
                        return Ok(());
                    }
                }
            }
        }
    }

    let usage = UsageStats {
        input_tokens,
        output_tokens,
        tool_uses,
    };
    let _ = event_tx.send(AgentEvent::done(id, usage));

    Ok(())
}

/// Execute a tool and return the result
fn execute_tool(
    ctx: &crate::cli::Context,
    name: &str,
    args: Value,
    bash_config: &crate::config::BashConfig,
) -> Result<Value, String> {
    use crate::tools;

    // Handle special tools
    if name == "ActivateSkill" {
        let skill_name = args["name"].as_str().unwrap_or("");
        let reason = args["reason"].as_str();

        if skill_name.is_empty() {
            return Ok(json!({
                "error": {
                    "code": "missing_name",
                    "message": "Missing required 'name' parameter"
                }
            }));
        }

        let skill_index = ctx.skill_index.borrow();
        let mut active_skills = ctx.active_skills.borrow_mut();
        match active_skills.activate(skill_name, &skill_index) {
            Ok(activation) => {
                let _ = ctx.transcript.borrow_mut().skill_activate(
                    &activation.name,
                    reason,
                    activation.allowed_tools.as_ref(),
                );
                Ok(json!({
                    "ok": true,
                    "name": activation.name,
                    "description": activation.description,
                    "allowed_tools": activation.allowed_tools,
                    "instructions_loaded": true,
                    "message": format!("Skill '{}' activated. Instructions loaded.", activation.name)
                }))
            }
            Err(e) => Ok(json!({
                "error": {
                    "code": "activation_failed",
                    "message": e.to_string()
                }
            })),
        }
    } else if name == "Task" {
        let (task_result, _sub_stats) = tools::task::execute(args.clone(), ctx)
            .map_err(|e| format!("Task error: {}", e))?;
        Ok(task_result)
    } else if name == "TodoWrite" {
        Ok(tools::todo::execute(args, &ctx.todo_state))
    } else if name == "EnterPlanMode" {
        let goal = args.get("goal").and_then(|g| g.as_str()).unwrap_or("");
        Ok(tools::plan_mode::execute_enter(&ctx.plan_mode, goal))
    } else if name == "ExitPlanMode" {
        Ok(tools::plan_mode::execute_exit(&ctx.plan_mode))
    } else {
        // Execute built-in tool
        tools::execute(name, args, &ctx.root, bash_config)
            .map_err(|e| format!("Tool error: {}", e))
    }
}

/// Spawn a worker task in a new thread
pub fn spawn_worker(request: AgentRequest) -> WorkerHandle {
    spawn_worker_with_config(request, WorkerConfig::default())
}

/// Spawn a worker task with configuration
pub fn spawn_worker_with_config(request: AgentRequest, config: WorkerConfig) -> WorkerHandle {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        if let Err(e) = run_agent_task_with_config(request, tx, &config) {
            eprintln!("[worker] Error: {}", e);
        }
    });

    WorkerHandle { events: rx }
}
