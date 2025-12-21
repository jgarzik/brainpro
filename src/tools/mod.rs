pub mod bash;
pub mod edit;
mod glob;
mod grep;
pub mod mcp_dispatch;
mod read;
pub mod task;
mod write;

use crate::config::BashConfig;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// Get all built-in tool schemas (excluding Task - used by subagents)
pub fn schemas() -> Vec<Value> {
    vec![
        read::schema(),
        write::schema(),
        edit::schema(),
        grep::schema(),
        glob::schema(),
        bash::schema(),
    ]
}

/// Get all tool schemas including Task (used by main agent)
pub fn schemas_with_task() -> Vec<Value> {
    vec![
        read::schema(),
        write::schema(),
        edit::schema(),
        grep::schema(),
        glob::schema(),
        bash::schema(),
        task::schema(),
    ]
}

/// Execute a tool by name
/// For Bash tool, uses the provided BashConfig; other tools ignore it
pub fn execute(name: &str, args: Value, root: &Path, bash_config: &BashConfig) -> Result<Value> {
    match name {
        "Read" => read::execute(args, root),
        "Write" => write::execute(args, root),
        "Edit" => edit::execute(args, root),
        "Grep" => grep::execute(args, root),
        "Glob" => glob::execute(args, root),
        "Bash" => bash::execute(args, root, bash_config),
        _ => Ok(
            json!({ "error": { "code": "unknown_tool", "message": format!("Unknown tool: {}", name) } }),
        ),
    }
}

#[allow(dead_code)]
pub fn requires_permission(name: &str) -> bool {
    matches!(name, "Write" | "Edit" | "Bash")
}

fn validate_path(path: &str, root: &Path) -> Result<std::path::PathBuf, Value> {
    if path.starts_with('/') {
        return Err(
            json!({ "error": { "code": "path_out_of_scope", "message": "Absolute paths not allowed" } }),
        );
    }

    let full = root.join(path);
    let canonical = match full.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            if let Some(parent) = full.parent() {
                if let Ok(cp) = parent.canonicalize() {
                    cp.join(full.file_name().unwrap_or_default())
                } else {
                    full.clone()
                }
            } else {
                full.clone()
            }
        }
    };

    if !canonical.starts_with(root) {
        let norm = normalize_path(&full);
        if !norm.starts_with(root) {
            return Err(
                json!({ "error": { "code": "path_out_of_scope", "message": "Path escapes project root" } }),
            );
        }
        return Ok(norm);
    }

    Ok(canonical)
}

fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut result = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            c => result.push(c),
        }
    }
    result
}

fn sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
