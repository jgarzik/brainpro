use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A validation error in the configuration
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.field, self.message)
    }
}

/// Permission mode for tool calls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
}

/// Hook event types for lifecycle callbacks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    SessionStart,
}

/// Configuration for a single hook
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookConfig {
    pub event: HookEvent,
    pub command: Vec<String>,
    #[serde(default)]
    pub matcher: Option<String>, // Regex pattern for tool name (PreToolUse/PostToolUse only)
    #[serde(default = "default_hook_timeout")]
    pub timeout_ms: u64,
}

fn default_hook_timeout() -> u64 {
    60_000 // 60 seconds
}

impl PermissionMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "default" => Some(Self::Default),
            "acceptedits" | "accept-edits" | "accept_edits" => Some(Self::AcceptEdits),
            "bypasspermissions" | "bypass-permissions" | "bypass_permissions" | "bypass" => {
                Some(Self::BypassPermissions)
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
        }
    }
}

/// Configuration for the permissions system
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub mode: PermissionMode,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

/// Configuration for the Bash tool
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct BashConfig {
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_output_bytes: Option<usize>,
}

/// Specification for a subagent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default = "default_allowed_tools")]
    pub allowed_tools: Vec<String>,
    #[serde(default = "default_permission_mode_str")]
    pub permission_mode: String,
    #[serde(default = "default_max_turns")]
    pub max_turns: usize,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

fn default_allowed_tools() -> Vec<String> {
    vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]
}

fn default_permission_mode_str() -> String {
    "default".to_string()
}

fn default_max_turns() -> usize {
    8
}

impl AgentSpec {
    /// Get the parsed permission mode
    pub fn get_permission_mode(&self) -> PermissionMode {
        PermissionMode::parse(&self.permission_mode).unwrap_or(PermissionMode::Default)
    }

    /// Load agent spec from a TOML file
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let spec: AgentSpec = toml::from_str(&content)?;
        Ok(spec)
    }
}

/// Load all agent specs from a directory
pub fn load_agents_from_dir(dir: &Path) -> HashMap<String, AgentSpec> {
    let mut agents = HashMap::new();

    if !dir.exists() {
        return agents;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match AgentSpec::load_from(&path) {
                    Ok(spec) => {
                        agents.insert(spec.name.clone(), spec);
                    }
                    Err(err) => {
                        eprintln!(
                            "Warning: failed to load agent spec from {}: {}",
                            path.display(),
                            err
                        );
                    }
                }
            }
        }
    }

    agents
}

/// Configuration for context management
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextConfig {
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    #[serde(default = "default_auto_compact_threshold")]
    pub auto_compact_threshold: f64,
    #[serde(default = "default_true")]
    pub auto_compact_enabled: bool,
    #[serde(default = "default_keep_last_turns")]
    pub keep_last_turns: usize,
}

fn default_max_chars() -> usize {
    250_000
}
fn default_auto_compact_threshold() -> f64 {
    0.95
}
fn default_true() -> bool {
    true
}
fn default_keep_last_turns() -> usize {
    10
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_chars: default_max_chars(),
            auto_compact_threshold: default_auto_compact_threshold(),
            auto_compact_enabled: default_true(),
            keep_last_turns: default_keep_last_turns(),
        }
    }
}

/// A parsed target: model@backend
#[derive(Debug, Clone)]
pub struct Target {
    pub model: String,
    pub backend: String,
}

impl Target {
    /// Parse a target string like "gpt-4@chatgpt" into model and backend
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.rsplitn(2, '@').collect();
        if parts.len() == 2 {
            Some(Target {
                model: parts[1].to_string(),
                backend: parts[0].to_string(),
            })
        } else {
            None
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.model, self.backend)
    }
}

/// Configuration for a single backend (API provider)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BackendConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl BackendConfig {
    /// Resolve the API key from config or environment
    /// Returns "ollama" as a dummy key for backends that don't require authentication
    pub fn resolve_api_key(&self) -> Result<String> {
        // Direct key takes priority
        if let Some(key) = &self.api_key {
            return Ok(key.clone());
        }

        // Try environment variable
        if let Some(env_var) = &self.api_key_env {
            if let Ok(key) = std::env::var(env_var) {
                return Ok(key);
            }
        }

        // For backends like Ollama that don't require auth, return a dummy key
        // (Ollama requires an API key header but ignores its value)
        Ok("ollama".to_string())
    }
}

use crate::cost::{CostConfig, ModelPricing};
use crate::model_routing::ModelRoutingConfig;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub default_target: Option<String>,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub bash: BashConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub model_routing: ModelRoutingConfig,
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
    #[serde(default)]
    pub cost_tracking: CostConfig,
    #[serde(default)]
    pub model_pricing: HashMap<String, ModelPricing>,
    #[serde(skip)]
    pub agents: HashMap<String, AgentSpec>,
}

