//! Session model.

use std::time::{Duration, SystemTime};

/// Represents an authenticated session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Session token
    pub token: String,
    /// Associated user ID
    pub user_id: u64,
    /// Session creation time
    pub created_at: SystemTime,
    /// Session expiration duration
    pub expires_in: Duration,
}

impl Session {
    /// Create a new session.
    pub fn new(token: String, user_id: u64) -> Self {
        Self {
            token,
            user_id,
            created_at: SystemTime::now(),
            expires_in: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Check if the session has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at
            .elapsed()
            .map(|elapsed| elapsed > self.expires_in)
            .unwrap_or(true)
    }
}
