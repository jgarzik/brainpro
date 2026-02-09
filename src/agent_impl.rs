//! Agent loop for processing user input and executing tool calls.

#![allow(dead_code)]
#![allow(clippy::await_holding_refcell_ref)]

use crate::{
    cli::Context,
    compact,
    llm::{self, LlmClient, StreamEvent},
    plan::{self, PlanPhase},
    policy::Decision,
    tool_display, tools,
};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};

const MAX_ITERATIONS: usize = 12;

/// Doom loop detection threshold - break after this many identical tool calls
const DOOM_LOOP_THRESHOLD: usize = 3;

/// Tools that are safe to run in parallel (read-only, no side effects)
const PURE_TOOLS: &[&str] = &["Read", "Glob", "Search", "Grep"];

/// Hash a tool call for doom loop detection
fn hash_tool_call(name: &str, args: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    args.to_string().hash(&mut hasher);
    hasher.finish()
}

/// Check if a tool is pure (read-only, parallelizable)
fn is_pure_tool(name: &str) -> bool {
    PURE_TOOLS.contains(&name)
}

/// Doom loop detector using a ring buffer of recent tool calls
#[derive(Debug, Default)]
struct DoomLoopDetector {
    recent_calls: Vec<u64>,
}

impl DoomLoopDetector {
    fn new() -> Self {
        Self {
            recent_calls: Vec::with_capacity(DOOM_LOOP_THRESHOLD),
        }
    }

    /// Record a tool call and return true if doom loop detected
    fn record(&mut self, hash: u64) -> bool {
        self.recent_calls.push(hash);

        if self.recent_calls.len() < DOOM_LOOP_THRESHOLD {
            return false;
        }

        let start = self.recent_calls.len() - DOOM_LOOP_THRESHOLD;
        let recent = &self.recent_calls[start..];
        recent.iter().all(|h| *h == hash)
    }
}

/// Statistics collected during command execution
#[derive(Debug, Default, Clone)]
pub struct CommandStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_uses: u64,
}

impl CommandStats {
    /// Total tokens used (input + output)
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Merge stats from another source (e.g., subagent)
    pub fn merge(&mut self, other: &CommandStats) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.tool_uses += other.tool_uses;
    }
}

/// Result of a turn, including stats and continuation info
#[derive(Debug, Default, Clone)]
pub struct TurnResult {
    pub stats: CommandStats,
    /// If true, a Stop hook requested continuation with the given prompt
    pub force_continue: bool,
    pub continue_prompt: Option<String>,
    /// If set, agent is waiting for user to answer questions
    pub pending_question: Option<PendingQuestion>,
    /// Collected response text from the assistant
    pub response_text: Option<String>,
}

/// Pending question that needs user input before continuing
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub tool_call_id: String,
    pub questions: Vec<tools::ask_user::Question>,
}

const SYSTEM_PROMPT: &str = r#"You are an agentic coding assistant running locally.
You can only access files via tools. All paths are relative to the project root.
Use Glob/Grep to find files before Read. Before Edit/Write, explain what you will change.
Use Bash for running builds, tests, formatters, and git operations.
Never use curl or wget - they are blocked by policy.
Keep edits minimal and precise."#;

fn trace(ctx: &Context, label: &str, content: &str) {
    if *ctx.tracing.borrow() {
        eprintln!("[TRACE:{}] {}", label, content);
    }
}

fn verbose(ctx: &Context, message: &str) {
    if ctx.args.verbose || ctx.args.debug {
        eprintln!("[VERBOSE] {}", message);
    }
}

