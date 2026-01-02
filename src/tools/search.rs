use super::SchemaOptions;
use glob::Pattern;
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Search",
                "description": "Search file contents by regex pattern",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "path": { "type": "string" },
                        "output_mode": { "type": "string" },
                        "glob": { "type": "string" },
                        "case_insensitive": { "type": "boolean" },
                        "context_before": { "type": "integer" },
                        "context_after": { "type": "integer" },
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
                "name": "Search",
                "description": "Search file contents for regex pattern. Recursively searches directory. Skips .git, target, .yo, node_modules.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Regex pattern to search for" },
                        "path": { "type": "string", "description": "Directory to search (default: project root)" },
                        "output_mode": { "type": "string", "description": "Output format: 'content', 'files_with_matches', or 'count' (default: files_with_matches)" },
                        "glob": { "type": "string", "description": "Filter files by glob pattern (e.g. '*.rs')" },
                        "case_insensitive": { "type": "boolean", "description": "Case-insensitive search (default: false)" },
                        "context_before": { "type": "integer", "description": "Lines before match (output_mode=content only)" },
                        "context_after": { "type": "integer", "description": "Lines after match (output_mode=content only)" },
                        "max_results": { "type": "integer", "description": "Max results to return (default: 100)" }
                    },
                    "required": ["pattern"]
                }
            }
        })
    }
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let search_path = args["path"].as_str();
    let output_mode = args["output_mode"].as_str().unwrap_or("files_with_matches");
    let glob_pattern = args["glob"].as_str();
    let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);
    let context_before = args["context_before"].as_u64().unwrap_or(0) as usize;
    let context_after = args["context_after"].as_u64().unwrap_or(0) as usize;
    let max_results = args["max_results"].as_u64().unwrap_or(100) as usize;

    // Build regex
    let re = match RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({ "error": { "code": "invalid_regex", "message": e.to_string() } }))
        }
    };

    // Parse glob pattern if provided
    let glob_matcher = if let Some(g) = glob_pattern {
        match Pattern::new(g) {
            Ok(p) => Some(p),
            Err(e) => {
                return Ok(json!({ "error": { "code": "invalid_glob", "message": e.to_string() } }))
            }
        }
    } else {
        None
    };

    // Determine search root
    let search_root = if let Some(p) = search_path {
        let path = root.join(p);
        if !path.exists() {
            return Ok(
                json!({ "error": { "code": "path_not_found", "message": format!("Path not found: {}", p) } }),
            );
        }
        path
    } else {
        root.to_path_buf()
    };

    // Execute search based on output mode
    match output_mode {
        "content" => search_content(
            &search_root,
            root,
            &re,
            glob_matcher.as_ref(),
            context_before,
            context_after,
            max_results,
        ),
        "count" => search_count(&search_root, root, &re, glob_matcher.as_ref(), max_results),
        _ => search_files_with_matches(&search_root, root, &re, glob_matcher.as_ref(), max_results),
    }
}

fn search_files_with_matches(
    search_root: &Path,
    project_root: &Path,
    re: &Regex,
    glob_matcher: Option<&Pattern>,
    max_results: usize,
) -> anyhow::Result<Value> {
    let mut paths = Vec::new();
    let mut truncated = false;

    for entry in WalkDir::new(search_root)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), project_root))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let rel_path = path.strip_prefix(project_root).unwrap_or(path);

        // Apply glob filter
        if let Some(g) = glob_matcher {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !g.matches(file_name) {
                continue;
            }
        }

        // Read and search
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if re.is_match(&content) {
            if paths.len() >= max_results {
                truncated = true;
                break;
            }
            paths.push(rel_path.to_string_lossy().to_string());
        }
    }

    let count = paths.len();
    eprintln!("Found {} files", count);

    Ok(json!({
        "paths": paths,
        "count": count,
        "truncated": truncated
    }))
}

