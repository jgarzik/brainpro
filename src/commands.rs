//! Slash commands system for user-defined markdown commands.
//!
//! Commands are defined as markdown files in:
//! - .yo/commands/<name>.md (project-level)
//! - ~/.yo/commands/<name>.md (user-level)
//!
//! The command name is derived from the filename (without .md extension).
//! The file content becomes the prompt when the command is invoked.
//! Use $ARGUMENTS as a placeholder for user-provided arguments.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Metadata parsed from optional YAML frontmatter
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandMeta {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
}

/// A loaded slash command
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub source: CommandSource,
    pub meta: CommandMeta,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    Project,
    User,
}

impl Command {
    /// Expand the command with the given arguments
    pub fn expand(&self, arguments: &str) -> String {
        self.content.replace("$ARGUMENTS", arguments)
    }
}

/// Index of available slash commands
#[derive(Debug, Default)]
pub struct CommandIndex {
    commands: HashMap<String, Command>,
    errors: Vec<(PathBuf, String)>,
}

impl CommandIndex {
    /// Build the command index by scanning .yo/commands/ and ~/.yo/commands/
    pub fn build(root: &Path) -> Self {
        let mut index = Self::default();

        // Load user-level commands first (lower priority)
        if let Some(home) = dirs::home_dir() {
            let user_commands_dir = home.join(".yo").join("commands");
            index.load_from_dir(&user_commands_dir, CommandSource::User);
        }

        // Load project-level commands (higher priority, overrides user)
        let project_commands_dir = root.join(".yo").join("commands");
        index.load_from_dir(&project_commands_dir, CommandSource::Project);

        index
    }

    fn load_from_dir(&mut self, dir: &Path, source: CommandSource) {
        if !dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy().to_string();
                    match self.load_command(&path, &name, source) {
                        Ok(cmd) => {
                            self.commands.insert(name, cmd);
                        }
                        Err(e) => {
                            self.errors.push((path.clone(), e.to_string()));
                        }
                    }
                }
            }
        }
    }

    fn load_command(&self, path: &Path, name: &str, source: CommandSource) -> Result<Command> {
        let content = std::fs::read_to_string(path)?;

        // Parse optional YAML frontmatter
        let (meta, content) = parse_frontmatter(&content);

        Ok(Command {
            name: name.to_string(),
            source,
            meta,
            content,
        })
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    /// List all available commands
    pub fn list(&self) -> Vec<&Command> {
        let mut commands: Vec<_> = self.commands.values().collect();
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        commands
    }

    /// Get parse errors
    pub fn errors(&self) -> &[(PathBuf, String)] {
        &self.errors
    }
}

/// Parse optional YAML frontmatter from markdown content
fn parse_frontmatter(content: &str) -> (CommandMeta, String) {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return (CommandMeta::default(), content.to_string());
    }

    // Find the closing ---
    if let Some(end_pos) = trimmed[3..].find("\n---") {
        let yaml_content = &trimmed[3..3 + end_pos].trim();
        let rest = &trimmed[3 + end_pos + 4..].trim_start();

        match serde_yaml::from_str(yaml_content) {
            Ok(meta) => (meta, rest.to_string()),
            Err(_) => (CommandMeta::default(), content.to_string()),
        }
    } else {
        (CommandMeta::default(), content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "Just some content";
        let (meta, body) = parse_frontmatter(content);
        assert!(meta.description.is_none());
        assert_eq!(body, "Just some content");
    }

    #[test]
    fn test_parse_frontmatter_with_metadata() {
        let content = r#"---
description: A test command
allowed_tools:
  - Read
  - Grep
---

The actual command content"#;
        let (meta, body) = parse_frontmatter(content);
        assert_eq!(meta.description, Some("A test command".to_string()));
        assert_eq!(
            meta.allowed_tools,
            Some(vec!["Read".to_string(), "Grep".to_string()])
        );
        assert_eq!(body, "The actual command content");
    }

    #[test]
    fn test_command_expand() {
        let cmd = Command {
            name: "test".to_string(),
            source: CommandSource::Project,
            meta: CommandMeta::default(),
            content: "Fix issue #$ARGUMENTS in the codebase".to_string(),
        };

        let expanded = cmd.expand("123");
        assert_eq!(expanded, "Fix issue #123 in the codebase");
    }
}
