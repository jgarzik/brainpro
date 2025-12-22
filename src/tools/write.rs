use super::{sha256, validate_path};
use serde_json::{json, Value};
use std::path::Path;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "Write",
            "description": "Create or overwrite a file. Requires permission.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to root" },
                    "content": { "type": "string", "description": "Content to write" },
                    "overwrite": { "type": "boolean", "description": "Allow overwrite (default true)" }
                },
                "required": ["path", "content"]
            }
        }
    })
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    let overwrite = args["overwrite"].as_bool().unwrap_or(true);

    let full_path = match validate_path(path, root) {
        Ok(p) => p,
        Err(e) => return Ok(e),
    };

    if full_path.exists() && !overwrite {
        return Ok(
            json!({ "error": { "code": "file_exists", "message": "File exists and overwrite=false" } }),
        );
    }

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let bytes = content.as_bytes();
    if let Err(e) = std::fs::write(&full_path, bytes) {
        return Ok(json!({ "error": { "code": "write_error", "message": e.to_string() } }));
    }

    let lines_written = content.lines().count();
    eprintln!("Wrote {} lines", lines_written);

    Ok(json!({
        "path": path,
        "bytes_written": bytes.len(),
        "lines": lines_written,
        "sha256": sha256(bytes)
    }))
}
