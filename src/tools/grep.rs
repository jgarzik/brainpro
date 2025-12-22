use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use walkdir::WalkDir;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "Grep",
            "description": "Search file contents for pattern. Skips .git, target, .yo dirs.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search" },
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Paths to search (default: all)" },
                    "max_results": { "type": "integer", "description": "Max matches (default 100)" }
                },
                "required": ["pattern"]
            }
        }
    })
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let max_results = args["max_results"].as_u64().unwrap_or(100) as usize;

    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({ "error": { "code": "invalid_regex", "message": e.to_string() } }))
        }
    };

    let search_paths: Vec<&str> = args["paths"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut matches = Vec::new();
    let mut truncated = false;

    let walker = WalkDir::new(root);

    for entry in walker
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), root))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let rel_path = path.strip_prefix(root).unwrap_or(path);

        if !search_paths.is_empty() {
            let rel_str = rel_path.to_string_lossy();
            if !search_paths.iter().any(|p| rel_str.starts_with(p)) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_num, line) in content.lines().enumerate() {
            if let Some(m) = re.find(line) {
                if matches.len() >= max_results {
                    truncated = true;
                    break;
                }
                matches.push(json!({
                    "path": rel_path.to_string_lossy(),
                    "line": line_num + 1,
                    "col": m.start() + 1,
                    "snippet": line.chars().take(200).collect::<String>()
                }));
            }
        }

        if truncated {
            break;
        }
    }

    let match_count = matches.len();
    eprintln!("Found {} lines", match_count);

    Ok(json!({
        "matches": matches,
        "matches_found": match_count,
        "truncated": truncated
    }))
}

fn is_excluded(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    for component in rel.components() {
        let name = component.as_os_str().to_string_lossy();
        if name == ".git" || name == "target" || name == ".yo" || name == "node_modules" {
            return true;
        }
    }
    false
}
