//! Subagent runtime for executing specialized, restricted agent tasks.

use crate::config::{AgentSpec, PermissionMode};
use crate::policy::Decision;
use crate::{cli::Context, llm, tools};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

/// Optional input context provided to a subagent
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InputContext {
    #[serde(default)]
    pub files: Vec<FileContext>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// File context for a subagent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileContext {
    pub path: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

/// Result from a subagent execution
#[derive(Debug, Clone, Serialize)]
pub struct SubagentResult {
    pub agent: String,
    pub ok: bool,
    pub output: SubagentOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SubagentError>,
}

/// Output from a successful subagent execution
#[derive(Debug, Clone, Serialize)]
pub struct SubagentOutput {
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files_referenced: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub proposed_edits: Vec<ProposedEdit>,
}

/// A proposed edit from a subagent (for patch-style returns)
#[derive(Debug, Clone, Serialize)]
pub struct ProposedEdit {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
}

/// Error from a subagent execution
#[derive(Debug, Clone, Serialize)]
pub struct SubagentError {
    pub code: String,
    pub message: String,
}

/// Clamp a permission mode to not exceed a parent mode
/// Order: Default (strictest) < AcceptEdits < BypassPermissions (most permissive)
pub fn clamp_mode(requested: PermissionMode, parent: PermissionMode) -> PermissionMode {
    let mode_order = |m: PermissionMode| match m {
        PermissionMode::Default => 0,
        PermissionMode::AcceptEdits => 1,
        PermissionMode::BypassPermissions => 2,
    };

    if mode_order(requested) > mode_order(parent) {
        parent
    } else {
        requested
    }
}

/// Check if a tool name matches an allowed pattern
fn tool_matches_pattern(tool_name: &str, pattern: &str) -> bool {
    if pattern == tool_name {
        return true;
    }

    // Handle MCP wildcard patterns like "mcp.*" or "mcp.server.*"
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return tool_name.starts_with(prefix) && tool_name.len() > prefix.len();
    }

    false
}

/// Filter tool schemas to only include allowed tools
pub fn filter_tool_schemas(allowed_tools: &[String]) -> Vec<Value> {
    let all_schemas = tools::schemas();

    all_schemas
        .into_iter()
        .filter(|schema| {
            if let Some(func) = schema.get("function") {
                if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                    return allowed_tools
                        .iter()
                        .any(|pattern| tool_matches_pattern(name, pattern));
                }
            }
            false
        })
        .collect()
}

/// Check if a tool call is allowed for this subagent
fn is_tool_allowed(tool_name: &str, allowed_tools: &[String]) -> bool {
    // Task is never allowed in subagents (prevents recursion)
    if tool_name == "Task" {
        return false;
    }

    allowed_tools
        .iter()
        .any(|pattern| tool_matches_pattern(tool_name, pattern))
}

fn trace(ctx: &Context, agent: &str, label: &str, content: &str) {
    if *ctx.tracing.borrow() {
        eprintln!("[TRACE:{}:{}] {}", agent, label, content);
    }
}

