//! Tool execution with policy and hooks.
//!
//! This module consolidates the duplicated tool execution logic across:
//! - agent.rs (2 locations: run_turn_sync, run_turn)
//! - mrcode/loop_impl.rs
//! - mrbot/loop_impl.rs
//! - worker.rs (execute_tool)

use crate::cli::Context;
use crate::config::BashConfig;
use crate::policy::Decision;
use crate::tool_output;
use crate::tools;
use anyhow::Result;
use serde_json::{json, Value};

/// Result of tool dispatch (execution only, no policy/hooks)
pub enum DispatchResult {
    /// Tool executed successfully
    Ok(Value),
    /// Tool returned an error
    Error(Value),
    /// AskUserQuestion - needs special handling
    AskUser {
        result: Value,
        questions: Vec<crate::tools::ask_user::Question>,
    },
    /// Task tool - includes subagent stats
    Task {
        result: Value,
        stats: crate::agent::CommandStats,
    },
}

/// Dispatch a tool call to the appropriate handler.
///
/// This handles the special tools (ActivateSkill, Task, TodoWrite, AskUserQuestion,
/// EnterPlanMode, ExitPlanMode) and delegates regular tools to tools::execute.
///
/// Note: This does NOT handle policy or hooks - those should be checked before calling.
pub fn dispatch_tool(
    ctx: &Context,
    name: &str,
    args: Value,
    bash_config: &BashConfig,
) -> Result<DispatchResult> {
    let result = match name {
        "ActivateSkill" => dispatch_activate_skill(ctx, &args),
        "Task" => return dispatch_task(ctx, args),
        "TodoWrite" => Ok(tools::todo::execute(args, &ctx.todo_state)),
        "AskUserQuestion" => return dispatch_ask_user(&args),
        "EnterPlanMode" => {
            let goal = args.get("goal").and_then(|g| g.as_str()).unwrap_or("");
            Ok(tools::plan_mode::execute_enter(&ctx.plan_mode, goal))
        }
        "ExitPlanMode" => Ok(tools::plan_mode::execute_exit(&ctx.plan_mode)),
        _ => tools::execute(name, args, &ctx.root, bash_config),
    };

    match result {
        Ok(v) => {
            if v.get("error").is_some() {
                Ok(DispatchResult::Error(v))
            } else {
                Ok(DispatchResult::Ok(v))
            }
        }
        Err(e) => Ok(DispatchResult::Error(json!({
            "error": {
                "code": "tool_error",
                "message": e.to_string()
            }
        }))),
    }
}

fn dispatch_activate_skill(ctx: &Context, args: &Value) -> Result<Value> {
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
}

fn dispatch_task(ctx: &Context, args: Value) -> Result<DispatchResult> {
    match tools::task::execute(args, ctx) {
        Ok((result, stats)) => {
            if result.get("error").is_some() {
                Ok(DispatchResult::Error(result))
            } else {
                Ok(DispatchResult::Task { result, stats })
            }
        }
        Err(e) => Ok(DispatchResult::Error(json!({
            "error": {
                "code": "task_error",
                "message": e.to_string()
            }
        }))),
    }
}

fn dispatch_ask_user(args: &Value) -> Result<DispatchResult> {
    match tools::ask_user::validate(args) {
        Ok(questions) => {
            let result = json!({
                "status": "awaiting_user_input",
                "message": "Waiting for user to answer questions"
            });
            Ok(DispatchResult::AskUser { result, questions })
        }
        Err(error) => Ok(DispatchResult::Error(error)),
    }
}

