//! Session mapping between channel targets and agent sessions.

use super::plugin::ChannelTarget;
use dashmap::DashMap;
use std::sync::Arc;

/// Maps channel targets to session IDs and vice versa
#[derive(Debug, Default)]
pub struct ChannelSessionMap {
    /// Target -> Session ID
    target_to_session: DashMap<String, SessionInfo>,
    /// Session ID -> Target
    session_to_target: DashMap<String, ChannelTarget>,
}

/// Information about a session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Agent session ID
    pub session_id: String,
    /// Target for the session
    pub target: ChannelTarget,
    /// Last activity timestamp
    pub last_activity: std::time::Instant,
    /// Current streaming message ID (for editing)
    pub streaming_message_id: Option<String>,
    /// Current turn ID (if awaiting approval)
    pub pending_turn_id: Option<String>,
}

impl ChannelSessionMap {
    /// Create a new session map
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            target_to_session: DashMap::new(),
            session_to_target: DashMap::new(),
        })
    }

    /// Get or create a session for a target
    pub fn get_or_create_session(&self, target: &ChannelTarget) -> SessionInfo {
        let key = target.session_key();

        // Try to get existing session
        if let Some(info) = self.target_to_session.get(&key) {
            return info.clone();
        }

        // Create new session
        let session_id = uuid::Uuid::new_v4().to_string();
        let info = SessionInfo {
            session_id: session_id.clone(),
            target: target.clone(),
            last_activity: std::time::Instant::now(),
            streaming_message_id: None,
            pending_turn_id: None,
        };

        self.target_to_session.insert(key, info.clone());
        self.session_to_target.insert(session_id, target.clone());

        info
    }

    /// Get session info for a target
    pub fn get_session(&self, target: &ChannelTarget) -> Option<SessionInfo> {
        let key = target.session_key();
        self.target_to_session.get(&key).map(|r| r.clone())
    }

    /// Get target for a session ID
    pub fn get_target(&self, session_id: &str) -> Option<ChannelTarget> {
        self.session_to_target.get(session_id).map(|r| r.clone())
    }

    /// Update session activity timestamp
    pub fn touch_session(&self, target: &ChannelTarget) {
        let key = target.session_key();
        if let Some(mut info) = self.target_to_session.get_mut(&key) {
            info.last_activity = std::time::Instant::now();
        }
    }

    /// Set the streaming message ID for a session
    pub fn set_streaming_message(&self, target: &ChannelTarget, message_id: Option<String>) {
        let key = target.session_key();
        if let Some(mut info) = self.target_to_session.get_mut(&key) {
            info.streaming_message_id = message_id;
        }
    }

    /// Get the streaming message ID for a session
    pub fn get_streaming_message(&self, target: &ChannelTarget) -> Option<String> {
        let key = target.session_key();
        self.target_to_session
            .get(&key)
            .and_then(|info| info.streaming_message_id.clone())
    }

    /// Set the pending turn ID for a session
    pub fn set_pending_turn(&self, target: &ChannelTarget, turn_id: Option<String>) {
        let key = target.session_key();
        if let Some(mut info) = self.target_to_session.get_mut(&key) {
            info.pending_turn_id = turn_id;
        }
    }

    /// Get the pending turn ID for a session
    pub fn get_pending_turn(&self, target: &ChannelTarget) -> Option<String> {
        let key = target.session_key();
        self.target_to_session
            .get(&key)
            .and_then(|info| info.pending_turn_id.clone())
    }

    /// Remove a session
    pub fn remove_session(&self, target: &ChannelTarget) {
        let key = target.session_key();
        if let Some((_, info)) = self.target_to_session.remove(&key) {
            self.session_to_target.remove(&info.session_id);
        }
    }

    /// Get all active sessions
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.target_to_session
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.target_to_session.len()
    }

    /// Remove stale sessions (older than max_age)
    pub fn cleanup_stale(&self, max_age: std::time::Duration) -> usize {
        let now = std::time::Instant::now();
        let mut removed = 0;

        // Collect keys to remove
        let stale_keys: Vec<String> = self
            .target_to_session
            .iter()
            .filter(|r| now.duration_since(r.last_activity) > max_age)
            .map(|r| r.key().clone())
            .collect();

        // Remove stale sessions
        for key in stale_keys {
            if let Some((_, info)) = self.target_to_session.remove(&key) {
                self.session_to_target.remove(&info.session_id);
                removed += 1;
            }
        }

        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_create_session() {
        let map = ChannelSessionMap::new();
        let target = ChannelTarget::telegram(12345, Some(67890));

        // First call creates session
        let info1 = map.get_or_create_session(&target);
        assert!(!info1.session_id.is_empty());

        // Second call returns same session
        let info2 = map.get_or_create_session(&target);
        assert_eq!(info1.session_id, info2.session_id);
    }

    #[test]
    fn test_bidirectional_lookup() {
        let map = ChannelSessionMap::new();
        let target = ChannelTarget::telegram(12345, Some(67890));

        let info = map.get_or_create_session(&target);

        // Lookup by session ID
        let found_target = map.get_target(&info.session_id).unwrap();
        assert_eq!(found_target.chat_id, "12345");

        // Lookup by target
        let found_info = map.get_session(&target).unwrap();
        assert_eq!(found_info.session_id, info.session_id);
    }

    #[test]
    fn test_streaming_message_tracking() {
        let map = ChannelSessionMap::new();
        let target = ChannelTarget::telegram(12345, None);

        map.get_or_create_session(&target);

        assert!(map.get_streaming_message(&target).is_none());

        map.set_streaming_message(&target, Some("msg-123".to_string()));
        assert_eq!(
            map.get_streaming_message(&target),
            Some("msg-123".to_string())
        );

        map.set_streaming_message(&target, None);
        assert!(map.get_streaming_message(&target).is_none());
    }

    #[test]
    fn test_session_cleanup() {
        let map = ChannelSessionMap::new();

        // Create a session
        let target = ChannelTarget::telegram(12345, None);
        map.get_or_create_session(&target);
        assert_eq!(map.session_count(), 1);

        // Cleanup with long max_age should not remove
        let removed = map.cleanup_stale(std::time::Duration::from_secs(3600));
        assert_eq!(removed, 0);
        assert_eq!(map.session_count(), 1);

        // Cleanup with zero max_age should remove
        let removed = map.cleanup_stale(std::time::Duration::ZERO);
        assert_eq!(removed, 1);
        assert_eq!(map.session_count(), 0);
    }
}
