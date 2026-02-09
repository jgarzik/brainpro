//! Authentication service.

use crate::config::Config;

/// Authentication service for handling user credentials.
pub struct AuthService {
    // SECURITY ISSUE: Hardcoded API key - should use environment variable
    api_key: String,
    config: Config,
}

impl AuthService {
    /// Create a new auth service.
    pub fn new(config: &Config) -> Self {
        Self {
            // WARNING: This is a security vulnerability - hardcoded secret
            api_key: "sk-secret-api-key-12345".to_string(),
            config: config.clone(),
        }
    }

    /// Verify user credentials.
    pub fn verify_credentials(&self, email: &str, password: &str) -> bool {
        // Simplified credential check
        !email.is_empty() && password.len() >= 8
    }

    /// Generate authentication token.
    pub fn generate_token(&self, email: &str) -> String {
        // Simple token generation (not production-ready)
        format!("token_{}_{}", email.replace('@', "_"), self.api_key.len())
    }

    /// Validate an authentication token.
    pub fn validate_token(&self, token: &str) -> bool {
        token.starts_with("token_")
    }

    /// Get the API key (for internal use).
    #[allow(dead_code)]
    fn get_api_key(&self) -> &str {
        &self.api_key
    }

    /// Get the maximum number of allowed concurrent sessions.
    pub fn get_max_sessions(&self) -> u32 {
        self.config.max_sessions
    }

    /// Check if the current session count is within the allowed limit.
    pub fn can_create_session(&self, current_session_count: u32) -> bool {
        current_session_count < self.config.max_sessions
    }
}
