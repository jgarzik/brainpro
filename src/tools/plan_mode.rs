//! Plan mode control tools for agent-initiated planning.

use super::SchemaOptions;
use crate::plan::PlanPhase;
use serde_json::{json, Value};
use std::cell::RefCell;

pub fn enter_schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "EnterPlanMode",
                "description": "Enter planning mode",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "EnterPlanMode",
                "description": "Enter planning mode to design an implementation approach before writing code. Use when the task requires architectural decisions or has multiple valid approaches.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        })
    }
}

pub fn exit_schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "ExitPlanMode",
                "description": "Exit planning mode",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "ExitPlanMode",
                "description": "Signal that planning is complete and you're ready for the user to approve the plan. Call this after writing your plan.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        })
    }
}

/// Execute EnterPlanMode tool
pub fn execute_enter(
    plan_state: &RefCell<crate::plan::PlanModeState>,
    goal: &str,
) -> Value {
    let mut state = plan_state.borrow_mut();

    if state.phase != PlanPhase::Inactive {
        return json!({
            "error": {
                "code": "already_in_plan_mode",
                "message": "Already in plan mode"
            }
        });
    }

    let goal = if goal.is_empty() {
        "Implementation planning".to_string()
    } else {
        goal.to_string()
    };

    state.enter_planning(goal);

    json!({
        "ok": true,
        "message": "Entered planning mode. You now have access to read-only tools. Design your implementation approach and call ExitPlanMode when ready."
    })
}

/// Execute ExitPlanMode tool
pub fn execute_exit(plan_state: &RefCell<crate::plan::PlanModeState>) -> Value {
    let mut state = plan_state.borrow_mut();

    if state.phase == PlanPhase::Inactive {
        return json!({
            "error": {
                "code": "not_in_plan_mode",
                "message": "Not in plan mode"
            }
        });
    }

    state.enter_review();

    json!({
        "ok": true,
        "message": "Exited planning mode. Plan is ready for user review."
    })
}