/// Run a subagent with the given specification and prompt
pub fn run_subagent(
    ctx: &Context,
    spec: &AgentSpec,
    prompt: &str,
    input_context: Option<InputContext>,
) -> Result<SubagentResult> {
    let start_time = Instant::now();
    let agent_name = &spec.name;

    // Get parent permission mode
    let parent_mode = ctx.config.borrow().permissions.mode;

    // Clamp subagent permission mode
    let requested_mode = spec.get_permission_mode();
    let effective_mode = clamp_mode(requested_mode, parent_mode);

    trace(
        ctx,
        agent_name,
        "START",
        &format!(
            "mode={} (requested={}, parent={})",
            effective_mode.as_str(),
            requested_mode.as_str(),
            parent_mode.as_str()
        ),
    );

    // Log subagent start to transcript
    let _ = ctx.transcript.borrow_mut().subagent_start(
        agent_name,
        effective_mode.as_str(),
        &spec.allowed_tools,
    );

    // Resolve target for subagent
    let config = ctx.config.borrow();
    let target = if let Some(target_str) = &spec.target {
        crate::config::Target::parse(target_str)
    } else if let Some(skill) = &spec.skill {
        config.resolve_skill(skill)
    } else {
        // Inherit from current skill
        let skill = ctx.current_skill.borrow().clone();
        config
            .resolve_skill(&skill)
            .or_else(|| config.default_target())
    };
    let target = target.ok_or_else(|| anyhow::anyhow!("No target configured for subagent"))?;
    let bash_config = config.bash.clone();
    drop(config);

    trace(ctx, agent_name, "TARGET", &format!("{}", target));

    // Build system prompt for subagent
    let system_prompt = spec.system_prompt.as_deref().unwrap_or(
        "You are a specialized subagent. Complete the assigned task using only your available tools."
    );

    // Build initial messages
    let mut messages: Vec<Value> = Vec::new();

    // Add any input context
    let mut task_prompt = prompt.to_string();
    if let Some(input_ctx) = &input_context {
        if let Some(notes) = &input_ctx.notes {
            task_prompt = format!("{}\n\nNotes: {}", task_prompt, notes);
        }
        if !input_ctx.files.is_empty() {
            task_prompt.push_str("\n\nRelevant files:");
            for file in &input_ctx.files {
                task_prompt.push_str(&format!("\n- {}", file.path));
            }
        }
    }

    messages.push(json!({
        "role": "user",
        "content": task_prompt
    }));

    // Get filtered tool schemas
    let tool_schemas = filter_tool_schemas(&spec.allowed_tools);

    // Also add allowed MCP tools if any
    let mut all_tool_schemas = tool_schemas;
    {
        let mcp_manager = ctx.mcp_manager.borrow();
        if mcp_manager.has_connected_servers() {
            for tool_def in mcp_manager.get_all_tools() {
                let mcp_tool_name = &tool_def.full_name;
                // Check if this MCP tool is allowed
                if is_tool_allowed(mcp_tool_name, &spec.allowed_tools) {
                    all_tool_schemas.push(tool_def.to_openai_schema());
                }
            }
        }
    }

    trace(
        ctx,
        agent_name,
        "TOOLS",
        &format!("{} tools available", all_tool_schemas.len()),
    );

    // Collect data for result
    let mut collected_text = String::new();
    let mut files_referenced: Vec<String> = Vec::new();
    let mut proposed_edits: Vec<ProposedEdit> = Vec::new();

    // Run subagent loop
    for iteration in 1..=spec.max_turns {
        trace(ctx, agent_name, "ITER", &format!("iteration {}", iteration));

        // Get client for target's backend
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)?;

            let mut req_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            req_messages.extend(messages.clone());

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: if all_tool_schemas.is_empty() {
                    None
                } else {
                    Some(all_tool_schemas.clone())
                },
                tool_choice: if all_tool_schemas.is_empty() {
                    None
                } else {
                    Some("auto".to_string())
                },
            };

            client.chat(&request)?
        };

        if response.choices.is_empty() {
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        // Collect assistant text
        if let Some(content) = &msg.content {
            if !content.is_empty() {
                if !collected_text.is_empty() {
                    collected_text.push('\n');
                }
                collected_text.push_str(content);
                trace(ctx, agent_name, "TEXT", content);
            }
        }

        // Check for tool calls
        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => tc,
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
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            trace(
                ctx,
                agent_name,
                "CALL",
                &format!(
                    "{}({})",
                    name,
                    serde_json::to_string(&args).unwrap_or_default()
                ),
            );

            // Log subagent tool call
            let _ = ctx
                .transcript
                .borrow_mut()
                .subagent_tool_call(agent_name, name, &args);

            // Check if tool is allowed for this subagent
            if !is_tool_allowed(name, &spec.allowed_tools) {
                let result = json!({
                    "error": {
                        "code": "tool_not_allowed",
                        "message": format!("Tool '{}' is not allowed for this subagent", name)
                    }
                });
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": serde_json::to_string(&result)?
                }));
                continue;
            }

            // Track file references for Read/Edit/Write tools
            if name == "Read" || name == "Edit" || name == "Write" {
                if let Some(path) = args.get("path").and_then(|p| p.as_str()) {
                    if !files_referenced.contains(&path.to_string()) {
                        files_referenced.push(path.to_string());
                    }
                }
            }

            // For Edit tool, track proposed edits
            if name == "Edit" {
                if let (Some(path), Some(old), Some(new)) = (
                    args.get("path").and_then(|p| p.as_str()),
                    args.get("old_string").and_then(|s| s.as_str()),
                    args.get("new_string").and_then(|s| s.as_str()),
                ) {
                    proposed_edits.push(ProposedEdit {
                        path: path.to_string(),
                        old_string: old.to_string(),
                        new_string: new.to_string(),
                    });
                }
            }

            // Check policy (with effective clamped mode)
            // For now, we use the parent's policy engine which already has the mode set
            // In a more complete implementation, we'd create a temporary policy with the clamped mode
            let (allowed, decision, matched_rule) =
                ctx.policy.borrow().check_permission(name, &args);

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
            let _ = ctx.transcript.borrow_mut().policy_decision(
                name,
                decision_str,
                matched_rule.as_deref(),
            );

            let result = if allowed {
                if name.starts_with("mcp.") {
                    // Execute MCP tool
                    let mut mcp_manager = ctx.mcp_manager.borrow_mut();
                    match tools::mcp_dispatch::execute(&mut mcp_manager, name, args.clone()) {
                        Ok(result) => result,
                        Err(e) => {
                            json!({ "error": { "code": "mcp_error", "message": e.to_string() } })
                        }
                    }
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

            trace(
                ctx,
                agent_name,
                "RESULT",
                &serde_json::to_string(&result).unwrap_or_default(),
            );

            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": serde_json::to_string(&result)?
            }));
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Log subagent end
    let _ = ctx
        .transcript
        .borrow_mut()
        .subagent_end(agent_name, true, duration_ms);

    trace(
        ctx,
        agent_name,
        "END",
        &format!("duration={}ms", duration_ms),
    );

    Ok(SubagentResult {
        agent: agent_name.clone(),
        ok: true,
        output: SubagentOutput {
            text: collected_text,
            files_referenced,
            proposed_edits,
        },
        error: None,
    })
}