impl Config {
    /// Create config with built-in default backends for all known providers
    pub fn with_builtin_backends() -> Self {
        let mut backends = HashMap::new();

        // Venice: try multiple env var names for flexibility
        backends.insert(
            "venice".to_string(),
            BackendConfig {
                base_url: "https://api.venice.ai/api/v1".to_string(),
                api_key_env: Some("VENICE_API_KEY".to_string()),
                api_key: std::env::var("venice_api_key").ok(), // fallback to lowercase
            },
        );

        backends.insert(
            "chatgpt".to_string(),
            BackendConfig {
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_env: Some("OPENAI_API_KEY".to_string()),
                api_key: None,
            },
        );

        backends.insert(
            "claude".to_string(),
            BackendConfig {
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_key: None,
            },
        );

        backends.insert(
            "ollama".to_string(),
            BackendConfig {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key_env: None,
                api_key: None,
            },
        );

        Config {
            backends,
            default_target: None,
            permissions: PermissionsConfig::default(),
            bash: BashConfig::default(),
            context: ContextConfig::default(),
            model_routing: ModelRoutingConfig::default(),
            hooks: Vec::new(),
            cost_tracking: CostConfig::default(),
            model_pricing: HashMap::new(),
            agents: HashMap::new(),
        }
    }

    /// Load configuration from default paths
    /// Priority: local (.brainpro/config.local.toml) > project (.brainpro/config.toml) > user (~/.brainpro/config.toml)
    /// Starts with built-in backends, then merges user/project/local configs
    /// Also loads agents from .brainpro/agents/ and ~/.brainpro/agents/
    pub fn load() -> Result<Self> {
        let mut config = Self::with_builtin_backends();

        // Try user-level config first
        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".brainpro").join("config.toml");
            if user_config.exists() {
                let user = Self::load_from(&user_config)?;
                config.merge(user);
            }

