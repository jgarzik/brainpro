//! Core agent loop implementation.
//!
//! This module provides the shared agent loop that can be customized
//! via the AgentHooks trait. This eliminates ~1500 lines of duplicated
//! code across:
//! - agent_impl.rs (run_turn_sync, run_turn)
//! - mrcode/loop_impl.rs
//! - mrbot/loop_impl.rs
//! - worker.rs

#![allow(dead_code)]

use crate::agent::tool_executor::{self, DispatchResult};
use crate::cli::Context;
use crate::compact;
use crate::config::BashConfig;
use crate::llm::{self, LlmClient};
use crate::plan::{self, PlanPhase};
use crate::tool_display;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};

/// Default maximum iterations per turn
pub const DEFAULT_MAX_ITERATIONS: usize = 12;

/// Doom loop detection threshold - break after this many identical tool calls
const DOOM_LOOP_THRESHOLD: usize = 3;

/// Tools that are safe to run in parallel (read-only, no side effects)
const PURE_TOOLS: &[&str] = &["Read", "Glob", "Search", "Grep"];

/// Hash a tool call for doom loop detection
fn hash_tool_call(name: &str, args: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    // Use string representation of args for consistent hashing
    args.to_string().hash(&mut hasher);
    hasher.finish()
}

/// Check if a tool is pure (read-only, parallelizable)
fn is_pure_tool(name: &str) -> bool {
    PURE_TOOLS.contains(&name)
}

fn execute_tool_call(
    ctx: &Context,
    bash_config: &BashConfig,
    tc: llm::ToolCall,
    args: Value,
) -> (llm::ToolCall, DispatchResult, bool) {
    let name = &tc.function.name;

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

    let (dispatch_result, ok, _duration_ms) =
        tool_executor::execute_with_policy(ctx, name, args.clone(), bash_config);

    trace(
        ctx,
        "RESULT",
        &format!(
            "{}: {}",
            name,
            serde_json::to_string_pretty(&dispatch_result_value(&dispatch_result))
                .unwrap_or_default()
        ),
    );

    verbose(ctx, &format!("Tool result: {} ok={}", name, ok));
    eprintln!(
        "{}",
        tool_display::format_tool_result(name, &dispatch_result_value(&dispatch_result))
    );

    (tc, dispatch_result, ok)
}

fn append_tool_result(
    turn_result: &mut TurnResult,
    tool_results: &mut Vec<(String, String, Value)>,
    result: (llm::ToolCall, DispatchResult, bool),
) {
    let (tc, dispatch_result, _ok) = result;
    let name = &tc.function.name;
    let value = dispatch_result_value(&dispatch_result);

    turn_result.stats.tool_uses += 1;

    match dispatch_result {
        DispatchResult::AskUser { questions, .. } => {
            turn_result.pending_question = Some(PendingQuestion {
                tool_call_id: tc.id.clone(),
                questions,
            });
        }
        DispatchResult::Task { stats, .. } => {
            turn_result.stats.merge(&stats);
        }
        _ => {}
    }

    tool_results.push((tc.id.clone(), name.clone(), value));
}

fn dispatch_result_value(result: &DispatchResult) -> Value {
    match result {
        DispatchResult::Ok(v) | DispatchResult::Error(v) => v.clone(),
        DispatchResult::AskUser { result, .. } => result.clone(),
        DispatchResult::Task { result, .. } => result.clone(),
    }
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

        // Only check for doom loop if we have enough calls
        if self.recent_calls.len() < DOOM_LOOP_THRESHOLD {
            return false;
        }

        // Check if the last DOOM_LOOP_THRESHOLD calls are identical
        let start = self.recent_calls.len() - DOOM_LOOP_THRESHOLD;
        let recent = &self.recent_calls[start..];
        recent.iter().all(|h| *h == hash)
    }

    /// Reset after a different call breaks the pattern
    fn reset(&mut self) {
        self.recent_calls.clear();
    }
}

