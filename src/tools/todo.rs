//! TodoWrite tool for task tracking.

use super::SchemaOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;

/// Status of a todo item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub content: String,
    #[serde(rename = "activeForm")]
    pub active_form: String,
    pub status: TodoStatus,
}

/// Todo list state
#[derive(Debug, Default)]
pub struct TodoState {
    pub todos: Vec<Todo>,
}

impl TodoState {
    pub fn new() -> Self {
        Self { todos: Vec::new() }
    }

    /// Update the entire todo list
    pub fn update(&mut self, todos: Vec<Todo>) {
        self.todos = todos;
    }

    /// Display the todo list to stderr
    pub fn display(&self) {
        if self.todos.is_empty() {
            return;
        }

        eprintln!("\n┌─ Tasks ─────────────────────────────────");
        for todo in &self.todos {
            let status_icon = match todo.status {
                TodoStatus::Pending => "○",
                TodoStatus::InProgress => "◉",
                TodoStatus::Completed => "✓",
            };
            let display_text = if todo.status == TodoStatus::InProgress {
                &todo.active_form
            } else {
                &todo.content
            };
            eprintln!("│ {} {}", status_icon, display_text);
        }
        eprintln!("└──────────────────────────────────────────\n");
    }

    /// Count by status
    pub fn count_by_status(&self) -> (usize, usize, usize) {
        let pending = self.todos.iter().filter(|t| t.status == TodoStatus::Pending).count();
        let in_progress = self.todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
        let completed = self.todos.iter().filter(|t| t.status == TodoStatus::Completed).count();
        (pending, in_progress, completed)
    }
}

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "TodoWrite",
                "description": "Update task list",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": { "type": "string" },
                                    "activeForm": { "type": "string" },
                                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                                },
                                "required": ["content", "activeForm", "status"]
                            }
                        }
                    },
                    "required": ["todos"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "TodoWrite",
                "description": "Create and manage a structured task list. Use to track progress on multi-step tasks. Mark tasks as in_progress before starting, completed when done.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "description": "The complete updated todo list",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": {
                                        "type": "string",
                                        "description": "Task description in imperative form (e.g., 'Fix the bug')"
                                    },
                                    "activeForm": {
                                        "type": "string",
                                        "description": "Present continuous form shown during execution (e.g., 'Fixing the bug')"
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": ["pending", "in_progress", "completed"],
                                        "description": "Task status: pending (not started), in_progress (currently working), completed (finished)"
                                    }
                                },
                                "required": ["content", "activeForm", "status"]
                            }
                        }
                    },
                    "required": ["todos"]
                }
            }
        })
    }
}

/// Execute the TodoWrite tool
pub fn execute(args: Value, todo_state: &RefCell<TodoState>) -> Value {
    let todos_value = match args.get("todos") {
        Some(v) => v,
        None => {
            return json!({
                "error": {
                    "code": "missing_todos",
                    "message": "Missing required 'todos' parameter"
                }
            });
        }
    };

    let todos: Vec<Todo> = match serde_json::from_value(todos_value.clone()) {
        Ok(t) => t,
        Err(e) => {
            return json!({
                "error": {
                    "code": "invalid_todos",
                    "message": format!("Invalid todos format: {}", e)
                }
            });
        }
    };

    // Validate: at most one in_progress
    let in_progress_count = todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
    if in_progress_count > 1 {
        return json!({
            "error": {
                "code": "multiple_in_progress",
                "message": "Only one task can be in_progress at a time"
            }
        });
    }

    // Update state and display
    let mut state = todo_state.borrow_mut();
    state.update(todos);
    state.display();

    let (pending, in_progress, completed) = state.count_by_status();

    json!({
        "ok": true,
        "pending": pending,
        "in_progress": in_progress,
        "completed": completed
    })
}