            // Load user-level agents (~/.brainpro/agents/)
            let user_agents_dir = home.join(".brainpro").join("agents");
            for (name, spec) in load_agents_from_dir(&user_agents_dir) {
                config.agents.insert(name, spec);
            }
        }

        // Try project-level config (overrides user-level)
        let project_config = Path::new(".brainpro").join("config.toml");
        if project_config.exists() {
            let project = Self::load_from(&project_config)?;
            config.merge(project);
        }

        // Load project-level agents (.brainpro/agents/) - overrides user-level
        let project_agents_dir = Path::new(".brainpro").join("agents");
        for (name, spec) in load_agents_from_dir(&project_agents_dir) {
            config.agents.insert(name, spec);
        }

        // Try local config (overrides project-level, should be gitignored)
        let local_config = Path::new(".brainpro").join("config.local.toml");
        if local_config.exists() {
            let local = Self::load_from(&local_config)?;
            config.merge(local);
        }

        Ok(config)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Merge another config into this one (other takes priority)
    /// For permissions: arrays are concatenated, mode is overridden if non-default
    /// For bash/context: scalars are overridden if set
    pub fn merge(&mut self, other: Config) {
        // Merge backends
        for (name, backend) in other.backends {
            self.backends.insert(name, backend);
        }

        // Override default_target if set in other
        if other.default_target.is_some() {
            self.default_target = other.default_target;
        }

        // Merge permissions: concatenate arrays, override mode if non-default
        self.permissions.allow.extend(other.permissions.allow);
        self.permissions.ask.extend(other.permissions.ask);
        self.permissions.deny.extend(other.permissions.deny);
        if other.permissions.mode != PermissionMode::Default {
            self.permissions.mode = other.permissions.mode;
        }

        // Merge bash config: override if set
        if other.bash.timeout_ms.is_some() {
            self.bash.timeout_ms = other.bash.timeout_ms;
        }
        if other.bash.max_output_bytes.is_some() {
            self.bash.max_output_bytes = other.bash.max_output_bytes;
        }

        // Merge context config: always override with other's values
        // (since there's no Option wrapper, we check if they differ from defaults)
        // For simplicity, we just take the other's values if the other config was loaded
        self.context = other.context;

        // Merge hooks (concatenate)
        self.hooks.extend(other.hooks);

        // Merge cost tracking (take other's values)
        self.cost_tracking = other.cost_tracking;

        // Merge model pricing (other takes priority)
        for (model, pricing) in other.model_pricing {
            self.model_pricing.insert(model, pricing);
        }
    }

    /// Get the default target
    pub fn get_default_target(&self) -> Option<Target> {
        self.default_target.as_ref().and_then(|s| Target::parse(s))
    }

    /// Create config from CLI arguments, starting with built-in backends
    /// The CLI-provided API key is applied to the backend matching the base_url
    pub fn from_cli_args(model: &str, base_url: &str, api_key: &str) -> Self {
        // Start with all built-in backends
        let mut config = Self::with_builtin_backends();

        // Determine which backend the CLI args are for
        let backend_name = if base_url.contains("venice") {
            "venice"
        } else if base_url.contains("openai") {
            "chatgpt"
        } else if base_url.contains("anthropic") {
            "claude"
        } else if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
            "ollama"
        } else {
            "venice" // Default fallback
        };

        // Override that backend with CLI-provided values
        config.backends.insert(
            backend_name.to_string(),
            BackendConfig {
                base_url: base_url.to_string(),
                api_key: Some(api_key.to_string()),
                api_key_env: None,
            },
        );

        // Set default target to use CLI-provided model
        config.default_target = Some(format!("{}@{}", model, backend_name));

        config
    }

    /// Check if config has any backends defined
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }

    /// Validate configuration and return any errors found
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate default_target format if set
        if let Some(target) = &self.default_target {
            if Target::parse(target).is_none() {
                errors.push(ValidationError {
                    field: "default_target".to_string(),
                    message: format!(
                        "Invalid target format '{}', expected 'model@backend'",
                        target
                    ),
                });
            }
        }

        // Validate context.auto_compact_threshold range
        if !(0.0..=1.0).contains(&self.context.auto_compact_threshold) {
            errors.push(ValidationError {
                field: "context.auto_compact_threshold".to_string(),
                message: format!(
                    "Must be between 0.0 and 1.0, got {}",
                    self.context.auto_compact_threshold
                ),
            });
        }

        // Validate agent specs
        for (name, spec) in &self.agents {
            if spec.max_turns == 0 {
                errors.push(ValidationError {
                    field: format!("agents.{}.max_turns", name),
                    message: "Must be greater than 0".to_string(),
                });
            }
            // Validate permission_mode is recognized
            if PermissionMode::parse(&spec.permission_mode).is_none() {
                errors.push(ValidationError {
                    field: format!("agents.{}.permission_mode", name),
                    message: format!("Invalid permission mode '{}'", spec.permission_mode),
                });
            }
        }

        // Validate hook matchers are valid regex
        for (i, hook) in self.hooks.iter().enumerate() {
            if let Some(matcher) = &hook.matcher {
                if regex::Regex::new(matcher).is_err() {
                    errors.push(ValidationError {
                        field: format!("hooks[{}].matcher", i),
                        message: format!("Invalid regex pattern '{}'", matcher),
                    });
                }
            }
            // Validate hook command is not empty
            if hook.command.is_empty() {
                errors.push(ValidationError {
                    field: format!("hooks[{}].command", i),
                    message: "Command must not be empty".to_string(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Save permissions to local config file (.brainpro/config.local.toml)
    /// Creates the .brainpro directory if it doesn't exist
    pub fn save_local_permissions(&self) -> Result<()> {
        let brainpro_dir = Path::new(".brainpro");
        if !brainpro_dir.exists() {
            std::fs::create_dir_all(brainpro_dir)?;
        }

        // Create a minimal config with just permissions
        let local_config = LocalPermissionsConfig {
            permissions: self.permissions.clone(),
        };

        let content = toml::to_string_pretty(&local_config)?;
        std::fs::write(brainpro_dir.join("config.local.toml"), content)?;
        Ok(())
    }
}

/// Minimal config for saving just permissions to local file
#[derive(Debug, Clone, Serialize)]
struct LocalPermissionsConfig {
    permissions: PermissionsConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target() {
        let target = Target::parse("gpt-4@chatgpt").unwrap();
        assert_eq!(target.model, "gpt-4");
        assert_eq!(target.backend, "chatgpt");

        let target = Target::parse("claude-3-sonnet@claude").unwrap();
        assert_eq!(target.model, "claude-3-sonnet");
        assert_eq!(target.backend, "claude");

        // Model with @ in the name
        let target = Target::parse("model@with@signs@backend").unwrap();
        assert_eq!(target.model, "model@with@signs");
        assert_eq!(target.backend, "backend");

        // No @ sign
        assert!(Target::parse("no-backend").is_none());
    }

    #[test]
    fn test_target_display() {
        let target = Target {
            model: "gpt-4".to_string(),
            backend: "chatgpt".to_string(),
        };
        assert_eq!(format!("{}", target), "gpt-4@chatgpt");
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config::with_builtin_backends();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_target() {
        let mut config = Config::with_builtin_backends();
        config.default_target = Some("no-backend".to_string());
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].field.contains("default_target"));
    }

    #[test]
    fn test_validate_invalid_threshold() {
        let mut config = Config::with_builtin_backends();
        config.context.auto_compact_threshold = 1.5;
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].field.contains("auto_compact_threshold"));
        assert!(errors[0].message.contains("between 0.0 and 1.0"));
    }

    #[test]
    fn test_validate_invalid_hook_regex() {
        let mut config = Config::with_builtin_backends();
        config.hooks.push(HookConfig {
            event: HookEvent::PreToolUse,
            command: vec!["echo".to_string(), "test".to_string()],
            matcher: Some("[invalid regex".to_string()),
            timeout_ms: 1000,
        });
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].field.contains("hooks"));
        assert!(errors[0].message.contains("Invalid regex"));
    }

    #[test]
    fn test_validate_empty_hook_command() {
        let mut config = Config::with_builtin_backends();
        config.hooks.push(HookConfig {
            event: HookEvent::Stop,
            command: vec![],
            matcher: None,
            timeout_ms: 1000,
        });
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("empty"));
    }
}
