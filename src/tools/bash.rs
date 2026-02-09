//! Bash tool for executing shell commands.
//!
//! Executes commands in the project root with timeout support and output capture.

use crate::config::BashConfig;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes
const MAX_TIMEOUT_MS: u64 = 600_000; // 10 minutes
const DEFAULT_MAX_OUTPUT_BYTES: usize = 200_000; // 200KB
const BLOCKED_ENV_PREFIXES: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_",
    "NODE_OPTIONS",
    "PYTHONPATH",
    "BASH_ENV",
    // Note: ENV is handled by exact match in sanitize_env(), not as prefix
    "IFS",
    // Note: PATH is NOT blocked - it's needed to find executables like cargo, git, etc.
];

#[derive(Debug, Deserialize)]
struct BashArgs {
    command: String,
    timeout_ms: Option<u64>,
    cwd: Option<String>,
}

use super::SchemaOptions;

/// Returns the JSON schema for the Bash tool
pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "Bash",
                "description": "Run shell command",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "timeout_ms": { "type": "integer" },
                        "cwd": { "type": "string" }
                    },
                    "required": ["command"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "Bash",
                "description": "Execute a shell command in the project directory. Commands are parsed as shell words (not passed to sh -c). Returns stdout, stderr, and exit code. Use for builds, tests, git operations, etc.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command to execute (parsed as shell words)"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Timeout in milliseconds (default 120000, max 600000)"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory relative to project root (default: project root)"
                        }
                    },
                    "required": ["command"]
                }
            }
        })
    }
}

/// Execute the Bash tool
pub fn execute(args: Value, root: &Path, config: &BashConfig) -> Result<Value> {
    let bash_args: BashArgs = serde_json::from_value(args.clone())
        .map_err(|e| anyhow::anyhow!("Invalid Bash args: {}", e))?;

    // Parse command into argv using shell-words (NOT sh -c)
    let argv = match shell_words::split(&bash_args.command) {
        Ok(v) if v.is_empty() => {
            return Ok(json!({
                "error": { "code": "empty_command", "message": "Command is empty" }
            }));
        }
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "error": { "code": "parse_error", "message": format!("Failed to parse command: {}", e) }
            }));
        }
    };

    // Resolve working directory
    let work_dir = match &bash_args.cwd {
        Some(cwd) => {
            let cwd_path = root.join(cwd);
            match validate_cwd(&cwd_path, root) {
                Ok(p) => p,
                Err(e) => return Ok(e),
            }
        }
        None => root.to_path_buf(),
    };

    // Compute timeout
    let config_timeout = config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    let timeout_ms = bash_args
        .timeout_ms
        .unwrap_or(config_timeout)
        .min(MAX_TIMEOUT_MS);
    let timeout = Duration::from_millis(timeout_ms);

    let max_output = config.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);

    let start = Instant::now();

    // Build command
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    sanitize_env(&mut cmd);

    // Spawn the process
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(json!({
                "error": { "code": "spawn_error", "message": e.to_string() },
                "cwd": work_dir.to_string_lossy(),
                "duration_ms": start.elapsed().as_millis() as u64
            }));
        }
    };

    // Wait with timeout
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            // Timeout - kill the process
            let _ = child.kill();
            let _ = child.wait();
            return Ok(json!({
                "error": { "code": "timeout", "message": format!("Command timed out after {}ms", timeout_ms) },
                "cwd": work_dir.to_string_lossy(),
                "duration_ms": start.elapsed().as_millis() as u64
            }));
        }
        Err(e) => {
            return Ok(json!({
                "error": { "code": "wait_error", "message": e.to_string() },
                "cwd": work_dir.to_string_lossy(),
                "duration_ms": start.elapsed().as_millis() as u64
            }));
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Read and truncate output
    let (stdout, stdout_truncated) = read_output(child.stdout.take(), max_output);
    let (stderr, stderr_truncated) = read_output(child.stderr.take(), max_output);

    Ok(json!({
        "exit_code": status.code(),
        "stdout": stdout,
        "stderr": stderr,
        "truncated": stdout_truncated || stderr_truncated,
        "duration_ms": duration_ms,
        "cwd": work_dir.to_string_lossy()
    }))
}

/// Read output from a reader, truncating to max_bytes
fn read_output<R: Read>(reader: Option<R>, max_bytes: usize) -> (String, bool) {
    let Some(mut r) = reader else {
        return (String::new(), false);
    };

    let mut buf = vec![0u8; max_bytes + 1];
    let bytes_read = match r.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return (String::new(), false),
    };

    let truncated = bytes_read > max_bytes;
    let actual_bytes = bytes_read.min(max_bytes);

    let content = String::from_utf8_lossy(&buf[..actual_bytes]).to_string();
    (content, truncated)
}

