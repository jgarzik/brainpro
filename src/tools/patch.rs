use super::{sha256, validate_path, SchemaOptions};
use diffy::{apply, Patch};
use serde_json::{json, Value};
use std::path::Path;

pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Patch",
                "description": "Apply unified diff patch",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "patch": { "type": "string" },
                        "path": { "type": "string" },
                        "dry_run": { "type": "boolean" }
                    },
                    "required": ["patch"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "Patch",
                "description": "Apply unified diff patch to file(s). Accepts standard unified diff format (from git diff or manual). Strips a/ and b/ prefixes automatically. Use dry_run:true to validate without applying. Preferred for large multi-hunk changes; use Edit for small targeted changes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "patch": { "type": "string", "description": "Unified diff content to apply" },
                        "path": { "type": "string", "description": "Target file (for single-file patches without headers)" },
                        "dry_run": { "type": "boolean", "description": "Validate without applying (default: false)" }
                    },
                    "required": ["patch"]
                }
            }
        })
    }
}

pub fn execute(args: Value, root: &Path) -> anyhow::Result<Value> {
    let patch_content = args["patch"].as_str().unwrap_or("");
    let explicit_path = args["path"].as_str();
    let dry_run = args["dry_run"].as_bool().unwrap_or(false);

    if patch_content.is_empty() {
        return Ok(
            json!({ "error": { "code": "invalid_patch", "message": "Patch content is empty" } }),
        );
    }

    // Parse the patch
    let patch = match Patch::from_str(patch_content) {
        Ok(p) => p,
        Err(e) => {
            return Ok(json!({ "error": { "code": "invalid_patch", "message": e.to_string() } }))
        }
    };

    // Determine target path
    let target_path = if let Some(p) = explicit_path {
        p.to_string()
    } else {
        // Extract from patch headers
        let original = patch.original().unwrap_or("");
        let modified = patch.modified().unwrap_or("");

        // Prefer modified path, fall back to original
        let header_path = if !modified.is_empty() && modified != "/dev/null" {
            modified
        } else if !original.is_empty() && original != "/dev/null" {
            original
        } else {
            return Ok(
                json!({ "error": { "code": "invalid_patch", "message": "No target path in patch headers and no path provided" } }),
            );
        };

        // Strip a/ or b/ prefix (git diff format)
        strip_git_prefix(header_path).to_string()
    };

    // Validate path
    let full_path = match validate_path(&target_path, root) {
        Ok(p) => p,
        Err(e) => return Ok(e),
    };

    // Check if this is a new file creation
    let is_new_file = patch.original().map(|o| o == "/dev/null").unwrap_or(false);

    // Read original content (or empty for new files)
    let original = if is_new_file {
        String::new()
    } else {
        match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                return Ok(json!({ "error": { "code": "read_error", "message": e.to_string() } }))
            }
        }
    };

    let before_sha = sha256(original.as_bytes());

    // Apply the patch
    let new_content = match apply(&original, &patch) {
        Ok(s) => s,
        Err(e) => {
            return Ok(json!({ "error": { "code": "hunk_failed", "message": e.to_string() } }))
        }
    };

    let after_sha = sha256(new_content.as_bytes());
    let hunks_applied = patch.hunks().len();

    // Write the file unless dry_run
    if !dry_run {
        // Create parent directories if needed (for new files)
        if is_new_file {
            if let Some(parent) = full_path.parent() {
                if !parent.exists() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return Ok(
                            json!({ "error": { "code": "write_error", "message": e.to_string() } }),
                        );
                    }
                }
            }
        }

        if let Err(e) = std::fs::write(&full_path, &new_content) {
            return Ok(json!({ "error": { "code": "write_error", "message": e.to_string() } }));
        }
    }

    Ok(json!({
        "success": true,
        "dry_run": dry_run,
        "files_modified": if dry_run { 0 } else { 1 },
        "files": [{
            "path": target_path,
            "status": "success",
            "before_sha256": before_sha,
            "after_sha256": after_sha,
            "hunks_applied": hunks_applied
        }]
    }))
}

/// Strip git diff prefix (a/ or b/) from path
fn strip_git_prefix(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
        dir
    }

    #[test]
    fn test_schema() {
        let opts = SchemaOptions { optimize: false };
        let schema = schema(&opts);
        assert_eq!(schema["function"]["name"].as_str().unwrap(), "Patch");
        assert!(schema["function"]["parameters"]["properties"]
            .get("patch")
            .is_some());
    }

    #[test]
    fn test_schema_optimized() {
        let opts = SchemaOptions { optimize: true };
        let schema = schema(&opts);
        assert_eq!(schema["function"]["name"].as_str().unwrap(), "Patch");
    }

    #[test]
    fn test_simple_patch() {
        let dir = setup_test_dir();
        let patch = r#"--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+new line
 line 2
 line 3
"#;
        let args = json!({ "patch": patch });
        let result = execute(args, dir.path()).unwrap();

        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["files_modified"].as_i64().unwrap(), 1);
        assert_eq!(result["files"][0]["hunks_applied"].as_i64().unwrap(), 1);

        let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("new line"));
    }

    #[test]
    fn test_patch_with_explicit_path() {
        let dir = setup_test_dir();
        // Patch without headers
        let patch = r#"@@ -1,3 +1,4 @@
 line 1
+inserted
 line 2
 line 3
"#;
        let args = json!({ "patch": patch, "path": "test.txt" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result["success"].as_bool().unwrap());

        let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("inserted"));
    }

    #[test]
    fn test_dry_run() {
        let dir = setup_test_dir();
        let original = fs::read_to_string(dir.path().join("test.txt")).unwrap();

        let patch = r#"--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+dry run line
 line 2
 line 3
"#;
        let args = json!({ "patch": patch, "dry_run": true });
        let result = execute(args, dir.path()).unwrap();

        assert!(result["success"].as_bool().unwrap());
        assert!(result["dry_run"].as_bool().unwrap());
        assert_eq!(result["files_modified"].as_i64().unwrap(), 0);

        // File should be unchanged
        let after = fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert_eq!(original, after);
    }

    #[test]
    fn test_invalid_patch() {
        let dir = setup_test_dir();
        let args = json!({ "patch": "not a valid patch" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "invalid_patch");
    }

    #[test]
    fn test_empty_patch() {
        let dir = setup_test_dir();
        let args = json!({ "patch": "" });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "invalid_patch");
    }

    #[test]
    fn test_context_mismatch() {
        let dir = setup_test_dir();
        // Patch with wrong context
        let patch = r#"--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 wrong context
+new line
 also wrong
 still wrong
"#;
        let args = json!({ "patch": patch });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "hunk_failed");
    }

    #[test]
    fn test_strip_git_prefix() {
        assert_eq!(strip_git_prefix("a/src/main.rs"), "src/main.rs");
        assert_eq!(strip_git_prefix("b/src/main.rs"), "src/main.rs");
        assert_eq!(strip_git_prefix("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn test_file_not_found() {
        let dir = setup_test_dir();
        let patch = r#"--- a/nonexistent.txt
+++ b/nonexistent.txt
@@ -1,3 +1,4 @@
 line 1
+new line
 line 2
 line 3
"#;
        let args = json!({ "patch": patch });
        let result = execute(args, dir.path()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"].as_str().unwrap(), "read_error");
    }
}
