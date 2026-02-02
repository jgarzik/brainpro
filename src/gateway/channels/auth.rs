//! Authorization system for channel access via pairing codes.

use super::config::ChannelAuthConfig;
use super::plugin::ChannelTarget;
use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Authorization status for a channel target
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    /// Fully authorized
    Authorized,
    /// Pending pairing with the given code
    PendingPairing(String),
    /// Authorization denied
    Denied,
}

/// Pending pairing request
#[derive(Debug, Clone)]
struct PendingPairing {
    /// 6-character pairing code
    code: String,
    /// Target requesting pairing
    target: ChannelTarget,
    /// Expiry timestamp
    expires_at: Instant,
}

/// Persisted authorization record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRecord {
    /// Channel type (telegram/discord)
    pub channel: String,
    /// Chat/channel ID
    pub chat_id: String,
    /// User ID (if applicable)
    pub user_id: Option<String>,
    /// Username/display name
    pub username: Option<String>,
    /// When authorized
    pub authorized_at: String,
}

/// Persisted authorization file
#[derive(Debug, Default, Serialize, Deserialize)]
struct AuthFile {
    authorizations: Vec<AuthRecord>,
}

/// Manager for channel authorization
pub struct ChannelAuthManager {
    /// Configuration
    config: ChannelAuthConfig,
    /// Pending pairing codes: code -> PendingPairing
    pending: DashMap<String, PendingPairing>,
    /// Authorized targets: session_key -> AuthRecord
    authorized: DashMap<String, AuthRecord>,
    /// Path to auth file
    auth_file_path: PathBuf,
}

impl ChannelAuthManager {
    /// Create a new auth manager
    pub fn new(config: ChannelAuthConfig) -> Arc<Self> {
        let auth_file_path = config.resolve_auth_file();

        let manager = Arc::new(Self {
            config,
            pending: DashMap::new(),
            authorized: DashMap::new(),
            auth_file_path,
        });

        // Load persisted authorizations
        if let Err(e) = manager.load_authorizations() {
            eprintln!("[channels] Failed to load authorizations: {}", e);
        }

        manager
    }

    /// Check if a target is authorized
    pub fn check_auth(&self, target: &ChannelTarget) -> AuthStatus {
        let key = target.session_key();

        // Check if already authorized
        if self.authorized.contains_key(&key) {
            return AuthStatus::Authorized;
        }

        // Check if there's a pending pairing
        for entry in self.pending.iter() {
            if entry.target.session_key() == key && entry.expires_at > Instant::now() {
                return AuthStatus::PendingPairing(entry.code.clone());
            }
        }

        AuthStatus::Denied
    }

    /// Create a pairing request for a target
    /// Returns the pairing code
    pub fn request_pairing(&self, target: &ChannelTarget) -> String {
        // Clean up expired pairings first
        self.cleanup_expired();

        // Check if already has pending
        let key = target.session_key();
        for entry in self.pending.iter() {
            if entry.target.session_key() == key && entry.expires_at > Instant::now() {
                return entry.code.clone();
            }
        }

        // Generate new 6-character alphanumeric code
        let code = generate_pairing_code();
        let expiry = Duration::from_secs(self.config.pairing_expiry_secs);

        let pairing = PendingPairing {
            code: code.clone(),
            target: target.clone(),
            expires_at: Instant::now() + expiry,
        };

        self.pending.insert(code.clone(), pairing);
        code
    }

    /// Approve a pairing request by code
    /// Returns the target that was paired
    pub fn approve_pairing(&self, code: &str) -> Result<ChannelTarget> {
        // Find and remove the pending pairing
        let (_, pairing) = self
            .pending
            .remove(code)
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired pairing code"))?;

        // Check if expired
        if pairing.expires_at <= Instant::now() {
            anyhow::bail!("Pairing code has expired");
        }

        // Add to authorized
        let record = AuthRecord {
            channel: pairing.target.channel.clone(),
            chat_id: pairing.target.chat_id.clone(),
            user_id: pairing.target.user_id.clone(),
            username: pairing.target.username.clone(),
            authorized_at: chrono::Utc::now().to_rfc3339(),
        };

        let key = pairing.target.session_key();
        self.authorized.insert(key, record);

        // Persist
        if let Err(e) = self.save_authorizations() {
            eprintln!("[channels] Failed to save authorizations: {}", e);
        }

        Ok(pairing.target)
    }

