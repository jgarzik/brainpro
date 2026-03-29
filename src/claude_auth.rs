//! Fallback loader for opencode's OAuth tokens.
//!
//! opencode stores Claude Max OAuth credentials at
//! `~/.local/share/opencode/auth.json`. This module is only consulted when
//! no API key is provided via environment variable or config file — see
//! [`BackendConfig::resolve_api_key`](crate::config::BackendConfig::resolve_api_key)
//! for the full lookup chain. You can supply an OAuth token directly through
//! `ANTHROPIC_API_KEY` or `api_key` in config and this file will be skipped.

use serde::Deserialize;
use std::path::PathBuf;

/// Top-level auth file structure from opencode.
#[derive(Debug, Deserialize)]
pub struct AuthFile {
    #[serde(default)]
    pub anthropic: Option<OAuthEntry>,
}

/// A single OAuth entry for a provider.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthEntry {
    /// Token type (e.g. "oauth")
    #[serde(default)]
    #[allow(dead_code)]
    pub r#type: Option<String>,
    /// Access token (e.g. "sk-ant-oat01-...")
    pub access: String,
    /// Refresh token
    #[serde(default)]
    #[allow(dead_code)]
    pub refresh: Option<String>,
    /// Expiry time in milliseconds since epoch
    #[serde(default)]
    pub expires: Option<u64>,
}

impl OAuthEntry {
    /// Check if the token is expired.
    /// Returns true if the token has an expiry time and it's in the past.
    /// Returns false if there is no expiry (assume valid).
    pub fn is_expired(&self) -> bool {
        match self.expires {
            Some(expires_ms) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                now_ms >= expires_ms
            }
            None => false,
        }
    }
}

/// Path to the opencode auth.json file.
fn auth_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".local/share/opencode/auth.json"))
}

/// Load the Anthropic OAuth token from opencode's auth.json.
/// Returns None if the file doesn't exist, can't be parsed, or has no anthropic entry.
pub fn load_opencode_token() -> Option<OAuthEntry> {
    let path = auth_file_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let auth_file: AuthFile = serde_json::from_str(&content).ok()?;
    auth_file.anthropic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_file() {
        let json = r#"{
            "anthropic": {
                "type": "oauth",
                "access": "sk-ant-oat01-test-token",
                "refresh": "sk-ant-ort01-refresh-token",
                "expires": 1770289891571
            }
        }"#;

        let auth_file: AuthFile = serde_json::from_str(json).unwrap();
        let entry = auth_file.anthropic.unwrap();
        assert_eq!(entry.access, "sk-ant-oat01-test-token");
        assert_eq!(entry.refresh.as_deref(), Some("sk-ant-ort01-refresh-token"));
        assert_eq!(entry.expires, Some(1770289891571));
        assert_eq!(entry.r#type.as_deref(), Some("oauth"));
    }

    #[test]
    fn test_parse_auth_file_minimal() {
        let json = r#"{
            "anthropic": {
                "access": "sk-ant-oat01-minimal"
            }
        }"#;

        let auth_file: AuthFile = serde_json::from_str(json).unwrap();
        let entry = auth_file.anthropic.unwrap();
        assert_eq!(entry.access, "sk-ant-oat01-minimal");
        assert!(entry.refresh.is_none());
        assert!(entry.expires.is_none());
        assert!(entry.r#type.is_none());
    }

    #[test]
    fn test_parse_auth_file_no_anthropic() {
        let json = r#"{}"#;
        let auth_file: AuthFile = serde_json::from_str(json).unwrap();
        assert!(auth_file.anthropic.is_none());
    }

    #[test]
    fn test_is_expired_future() {
        let entry = OAuthEntry {
            r#type: Some("oauth".to_string()),
            access: "token".to_string(),
            refresh: None,
            // Far in the future
            expires: Some(9999999999999),
        };
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_is_expired_past() {
        let entry = OAuthEntry {
            r#type: Some("oauth".to_string()),
            access: "token".to_string(),
            refresh: None,
            // In the past
            expires: Some(1000000000000),
        };
        assert!(entry.is_expired());
    }

    #[test]
    fn test_is_expired_none() {
        let entry = OAuthEntry {
            r#type: Some("oauth".to_string()),
            access: "token".to_string(),
            refresh: None,
            expires: None,
        };
        // No expiry means not expired
        assert!(!entry.is_expired());
    }
}
