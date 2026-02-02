//! Telegram markdown formatting helpers.

/// Escape special characters for Telegram MarkdownV2
pub fn escape_markdown(text: &str) -> String {
    // Characters that need escaping in MarkdownV2
    const SPECIAL_CHARS: &[char] = &[
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];

    let mut result = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        if SPECIAL_CHARS.contains(&ch) {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}

/// Convert generic markdown to Telegram MarkdownV2 format
pub fn to_telegram_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Handle code blocks
        if ch == '`' && i + 2 < len && chars[i + 1] == '`' && chars[i + 2] == '`' {
            // Find closing ```
            let start = i + 3;
            if let Some(end_pos) = find_triple_backtick(&chars, start) {
                result.push_str("```");
                // Extract language hint if present
                let code_start =
                    if let Some(newline) = chars[start..end_pos].iter().position(|&c| c == '\n') {
                        result.extend(&chars[start..start + newline + 1]);
                        start + newline + 1
                    } else {
                        result.push('\n');
                        start
                    };
                // Code content doesn't need escaping
                result.extend(&chars[code_start..end_pos]);
                result.push_str("```");
                i = end_pos + 3;
                continue;
            }
        }

        // Handle inline code
        if ch == '`' {
            if let Some(end_pos) = chars[i + 1..].iter().position(|&c| c == '`') {
                result.push('`');
                // Code content doesn't need escaping
                result.extend(&chars[i + 1..i + 1 + end_pos]);
                result.push('`');
                i += 2 + end_pos;
                continue;
            }
        }

        // Handle bold **text**
        if ch == '*' && i + 1 < len && chars[i + 1] == '*' {
            if let Some(end_pos) = find_double_char(&chars, i + 2, '*') {
                result.push('*');
                for ch in chars.iter().take(end_pos).skip(i + 2) {
                    result.push_str(&escape_single(*ch));
                }
                result.push('*');
                i = end_pos + 2;
                continue;
            }
        }

        // Handle italic _text_
        if ch == '_' {
            if let Some(end_pos) = chars[i + 1..].iter().position(|&c| c == '_') {
                result.push_str("__");
                for ch in chars.iter().skip(i + 1).take(end_pos) {
                    result.push_str(&escape_single(*ch));
                }
                result.push_str("__");
                i += 2 + end_pos;
                continue;
            }
        }

        // Handle links [text](url)
        if ch == '[' {
            if let Some((text_end, url_start, url_end)) = parse_markdown_link(&chars, i) {
                result.push('[');
                for ch in chars.iter().take(text_end).skip(i + 1) {
                    result.push_str(&escape_single(*ch));
                }
                result.push_str("](");
                result.extend(&chars[url_start..url_end]);
                result.push(')');
                i = url_end + 1;
                continue;
            }
        }

        // Escape special characters outside of special contexts
        result.push_str(&escape_single(ch));
        i += 1;
    }

    result
}

/// Escape a single character if needed
fn escape_single(ch: char) -> String {
    const SPECIAL_CHARS: &[char] = &[
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];

    if SPECIAL_CHARS.contains(&ch) {
        format!("\\{}", ch)
    } else {
        ch.to_string()
    }
}

/// Find closing ``` starting from position
fn find_triple_backtick(chars: &[char], start: usize) -> Option<usize> {
    let len = chars.len();
    let mut i = start;
    while i + 2 < len {
        if chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find double char (like **) starting from position
fn find_double_char(chars: &[char], start: usize, ch: char) -> Option<usize> {
    let len = chars.len();
    let mut i = start;
    while i + 1 < len {
        if chars[i] == ch && chars[i + 1] == ch {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Parse a markdown link [text](url)
fn parse_markdown_link(chars: &[char], start: usize) -> Option<(usize, usize, usize)> {
    let len = chars.len();
    let mut i = start + 1;

    // Find closing ]
    let mut depth = 1;
    while i < len && depth > 0 {
        match chars[i] {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }

    if depth != 0 || i >= len {
        return None;
    }

    let text_end = i - 1;

    // Expect (
    if chars[i] != '(' {
        return None;
    }
    let url_start = i + 1;

    // Find closing )
    i = url_start;
    depth = 1;
    while i < len && depth > 0 {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }

    if depth != 0 {
        return None;
    }

    let url_end = i - 1;

    Some((text_end, url_start, url_end))
}

/// Truncate text to fit Telegram message limits (4096 chars)
pub fn truncate_for_telegram(text: &str, max_len: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_len {
        return text.to_string();
    }

    // Find a good breaking point (in chars, not bytes)
    let truncate_at = max_len.saturating_sub(20);

    // Collect chars up to truncate_at to get a valid UTF-8 boundary
    let truncated: String = text.chars().take(truncate_at).collect();

    // Try to break at newline
    if let Some(pos) = truncated.rfind('\n') {
        format!("{}\n\n_\\.\\.\\. \\(truncated\\)_", &truncated[..pos])
    } else {
        format!("{}\n\n_\\.\\.\\. \\(truncated\\)_", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_markdown() {
        assert_eq!(escape_markdown("hello"), "hello");
        assert_eq!(escape_markdown("hello_world"), "hello\\_world");
        assert_eq!(escape_markdown("**bold**"), "\\*\\*bold\\*\\*");
    }

    #[test]
    fn test_truncate() {
        let short = "hello";
        assert_eq!(truncate_for_telegram(short, 100), "hello");

        let long = "a".repeat(200);
        let truncated = truncate_for_telegram(&long, 100);
        assert!(truncated.len() < 150); // Leaves room for suffix
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_truncate_utf8() {
        // Test with multi-byte UTF-8 characters (emoji are 4 bytes each)
        let emoji_text = "🎉".repeat(50); // 50 emojis = 50 chars but 200 bytes
        let truncated = truncate_for_telegram(&emoji_text, 30);
        assert!(truncated.contains("truncated"));
        // Should not panic and should produce valid UTF-8
        assert!(truncated.is_ascii() || truncated.chars().count() > 0);

        // Test with mixed content
        let mixed = format!("Hello 世界! {}", "🚀".repeat(40));
        let truncated = truncate_for_telegram(&mixed, 25);
        assert!(truncated.contains("truncated"));
    }
}
