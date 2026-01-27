//! MrCode agent loop implementation.
//!
//! This is the core agent loop for MrCode, migrated from agent.rs.
//! It handles user input, LLM calls, and tool execution.

use crate::{
    agent::{PendingQuestion, TurnResult},
    cli::Context,
    llm::{self, LlmClient},
    personality::PromptContext,
    plan::{self, PlanPhase},
    policy::Decision,
    tool_display, tools,
};
use anyhow::Result;
use serde_json::{json, Value};

use super::prompts;

const MAX_ITERATIONS: usize = 12;

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

/// Run a single turn of the MrCode agent loop
pub fn run_turn(ctx: &Context, user_input: &str, messages: &mut Vec<Value>) -> Result<TurnResult> {
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

    // Get built-in tool schemas - MrCode has a minimal toolset
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
        // MrCode uses base schemas without Task tool by default
        tools::schemas(&schema_opts)
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

            // Build system prompt using MrCode prompts
            let prompt_ctx = PromptContext::from_context(ctx);
            let mut system_prompt = prompts::build_system_prompt(&prompt_ctx);

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
                "Warning: Response truncated (max tokens reached). Consider increasing max_tokens or using /compact."
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
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

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

        // If we have a pending question, break out of the iteration loop
        if turn_result.pending_question.is_some() {
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
