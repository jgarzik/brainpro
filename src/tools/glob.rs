use super::SchemaOptions;
use serde_json::{json, Value};
use std::path::Path;

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Glob",
                "description": "Find files by pattern",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "max_results": { "type": "integer" }
                    },
                    "required": ["pattern"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "Glob",
                "description": "Find files matching glob pattern. Returns paths relative to project root, sorted by modification time. Skips .git, target, .brainpro, node_modules dirs. Use **/*.rs for recursive, src/*.rs for single-directory. For open-ended searches requiring multiple rounds of globbing/grepping, use Task tool instead.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Glob pattern (e.g. **/*.rs)" },
                        "max_results": { "type": "integer", "description": "Max files (default 2000)" }
                    },
                    "required": ["pattern"]
                }
            }
        })
    }
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let max_results = args["max_results"].as_u64().unwrap_or(2000) as usize;

    let full_pattern = root.join(pattern).to_string_lossy().to_string();

    let entries = match glob::glob(&full_pattern) {
        Ok(e) => e,
        Err(e) => {
            return Ok(json!({ "error": { "code": "invalid_glob", "message": e.to_string() } }))
        }
    };

    let mut paths = Vec::new();
    let mut truncated = false;

    for entry in entries {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };

        if is_excluded(&path, root) {
            continue;
        }

        let rel = path.strip_prefix(root).unwrap_or(&path);

        if paths.len() >= max_results {
            truncated = true;
            break;
        }

        paths.push(rel.to_string_lossy().to_string());
    }

    Ok(json!({
        "paths": paths,
        "truncated": truncated
    }))
}

fn is_excluded(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    for component in rel.components() {
        let name = component.as_os_str().to_string_lossy();
        if name == ".git" || name == "target" || name == ".brainpro" || name == "node_modules" {
            return true;
        }
    }
    false
}