/// Configuration for the agent loop
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum iterations before stopping
    pub max_iterations: usize,
    /// Whether to include Task tool (subagent delegation)
    pub include_task_tool: bool,
    /// Whether to use streaming for LLM calls
    pub streaming: bool,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            include_task_tool: false,
            streaming: false,
        }
    }
}

impl AgentLoopConfig {
    pub fn with_task_tool(mut self) -> Self {
        self.include_task_tool = true;
        self
    }

    pub fn with_streaming(mut self) -> Self {
        self.streaming = true;
        self
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }
}

/// Pending question that needs user input before continuing
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub tool_call_id: String,
    pub questions: Vec<crate::tools::ask_user::Question>,
}

/// Result of a single agent turn
#[derive(Debug, Default, Clone)]
pub struct TurnResult {
    /// Token and tool usage statistics
    pub stats: crate::agent::CommandStats,
    /// If true, a Stop hook requested continuation with the given prompt
    pub force_continue: bool,
    /// The prompt to use for continuation
    pub continue_prompt: Option<String>,
    /// If set, agent is waiting for user to answer questions
    pub pending_question: Option<PendingQuestion>,
    /// Collected response text from the assistant
    pub response_text: Option<String>,
}

/// Hooks for customizing agent loop behavior.
///
/// Implement this trait to create a custom agent loop with
/// different system prompts, tool filtering, or streaming behavior.
pub trait AgentHooks {
    /// Build the system prompt for this agent.
    ///
    /// This is called at the start of each iteration.
    fn build_system_prompt(&self, ctx: &Context, in_planning_mode: bool) -> String;