/// Execute a tool with policy check and hooks.
///
/// Returns (result, ok, duration_ms).
/// The caller is responsible for logging and message handling.
pub fn execute_with_policy(
    ctx: &Context,
    name: &str,
    args: Value,
    bash_config: &BashConfig,
) -> (DispatchResult, bool, u64) {
    // Check policy
    let (allowed, decision, matched_rule) = ctx.policy.borrow().check_permission(name, &args);

    // Log policy decision
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
    let _ =
        ctx.transcript
            .borrow_mut()
            .policy_decision(name, decision_str, matched_rule.as_deref());

    // Run PreToolUse hooks
    let (hook_proceed, updated_args) = ctx.hooks.borrow().pre_tool_use(name, &args);
    let args = updated_args.unwrap_or(args);

    // Track execution time
    let tool_start = std::time::Instant::now();

    let result = if !hook_proceed {
        // Blocked by hook
        DispatchResult::Error(json!({
            "error": {
                "code": "hook_blocked",
                "message": "Blocked by PreToolUse hook"
            }
        }))
    } else if !allowed {
        // Blocked by policy
        let reason = match decision {
            Decision::Deny => "Denied by policy",
            _ => "User denied permission",
        };
        DispatchResult::Error(json!({
            "error": {
                "code": "permission_denied",
                "message": reason
            }
        }))
    } else {
        // Execute the tool
        match dispatch_tool(ctx, name, args.clone(), bash_config) {
            Ok(r) => r,
            Err(e) => DispatchResult::Error(json!({
                "error": {
                    "code": "dispatch_error",
                    "message": e.to_string()
                }
            })),
        }
    };

    let duration_ms = tool_start.elapsed().as_millis() as u64;

    let result = match result {
        DispatchResult::Ok(value) => {
            let truncated = tool_output::maybe_truncate(name, &value, &ctx.root);
            DispatchResult::Ok(truncated)
        }
        DispatchResult::Error(value) => {
            let truncated = tool_output::maybe_truncate(name, &value, &ctx.root);
            DispatchResult::Error(truncated)
        }
        DispatchResult::AskUser { result, questions } => {
            let truncated = tool_output::maybe_truncate(name, &result, &ctx.root);
            DispatchResult::AskUser {
                result: truncated,
                questions,
            }
        }
        DispatchResult::Task { result, stats } => {
            let truncated = tool_output::maybe_truncate(name, &result, &ctx.root);
            DispatchResult::Task {
                result: truncated,
                stats,
            }
        }
    };

    // Extract the Value for hooks
    let result_value = match &result {
        DispatchResult::Ok(v) | DispatchResult::Error(v) => v.clone(),
        DispatchResult::AskUser { result, .. } => result.clone(),
        DispatchResult::Task { result, .. } => result.clone(),
    };

    let ok = !matches!(&result, DispatchResult::Error(_));

    // Log result
    let _ = ctx
        .transcript
        .borrow_mut()
        .tool_result(name, ok, &result_value);

    // Run PostToolUse hooks
    ctx.hooks
        .borrow()
        .post_tool_use(name, &args, &result_value, duration_ms);

    (result, ok, duration_ms)
}

/// Simple tool execution (for worker.rs compatibility)
pub fn execute_simple(
    ctx: &Context,
    name: &str,
    args: Value,
    bash_config: &BashConfig,
) -> Result<Value, String> {
    match dispatch_tool(ctx, name, args, bash_config) {
        Ok(DispatchResult::Ok(v)) => Ok(v),
        Ok(DispatchResult::Error(v)) => Ok(v),
        Ok(DispatchResult::AskUser { result, .. }) => Ok(result),
        Ok(DispatchResult::Task { result, .. }) => Ok(result),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_output;

    #[test]
    fn test_dispatch_result_variants() {
        // Just verify the enum variants compile
        let _ok = DispatchResult::Ok(json!({"ok": true}));
        let _err = DispatchResult::Error(json!({"error": {"code": "test"}}));
    }

    #[test]
    fn test_truncation_applied_to_dispatch_result() {
        let root = tempfile::TempDir::new().unwrap();
        let result = json!({
            "content": "a".repeat(tool_output::MAX_TOOL_OUTPUT_BYTES + 10)
        });
        let truncated = tool_output::maybe_truncate("Read", &result, root.path());
        let dispatched = DispatchResult::Ok(truncated);
        match dispatched {
            DispatchResult::Ok(value) => {
                assert!(value.get("output_truncated").is_some());
            }
            _ => panic!("unexpected result"),
        }
    }
}
