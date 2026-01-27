//! Miscellaneous helper functions.

/// Format a timestamp as a string.
pub fn format_timestamp(secs: u64) -> String {
    format!("{}s ago", secs)
}

/// Generate a simple hash of a string.
pub fn simple_hash(input: &str) -> u64 {
    input.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64))
}

/// Truncate a string to a maximum length.
pub fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}
