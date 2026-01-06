//! AskUserQuestion tool for interactive question-answer flow.

use super::SchemaOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A single option for a question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

/// A single question to ask the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(rename = "multiSelect", default)]
    pub multi_select: bool,
}

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "AskUserQuestion",
                "description": "Ask user questions",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 4,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "question": { "type": "string" },
                                    "header": { "type": "string" },
                                    "options": {
                                        "type": "array",
                                        "minItems": 2,
                                        "maxItems": 4,
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": { "type": "string" },
                                                "description": { "type": "string" }
                                            },
                                            "required": ["label", "description"]
                                        }
                                    },
                                    "multiSelect": { "type": "boolean" }
                                },
                                "required": ["question", "header", "options", "multiSelect"]
                            }
                        }
                    },
                    "required": ["questions"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "AskUserQuestion",
                "description": "Ask the user questions when you need clarification or input. Present options for the user to choose from.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "description": "Questions to ask the user (1-4 questions)",
                            "minItems": 1,
                            "maxItems": 4,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "question": {
                                        "type": "string",
                                        "description": "The complete question to ask. Should end with a question mark."
                                    },
                                    "header": {
                                        "type": "string",
                                        "description": "Short label (max 12 chars). Examples: 'Auth method', 'Library'"
                                    },
                                    "options": {
                                        "type": "array",
                                        "description": "2-4 options for the user to choose from",
                                        "minItems": 2,
                                        "maxItems": 4,
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": {
                                                    "type": "string",
                                                    "description": "Short option name (1-5 words)"
                                                },
                                                "description": {
                                                    "type": "string",
                                                    "description": "Explanation of what this option means"
                                                }
                                            },
                                            "required": ["label", "description"]
                                        }
                                    },
                                    "multiSelect": {
                                        "type": "boolean",
                                        "description": "If true, user can select multiple options"
                                    }
                                },
                                "required": ["question", "header", "options", "multiSelect"]
                            }
                        }
                    },
                    "required": ["questions"]
                }
            }
        })
    }
}

/// Validate the questions and return parsed questions or error
pub fn validate(args: &Value) -> Result<Vec<Question>, Value> {
    let questions_value = match args.get("questions") {
        Some(v) => v,
        None => {
            return Err(json!({
                "error": {
                    "code": "missing_questions",
                    "message": "Missing required 'questions' parameter"
                }
            }));
        }
    };

    let questions: Vec<Question> = match serde_json::from_value(questions_value.clone()) {
        Ok(q) => q,
        Err(e) => {
            return Err(json!({
                "error": {
                    "code": "invalid_questions",
                    "message": format!("Invalid questions format: {}", e)
                }
            }));
        }
    };

    if questions.is_empty() || questions.len() > 4 {
        return Err(json!({
            "error": {
                "code": "invalid_question_count",
                "message": "Must provide 1-4 questions"
            }
        }));
    }

    for q in &questions {
        if q.options.len() < 2 || q.options.len() > 4 {
            return Err(json!({
                "error": {
                    "code": "invalid_option_count",
                    "message": format!("Question '{}' must have 2-4 options", q.header)
                }
            }));
        }
    }

    Ok(questions)
}

/// Display questions to the user and collect answers
pub fn display_and_collect(
    questions: &[Question],
    readline: &mut rustyline::DefaultEditor,
) -> Result<Value, String> {
    let mut answers: std::collections::HashMap<String, Value> = std::collections::HashMap::new();

    for question in questions {
        eprintln!("\n┌─ {} ─────────────────────────────────", question.header);
        eprintln!("│ {}", question.question);
        eprintln!("│");

        for (i, opt) in question.options.iter().enumerate() {
            eprintln!("│ [{}] {} - {}", i + 1, opt.label, opt.description);
        }
        eprintln!("│ [{}] Other (custom input)", question.options.len() + 1);
        eprintln!("└──────────────────────────────────────────");

        let prompt = if question.multi_select {
            "Select options (comma-separated, e.g., 1,3): "
        } else {
            "Select option: "
        };

        let input = match readline.readline(prompt) {
            Ok(line) => line.trim().to_string(),
            Err(_) => return Err("Input cancelled".to_string()),
        };

        if question.multi_select {
            // Parse comma-separated selections
            let selections: Vec<String> = input
                .split(',')
                .filter_map(|s| {
                    let s = s.trim();
                    if let Ok(n) = s.parse::<usize>() {
                        if n > 0 && n <= question.options.len() {
                            Some(question.options[n - 1].label.clone())
                        } else if n == question.options.len() + 1 {
                            // "Other" selected - would need follow-up, for now just mark it
                            Some("Other".to_string())
                        } else {
                            None
                        }
                    } else {
                        // Treat as custom input
                        Some(s.to_string())
                    }
                })
                .collect();
            answers.insert(question.question.clone(), json!(selections));
        } else {
            let answer = if let Ok(n) = input.parse::<usize>() {
                if n > 0 && n <= question.options.len() {
                    question.options[n - 1].label.clone()
                } else if n == question.options.len() + 1 {
                    // "Other" - ask for custom input
                    match readline.readline("Enter your answer: ") {
                        Ok(custom) => custom.trim().to_string(),
                        Err(_) => return Err("Input cancelled".to_string()),
                    }
                } else {
                    input.clone()
                }
            } else {
                // Treat as custom input directly
                input.clone()
            };
            answers.insert(question.question.clone(), json!(answer));
        }
    }

    Ok(json!({
        "ok": true,
        "answers": answers
    }))
}
