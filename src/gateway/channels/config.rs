//! Configuration for messaging channel integrations.

use serde::{Deserialize, Serialize};

/// Top-level channels configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChannelsConfig {
    /// Telegram channel configuration
    #[serde(default)]
    pub telegram: TelegramConfig,

    /// Discord channel configuration
    #[serde(default)]
    pub discord: DiscordConfig,

    /// Authorization settings
    #[serde(default)]
    pub auth: ChannelAuthConfig,
}

/// DM policy for channel authorization
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DmPolicy {
    /// Require pairing code authorization
    #[default]
    RequirePairing,
    /// Allow all DMs (not recommended)
    AllowAll,
    /// Deny all DMs
    DenyAll,
}

/// Telegram-specific configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TelegramConfig {
    /// Whether Telegram integration is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Bot token (or env var reference like ${TELEGRAM_BOT_TOKEN})
    #[serde(default)]
    pub bot_token: Option<String>,

    /// DM authorization policy
    #[serde(default)]
    pub dm_policy: DmPolicy,

    /// Allowed chat IDs (empty = allow all authorized)
    #[serde(default)]
    pub allowed_chats: Vec<i64>,
}

impl TelegramConfig {
    /// Resolve bot token from environment if needed
    pub fn resolve_bot_token(&self) -> Option<String> {
        self.bot_token.as_ref().and_then(|token| {
            if token.starts_with("${") && token.ends_with('}') {
                let env_var = &token[2..token.len() - 1];
                std::env::var(env_var).ok()
            } else {
                Some(token.clone())
            }
        })
    }
}

/// Discord-specific configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DiscordConfig {
    /// Whether Discord integration is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Bot token (or env var reference like ${DISCORD_BOT_TOKEN})
    #[serde(default)]
    pub bot_token: Option<String>,

    /// DM authorization policy
    #[serde(default)]
    pub dm_policy: DmPolicy,

    /// Allowed guild (server) IDs
    #[serde(default)]
    pub allowed_guilds: Vec<u64>,

    /// Allowed channel IDs within guilds
    #[serde(default)]
    pub allowed_channels: Vec<u64>,
}

impl DiscordConfig {
    /// Resolve bot token from environment if needed
    pub fn resolve_bot_token(&self) -> Option<String> {
        self.bot_token.as_ref().and_then(|token| {
            if token.starts_with("${") && token.ends_with('}') {
                let env_var = &token[2..token.len() - 1];
                std::env::var(env_var).ok()
            } else {
                Some(token.clone())
            }
        })
    }
}

/// Authorization persistence settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelAuthConfig {
    /// Path to authorization file (supports ~ expansion)
    #[serde(default = "default_auth_file")]
    pub auth_file: String,

    /// Pairing code expiry in seconds
    #[serde(default = "default_pairing_expiry")]
    pub pairing_expiry_secs: u64,
}

fn default_auth_file() -> String {
    "~/.brainpro/channel_auth.toml".to_string()
}

fn default_pairing_expiry() -> u64 {
    300 // 5 minutes
}

impl Default for ChannelAuthConfig {
    fn default() -> Self {
        Self {
            auth_file: default_auth_file(),
            pairing_expiry_secs: default_pairing_expiry(),
        }
    }
}

impl ChannelAuthConfig {
    /// Expand ~ in auth_file path
    pub fn resolve_auth_file(&self) -> std::path::PathBuf {
        if self.auth_file.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&self.auth_file[2..]);
            }
        }
        std::path::PathBuf::from(&self.auth_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_token_resolution() {
        let config = TelegramConfig {
            enabled: true,
            bot_token: Some("direct_token".to_string()),
            dm_policy: DmPolicy::RequirePairing,
            allowed_chats: vec![],
        };
        assert_eq!(config.resolve_bot_token(), Some("direct_token".to_string()));
    }

    #[test]
    fn test_auth_file_expansion() {
        let config = ChannelAuthConfig::default();
        let path = config.resolve_auth_file();
        assert!(path.to_string_lossy().contains("channel_auth.toml"));
    }
}
