//! Discord markdown and embed formatting helpers.

use serenity::builder::{CreateEmbed, CreateEmbedFooter};

/// Discord color codes
pub mod colors {
    pub const BLUE: u32 = 0x3498db;
    pub const GREEN: u32 = 0x2ecc71;
    pub const RED: u32 = 0xe74c3c;
    pub const YELLOW: u32 = 0xf1c40f;
    pub const PURPLE: u32 = 0x9b59b6;
    pub const GRAY: u32 = 0x95a5a6;
}

/// Discord markdown is mostly standard, but has some differences.
/// This function converts generic markdown to Discord-safe markdown.
pub fn to_discord_markdown(text: &str) -> String {
    // Discord uses standard markdown with some extensions
    // Main differences:
    // - Supports spoilers: ||text||
    // - Supports strikethrough: ~~text~~
    // - Code blocks work the same
    // - No need to escape most characters

    // For now, just pass through as Discord handles standard markdown well
    text.to_string()
}

/// Escape Discord markdown special characters
pub fn escape_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        match ch {
            '*' | '_' | '~' | '`' | '|' | '>' => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
        }
    }
    result
}

/// Create an embed for agent messages
pub fn create_message_embed(content: &str) -> CreateEmbed {
    CreateEmbed::new()
        .description(truncate(content, 4000))
        .color(colors::BLUE)
}

/// Create an embed for error messages
pub fn create_error_embed(code: &str, message: &str) -> CreateEmbed {
    CreateEmbed::new()
        .title(format!("Error: {}", code))
        .description(truncate(message, 4000))
        .color(colors::RED)
}

/// Create an embed for tool approval requests
pub fn create_approval_embed(
    tool_name: &str,
    tool_args: &serde_json::Value,
    policy_rule: Option<&str>,
) -> CreateEmbed {
    let args_display = if tool_args.is_object() {
        serde_json::to_string_pretty(tool_args).unwrap_or_else(|_| "{}".to_string())
    } else {
        tool_args.to_string()
    };

    let args_truncated = truncate(&args_display, 1000);

    let mut embed = CreateEmbed::new()
        .title("🔒 Tool Approval Required")
        .field("Tool", format!("`{}`", tool_name), false)
        .field(
            "Arguments",
            format!("```json\n{}\n```", args_truncated),
            false,
        )
        .color(colors::YELLOW);

    if let Some(rule) = policy_rule {
        embed = embed.footer(CreateEmbedFooter::new(format!("Policy: {}", rule)));
    }

    embed
}

/// Create an embed for completion
pub fn create_done_embed(input_tokens: u64, output_tokens: u64) -> CreateEmbed {
    CreateEmbed::new()
        .description("✅ Task completed")
        .field(
            "Tokens",
            format!("In: {} | Out: {}", input_tokens, output_tokens),
            true,
        )
        .color(colors::GREEN)
}

/// Truncate text to fit Discord limits
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let truncate_at = max_len.saturating_sub(20);
    let truncated = &text[..truncate_at];

    // Try to break at newline
    if let Some(pos) = truncated.rfind('\n') {
        format!("{}\n\n*... (truncated)*", &truncated[..pos])
    } else {
        format!("{}\n\n*... (truncated)*", truncated)
    }
}

/// Split long text into chunks for multiple messages
pub fn split_for_discord(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Find a good breaking point
        let break_at = remaining[..max_len]
            .rfind('\n')
            .or_else(|| remaining[..max_len].rfind(' '))
            .unwrap_or(max_len);

        chunks.push(remaining[..break_at].to_string());
        remaining = remaining[break_at..].trim_start();
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_markdown() {
        assert_eq!(escape_markdown("hello"), "hello");
        assert_eq!(escape_markdown("**bold**"), "\\*\\*bold\\*\\*");
        assert_eq!(escape_markdown("_italic_"), "\\_italic\\_");
    }

    #[test]
    fn test_truncate() {
        let short = "hello";
        assert_eq!(truncate(short, 100), "hello");

        let long = "a".repeat(200);
        let truncated = truncate(&long, 100);
        assert!(truncated.len() < 150);
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_split() {
        let short = "hello";
        let chunks = split_for_discord(short, 100);
        assert_eq!(chunks.len(), 1);

        let long = "word ".repeat(50);
        let chunks = split_for_discord(&long, 100);
        assert!(chunks.len() > 1);
    }
}