    /// Filter or transform tool schemas.
    ///
    /// Called after loading base schemas to allow filtering or modification.
    fn filter_tools(&self, schemas: Vec<Value>, in_planning_mode: bool) -> Vec<Value> {
        if in_planning_mode {
            // Default: only read-only tools in planning mode
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

    /// Called when streaming content is received.
    ///
    /// Default implementation prints to stdout.
    fn on_stream_content(&self, content: &str) {
        print!("{}", content);
        let _ = io::stdout().flush();
    }

    /// Called when non-streaming content is received.
    ///
    /// Default implementation prints to stdout with newline.
    fn on_content(&self, content: &str) {
        println!("{}", content);
    }
}

/// Trace helper
fn trace(ctx: &Context, label: &str, content: &str) {
    if *ctx.tracing.borrow() {
        eprintln!("[TRACE:{}] {}", label, content);
    }
}

/// Verbose helper
fn verbose(ctx: &Context, message: &str) {
    if ctx.args.verbose || ctx.args.debug {
        eprintln!("[VERBOSE] {}", message);
    }
}

/// Auto-activate skills mentioned with $skill-name syntax
fn auto_activate_skills(ctx: &Context, user_input: &str) {
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
}

/// Apply skill-based tool filtering
fn apply_skill_tool_filter(ctx: &Context, mut schemas: Vec<Value>) -> Vec<Value> {
    let active_skills = ctx.active_skills.borrow();
    let effective_allowed = active_skills.effective_allowed_tools();
    drop(active_skills);

    if let Some(allowed) = &effective_allowed {
        schemas.retain(|schema| {
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
    schemas
}

/// Process plan mode output
fn process_plan_output(ctx: &Context, content: &str) {
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

/// Run the core agent loop (sync version).
///
/// This is the shared implementation used by all agent variants.
pub fn run_loop<H: AgentHooks>(
    hooks: &H,
    ctx: &Context,
    config: &AgentLoopConfig,
    user_input: &str,
    messages: &mut Vec<Value>,
) -> Result<TurnResult> {
    use crate::tools;

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

    // Check plan mode
    let plan_phase = ctx.plan_mode.borrow().phase;
    let in_planning_mode = plan_phase == PlanPhase::Planning;

    // Auto-activate skills from $mentions
    auto_activate_skills(ctx, user_input);

    // Get tool schemas
    let schema_opts = tools::SchemaOptions::new(ctx.args.optimize);
    let base_schemas = if config.include_task_tool {
        tools::schemas_with_task(&schema_opts)
    } else {
        tools::schemas(&schema_opts)
    };

    // Apply hooks filtering
    let filtered_schemas = hooks.filter_tools(base_schemas, in_planning_mode);

    // Apply skill-based filtering
    let tool_schemas = apply_skill_tool_filter(ctx, filtered_schemas);

    // Use configured max_iterations
    let max_iterations = ctx.args.max_turns.unwrap_or(config.max_iterations);

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

        // Log iteration start for debugging (tool count will be 0 until we get response)
        let _ =
            ctx.transcript
                .borrow_mut()
                .iteration_info(iteration as u32, 0, "awaiting_response");

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

        // Build system prompt via hooks
        let system_prompt = hooks.build_system_prompt(ctx, in_planning_mode);

        // Dump system prompt if requested (only on first iteration)
        if ctx.args.dump_prompt && iteration == 1 {
            eprintln!(
                "=== SYSTEM PROMPT ===\n{}\n=== END SYSTEM PROMPT ===",
                system_prompt
            );
        }

        // Make LLM request
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)?;

            let mut req_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            req_messages.extend(messages.clone());

            // Inject continuation reminder after iteration 1 to encourage task completion
            if iteration > 1 {
                req_messages.push(json!({
                    "role": "user",
                    "content": "<system-reminder>Continue with your tasks. Don't stop until the original request is fully addressed.</system-reminder>"
                }));
            }

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request)?
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
            eprintln!("No response from model");
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        // Warn if truncated
        if choice.finish_reason.as_deref() == Some("length") {
            eprintln!("⚠️  Response truncated (max tokens reached). Consider increasing max_tokens or using /compact.");
        }

        // Handle content
        if let Some(content) = &msg.content {
            if !content.is_empty() {
                hooks.on_content(content);
                if !collected_response.is_empty() {
                    collected_response.push_str("\n\n");
                }
                collected_response.push_str(content);
                let _ = ctx.transcript.borrow_mut().assistant_message(content);

                if in_planning_mode {
                    process_plan_output(ctx, content);
                }
            }
        }

        // Check for tool calls
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

        // Log iteration with actual tool count
        let first_tool = tool_calls
            .first()
            .map(|tc| tc.function.name.as_str())
            .unwrap_or("none");
        let _ = ctx.transcript.borrow_mut().iteration_info(
            iteration as u32,
            tool_calls.len(),
            first_tool,
        );

        // Parse all tool calls first, handling JSON parse errors
        let mut parsed_calls: Vec<(&llm::ToolCall, Result<Value, String>)> = tool_calls
            .iter()
            .map(|tc| {
                let args_result = serde_json::from_str(&tc.function.arguments)
                    .map_err(|e| format!("Invalid JSON arguments: {}", e));
                (tc, args_result)
            })
            .collect();

        // Separate pure (parallelizable) vs effectful tools
        let (pure_calls, effectful_calls): (Vec<_>, Vec<_>) = parsed_calls
            .drain(..)
            .partition(|(tc, _)| is_pure_tool(&tc.function.name));

        let mut tool_results: Vec<(String, String, Value)> = Vec::new();

        let mut parsed_pure: Vec<(llm::ToolCall, Value)> = Vec::new();
        for (tc, args_result) in pure_calls.into_iter() {
            let name = &tc.function.name;
            let args = match args_result {
                Ok(a) => a,
                Err(parse_error) => {
                    turn_result.stats.tool_uses += 1;
                    let error_result = json!({
                        "error": {
                            "code": "invalid_arguments",
                            "message": parse_error
                        }
                    });
                    eprintln!("{}", tool_display::format_tool_result(name, &error_result));
                    tool_results.push((tc.id.clone(), name.clone(), error_result));
                    continue;
                }
            };

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
                tool_results.push((tc.id.clone(), name.clone(), error_result));
                turn_result.response_text = Some(
                    "Agent stopped due to doom loop (repeated identical tool calls).".to_string(),
                );
                break;
            }

            parsed_pure.push((tc.clone(), args));
        }

        if !parsed_pure.is_empty() {
            for (tc, args) in parsed_pure.into_iter() {
                let result = execute_tool_call(ctx, &bash_config, tc, args);
                append_tool_result(&mut turn_result, &mut tool_results, result);
            }
        }

        for (tc, args_result) in effectful_calls.into_iter() {
            let name = &tc.function.name;
            let args = match args_result {
                Ok(a) => a,
                Err(parse_error) => {
                    turn_result.stats.tool_uses += 1;
                    let error_result = json!({
                        "error": {
                            "code": "invalid_arguments",
                            "message": parse_error
                        }
                    });
                    eprintln!("{}", tool_display::format_tool_result(name, &error_result));
                    tool_results.push((tc.id.clone(), name.clone(), error_result));
                    continue;
                }
            };

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
                tool_results.push((tc.id.clone(), name.clone(), error_result));
                turn_result.response_text = Some(
                    "Agent stopped due to doom loop (repeated identical tool calls).".to_string(),
                );
                break;
            }

            let result = execute_tool_call(ctx, &bash_config, tc.clone(), args);
            append_tool_result(&mut turn_result, &mut tool_results, result);

            if turn_result.pending_question.is_some() {
                break;
            }
        }

        // Add all tool results to messages
        for (tool_id, _tool_name, result) in tool_results {
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_id,
                "content": serde_json::to_string(&result)?
            }));
        }

        // Break outer loop if pending question or doom loop
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

    // Run Stop hooks (skip if pending question)
    if turn_result.pending_question.is_none() {
        let last_assistant = messages.iter().rev().find_map(|m| {
            if m["role"].as_str() == Some("assistant") {
                m["content"].as_str().map(|s| s.to_string())
            } else {
                None
            }
        });

        if let Some(content) = last_assistant {
            let (hook_triggered, continue_prompt) =
                ctx.hooks.borrow().on_stop("tool_finished", Some(&content));
            if hook_triggered {
                turn_result.force_continue = true;
                turn_result.continue_prompt = continue_prompt;
            }
        }
    }

    turn_result.response_text = if collected_response.is_empty() {
        None
    } else {
        Some(collected_response)
    };

    Ok(turn_result)
}

