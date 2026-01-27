//! Persona prompt loader.
//!
//! Loads persona configurations and prompt sections from files.
//! Supports YAML frontmatter + Markdown body format.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::PermissionMode;
use crate::persona::PromptContext;

/// Parsed YAML frontmatter from a prompt section file
#[derive(Debug, Deserialize)]
struct SectionFrontmatter {
    name: String,
    #[serde(default)]
    order: i32,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    condition: Option<String>,
}

/// A loaded prompt section
#[derive(Debug, Clone)]
pub struct PromptSection {
    pub name: String,
    pub order: i32,
    pub required: bool,
    pub condition: Option<String>,
    pub content: String,
}

/// Parsed manifest file
#[derive(Debug, Deserialize)]
struct ManifestFrontmatter {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    description: String,
    default_tools: Vec<String>,
    #[serde(default = "default_permission_mode")]
    permission_mode: String,
}

fn default_permission_mode() -> String {
    "default".to_string()
}

/// Configuration loaded from a persona's manifest
#[derive(Debug, Clone)]
pub struct PersonaConfig {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub default_tools: Vec<String>,
    pub permission_mode: PermissionMode,
    pub sections: Vec<PromptSection>,
}

impl PersonaConfig {
    /// Get tools as static string slices (for backward compatibility)
    pub fn tools_as_static(&self) -> Vec<&'static str> {
        self.default_tools
            .iter()
            .filter_map(|s| match s.as_str() {
                "Read" => Some("Read"),
                "Write" => Some("Write"),
                "Edit" => Some("Edit"),
                "Glob" => Some("Glob"),
                "Grep" => Some("Grep"),
                "Bash" => Some("Bash"),
                "Search" => Some("Search"),
                "Task" => Some("Task"),
                "TodoWrite" => Some("TodoWrite"),
                "AskUserQuestion" => Some("AskUserQuestion"),
                "ActivateSkill" => Some("ActivateSkill"),
                "EnterPlanMode" => Some("EnterPlanMode"),
                "ExitPlanMode" => Some("ExitPlanMode"),
                _ => None,
            })
            .collect()
    }
}

/// Parse YAML frontmatter and markdown body from a file
fn parse_frontmatter<T: for<'de> Deserialize<'de>>(content: &str) -> Result<(T, String)> {
    let content = content.trim();
    if !content.starts_with("---") {
        return Err(anyhow!("Missing YAML frontmatter delimiter"));
    }

    let after_first = &content[3..];
    let end_pos = after_first
        .find("\n---")
        .ok_or_else(|| anyhow!("Missing closing YAML frontmatter delimiter"))?;

    let yaml_str = &after_first[..end_pos];
    let body_start = end_pos + 4; // Skip "\n---"
    let body = after_first[body_start..].trim().to_string();

    let frontmatter: T = serde_yaml::from_str(yaml_str)
        .map_err(|e| anyhow!("Failed to parse YAML frontmatter: {}", e))?;

    Ok((frontmatter, body))
}

/// Load a single prompt section from a file
pub fn load_prompt_section(path: &Path) -> Result<PromptSection> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read section file {:?}: {}", path, e))?;

    let (frontmatter, body): (SectionFrontmatter, String) = parse_frontmatter(&content)?;

    Ok(PromptSection {
        name: frontmatter.name,
        order: frontmatter.order,
        required: frontmatter.required,
        condition: frontmatter.condition,
        content: body,
    })
}

/// Get the base path for persona configs
fn config_base_path() -> PathBuf {
    // Check for BRAINPRO_CONFIG_DIR env var first
    if let Ok(config_dir) = std::env::var("BRAINPRO_CONFIG_DIR") {
        return PathBuf::from(config_dir).join("persona");
    }

    // Try to find config/ relative to the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // Check if we're in target/debug or target/release
            let config_path = if exe_dir.ends_with("debug") || exe_dir.ends_with("release") {
                exe_dir.join("../../../config/persona")
            } else {
                exe_dir.join("config/persona")
            };
            if config_path.exists() {
                return config_path;
            }
        }
    }

    // Fall back to current working directory
    PathBuf::from("config/persona")
}

/// Load a persona configuration from files
pub fn load_persona(name: &str) -> Result<PersonaConfig> {
    let base_path = config_base_path();
    let persona_dir = base_path.join(name);

    if !persona_dir.exists() {
        return Err(anyhow!(
            "Persona directory not found: {:?}",
            persona_dir
        ));
    }

    // Load manifest
    let manifest_path = persona_dir.join("manifest.md");
    let manifest_content = fs::read_to_string(&manifest_path)
        .map_err(|e| anyhow!("Failed to read manifest {:?}: {}", manifest_path, e))?;

    let (manifest, _body): (ManifestFrontmatter, String) = parse_frontmatter(&manifest_content)?;

    // Parse permission mode
    let permission_mode = match manifest.permission_mode.as_str() {
        "accept_edits" => PermissionMode::AcceptEdits,
        "bypass" | "bypass_permissions" => PermissionMode::BypassPermissions,
        _ => PermissionMode::Default,
    };

    // Load all .md files except manifest.md
    let mut sections = Vec::new();
    for entry in fs::read_dir(&persona_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "md").unwrap_or(false)
            && path
                .file_name()
                .map(|n| n != "manifest.md")
                .unwrap_or(false)
        {
            match load_prompt_section(&path) {
                Ok(section) => sections.push(section),
                Err(e) => {
                    return Err(anyhow!("Failed to load section {:?}: {}", path, e));
                }
            }
        }
    }

    // Sort by order
    sections.sort_by_key(|s| s.order);

    // Use display_name if provided, otherwise capitalize the name
    let display_name = manifest.display_name.unwrap_or_else(|| {
        let mut chars = manifest.name.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    });

    Ok(PersonaConfig {
        name: manifest.name,
        display_name,
        description: manifest.description,
        default_tools: manifest.default_tools,
        permission_mode,
        sections,
    })
}