/// Sync wrapper for worker.rs compatibility (deprecated - will be removed)
pub fn run_turn_sync(
    ctx: &Context,
    user_input: &str,
    messages: &mut Vec<Value>,
) -> Result<TurnResult> {
    let mut turn_result = TurnResult::default();
    let mut collected_response = String::new();
    let _ = ctx.transcript.borrow_mut().user_message(user_input);

    messages.push(json!({
        "role": "user",
        "content": user_input
    }));

    // Resolve target: override > config default
    let target = {
        let current = ctx.current_target.borrow();
        if let Some(t) = current.as_ref() {
            t.clone()
        } else {
            ctx.config
                .borrow()
                .get_default_target()
                .ok_or_else(|| anyhow::anyhow!("No target configured. Use --target or /target"))?
        }
    };
    let bash_config = ctx.config.borrow().bash.clone();

    trace(ctx, "TARGET", &target.to_string());

    // Check if we're in plan mode
    let plan_phase = ctx.plan_mode.borrow().phase;
    let in_planning_mode = plan_phase == PlanPhase::Planning;

    // Check for $skill-name mentions and auto-activate
    for word in user_input.split_whitespace() {
        if word.starts_with('$') && word.len() > 1 {
            let skill_name =
                &word[1..].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
            let index = ctx.skill_index.borrow();
            if index.get(skill_name).is_some() {
                let active = ctx.active_skills.borrow();
                if active.get(skill_name).is_none() {
                    drop(active);
                    let mut active = ctx.active_skills.borrow_mut();
                    if let Ok(activation) = active.activate(skill_name, &index) {
                        let _ = ctx.transcript.borrow_mut().skill_activate(
                            &activation.name,
                            Some("auto-activated from $mention"),
                            activation.allowed_tools.as_ref(),
                        );
                        trace(ctx, "SKILL", &format!("Auto-activated: {}", skill_name));
                    }
                }
            }
        }
    }

    // Get built-in tool schemas (including Task for main agent) and add MCP tools
    let schema_opts = tools::SchemaOptions::new(ctx.args.optimize);
    let mut tool_schemas = if in_planning_mode {
        // In planning mode, only provide read-only tools
        tools::schemas(&schema_opts)
            .into_iter()
            .filter(|schema| {
                if let Some(name) = schema
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    matches!(name, "Read" | "Glob" | "Search")
                } else {
                    false
                }
            })
            .collect()
    } else {
        tools::schemas_with_task(&schema_opts)
    };

    // Apply allowed-tools restriction from active skills
    let active_skills = ctx.active_skills.borrow();
    let effective_allowed = active_skills.effective_allowed_tools();
    drop(active_skills);

    if let Some(allowed) = &effective_allowed {
        tool_schemas.retain(|schema| {
            if let Some(name) = schema
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                // ActivateSkill is always available
                if name == "ActivateSkill" {
                    return true;
                }
                // Task is always available for subagent delegation
                if name == "Task" {
                    return true;
                }
                allowed.iter().any(|a| a == name)
            } else {
                false
            }
        });
    }

    // Use max_turns from CLI if provided, otherwise default
    let max_iterations = ctx.args.max_turns.unwrap_or(MAX_ITERATIONS);

    // Initialize doom loop detector
    let mut doom_detector = DoomLoopDetector::new();

    for iteration in 1..=max_iterations {
        if iteration == max_iterations {
            messages.push(json!({
                "role": "system",
                "content": "Max tool iterations reached. Summarize progress, list next steps, and stop calling tools."
            }));
        }
        trace(ctx, "ITER", &format!("Starting iteration {}", iteration));

        // Auto-compaction: check if context is approaching limit
        {
            let context_config = &ctx.config.borrow().context;
            if compact::needs_compaction(messages, context_config) {
                trace(ctx, "COMPACT", "Auto-compacting context");
                let mut backends = ctx.backends.borrow_mut();
                if let Ok(client) = backends.get_client(&target.backend) {
                    match compact::compact_messages(messages, context_config, client, &target.model)
                    {
                        Ok((compacted, result)) => {
                            eprintln!("[auto-compact] {}", compact::format_result(&result));
                            *messages = compacted;
                        }
                        Err(e) => {
                            eprintln!("[auto-compact] Failed: {}", e);
                        }
                    }
                }
            }
        }

        // Get client for target's backend (lazy-loaded)
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)?;

            // Build system prompt with skill pack info
            let mut system_prompt = if in_planning_mode {
                plan::PLAN_MODE_SYSTEM_PROMPT.to_string()
            } else {
                SYSTEM_PROMPT.to_string()
            };

            // Add optimization mode instructions if -O flag is set
            if ctx.args.optimize {
                system_prompt.push_str("\n\nAI-to-AI mode. Maximum information density. Structure over prose. No narration.");
            }

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
            drop(active_skills);

            let mut req_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            req_messages.extend(messages.clone());

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request)?
        };

        // Track token usage from this LLM call
        if let Some(usage) = &response.usage {
            turn_result.stats.input_tokens += usage.prompt_tokens;
            turn_result.stats.output_tokens += usage.completion_tokens;

            // Record cost for this operation
            let turn_number = *ctx.turn_counter.borrow();
            let op = ctx.session_costs.borrow_mut().record_operation(
                turn_number,
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
            );

            // Log token usage to transcript
            let _ = ctx.transcript.borrow_mut().token_usage(
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
                op.cost_usd,
            );
        }

        if response.choices.is_empty() {
            println!("No response from model");
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        // Warn if response was truncated due to length limit
        if choice.finish_reason.as_deref() == Some("length") {
            eprintln!(
                "⚠️  Response truncated (max tokens reached). Consider increasing max_tokens or using /compact."
            );
        }

        if let Some(content) = &msg.content {
            if !content.is_empty() {
                println!("{}", content);
                if !collected_response.is_empty() {
                    collected_response.push_str("\n\n");
                }
                collected_response.push_str(content);
                let _ = ctx.transcript.borrow_mut().assistant_message(content);

                // In planning mode, try to parse the output for a plan
                if in_planning_mode {
                    let goal = ctx
                        .plan_mode
                        .borrow()
                        .current_plan
                        .as_ref()
                        .map(|p| p.goal.clone())
                        .unwrap_or_default();

                    if let Ok(parsed_plan) = plan::parse_plan_output(content, &goal) {
                        // Update the plan in plan mode state
                        let mut state = ctx.plan_mode.borrow_mut();
                        if let Some(current_plan) = &mut state.current_plan {
                            current_plan.summary = parsed_plan.summary;
                            current_plan.steps = parsed_plan.steps;
                            current_plan.status = plan::PlanStatus::Ready;
                        }
                        state.enter_review();

                        // Log plan created
                        let plan_name = state
                            .current_plan
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        let step_count = state
                            .current_plan
                            .as_ref()
                            .map(|p| p.steps.len())
                            .unwrap_or(0);
                        drop(state);
                        let _ = ctx
                            .transcript
                            .borrow_mut()
                            .plan_created(&plan_name, step_count);
                    }
                }
            }
        }

        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => {
                // Trace thinking when there's content along with tool calls
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        trace(ctx, "THINK", content);
                    }
                }
                tc
            }
            _ => {
                messages.push(json!({
                    "role": "assistant",
                    "content": msg.content
                }));
                break;
            }
        };

        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        messages.push(assistant_msg);

        for tc in tool_calls {
            let name = &tc.function.name;

            // Handle JSON parse errors - return error to LLM so it can learn
            let args: Value = match serde_json::from_str(&tc.function.arguments) {
                Ok(a) => a,
                Err(e) => {
                    turn_result.stats.tool_uses += 1;
                    let error_result = json!({
                        "error": {
                            "code": "invalid_arguments",
                            "message": format!("Invalid JSON arguments: {}", e)
                        }
                    });
                    eprintln!("{}", tool_display::format_tool_result(name, &error_result));
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&error_result)?
                    }));
                    continue;
                }
            };

            // Doom loop detection
            let call_hash = hash_tool_call(name, &args);
            if doom_detector.record(call_hash) {
                eprintln!(
                    "⚠️  Doom loop detected: {} called {} times with same arguments. Breaking.",
                    name, DOOM_LOOP_THRESHOLD
                );
                let error_result = json!({
                    "error": {
                        "code": "doom_loop_detected",
                        "message": format!(
                            "Tool '{}' called {} times with identical arguments. \
                             This appears to be a stuck loop. Please try a different approach.",
                            name, DOOM_LOOP_THRESHOLD
                        )
                    }
                });
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": serde_json::to_string(&error_result)?
                }));
                turn_result.response_text = Some(
                    "Agent stopped due to doom loop (repeated identical tool calls).".to_string(),
                );
                break;
            }

            // Count this tool use
            turn_result.stats.tool_uses += 1;

            trace(
                ctx,
                "CALL",
                &format!(
                    "{}({})",
                    name,
                    serde_json::to_string_pretty(&args).unwrap_or_default()
                ),
            );

            verbose(
                ctx,
                &format!("Tool call: {}({})", name, tc.function.arguments),
            );

            // Display tool call
            eprintln!("{}", tool_display::format_tool_call(name, &args));

            let _ = ctx.transcript.borrow_mut().tool_call(name, &args);

            // Use PolicyEngine for permission decisions
            let (allowed, decision, matched_rule) =
                ctx.policy.borrow().check_permission(name, &args);

            // Log policy decision to transcript
            let decision_str = match decision {
                Decision::Allow => "allowed",
                Decision::Deny => "denied",
                Decision::Ask => {
                    if allowed {
                        "prompted_yes"
                    } else {
                        "prompted_no"
                    }
                }
            };
            let _ = ctx.transcript.borrow_mut().policy_decision(
                name,
                decision_str,
                matched_rule.as_deref(),
            );

            // Run PreToolUse hooks (can block or modify args)
            let (hook_proceed, updated_args) = ctx.hooks.borrow().pre_tool_use(name, &args);
            let args = updated_args.unwrap_or(args);

            // Track tool execution time
            let tool_start = std::time::Instant::now();

            let result = if !hook_proceed {
                // PreToolUse hook blocked the tool
                json!({
                    "error": {
                        "code": "hook_blocked",
                        "message": "Blocked by PreToolUse hook"
                    }
                })
            } else if allowed {
                if name == "ActivateSkill" {
                    // Execute ActivateSkill tool
                    let skill_name = args["name"].as_str().unwrap_or("");
                    let reason = args["reason"].as_str();

                    if skill_name.is_empty() {
                        json!({
                            "error": {
                                "code": "missing_name",
                                "message": "Missing required 'name' parameter"
                            }
                        })
                    } else {
                        let skill_index = ctx.skill_index.borrow();
                        let mut active_skills = ctx.active_skills.borrow_mut();
                        match active_skills.activate(skill_name, &skill_index) {
                            Ok(activation) => {
                                let _ = ctx.transcript.borrow_mut().skill_activate(
                                    &activation.name,
                                    reason,
                                    activation.allowed_tools.as_ref(),
                                );
                                json!({
                                    "ok": true,
                                    "name": activation.name,
                                    "description": activation.description,
                                    "allowed_tools": activation.allowed_tools,
                                    "instructions_loaded": true,
                                    "message": format!("Skill '{}' activated. Instructions loaded.", activation.name)
                                })
                            }
                            Err(e) => {
                                json!({
                                    "error": {
                                        "code": "activation_failed",
                                        "message": e.to_string()
                                    }
                                })
                            }
                        }
                    }
                } else if name == "Task" {
                    // Execute Task tool (subagent delegation)
                    let (task_result, sub_stats) = tools::task::execute(args.clone(), ctx)?;
                    turn_result.stats.merge(&sub_stats);
                    task_result
                } else if name == "TodoWrite" {
                    // Execute TodoWrite tool
                    tools::todo::execute(args.clone(), &ctx.todo_state)
                } else if name == "AskUserQuestion" {
                    // Validate questions and signal that we need user input
                    match tools::ask_user::validate(&args) {
                        Ok(questions) => {
                            // Set pending question and break out after this tool call
                            turn_result.pending_question = Some(PendingQuestion {
                                tool_call_id: tc.id.clone(),
                                questions,
                            });
                            // Return a placeholder - the actual result will be injected later
                            json!({
                                "status": "awaiting_user_input",
                                "message": "Waiting for user to answer questions"
                            })
                        }
                        Err(error) => error,
                    }
                } else if name == "EnterPlanMode" {
                    // Enter plan mode
                    let goal = args.get("goal").and_then(|g| g.as_str()).unwrap_or("");
                    tools::plan_mode::execute_enter(&ctx.plan_mode, goal)
                } else if name == "ExitPlanMode" {
                    // Exit plan mode
                    tools::plan_mode::execute_exit(&ctx.plan_mode)
                } else {
                    // Execute built-in tool
                    tools::execute(name, args.clone(), &ctx.root, &bash_config)?
                }
            } else {
                let reason = match decision {
                    Decision::Deny => "Denied by policy",
                    _ => "User denied permission",
                };
                json!({ "error": { "code": "permission_denied", "message": reason } })
            };

            let ok = result.get("error").is_none();
            let tool_duration_ms = tool_start.elapsed().as_millis() as u64;
            let _ = ctx.transcript.borrow_mut().tool_result(name, ok, &result);

            // Run PostToolUse hooks
            ctx.hooks
                .borrow()
                .post_tool_use(name, &args, &result, tool_duration_ms);

            trace(
                ctx,
                "RESULT",
                &format!(
                    "{}: {}",
                    name,
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                ),
            );

            verbose(ctx, &format!("Tool result: {} ok={}", name, ok));

            // Display tool result
            eprintln!("{}", tool_display::format_tool_result(name, &result));

            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": serde_json::to_string(&result)?
            }));

            // If we have a pending question, break out of both loops
            if turn_result.pending_question.is_some() {
                break;
            }
        }

        // If we have a pending question or doom loop, break out of the iteration loop
        if turn_result.pending_question.is_some() {
            break;
        }
        if turn_result
            .response_text
            .as_ref()
            .map(|s| s.contains("doom loop"))
            .unwrap_or(false)
        {
            break;
        }
    }

    // Run Stop hooks - may request continuation (skip if waiting for user input)
    let last_assistant_message = messages.iter().rev().find_map(|m| {
        if m["role"].as_str() == Some("assistant") {
            m["content"].as_str().map(|s| s.to_string())
        } else {
            None
        }
    });

    let (force_continue, continue_prompt) = ctx
        .hooks
        .borrow()
        .on_stop("end_turn", last_assistant_message.as_deref());

    // If force_continue is requested, signal to caller to run another turn
    // (but not if we're waiting for user input)
    if force_continue && turn_result.pending_question.is_none() {
        if let Some(prompt) = continue_prompt {
            turn_result.force_continue = true;
            turn_result.continue_prompt = Some(prompt);
            verbose(ctx, "Stop hook requested continuation");
        }
    }

    // Store collected response text
    if !collected_response.is_empty() {
        turn_result.response_text = Some(collected_response);
    }

    Ok(turn_result)
}