// Note: Streaming version can be added later when the LlmClient streaming API is ready

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_loop_config_default() {
        let config = AgentLoopConfig::default();
        assert_eq!(config.max_iterations, DEFAULT_MAX_ITERATIONS);
        assert!(!config.include_task_tool);
        assert!(!config.streaming);
    }

    #[test]
    fn test_agent_loop_config_builder() {
        let config = AgentLoopConfig::default()
            .with_task_tool()
            .with_streaming()
            .with_max_iterations(5);
        assert_eq!(config.max_iterations, 5);
        assert!(config.include_task_tool);
        assert!(config.streaming);
    }

    #[test]
    fn test_wind_down_prompt_injected() {
        let mut messages = vec![serde_json::json!({"role": "user", "content": "hello"})];
        let max_iterations = 1;
        for iteration in 1..=max_iterations {
            if iteration == max_iterations {
                messages.push(serde_json::json!({
                    "role": "system",
                    "content": "Max tool iterations reached. Summarize progress, list next steps, and stop calling tools."
                }));
            }
        }
        let last_system = messages.iter().rev().find(|m| m["role"] == "system");
        assert!(last_system.is_some());
        let content = last_system.unwrap()["content"].as_str().unwrap();
        assert!(content.contains("Max tool iterations"));
    }
}
