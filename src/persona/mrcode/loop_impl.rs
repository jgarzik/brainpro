//! MrCode agent loop implementation.
//!
//! This uses the shared core loop from agent::core.
//! MrCode is the minimal toolset variant (no Task tool).

use crate::agent::core::AgentLoopConfig;
use crate::agent::{self, run_loop};
use crate::cli::Context;
use crate::persona::hooks::PersonaHooks;
use crate::persona::loader::PersonaConfig;
use anyhow::Result;
use serde_json::Value;

/// Run a single turn of the MrCode agent loop
#[allow(dead_code)] // Used by library consumers (yo binary)
pub fn run_turn(
    config: &PersonaConfig,
    ctx: &Context,
    user_input: &str,
    messages: &mut Vec<Value>,
) -> Result<agent::TurnResult> {
    let hooks = PersonaHooks::new(config);

    // MrCode uses full tool set (matching Claude Code / OpenCode)
    let loop_config = AgentLoopConfig::default().with_task_tool();

    // Run the core loop
    let result = run_loop(&hooks, ctx, &loop_config, user_input, messages)?;

    // Convert core::TurnResult to agent::TurnResult
    Ok(agent::TurnResult {
        stats: result.stats,
        force_continue: result.force_continue,
        continue_prompt: result.continue_prompt,
        pending_question: result.pending_question.map(|pq| agent::PendingQuestion {
            tool_call_id: pq.tool_call_id,
            questions: pq.questions,
        }),
        response_text: result.response_text,
    })
}
