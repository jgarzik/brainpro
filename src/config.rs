use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Permission mode for tool calls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
}

impl PermissionMode {
    pub fn from_str(s: &str) -> Option<Self> {
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

/// Configuration for an MCP server
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_cwd() -> String {
    ".".to_string()
}

fn default_timeout_ms() -> u64 {
    30_000
}

/// MCP configuration section
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

/// Specification for a subagent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub skill: Option<String>,
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
        PermissionMode::from_str(&self.permission_mode).unwrap_or(PermissionMode::Default)
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
                if let Ok(spec) = AgentSpec::load_from(&path) {
                    agents.insert(spec.name.clone(), spec);
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

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub skills: HashMap<String, String>,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub bash: BashConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub mcp: McpConfig,
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
            skills: HashMap::new(),
            permissions: PermissionsConfig::default(),
            bash: BashConfig::default(),
            context: ContextConfig::default(),
            mcp: McpConfig::default(),
            agents: HashMap::new(),
        }
    }

    /// Load configuration from default paths
    /// Priority: local (.yo/config.local.toml) > project (.yo/config.toml) > user (~/.yo/config.toml)
    /// Starts with built-in backends, then merges user/project/local configs
    /// Also loads agents from .yo/agents/ and ~/.yo/agents/
    pub fn load() -> Result<Self> {
        let mut config = Self::with_builtin_backends();

        // Try user-level config first
        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".yo").join("config.toml");
            if user_config.exists() {
                let user = Self::load_from(&user_config)?;
                config.merge(user);
            }

            // Load user-level agents (~/.yo/agents/)
            let user_agents_dir = home.join(".yo").join("agents");
            for (name, spec) in load_agents_from_dir(&user_agents_dir) {
                config.agents.insert(name, spec);
            }
        }

        // Try project-level config (overrides user-level)
        let project_config = Path::new(".yo").join("config.toml");
        if project_config.exists() {
            let project = Self::load_from(&project_config)?;
            config.merge(project);
        }

        // Load project-level agents (.yo/agents/) - overrides user-level
        let project_agents_dir = Path::new(".yo").join("agents");
        for (name, spec) in load_agents_from_dir(&project_agents_dir) {
            config.agents.insert(name, spec);
        }

        // Try local config (overrides project-level, should be gitignored)
        let local_config = Path::new(".yo").join("config.local.toml");
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
        // Merge backends and skills
        for (name, backend) in other.backends {
            self.backends.insert(name, backend);
        }
        for (skill, target) in other.skills {
            self.skills.insert(skill, target);
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

        // Merge MCP servers
        for (name, server) in other.mcp.servers {
            self.mcp.servers.insert(name, server);
        }
    }

    /// Resolve a skill to its target
    pub fn resolve_skill(&self, skill: &str) -> Option<Target> {
        self.skills.get(skill).and_then(|s| Target::parse(s))
    }

    /// Get the default target (from "default" skill)
    pub fn default_target(&self) -> Option<Target> {
        self.resolve_skill("default")
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

        // Set default skill to use CLI-provided model
        config
            .skills
            .insert("default".to_string(), format!("{}@{}", model, backend_name));

        config
    }

    /// Check if config has any backends defined
    pub fn has_backends(&self) -> bool {
        !self.backends.is_empty()
    }

    /// Save permissions to local config file (.yo/config.local.toml)
    /// Creates the .yo directory if it doesn't exist
    pub fn save_local_permissions(&self) -> Result<()> {
        let yo_dir = Path::new(".yo");
        if !yo_dir.exists() {
            std::fs::create_dir_all(yo_dir)?;
        }

        // Create a minimal config with just permissions
        let local_config = LocalPermissionsConfig {
            permissions: self.permissions.clone(),
        };

        let content = toml::to_string_pretty(&local_config)?;
        std::fs::write(yo_dir.join("config.local.toml"), content)?;
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
}
