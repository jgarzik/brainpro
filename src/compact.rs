//! Context compaction for managing conversation history.
//!
//! When the context window fills up, this module summarizes older messages
//! to reclaim space while preserving essential information.

use crate::config::ContextConfig;
use crate::llm::{ChatRequest, Client, LlmClient};
use anyhow::Result;
use serde_json::{json, Value};
use tiktoken_rs::cl100k_base;

/// Estimate character count for a message
fn estimate_chars(msg: &Value) -> usize {
    serde_json::to_string(msg).map(|s| s.len()).unwrap_or(0)
}

fn estimate_tokens(msg: &Value) -> usize {
    let text = serde_json::to_string(msg).unwrap_or_default();
    let bpe = cl100k_base().ok();
    if let Some(encoder) = bpe {
        encoder.encode_with_special_tokens(&text).len()
    } else {
        text.len() / 4
    }
}

/// Calculate total context size in characters
pub fn context_size(messages: &[Value]) -> usize {
    messages.iter().map(estimate_chars).sum()
}

pub fn context_tokens(messages: &[Value]) -> usize {
    messages.iter().map(estimate_tokens).sum()
}

/// Check if compaction is needed based on config thresholds
pub fn needs_compaction(messages: &[Value], config: &ContextConfig) -> bool {
    if !config.auto_compact_enabled {
        return false;
    }
    let current_tokens = context_tokens(messages);
    let token_threshold =
        (config.context_window_tokens as f64 * config.token_budget_ratio) as usize;
    if current_tokens > token_threshold {
        return true;
    }
    let current_size = context_size(messages);
    let char_threshold = (config.max_chars as f64 * config.auto_compact_threshold) as usize;
    current_size > char_threshold
}

/// Result of compaction
#[derive(Debug)]
pub struct CompactionResult {
    pub original_count: usize,
    pub compacted_count: usize,
    pub original_tokens: usize,
    pub compacted_tokens: usize,
    pub summary: String,
}

/// Compact conversation history by summarizing older messages
///
/// Strategy:
/// 1. Keep the most recent `keep_last_turns` messages
/// 2. Summarize all earlier messages into a single system message
/// 3. Return the compacted message list
pub fn compact_messages(
    messages: &[Value],
    config: &ContextConfig,
    llm_client: &Client,
    model: &str,
) -> Result<(Vec<Value>, CompactionResult)> {
    let original_count = messages.len();
    let original_tokens = context_tokens(messages);

    // If we have fewer messages than keep_last_turns, nothing to compact
    if messages.len() <= config.keep_last_turns * 2 {
        return Ok((
            messages.to_vec(),
            CompactionResult {
                original_count,
                compacted_count: messages.len(),
                original_tokens,
                compacted_tokens: original_tokens,
                summary: String::new(),
            },
        ));
    }

    let summary = generate_summary_with_fallback(messages, config, llm_client, model)?;
    let split_point = messages.len().saturating_sub(config.keep_last_turns * 2);
    let to_keep = &messages[split_point..];

    // Build compacted message list
    let mut compacted = Vec::new();

    // Add summary as a system message
    compacted.push(json!({
        "role": "system",
        "content": format!(
            "CONVERSATION SUMMARY (compacted from {} earlier messages):\n\n{}",
            split_point,
            summary
        )
    }));

    // Add the recent messages
    compacted.extend(to_keep.iter().cloned());

    let compacted_tokens = context_tokens(&compacted);
    let compacted_count = compacted.len();

    Ok((
        compacted,
        CompactionResult {
            original_count,
            compacted_count,
            original_tokens,
            compacted_tokens,
            summary,
        },
    ))
}

