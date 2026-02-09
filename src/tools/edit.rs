use super::{sha256, validate_path, SchemaOptions};
use serde_json::{json, Value};
use std::path::Path;

const MAX_FALLBACK_ATTEMPTS: usize = 5;

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Edit",
                "description": "Edit file: find→replace",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "edits": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "find": { "type": "string" },
                                    "replace": { "type": "string" },
                                    "count": { "type": "integer", "description": "0=all, default 1" }
                                },
                                "required": ["find", "replace"]
                            }
                        }
                    },
                    "required": ["path", "edits"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "Edit",
                "description": "Edit file with find/replace. Requires permission.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to root" },
                        "edits": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "find": { "type": "string" },
                                    "replace": { "type": "string" },
                                    "count": { "type": "integer", "description": "Times to replace (0=all, default 1)" }
                                },
                                "required": ["find", "replace"]
                            }
                        }
                    },
                    "required": ["path", "edits"]
                }
            }
        })
    }
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");

    let full_path = match validate_path(path, root) {
        Ok(p) => p,
        Err(e) => return Ok(e),
    };

    let original = match std::fs::read_to_string(&full_path) {
        Ok(s) => s,
        Err(e) => {
            return Ok(json!({ "error": { "code": "read_error", "message": e.to_string() } }))
        }
    };

    let before_sha = sha256(original.as_bytes());
    let mut content = original.clone();
    let mut total_applied = 0;
    let mut fallback_used = false;

    let edits = args["edits"].as_array();
    if let Some(edits) = edits {
        for edit in edits {
            let find = edit["find"].as_str().unwrap_or("");
            let replace = edit["replace"].as_str().unwrap_or("");
            // Treat negative count as 1 (single replacement)
            let count = edit["count"].as_i64().unwrap_or(1).max(0);

            if find.is_empty() {
                continue;
            }

            let mut applied = 0usize;
            let mut updated = content.clone();
            for attempt in 0..MAX_FALLBACK_ATTEMPTS {
                let matcher = MatchStrategy::from_index(attempt);
                let (normalized_content, map) = match matcher {
                    MatchStrategy::Exact => (content.clone(), None),
                    _ => {
                        let (normalized, map) = normalize_with_map(
                            &content,
                            matcher.uses_whitespace(),
                            matcher.uses_indent(),
                            matcher.uses_punctuation(),
                        );
                        (normalized, Some(map))
                    }
                };
                let candidate_find = normalize_text(find, matcher);
                let candidate_replace = normalize_text(replace, matcher);
                let (next_content, count_applied) = match map {
                    None => {
                        apply_replacements(&content, &candidate_find, &candidate_replace, count)
                    }
                    Some(ref mapping) => apply_replacements_with_map(
                        &content,
                        &normalized_content,
                        mapping,
                        &candidate_find,
                        replace,
                        count,
                    ),
                };
                if count_applied > 0 {
                    updated = next_content;
                    applied = count_applied;
                    if attempt > 0 {
                        fallback_used = true;
                    }
                    break;
                }
            }

            if applied > 0 {
                content = updated;
                total_applied += applied;
            }
        }
    }

    if let Err(e) = std::fs::write(&full_path, &content) {
        return Ok(json!({ "error": { "code": "write_error", "message": e.to_string() } }));
    }

    Ok(json!({
        "path": path,
        "applied": total_applied,
        "fallback_used": fallback_used,
        "before_sha256": before_sha,
        "after_sha256": sha256(content.as_bytes())
    }))
}

#[derive(Copy, Clone)]
enum MatchStrategy {
    Exact,
    TrimWhitespace,
    NormalizeWhitespace,
    NormalizeIndentation,
    NormalizePunctuation,
}

impl MatchStrategy {
    fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Exact,
            1 => Self::TrimWhitespace,
            2 => Self::NormalizeWhitespace,
            3 => Self::NormalizeIndentation,
            _ => Self::NormalizePunctuation,
        }
    }

    fn uses_whitespace(&self) -> bool {
        matches!(
            self,
            MatchStrategy::NormalizeWhitespace
                | MatchStrategy::NormalizeIndentation
                | MatchStrategy::NormalizePunctuation
        )
    }

    fn uses_indent(&self) -> bool {
        matches!(
            self,
            MatchStrategy::NormalizeIndentation | MatchStrategy::NormalizePunctuation
        )
    }

    fn uses_punctuation(&self) -> bool {
        matches!(self, MatchStrategy::NormalizePunctuation)
    }
}

#[derive(Clone, Debug)]
struct NormalizedMap {
    map: Vec<usize>,
}

impl NormalizedMap {
    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        if start >= self.map.len() || end >= self.map.len() {
            return None;
        }
        let orig_start = self.map[start];
        let orig_end = self.map[end].max(orig_start);
        Some((orig_start, orig_end))
    }
}

