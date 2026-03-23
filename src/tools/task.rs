//! Task tool for delegating work to subagents.

use super::SchemaOptions;
use crate::agent::CommandStats;
use crate::cli::Context;
use crate::subagent::{self, InputContext, SubagentResult};
use serde_json::{json, Value};

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Task",
                "description": "Delegate to subagent",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agent": { "type": "string" },
                        "prompt": { "type": "string" },
                        "input_context": {
                            "type": "object",
                            "properties": {
                                "files": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": { "path": { "type": "string" } },
                                        "required": ["path"]
                                    }
                                },
                                "notes": { "type": "string" }
                            }
                        }
                    },
                    "required": ["agent", "prompt"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "Task",
                "description": "Delegate a task to a specialized subagent. NOT for simple file reads or single-tool operations — use Read/Glob/Search directly. Launch multiple Task calls concurrently in a single turn when tasks are independent. Specify in prompt whether expecting code changes, research/analysis, or test results. Provide input_context.files with relevant file paths for context. Use /agents to see available agents.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "agent": {
                            "type": "string",
                            "description": "Name of the subagent to delegate to (e.g., 'scout', 'patch', 'test', 'docs')"
                        },
                        "prompt": {
                            "type": "string",
                            "description": "Task description for the subagent"
                        },
                        "input_context": {
                            "type": "object",
                            "description": "Optional context to provide to the subagent",
                            "properties": {
                                "files": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "path": { "type": "string", "description": "File path hint for the subagent" }
                                        },
                                        "required": ["path"]
                                    },
                                    "description": "File paths to hint to the subagent (it can use Read tool to access them)"
                                },
                                "notes": {
                                    "type": "string",
                                    "description": "Additional notes for the subagent"
                                }
                            }
                        }
                    },
                    "required": ["agent", "prompt"]
                }
            }
        })
    }
}

/// Execute the Task tool - delegates to a subagent
/// Returns both the result JSON and collected stats
pub fn execute(args: Value, ctx: &Context) -> anyhow::Result<(Value, CommandStats)> {
    let agent_name = match args["agent"].as_str() {
        Some(name) => name,
        None => {
            return Ok((
                json!({
                    "error": {
                        "code": "missing_agent",
                        "message": "Missing required 'agent' parameter"
                    }
                }),
                CommandStats::default(),
            ));
        }
    };

    let prompt = match args["prompt"].as_str() {
        Some(p) => p,
        None => {
            return Ok((
                json!({
                    "error": {
                        "code": "missing_prompt",
                        "message": "Missing required 'prompt' parameter"
                    }
                }),
                CommandStats::default(),
            ));
        }
    };

    // Get agent spec from config
    let config = ctx.config.borrow();
    let spec = match config.agents.get(agent_name) {
        Some(s) => s.clone(),
        None => {
            let available: Vec<&String> = config.agents.keys().collect();
            return Ok((
                json!({
                    "error": {
                        "code": "agent_not_found",
                        "message": format!("Agent '{}' not found. Available agents: {:?}", agent_name, available)
                    }
                }),
                CommandStats::default(),
            ));
        }
    };
    drop(config);

    // Parse optional input context
    let input_context: Option<InputContext> = args
        .get("input_context")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    // Run the subagent
    match subagent::run_subagent(ctx, &spec, prompt, input_context) {
        Ok((result, sub_stats)) => Ok((subagent_result_to_json(&result), sub_stats)),
        Err(e) => Ok((
            json!({
                "error": {
                    "code": "subagent_error",
                    "message": e.to_string()
                }
            }),
            CommandStats::default(),
        )),
    }
}

fn subagent_result_to_json(result: &SubagentResult) -> Value {
    let mut json_result = json!({
        "agent": result.agent,
        "ok": result.ok,
        "output": {
            "text": result.output.text
        }
    });

    if !result.output.files_referenced.is_empty() {
        json_result["output"]["files_referenced"] = json!(result.output.files_referenced);
    }

    if !result.output.proposed_edits.is_empty() {
        let edits: Vec<Value> = result
            .output
            .proposed_edits
            .iter()
            .map(|e| {
                json!({
                    "path": e.path,
                    "old_string": e.old_string,
                    "new_string": e.new_string
                })
            })
            .collect();
        json_result["output"]["proposed_edits"] = json!(edits);
    }

    if let Some(error) = &result.error {
        json_result["error"] = json!({
            "code": error.code,
            "message": error.message
        });
    }

    json_result
}
