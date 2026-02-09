use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const MAX_TOOL_OUTPUT_BYTES: usize = 50_000;
pub const MAX_TOOL_OUTPUT_LINES: usize = 2000;
pub const MAX_ARRAY_ITEMS: usize = 2000;
const TRUNCATION_SUFFIX: &str = "\n... [truncated]";

pub fn maybe_truncate(tool_name: &str, result: &Value, root: &Path) -> Value {
    let full_json = match serde_json::to_string_pretty(result) {
        Ok(s) => s,
        Err(_) => return result.clone(),
    };

    let full_bytes = full_json.len();
    let full_lines = count_lines(&full_json);

    if full_bytes <= MAX_TOOL_OUTPUT_BYTES && full_lines <= MAX_TOOL_OUTPUT_LINES {
        return result.clone();
    }

    let file_path = write_full_output(tool_name, root, &full_json).ok();

    let mut truncated_value = result.clone();
    let mut truncated_any = false;
    truncate_value(&mut truncated_value, &mut truncated_any);

    let truncation_info =
        build_truncation_info(file_path.as_ref(), full_bytes, full_lines, &truncated_value);

    if let Value::Object(ref mut map) = truncated_value {
        map.insert("output_truncated".to_string(), Value::Bool(true));
        map.insert("output_truncation".to_string(), truncation_info);
    } else {
        truncated_value = json!({
            "output_truncated": true,
            "output_truncation": truncation_info,
            "value": truncated_value,
        });
    }

    let truncated_json = match serde_json::to_string_pretty(&truncated_value) {
        Ok(s) => s,
        Err(_) => return truncated_fallback(file_path.as_ref(), &full_json),
    };

    if truncated_json.len() > MAX_TOOL_OUTPUT_BYTES
        || count_lines(&truncated_json) > MAX_TOOL_OUTPUT_LINES
    {
        return truncated_fallback(file_path.as_ref(), &full_json);
    }

    if truncated_any {
        truncated_value
    } else {
        result.clone()
    }
}

fn write_full_output(tool_name: &str, root: &Path, content: &str) -> std::io::Result<PathBuf> {
    let dir = root.join(".brainpro").join("tool_output");
    std::fs::create_dir_all(&dir)?;
    let file_name = format!("{}_{}.json", tool_name.to_lowercase(), Uuid::new_v4());
    let path = dir.join(file_name);
    std::fs::write(&path, content.as_bytes())?;
    Ok(path)
}

fn build_truncation_info(
    file_path: Option<&PathBuf>,
    full_bytes: usize,
    full_lines: usize,
    preview: &Value,
) -> Value {
    let preview_json = serde_json::to_string(preview).unwrap_or_default();
    let preview_bytes = preview_json.len();
    let preview_lines = count_lines(&preview_json);
    let rel_path = file_path.and_then(|p| p.to_str()).unwrap_or("").to_string();

    json!({
        "file": rel_path,
        "bytes": full_bytes,
        "lines": full_lines,
        "preview_bytes": preview_bytes,
        "preview_lines": preview_lines
    })
}

fn truncated_fallback(file_path: Option<&PathBuf>, full_json: &str) -> Value {
    let (preview, _) = truncate_text(full_json, MAX_TOOL_OUTPUT_BYTES, MAX_TOOL_OUTPUT_LINES);
    let preview_lines = count_lines(&preview);
    let preview_bytes = preview.len();
    let rel_path = file_path.and_then(|p| p.to_str()).unwrap_or("").to_string();

    json!({
        "output_truncated": true,
        "output_truncation": {
            "file": rel_path,
            "bytes": full_json.len(),
            "lines": count_lines(full_json),
            "preview_bytes": preview_bytes,
            "preview_lines": preview_lines
        },
        "preview": preview
    })
}

fn truncate_value(value: &mut Value, truncated_any: &mut bool) {
    match value {
        Value::String(s) => {
            let (truncated, did_truncate) =
                truncate_text(s, MAX_TOOL_OUTPUT_BYTES, MAX_TOOL_OUTPUT_LINES);
            if did_truncate {
                *truncated_any = true;
                *s = truncated;
            }
        }
        Value::Array(items) => {
            if items.len() > MAX_ARRAY_ITEMS {
                items.truncate(MAX_ARRAY_ITEMS);
                *truncated_any = true;
            }
            for item in items.iter_mut() {
                truncate_value(item, truncated_any);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                truncate_value(value, truncated_any);
            }
        }
        _ => {}
    }
}

fn truncate_text(text: &str, max_bytes: usize, max_lines: usize) -> (String, bool) {
    let mut truncated = false;
    let mut line_count = 1usize;
    let mut end_idx = text.len();

    for (idx, ch) in text.char_indices() {
        let bytes = idx + ch.len_utf8();
        if ch == '\n' {
            line_count += 1;
        }
        if bytes > max_bytes || line_count > max_lines {
            truncated = true;
            end_idx = idx;
            break;
        }
    }

    if !truncated {
        return (text.to_string(), false);
    }

    let mut out = text[..end_idx].to_string();
    if out.len() + TRUNCATION_SUFFIX.len() > max_bytes {
        let allowed = max_bytes.saturating_sub(TRUNCATION_SUFFIX.len());
        if allowed < out.len() {
            out.truncate(allowed);
        }
    }
    out.push_str(TRUNCATION_SUFFIX);
    (out, true)
}

fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.bytes().filter(|b| *b == b'\n').count() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_no_truncation_for_small_output() {
        let root = TempDir::new().unwrap();
        let result = json!({"ok": true, "message": "hi"});
        let truncated = maybe_truncate("Read", &result, root.path());
        assert_eq!(truncated, result);
    }

    #[test]
    fn test_truncates_large_string() {
        let root = TempDir::new().unwrap();
        let big = "a".repeat(MAX_TOOL_OUTPUT_BYTES + 1000);
        let result = json!({"content": big});
        let truncated = maybe_truncate("Read", &result, root.path());
        assert!(truncated["output_truncated"].as_bool().unwrap());
        let content = truncated
            .get("content")
            .and_then(|v| v.as_str())
            .or_else(|| truncated.get("preview").and_then(|v| v.as_str()))
            .unwrap_or("");
        assert!(content.contains("[truncated]"));
    }

    #[test]
    fn test_truncates_large_array() {
        let root = TempDir::new().unwrap();
        let items: Vec<Value> = (0..(MAX_ARRAY_ITEMS + 10))
            .map(|i| json!({"id": i}))
            .collect();
        let result = json!({"items": items});
        let truncated = maybe_truncate("Search", &result, root.path());
        if let Some(arr) = truncated.get("items").and_then(|v| v.as_array()) {
            assert_eq!(arr.len(), MAX_ARRAY_ITEMS);
        } else {
            assert!(truncated.get("output_truncated").is_some());
        }
    }
}
