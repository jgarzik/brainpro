use super::{sha256, validate_path};
use serde_json::{json, Value};
use std::path::Path;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "Read",
            "description": "Read file content. Paths relative to project root.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to root" },
                    "max_bytes": { "type": "integer", "description": "Max bytes to read (default 65536)" },
                    "offset": { "type": "integer", "description": "Byte offset to start from (default 0)" }
                },
                "required": ["path"]
            }
        }
    })
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");
    let max_bytes = args["max_bytes"].as_u64().unwrap_or(65536) as usize;
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;

    let full_path = match validate_path(path, root) {
        Ok(p) => p,
        Err(e) => return Ok(e),
    };

    let data = match std::fs::read(&full_path) {
        Ok(d) => d,
        Err(e) => {
            return Ok(json!({ "error": { "code": "read_error", "message": e.to_string() } }))
        }
    };

    let end = (offset + max_bytes).min(data.len());
    let slice = &data[offset.min(data.len())..end];
    let truncated = end < data.len();

    let (content, encoding, lines_read) = match std::str::from_utf8(slice) {
        Ok(s) => {
            let line_count = s.lines().count();
            (s.to_string(), None, line_count)
        }
        Err(_) => (
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, slice),
            Some("base64"),
            0,
        ),
    };

    eprintln!("Read {} lines", lines_read);

    let mut result = json!({
        "path": path,
        "offset": offset,
        "truncated": truncated,
        "content": content,
        "sha256": sha256(&data),
        "lines": lines_read
    });

    if let Some(enc) = encoding {
        result["encoding"] = json!(enc);
    }

    Ok(result)
}