/// Validate that cwd stays within project root
fn validate_cwd(cwd: &Path, root: &Path) -> Result<std::path::PathBuf, Value> {
    // Check if it exists
    if !cwd.exists() {
        return Err(json!({
            "error": { "code": "invalid_cwd", "message": "Working directory does not exist" }
        }));
    }

    // Canonicalize and verify stays within root
    let canonical = match cwd.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(json!({
                "error": { "code": "invalid_cwd", "message": format!("Cannot resolve working directory: {}", e) }
            }));
        }
    };

    let canonical_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => root.to_path_buf(),
    };

    if !canonical.starts_with(&canonical_root) {
        return Err(json!({
            "error": { "code": "cwd_out_of_scope", "message": "Working directory escapes project root" }
        }));
    }

    Ok(canonical)
}

fn sanitize_env(cmd: &mut Command) {
    cmd.env_remove("LD_PRELOAD");
    cmd.env_remove("LD_LIBRARY_PATH");
    cmd.env_remove("NODE_OPTIONS");
    cmd.env_remove("PYTHONPATH");
    cmd.env_remove("BASH_ENV");
    cmd.env_remove("ENV");
    cmd.env_remove("IFS");

    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    } else {
        cmd.env_remove("PATH");
    }

    for (key, _) in std::env::vars() {
        if BLOCKED_ENV_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            cmd.env_remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn default_config() -> BashConfig {
        BashConfig::default()
    }

    #[test]
    fn test_schema() {
        let opts = SchemaOptions::default();
        let schema = schema(&opts);
        assert_eq!(schema["function"]["name"], "Bash");
    }

    #[test]
    fn test_schema_optimized() {
        let opts = SchemaOptions::new(true);
        let schema = schema(&opts);
        assert_eq!(schema["function"]["name"], "Bash");
        // Optimized schema should have shorter description
        let desc = schema["function"]["description"].as_str().unwrap();
        assert_eq!(desc, "Run shell command");
    }

    #[test]
    fn test_execute_simple_command() {
        let root = env::current_dir().unwrap();
        let result = execute(json!({"command": "echo hello"}), &root, &default_config()).unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[test]
    fn test_execute_command_with_args() {
        let root = env::current_dir().unwrap();
        let result = execute(
            json!({"command": "echo 'hello world'"}),
            &root,
            &default_config(),
        )
        .unwrap();

        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("hello world"));
    }

    #[test]
    fn test_execute_nonexistent_command() {
        let root = env::current_dir().unwrap();
        let result = execute(
            json!({"command": "nonexistent_command_12345"}),
            &root,
            &default_config(),
        )
        .unwrap();

        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_execute_empty_command() {
        let root = env::current_dir().unwrap();
        let result = execute(json!({"command": ""}), &root, &default_config()).unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"]["code"], "empty_command");
    }

    #[test]
    fn test_execute_captures_stderr() {
        let root = env::current_dir().unwrap();
        // Use a command that writes to stderr
        let result = execute(
            json!({"command": "ls /nonexistent_path_12345"}),
            &root,
            &default_config(),
        )
        .unwrap();

        // Should have non-zero exit code
        assert_ne!(result["exit_code"], 0);
        // Should have stderr output
        assert!(!result["stderr"].as_str().unwrap().is_empty());
    }

    #[test]
    fn test_cwd_validation() {
        let root = env::current_dir().unwrap();
        let result = execute(
            json!({"command": "pwd", "cwd": ".."}),
            &root,
            &default_config(),
        )
        .unwrap();

        // Should be denied as it escapes root
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_env_filtering_removes_preload() {
        let mut cmd = Command::new("true");
        cmd.env("LD_PRELOAD", "bad.so");
        sanitize_env(&mut cmd);
        let mut removed = false;
        for (key, value) in cmd.get_envs() {
            if key.to_str() == Some("LD_PRELOAD") {
                removed = value.is_none();
                break;
            }
        }
        assert!(removed);
    }

    #[test]
    fn test_env_filtering_removes_node_options_prefix() {
        let mut cmd = Command::new("true");
        cmd.env("NODE_OPTIONS", "--require evil");
        sanitize_env(&mut cmd);
        let mut removed = false;
        for (key, value) in cmd.get_envs() {
            if key.to_str() == Some("NODE_OPTIONS") {
                removed = value.is_none();
                break;
            }
        }
        assert!(removed);
    }
}
