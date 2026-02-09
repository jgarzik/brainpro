use crate::llm::{ChatRequest, ChatResponse, Choice, LlmClient, Message, Usage};
use anyhow::Result;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct MockLlmClient {
    responses: Arc<Mutex<Vec<ChatResponse>>>,
    requests: Arc<Mutex<Vec<ChatRequest>>>,
}

impl MockLlmClient {
    pub fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn requests(&self) -> Vec<ChatRequest> {
        self.requests.lock().expect("requests lock").clone()
    }

    pub fn response_with_content(content: &str) -> ChatResponse {
        ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: "assistant".to_string(),
                    content: Some(content.to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
            }),
        }
    }

    pub fn response_with_content_and_usage(
        content: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> ChatResponse {
        ChatResponse {
            choices: vec![Choice {
                message: Message {
                    role: "assistant".to_string(),
                    content: Some(content.to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens,
            }),
        }
    }

    pub fn response_with_message(message: Message, usage: Option<Usage>) -> ChatResponse {
        ChatResponse {
            choices: vec![Choice {
                message,
                finish_reason: None,
            }],
            usage,
        }
    }
}

impl LlmClient for MockLlmClient {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        let mut responses = self.responses.lock().expect("responses lock");
        if responses.is_empty() {
            return Ok(MockLlmClient::response_with_content(""));
        }
        Ok(responses.remove(0))
    }
}

pub fn message(role: &str, content: &str) -> Value {
    serde_json::json!({
        "role": role,
        "content": content,
    })
}

pub fn tool_message(tool_call_id: &str, content: &str) -> Value {
    serde_json::json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": content,
    })
}