fn search_content(
    search_root: &Path,
    project_root: &Path,
    re: &Regex,
    glob_matcher: Option<&Pattern>,
    context_before: usize,
    context_after: usize,
    max_results: usize,
) -> anyhow::Result<Value> {
    let mut matches = Vec::new();
    let mut truncated = false;

    for entry in WalkDir::new(search_root)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), project_root))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let rel_path = path.strip_prefix(project_root).unwrap_or(path);

        // Apply glob filter
        if let Some(g) = glob_matcher {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !g.matches(file_name) {
                continue;
            }
        }

        // Read file
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            if let Some(m) = re.find(line) {
                if matches.len() >= max_results {
                    truncated = true;
                    break;
                }

                // Build context
                let start_idx = line_idx.saturating_sub(context_before);
                let end_idx = (line_idx + context_after + 1).min(lines.len());
                let context: Vec<String> = lines[start_idx..end_idx]
                    .iter()
                    .map(|l| l.chars().take(200).collect())
                    .collect();

                matches.push(json!({
                    "path": rel_path.to_string_lossy(),
                    "line": line_idx + 1,
                    "col": m.start() + 1,
                    "snippet": line.chars().take(200).collect::<String>(),
                    "context": context
                }));
            }
        }

        if truncated {
            break;
        }
    }

    let count = matches.len();
    eprintln!("Found {} matches", count);

    Ok(json!({
        "matches": matches,
        "count": count,
        "truncated": truncated
    }))
}

fn search_count(
    search_root: &Path,
    project_root: &Path,
    re: &Regex,
    glob_matcher: Option<&Pattern>,
    max_results: usize,
) -> anyhow::Result<Value> {
    let mut by_file: HashMap<String, usize> = HashMap::new();
    let mut total_count = 0;
    let mut truncated = false;

    for entry in WalkDir::new(search_root)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), project_root))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let rel_path = path.strip_prefix(project_root).unwrap_or(path);

        // Apply glob filter
        if let Some(g) = glob_matcher {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !g.matches(file_name) {
                continue;
            }
        }

        // Read and count
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file_count = re.find_iter(&content).count();
        if file_count > 0 {
            if by_file.len() >= max_results {
                truncated = true;
                break;
            }
            by_file.insert(rel_path.to_string_lossy().to_string(), file_count);
            total_count += file_count;
        }
    }

    eprintln!("Found {} matches in {} files", total_count, by_file.len());

    Ok(json!({
        "count": total_count,
        "by_file": by_file,
        "files_searched": by_file.len(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(
            src.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(
            src.join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("README.md"),
            "# Test Project\nThis is a test.\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_search_files_with_matches() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("paths").is_some());
        let paths = result["paths"].as_array().unwrap();
        assert_eq!(paths.len(), 2); // main.rs and lib.rs
        assert!(!result["truncated"].as_bool().unwrap());
    }

    #[test]
    fn test_search_content_mode() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn main", "output_mode": "content" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("matches").is_some());
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["line"].as_u64().unwrap(), 1);
    }

    #[test]
    fn test_search_count_mode() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn", "output_mode": "count" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("count").is_some());
        assert!(result["count"].as_u64().unwrap() >= 2);
        assert!(result.get("by_file").is_some());
    }

    #[test]
    fn test_search_with_glob_filter() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn", "glob": "*.rs" });
        let result = execute(args, dir.path()).unwrap();

        let paths = result["paths"].as_array().unwrap();
        for path in paths {
            assert!(path.as_str().unwrap().ends_with(".rs"));
        }
    }

    #[test]
    fn test_search_case_insensitive() {
        let dir = setup_test_dir();
        let args =
            json!({ "pattern": "FN MAIN", "case_insensitive": true, "output_mode": "content" });
        let result = execute(args, dir.path()).unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_search_with_path() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn", "path": "src" });
        let result = execute(args, dir.path()).unwrap();

        let paths = result["paths"].as_array().unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_search_invalid_regex() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "[invalid" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "invalid_regex");
    }

    #[test]
    fn test_search_path_not_found() {
        let dir = setup_test_dir();
        let args = json!({ "pattern": "fn", "path": "nonexistent" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "path_not_found");
    }

    #[test]
    fn test_schema() {
        let opts = SchemaOptions { optimize: false };
        let schema = schema(&opts);

        assert_eq!(schema["function"]["name"].as_str().unwrap(), "Search");
        assert!(schema["function"]["parameters"]["properties"]
            .get("pattern")
            .is_some());
        assert!(schema["function"]["parameters"]["properties"]
            .get("path")
            .is_some());
        assert!(schema["function"]["parameters"]["properties"]
            .get("output_mode")
            .is_some());
    }

    #[test]
    fn test_schema_optimized() {
        let opts = SchemaOptions { optimize: true };
        let schema = schema(&opts);

        assert_eq!(schema["function"]["name"].as_str().unwrap(), "Search");
        // Optimized schema has fewer details
        assert!(schema["function"]["parameters"]["properties"]["pattern"]
            .get("description")
            .is_none());
    }
}