/// Run a turn with streaming output.
/// Content deltas are printed to stdout in real-time.
pub async fn run_turn(
    ctx: &Context,
    user_input: &str,
    messages: &mut Vec<Value>,
) -> Result<TurnResult> {
    let mut turn_result = TurnResult::default();
    let mut collected_response = String::new();
    let _ = ctx.transcript.borrow_mut().user_message(user_input);

    messages.push(json!({
        "role": "user",
        "content": user_input
    }));

    // Resolve target
    let target = {
        let current = ctx.current_target.borrow();
        if let Some(t) = current.as_ref() {
            t.clone()
        } else {
            ctx.config
                .borrow()
                .get_default_target()
                .ok_or_else(|| anyhow::anyhow!("No target configured. Use --target or /target"))?
        }
    };
    let bash_config = ctx.config.borrow().bash.clone();

    trace(ctx, "TARGET", &target.to_string());

    let plan_phase = ctx.plan_mode.borrow().phase;
    let in_planning_mode = plan_phase == PlanPhase::Planning;

    // Handle skill auto-activation (same as sync version)
    for word in user_input.split_whitespace() {
        if word.starts_with('$') && word.len() > 1 {
            let skill_name =
                &word[1..].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
            let index = ctx.skill_index.borrow();
            if index.get(skill_name).is_some() {
                let active = ctx.active_skills.borrow();
                if active.get(skill_name).is_none() {
                    drop(active);
                    let mut active = ctx.active_skills.borrow_mut();
                    if let Ok(activation) = active.activate(skill_name, &index) {
                        let _ = ctx.transcript.borrow_mut().skill_activate(
                            &activation.name,
                            Some("auto-activated from $mention"),
                            activation.allowed_tools.as_ref(),
                        );
                        trace(ctx, "SKILL", &format!("Auto-activated: {}", skill_name));
                    }
                }
            }
        }
    }

    // Build tool schemas
    let schema_opts = tools::SchemaOptions::new(ctx.args.optimize);
    let mut tool_schemas = if in_planning_mode {
        tools::schemas(&schema_opts)
            .into_iter()
            .filter(|schema| {
                if let Some(name) = schema
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    matches!(name, "Read" | "Glob" | "Search")
                } else {
                    false
                }
            })
            .collect()
    } else {
        tools::schemas_with_task(&schema_opts)
    };

    // Apply allowed-tools restriction
    let active_skills = ctx.active_skills.borrow();
    let effective_allowed = active_skills.effective_allowed_tools();
    drop(active_skills);

    if let Some(allowed) = &effective_allowed {
        tool_schemas.retain(|schema| {
            if let Some(name) = schema
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                if name == "ActivateSkill" || name == "Task" {
                    return true;
                }
                allowed.iter().any(|a| a == name)
            } else {
                false
            }
        });
    }

    let max_iterations = ctx.args.max_turns.unwrap_or(MAX_ITERATIONS);

    // Initialize doom loop detector
    let mut doom_detector = DoomLoopDetector::new();

    for iteration in 1..=max_iterations {
        trace(ctx, "ITER", &format!("Starting iteration {}", iteration));

        // Auto-compaction: check if context is approaching limit
        {
            let context_config = &ctx.config.borrow().context;
            if compact::needs_compaction(messages, context_config) {
                trace(ctx, "COMPACT", "Auto-compacting context");
                let mut backends = ctx.backends.borrow_mut();
                if let Ok(client) = backends.get_client(&target.backend) {
                    match compact::compact_messages(messages, context_config, client, &target.model)
                    {
                        Ok((compacted, result)) => {
                            eprintln!("[auto-compact] {}", compact::format_result(&result));
                            *messages = compacted;
                        }
                        Err(e) => {
                            eprintln!("[auto-compact] Failed: {}", e);
                        }
                    }
                }
            }
        }

        // Get streaming client and make request
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_streaming_client(&target.backend)?;

            let mut system_prompt = if in_planning_mode {
                plan::PLAN_MODE_SYSTEM_PROMPT.to_string()
            } else {
                SYSTEM_PROMPT.to_string()
            };

            if ctx.args.optimize {
                system_prompt.push_str("\n\nAI-to-AI mode. Maximum information density. Structure over prose. No narration.");
            }

            let skill_index = ctx.skill_index.borrow();
            let skill_prompt = skill_index.format_for_prompt(50);
            drop(skill_index);
            if !skill_prompt.is_empty() {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(&skill_prompt);
            }

            let active_skills = ctx.active_skills.borrow();
            if !active_skills.is_empty() {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(&active_skills.format_for_conversation());
            }
            drop(active_skills);

            let mut req_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            req_messages.extend(messages.clone());

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            // Create channel for streaming events
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<StreamEvent>(100);

            // Spawn the streaming request
            let response_future = client.chat_stream(&request, event_tx);

            // Process events as they arrive
            let mut iteration_content = String::new();
            let response = tokio::select! {
                result = response_future => result?,
                _ = async {
                    while let Some(event) = event_rx.recv().await {
                        match event {
                            StreamEvent::ContentDelta(delta) => {
                                // Print delta immediately
                                print!("{}", delta);
                                let _ = io::stdout().flush();
                                iteration_content.push_str(&delta);
                            }
                            StreamEvent::ToolCallStart { name, .. } => {
                                trace(ctx, "STREAM", &format!("Tool call starting: {}", name));
                            }
                            StreamEvent::ToolCallDelta { .. } => {
                                // Tool call args being streamed
                            }
                            StreamEvent::Done { .. } => {
                                break;
                            }
                        }
                    }
                    // This future never returns on its own
                    futures::future::pending::<()>().await
                } => unreachable!(),
            };

            // Drain remaining events
            while let Ok(event) = event_rx.try_recv() {
                if let StreamEvent::ContentDelta(delta) = event {
                    print!("{}", delta);
                    let _ = io::stdout().flush();
                    iteration_content.push_str(&delta);
                }
            }

            // Add newline after streaming content
            if !iteration_content.is_empty() {
                println!();
            }

            response
        };

        // Track token usage
        if let Some(usage) = &response.usage {
            turn_result.stats.input_tokens += usage.prompt_tokens;
            turn_result.stats.output_tokens += usage.completion_tokens;

            let turn_number = *ctx.turn_counter.borrow();
            let op = ctx.session_costs.borrow_mut().record_operation(
                turn_number,
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
            );

            let _ = ctx.transcript.borrow_mut().token_usage(
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
                op.cost_usd,
            );
        }

        if response.choices.is_empty() {
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        if choice.finish_reason.as_deref() == Some("length") {
            eprintln!(
                "⚠️  Response truncated (max tokens reached). Consider increasing max_tokens or using /compact."
            );
        }

        // Content already streamed, but add to collected response
        if let Some(content) = &msg.content {
            if !content.is_empty() {
                if !collected_response.is_empty() {
                    collected_response.push_str("\n\n");
                }
                collected_response.push_str(content);
                let _ = ctx.transcript.borrow_mut().assistant_message(content);

                // Handle plan mode parsing
                if in_planning_mode {
                    let goal = ctx
                        .plan_mode
                        .borrow()
                        .current_plan
                        .as_ref()
                        .map(|p| p.goal.clone())
                        .unwrap_or_default();

                    if let Ok(parsed_plan) = plan::parse_plan_output(content, &goal) {
                        let mut state = ctx.plan_mode.borrow_mut();
                        if let Some(current_plan) = &mut state.current_plan {
                            current_plan.summary = parsed_plan.summary;
                            current_plan.steps = parsed_plan.steps;
                            current_plan.status = plan::PlanStatus::Ready;
                        }
                        state.enter_review();

                        let plan_name = state
                            .current_plan
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        let step_count = state
                            .current_plan
                            .as_ref()
                            .map(|p| p.steps.len())
                            .unwrap_or(0);
                        drop(state);
                        let _ = ctx
                            .transcript
                            .borrow_mut()
                            .plan_created(&plan_name, step_count);
                    }
                }
            }
        }

        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => {
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        trace(ctx, "THINK", content);
                    }
                }
                tc
            }
            _ => {
                messages.push(json!({
                    "role": "assistant",
                    "content": msg.content
                }));
                break;
            }
        };

        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        messages.push(assistant_msg);

        // Process tool calls (same as sync version)
        for tc in tool_calls {
            let name = &tc.function.name;

            // Handle JSON parse errors - return error to LLM so it can learn
            let args: Value = match serde_json::from_str(&tc.function.arguments) {
                Ok(a) => a,
                Err(e) => {
                    turn_result.stats.tool_uses += 1;
                    let error_result = json!({
                        "error": {
                            "code": "invalid_arguments",
                            "message": format!("Invalid JSON arguments: {}", e)
                        }
                    });
                    eprintln!("{}", tool_display::format_tool_result(name, &error_result));
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": serde_json::to_string(&error_result)?
                    }));
                    continue;
                }
            };

            // Doom loop detection
            let call_hash = hash_tool_call(name, &args);
            if doom_detector.record(call_hash) {
                eprintln!(
                    "⚠️  Doom loop detected: {} called {} times with same arguments. Breaking.",
                    name, DOOM_LOOP_THRESHOLD
                );
                let error_result = json!({
                    "error": {
                        "code": "doom_loop_detected",
                        "message": format!(
                            "Tool '{}' called {} times with identical arguments. \
                             This appears to be a stuck loop. Please try a different approach.",
                            name, DOOM_LOOP_THRESHOLD
                        )
                    }
                });
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": serde_json::to_string(&error_result)?
                }));
                turn_result.response_text = Some(
                    "Agent stopped due to doom loop (repeated identical tool calls).".to_string(),
                );
                break;
            }

            turn_result.stats.tool_uses += 1;

            trace(
                ctx,
                "CALL",
                &format!(
                    "{}({})",
                    name,
                    serde_json::to_string_pretty(&args).unwrap_or_default()
                ),
            );

            verbose(
                ctx,
                &format!("Tool call: {}({})", name, tc.function.arguments),
            );

            eprintln!("{}", tool_display::format_tool_call(name, &args));

            let _ = ctx.transcript.borrow_mut().tool_call(name, &args);

            let (allowed, decision, matched_rule) =
                ctx.policy.borrow().check_permission(name, &args);

            let decision_str = match decision {
                Decision::Allow => "allowed",
                Decision::Deny => "denied",
                Decision::Ask => {
                    if allowed {
                        "prompted_yes"
                    } else {
                        "prompted_no"
                    }
                }
            };
            let _ = ctx.transcript.borrow_mut().policy_decision(
                name,
                decision_str,
                matched_rule.as_deref(),
            );

            let (hook_proceed, updated_args) = ctx.hooks.borrow().pre_tool_use(name, &args);
            let args = updated_args.unwrap_or(args);

            let tool_start = std::time::Instant::now();

            let result = if !hook_proceed {
                json!({
                    "error": {
                        "code": "hook_blocked",
                        "message": "Blocked by PreToolUse hook"
                    }
                })
            } else if allowed {
                if name == "ActivateSkill" {
                    let skill_name = args["name"].as_str().unwrap_or("");
                    let reason = args["reason"].as_str();

                    if skill_name.is_empty() {
                        json!({
                            "error": {
                                "code": "missing_name",
                                "message": "Missing required 'name' parameter"
                            }
                        })
                    } else {
                        let skill_index = ctx.skill_index.borrow();
                        let mut active_skills = ctx.active_skills.borrow_mut();
                        match active_skills.activate(skill_name, &skill_index) {
                            Ok(activation) => {
                                let _ = ctx.transcript.borrow_mut().skill_activate(
                                    &activation.name,
                                    reason,
                                    activation.allowed_tools.as_ref(),
                                );
                                json!({
                                    "ok": true,
                                    "name": activation.name,
                                    "description": activation.description,
                                    "allowed_tools": activation.allowed_tools,
                                    "instructions_loaded": true,
                                    "message": format!("Skill '{}' activated. Instructions loaded.", activation.name)
                                })
                            }
                            Err(e) => {
                                json!({
                                    "error": {
                                        "code": "activation_failed",
                                        "message": e.to_string()
                                    }
                                })
                            }
                        }
                    }
                } else if name == "Task" {
                    let (task_result, sub_stats) = tools::task::execute(args.clone(), ctx)?;
                    turn_result.stats.merge(&sub_stats);
                    task_result
                } else if name == "TodoWrite" {
                    tools::todo::execute(args.clone(), &ctx.todo_state)
                } else if name == "AskUserQuestion" {
                    match tools::ask_user::validate(&args) {
                        Ok(questions) => {
                            turn_result.pending_question = Some(PendingQuestion {
                                tool_call_id: tc.id.clone(),
                                questions,
                            });
                            json!({
                                "status": "awaiting_user_input",
                                "message": "Waiting for user to answer questions"
                            })
                        }
                        Err(error) => error,
                    }
                } else if name == "EnterPlanMode" {
                    let goal = args.get("goal").and_then(|g| g.as_str()).unwrap_or("");
                    tools::plan_mode::execute_enter(&ctx.plan_mode, goal)
                } else if name == "ExitPlanMode" {
                    tools::plan_mode::execute_exit(&ctx.plan_mode)
                } else {
                    tools::execute(name, args.clone(), &ctx.root, &bash_config)?
                }
            } else {
                let reason = match decision {
                    Decision::Deny => "Denied by policy",
                    _ => "User denied permission",
                };
                json!({ "error": { "code": "permission_denied", "message": reason } })
            };

            let ok = result.get("error").is_none();
            let tool_duration_ms = tool_start.elapsed().as_millis() as u64;
            let _ = ctx.transcript.borrow_mut().tool_result(name, ok, &result);

            ctx.hooks
                .borrow()
                .post_tool_use(name, &args, &result, tool_duration_ms);

            trace(
                ctx,
                "RESULT",
                &format!(
                    "{}: {}",
                    name,
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                ),
            );

            verbose(ctx, &format!("Tool result: {} ok={}", name, ok));

            eprintln!("{}", tool_display::format_tool_result(name, &result));

            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": serde_json::to_string(&result)?
            }));

            if turn_result.pending_question.is_some() {
                break;
            }
        }

        // If we have a pending question or doom loop, break out of the iteration loop
        if turn_result.pending_question.is_some() {
            break;
        }
        if turn_result
            .response_text
            .as_ref()
            .map(|s| s.contains("doom loop"))
            .unwrap_or(false)
        {
            break;
        }
    }

    // Run Stop hooks
    let last_assistant_message = messages.iter().rev().find_map(|m| {
        if m["role"].as_str() == Some("assistant") {
            m["content"].as_str().map(|s| s.to_string())
        } else {
            None
        }
    });

    let (force_continue, continue_prompt) = ctx
        .hooks
        .borrow()
        .on_stop("end_turn", last_assistant_message.as_deref());

    if force_continue && turn_result.pending_question.is_none() {
        if let Some(prompt) = continue_prompt {
            turn_result.force_continue = true;
            turn_result.continue_prompt = Some(prompt);
            verbose(ctx, "Stop hook requested continuation");
        }
    }

    if !collected_response.is_empty() {
        turn_result.response_text = Some(collected_response);
    }

    Ok(turn_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doom_loop_detector_no_loop() {
        let mut detector = DoomLoopDetector::new();

        // Different calls shouldn't trigger
        assert!(!detector.record(hash_tool_call("Read", &json!({"path": "a.txt"}))));
        assert!(!detector.record(hash_tool_call("Read", &json!({"path": "b.txt"}))));
        assert!(!detector.record(hash_tool_call("Read", &json!({"path": "c.txt"}))));
    }

    #[test]
    fn test_doom_loop_detector_triggers() {
        let mut detector = DoomLoopDetector::new();

        let hash = hash_tool_call("Read", &json!({"path": "same.txt"}));

        // First two calls shouldn't trigger
        assert!(!detector.record(hash));
        assert!(!detector.record(hash));
        // Third identical call triggers
        assert!(detector.record(hash));
    }

    #[test]
    fn test_doom_loop_detector_reset_by_different_call() {
        let mut detector = DoomLoopDetector::new();

        let hash1 = hash_tool_call("Read", &json!({"path": "a.txt"}));
        let hash2 = hash_tool_call("Read", &json!({"path": "b.txt"}));

        // Two identical calls
        assert!(!detector.record(hash1));
        assert!(!detector.record(hash1));
        // Different call breaks the pattern
        assert!(!detector.record(hash2));
        // Start over with first hash - need 3 more
        assert!(!detector.record(hash1));
        assert!(!detector.record(hash1));
        // Third identical triggers
        assert!(detector.record(hash1));
    }

    #[test]
    fn test_hash_tool_call_different_args() {
        let hash1 = hash_tool_call("Read", &json!({"path": "a.txt"}));
        let hash2 = hash_tool_call("Read", &json!({"path": "b.txt"}));
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_tool_call_different_tools() {
        let hash1 = hash_tool_call("Read", &json!({"path": "a.txt"}));
        let hash2 = hash_tool_call("Write", &json!({"path": "a.txt"}));
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_is_pure_tool() {
        assert!(is_pure_tool("Read"));
        assert!(is_pure_tool("Glob"));
        assert!(is_pure_tool("Search"));
        assert!(is_pure_tool("Grep"));
        assert!(!is_pure_tool("Write"));
        assert!(!is_pure_tool("Edit"));
        assert!(!is_pure_tool("Bash"));
    }
}