    /// Revoke authorization for a target
    pub fn revoke(&self, target: &ChannelTarget) -> bool {
        let key = target.session_key();
        let removed = self.authorized.remove(&key).is_some();

        if removed {
            if let Err(e) = self.save_authorizations() {
                eprintln!("[channels] Failed to save authorizations: {}", e);
            }
        }

        removed
    }

    /// Revoke authorization by ID (channel:chat_id)
    pub fn revoke_by_id(&self, id: &str) -> bool {
        let removed = self.authorized.remove(id).is_some();

        if removed {
            if let Err(e) = self.save_authorizations() {
                eprintln!("[channels] Failed to save authorizations: {}", e);
            }
        }

        removed
    }

    /// List all authorizations
    pub fn list_authorizations(&self) -> Vec<AuthRecord> {
        self.authorized.iter().map(|r| r.value().clone()).collect()
    }

    /// Get pending pairing count
    pub fn pending_count(&self) -> usize {
        self.cleanup_expired();
        self.pending.len()
    }

    /// Clean up expired pairings
    fn cleanup_expired(&self) {
        let now = Instant::now();
        self.pending.retain(|_, p| p.expires_at > now);
    }

    /// Load authorizations from file
    fn load_authorizations(&self) -> Result<()> {
        if !self.auth_file_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.auth_file_path)?;
        let auth_file: AuthFile = toml::from_str(&content)?;

        for record in auth_file.authorizations {
            let key = format!("{}:{}", record.channel, record.chat_id);
            self.authorized.insert(key, record);
        }

        Ok(())
    }

    /// Save authorizations to file
    fn save_authorizations(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.auth_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let auth_file = AuthFile {
            authorizations: self.authorized.iter().map(|r| r.value().clone()).collect(),
        };

        let content = toml::to_string_pretty(&auth_file)?;
        std::fs::write(&self.auth_file_path, content)?;

        Ok(())
    }
}

/// Generate a 6-character alphanumeric pairing code
fn generate_pairing_code() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // Omit confusing chars

    let mut rng = rand::thread_rng();
    (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config() -> ChannelAuthConfig {
        let dir = tempdir().unwrap();
        ChannelAuthConfig {
            auth_file: dir.path().join("auth.toml").to_string_lossy().to_string(),
            pairing_expiry_secs: 60,
        }
    }

    #[test]
    fn test_pairing_flow() {
        let manager = ChannelAuthManager::new(test_config());
        let target = ChannelTarget::telegram(12345, Some(67890));

        // Initially denied
        assert_eq!(manager.check_auth(&target), AuthStatus::Denied);

        // Request pairing
        let code = manager.request_pairing(&target);
        assert_eq!(code.len(), 6);

        // Check shows pending
        match manager.check_auth(&target) {
            AuthStatus::PendingPairing(c) => assert_eq!(c, code),
            _ => panic!("Expected pending pairing"),
        }

        // Approve pairing
        let approved = manager.approve_pairing(&code).unwrap();
        assert_eq!(approved.chat_id, "12345");

        // Now authorized
        assert_eq!(manager.check_auth(&target), AuthStatus::Authorized);

        // Can't approve same code again
        assert!(manager.approve_pairing(&code).is_err());
    }

    #[test]
    fn test_revocation() {
        let manager = ChannelAuthManager::new(test_config());
        let target = ChannelTarget::telegram(12345, None);

        // Authorize via pairing
        let code = manager.request_pairing(&target);
        manager.approve_pairing(&code).unwrap();
        assert_eq!(manager.check_auth(&target), AuthStatus::Authorized);

        // Revoke
        assert!(manager.revoke(&target));
        assert_eq!(manager.check_auth(&target), AuthStatus::Denied);

        // Can't revoke twice
        assert!(!manager.revoke(&target));
    }

    #[test]
    fn test_pairing_code_uniqueness() {
        let manager = ChannelAuthManager::new(test_config());

        let mut codes = std::collections::HashSet::new();
        for i in 0..100 {
            let target = ChannelTarget::telegram(i, None);
            let code = manager.request_pairing(&target);
            codes.insert(code);
        }

        // All codes should be unique
        assert_eq!(codes.len(), 100);
    }
}