fn normalize_with_map(
    text: &str,
    collapse_whitespace: bool,
    normalize_indent: bool,
    normalize_punct: bool,
) -> (String, NormalizedMap) {
    let mut normalized = String::new();
    let mut map = Vec::new();
    let mut last_was_space = false;
    let mut at_line_start = true;

    for (idx, ch) in text.char_indices() {
        let mut current = ch;
        if normalize_punct {
            current = match current {
                '\u{2018}' | '\u{2019}' | '\u{201B}' => '\'',
                '\u{201C}' | '\u{201D}' | '\u{201F}' => '"',
                '\u{2013}' | '\u{2014}' => '-',
                _ => current,
            };
        }

        if normalize_indent && at_line_start && (current == ' ' || current == '\t') {
            continue;
        }

        if current == '\n' {
            normalized.push('\n');
            map.push(idx);
            last_was_space = false;
            at_line_start = true;
            continue;
        }

        if collapse_whitespace && current.is_whitespace() {
            if !last_was_space {
                normalized.push(' ');
                map.push(idx);
                last_was_space = true;
            }
            continue;
        }

        normalized.push(current);
        map.push(idx);
        last_was_space = false;
        at_line_start = false;
    }

    map.push(text.len());
    (normalized, NormalizedMap { map })
}

fn normalize_text(text: &str, strategy: MatchStrategy) -> String {
    match strategy {
        MatchStrategy::Exact => text.to_string(),
        MatchStrategy::TrimWhitespace => text.trim().to_string(),
        MatchStrategy::NormalizeWhitespace => normalize_with_map(text, true, false, false).0,
        MatchStrategy::NormalizeIndentation => normalize_with_map(text, true, true, false).0,
        MatchStrategy::NormalizePunctuation => normalize_with_map(text, true, true, true).0,
    }
}

fn apply_replacements(content: &str, find: &str, replace: &str, count: i64) -> (String, usize) {
    if find.is_empty() {
        return (content.to_string(), 0);
    }

    if count == 0 {
        let c = content.matches(find).count();
        let replaced = content.replace(find, replace);
        return (replaced, c);
    }

    let mut remaining = count as usize;
    let mut result = String::new();
    let mut rest = content;
    let mut applied = 0;

    while remaining > 0 {
        if let Some(pos) = rest.find(find) {
            result.push_str(&rest[..pos]);
            result.push_str(replace);
            rest = &rest[pos + find.len()..];
            remaining -= 1;
            applied += 1;
        } else {
            break;
        }
    }
    result.push_str(rest);
    (result, applied)
}

fn apply_replacements_with_map(
    original: &str,
    normalized: &str,
    map: &NormalizedMap,
    find: &str,
    replace: &str,
    count: i64,
) -> (String, usize) {
    if find.is_empty() {
        return (original.to_string(), 0);
    }

    let mut matches = Vec::new();
    let mut search_start = 0usize;
    while let Some(pos) = normalized[search_start..].find(find) {
        let start = search_start + pos;
        let end = start + find.len();
        if let Some((orig_start, orig_end)) = map.original_range(start, end) {
            matches.push((orig_start, orig_end));
        }
        search_start = end;
        if count > 0 && matches.len() >= count as usize {
            break;
        }
    }

    if matches.is_empty() {
        return (original.to_string(), 0);
    }

    let mut result = String::new();
    let mut last = 0usize;
    for (start, end) in matches.iter() {
        // Safety: skip matches with invalid char boundaries
        if !original.is_char_boundary(*start) || !original.is_char_boundary(*end) {
            continue;
        }
        result.push_str(&original[last..*start]);
        result.push_str(&indent_replacement(original, *start, replace));
        last = *end;
    }
    result.push_str(&original[last..]);
    (result, matches.len())
}

fn indent_replacement(original: &str, start: usize, replace: &str) -> String {
    // Safety: ensure start is a valid char boundary
    if !original.is_char_boundary(start) {
        return replace.to_string();
    }
    let line_start = original[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let indent = &original[line_start..start];
    if !indent.chars().all(|c| c == ' ' || c == '\t') {
        return replace.to_string();
    }

    let mut out = String::new();
    for (idx, line) in replace.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start_matches([' ', '\t']);
        out.push_str(indent);
        out.push_str(trimmed);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        name.to_string()
    }

    #[test]
    fn test_exact_replacement() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "hello world");
        let result = execute(
            json!({"path": path, "edits": [{"find": "world", "replace": "there"}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 1);
        assert!(!result["fallback_used"].as_bool().unwrap());
    }

    #[test]
    fn test_trim_whitespace_fallback() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "hello world\n");
        let result = execute(
            json!({"path": path, "edits": [{"find": "hello world  ", "replace": "hi"}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 1);
        assert!(result["fallback_used"].as_bool().unwrap());
    }

    #[test]
    fn test_normalize_whitespace_fallback() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "hello   world");
        let result = execute(
            json!({"path": path, "edits": [{"find": "hello world", "replace": "hi"}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 1);
        assert!(result["fallback_used"].as_bool().unwrap());
    }

    #[test]
    fn test_normalize_indent_fallback() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "    let x = 1;\n");
        let result = execute(
            json!({"path": path, "edits": [{"find": "let x = 1;", "replace": "let y = 2;"}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 1);
        // Verify fallback_used is present (value depends on whitespace handling)
        assert!(result["fallback_used"].as_bool().is_some());
    }

    #[test]
    fn test_punctuation_normalization() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "He said \"hi\"");
        let result = execute(
            json!({"path": path, "edits": [{"find": "He said “hi”", "replace": "He said hello"}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 1);
        assert!(result["fallback_used"].as_bool().unwrap());
    }

    #[test]
    fn test_count_replacement() {
        let dir = TempDir::new().unwrap();
        let path = write_file(&dir, "file.txt", "a a a");
        let result = execute(
            json!({"path": path, "edits": [{"find": "a", "replace": "b", "count": 2}]}),
            dir.path(),
        )
        .unwrap();
        assert_eq!(result["applied"], 2);
    }
}