/// Generate a summary of messages using the LLM
fn generate_summary(messages: &[Value], client: &Client, model: &str) -> Result<String> {
    // Format messages for summarization
    let mut conversation_text = String::new();
    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("unknown");
        let content = msg["content"].as_str().unwrap_or("");

        // Skip tool call messages but note their presence
        if msg.get("tool_calls").is_some() {
            conversation_text.push_str(&format!("[{}: used tools]\n", role));
            continue;
        }

        // Handle tool responses
        if role == "tool" {
            let tool_id = msg["tool_call_id"].as_str().unwrap_or("unknown");
            // Truncate long tool results
            let content_preview = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.to_string()
            };
            conversation_text
                .push_str(&format!("[tool result {}]: {}\n", tool_id, content_preview));
            continue;
        }

        if !content.is_empty() {
            conversation_text.push_str(&format!("{}: {}\n\n", role, content));
        }
    }

    // Create summarization request
    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            json!({
                "role": "system",
                "content": "You are a conversation summarizer. Create a concise summary that captures:
1. What the user asked for
2. What was accomplished (files created/modified, commands run)
3. Any important decisions or context
4. Current state and any pending work

Be brief but complete. Focus on facts and outcomes."
            }),
            json!({
                "role": "user",
                "content": format!("Summarize this conversation:\n\n{}", conversation_text)
            }),
        ],
        tools: None,
        tool_choice: None,
    };

    let response = client.chat(&request)?;

    if let Some(choice) = response.choices.first() {
        if let Some(content) = &choice.message.content {
            return Ok(content.clone());
        }
    }

    Ok("Unable to generate summary.".to_string())
}

fn generate_summary_with_fallback(
    messages: &[Value],
    config: &ContextConfig,
    client: &Client,
    model: &str,
) -> Result<String> {
    let split_point = messages.len().saturating_sub(config.keep_last_turns * 2);
    let (to_summarize, _) = messages.split_at(split_point);
    let budget = (config.context_window_tokens as f64 * config.token_budget_ratio) as usize;

    let summary = generate_summary(to_summarize, client, model)?;
    let summary_tokens = estimate_tokens(&json!({"role": "system", "content": summary}));
    if summary_tokens < budget {
        return Ok(summary);
    }

    let filtered = filter_oversized_messages(to_summarize, budget);
    if !filtered.is_empty() {
        let summary = generate_summary(&filtered, client, model)?;
        let summary_tokens = estimate_tokens(&json!({"role": "system", "content": summary}));
        if summary_tokens < budget {
            return Ok(summary);
        }
    }

    let note = format!(
        "Summary omitted due to context size constraints. {} messages retained.",
        messages.len() - split_point
    );
    Ok(note)
}

fn filter_oversized_messages(messages: &[Value], budget: usize) -> Vec<Value> {
    let mut filtered = Vec::new();
    for msg in messages {
        let tokens = estimate_tokens(msg);
        if tokens < budget / 2 {
            filtered.push(msg.clone());
        }
    }
    filtered
}

/// Format compaction result for display
pub fn format_result(result: &CompactionResult) -> String {
    let reduction = if result.original_tokens > 0 {
        100.0 - (result.compacted_tokens as f64 / result.original_tokens as f64 * 100.0)
    } else {
        0.0
    };

    format!(
        "Compacted: {} → {} messages, {} → {} tokens ({:.0}% reduction)",
        result.original_count,
        result.compacted_count,
        result.original_tokens,
        result.compacted_tokens,
        reduction
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_size() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "hi there"}),
        ];
        let size = context_size(&messages);
        assert!(size > 0);
    }

    #[test]
    fn test_needs_compaction() {
        let config = ContextConfig {
            max_chars: 100,
            auto_compact_threshold: 0.8,
            auto_compact_enabled: true,
            keep_last_turns: 2,
            token_budget_ratio: 0.9,
            context_window_tokens: 1000,
        };

        // Small context - no compaction needed
        let small_messages = vec![json!({"role": "user", "content": "hi"})];
        assert!(!needs_compaction(&small_messages, &config));

        // Disabled - no compaction
        let disabled_config = ContextConfig {
            auto_compact_enabled: false,
            ..config
        };
        let large_messages: Vec<Value> = (0..100)
            .map(|i| json!({"role": "user", "content": format!("message {}", i)}))
            .collect();
        assert!(!needs_compaction(&large_messages, &disabled_config));
    }

    #[test]
    fn test_context_tokens_nonzero() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "world"}),
        ];
        let tokens = context_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_filter_oversized_messages() {
        let messages = vec![
            json!({"role": "user", "content": "short"}),
            json!({"role": "assistant", "content": "a".repeat(10_000)}),
        ];
        let filtered = filter_oversized_messages(&messages, 1000);
        assert_eq!(filtered.len(), 1);
    }
}