/// Render template variables in a string
pub fn render_template(template: &str, ctx: &PromptContext, persona_name: &str) -> String {
    let mut result = template.to_string();

    // Replace template variables
    result = result.replace("{{persona_name}}", persona_name);
    result = result.replace("{{working_dir}}", &ctx.working_dir.display().to_string());
    result = result.replace("{{active_skills}}", &ctx.active_skills.join(", "));

    result
}

/// Check if a section should be included based on its condition
fn should_include_section(section: &PromptSection, ctx: &PromptContext) -> bool {
    match section.condition.as_deref() {
        None => true,
        Some("plan_mode") => ctx.plan_mode,
        Some("optimize_mode") => ctx.optimize_mode,
        Some(_) => false, // Unknown conditions are excluded
    }
}

/// Build the complete system prompt from loaded config
pub fn build_system_prompt(config: &PersonaConfig, ctx: &PromptContext) -> String {
    let mut prompt_parts = Vec::new();

    for section in &config.sections {
        if should_include_section(section, ctx) {
            let rendered = render_template(&section.content, ctx, &config.display_name);
            prompt_parts.push(rendered);
        }
    }

    // Add dynamic workspace section if working_dir is set
    if !ctx.working_dir.as_os_str().is_empty() {
        prompt_parts.push(format!(
            "## Workspace\nWorking directory: {}",
            ctx.working_dir.display()
        ));
    }

    // Add active skills section if any
    if !ctx.active_skills.is_empty() {
        prompt_parts.push(format!(
            "## Active Skills\n{}",
            ctx.active_skills.join(", ")
        ));
    }

    prompt_parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test
order: 1
required: true
---

This is the body content."#;

        let (fm, body): (SectionFrontmatter, String) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test");
        assert_eq!(fm.order, 1);
        assert!(fm.required);
        assert_eq!(body, "This is the body content.");
    }

    #[test]
    fn test_render_template() {
        let ctx = PromptContext {
            working_dir: PathBuf::from("/home/user/project"),
            active_skills: vec!["rust".to_string(), "testing".to_string()],
            ..Default::default()
        };

        let template = "You are {{persona_name}}. Skills: {{active_skills}}";
        let result = render_template(template, &ctx, "MrCode");

        assert!(result.contains("MrCode"));
        assert!(result.contains("rust, testing"));
    }

    #[test]
    fn test_should_include_section() {
        let section = PromptSection {
            name: "test".to_string(),
            order: 1,
            required: false,
            condition: Some("plan_mode".to_string()),
            content: "test".to_string(),
        };

        let ctx_no_plan = PromptContext::default();
        assert!(!should_include_section(&section, &ctx_no_plan));

        let ctx_plan = PromptContext {
            plan_mode: true,
            ..Default::default()
        };
        assert!(should_include_section(&section, &ctx_plan));
    }

    #[test]
    fn test_load_mrcode_persona() {
        let config = load_persona("mrcode").expect("Failed to load mrcode");
        assert_eq!(config.name, "mrcode");
        assert!(!config.default_tools.is_empty());
        assert!(config.default_tools.contains(&"Read".to_string()));
        assert!(config.default_tools.contains(&"Bash".to_string()));
        assert!(!config.sections.is_empty());

        // Verify prompt renders with template variable
        let ctx = PromptContext::default();
        let prompt = build_system_prompt(&config, &ctx);
        assert!(
            prompt.contains("MrCode"),
            "Prompt should contain rendered persona name"
        );
        assert!(
            prompt.contains("coding assistant"),
            "Prompt should contain identity content"
        );
    }

    #[test]
    fn test_load_mrbot_persona() {
        let config = load_persona("mrbot").expect("Failed to load mrbot");
        assert_eq!(config.name, "mrbot");
        assert!(!config.default_tools.is_empty());
        assert!(config.default_tools.contains(&"Task".to_string()));
        assert!(!config.sections.is_empty());

        // Verify prompt renders with soul content
        let ctx = PromptContext::default();
        let prompt = build_system_prompt(&config, &ctx);
        assert!(
            prompt.contains("MrBot"),
            "Prompt should contain rendered persona name"
        );
        assert!(
            prompt.contains("Personality & Values"),
            "Prompt should contain soul section"
        );
        assert!(
            prompt.contains("Core Truths"),
            "Prompt should contain soul content"
        );
    }

    #[test]
    fn test_mrcode_plan_mode_prompt() {
        let config = load_persona("mrcode").expect("Failed to load mrcode");
        let ctx = PromptContext {
            plan_mode: true,
            ..Default::default()
        };
        let prompt = build_system_prompt(&config, &ctx);
        assert!(
            prompt.contains("Plan Mode"),
            "Plan mode prompt should be included"
        );
        assert!(
            prompt.contains("read-only tools"),
            "Plan mode should mention read-only"
        );
    }

    #[test]
    fn test_mrcode_optimize_mode_prompt() {
        let config = load_persona("mrcode").expect("Failed to load mrcode");
        let ctx = PromptContext {
            optimize_mode: true,
            ..Default::default()
        };
        let prompt = build_system_prompt(&config, &ctx);
        assert!(
            prompt.contains("AI-to-AI mode"),
            "Optimize mode should be included"
        );
    }
}
