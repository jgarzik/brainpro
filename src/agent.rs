//! Agent loop for processing user input and executing tool calls.

use crate::{
    cli::Context,
    llm::{self, LlmClient},
    plan::{self, PlanPhase},
    policy::Decision,
    tool_display,
    tools,
};
use anyhow::Result;
use serde_json::{json, Value};

const MAX_ITERATIONS: usize = 12;

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

pub fn run_turn(ctx: &Context, user_input: &str, messages: &mut Vec<Value>) -> Result<TurnResult> {
    let mut result = TurnResult::default();
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
                    matches!(name, "Read" | "Grep" | "Glob")
                } else {
                    false
                }
            })
            .collect()
    } else {
        tools::schemas_with_task(&schema_opts)
    };

    // Only add MCP tools if not in planning mode
    if !in_planning_mode {
        let mcp_manager = ctx.mcp_manager.borrow();
        if mcp_manager.has_connected_servers() {
            // Add MCP tools to the schema
            for tool_def in mcp_manager.get_all_tools() {
                tool_schemas.push(tool_def.to_openai_schema());
            }
        }
    }

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

    for iteration in 1..=max_iterations {
        trace(ctx, "ITER", &format!("Starting iteration {}", iteration));

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
            result.stats.input_tokens += usage.prompt_tokens;
            result.stats.output_tokens += usage.completion_tokens;

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
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            // Count this tool use
            result.stats.tool_uses += 1;

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
                    result.stats.merge(&sub_stats);
                    task_result
                } else if name.starts_with("mcp.") {
                    // Execute MCP tool
                    let start = std::time::Instant::now();
                    let mut mcp_manager = ctx.mcp_manager.borrow_mut();

                    // Log the MCP tool call
                    let parts: Vec<&str> = name.splitn(3, '.').collect();
                    let (server, tool_name) = if parts.len() == 3 {
                        (parts[1], parts[2])
                    } else {
                        ("unknown", name.as_str())
                    };
                    let _ = ctx
                        .transcript
                        .borrow_mut()
                        .mcp_tool_call(server, tool_name, &args);

                    match tools::mcp_dispatch::execute(&mut mcp_manager, name, args.clone()) {
                        Ok(result) => {
                            let duration_ms = start.elapsed().as_millis() as u64;
                            let truncated = result
                                .get("truncated")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
                            let _ = ctx.transcript.borrow_mut().mcp_tool_result(
                                server,
                                tool_name,
                                ok,
                                duration_ms,
                                truncated,
                            );
                            result
                        }
                        Err(e) => {
                            let duration_ms = start.elapsed().as_millis() as u64;
                            let _ = ctx.transcript.borrow_mut().mcp_tool_result(
                                server,
                                tool_name,
                                false,
                                duration_ms,
                                false,
                            );
                            // Check if server died
                            if let Some(exit_status) = mcp_manager.check_server_health(server) {
                                let _ = ctx
                                    .transcript
                                    .borrow_mut()
                                    .mcp_server_died(server, Some(exit_status));
                            }
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
        }
    }

    // Run Stop hooks - may request continuation
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
    if force_continue {
        if let Some(prompt) = continue_prompt {
            result.force_continue = true;
            result.continue_prompt = Some(prompt);
            verbose(ctx, "Stop hook requested continuation");
        }
    }

    Ok(result)
}
