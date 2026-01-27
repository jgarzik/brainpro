//! Turn state persistence for yield/resume semantics.
//!
//! Stores turn state to disk for durability across agent restarts.
//! State files are stored in $BRAINPRO_DATA_DIR/turns/{turn_id}.json
//! with a 30-minute TTL for cleanup.

use crate::protocol::internal::YieldReason;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// TTL for turn state (30 minutes)
const TURN_STATE_TTL_SECS: u64 = 30 * 60;

/// Pending tool call that caused the yield
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_args: Value,
    /// Policy rule that triggered the ask (for approval)
    pub policy_rule: Option<String>,
    /// Questions (for AskUserQuestion)
    pub questions: Option<Vec<Value>>,
}

/// Saved state for a yielded turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnState {
    /// Unique turn ID
    pub turn_id: String,
    /// Session this turn belongs to
    pub session_id: String,
    /// Request ID for correlation
    pub request_id: String,
    /// Conversation messages at yield point
    pub messages: Vec<Value>,
    /// Tool call that caused the yield
    pub pending_tool_call: PendingToolCall,
    /// Why the turn yielded
    pub yield_reason: YieldReason,
    /// Unix timestamp when created
    pub created_at: u64,
    /// Target model@backend
    pub target: Option<String>,
    /// Working directory
    pub working_dir: Option<String>,
}

impl TurnState {
    /// Create a new turn state
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        turn_id: String,
        session_id: String,
        request_id: String,
        messages: Vec<Value>,
        pending_tool_call: PendingToolCall,
        yield_reason: YieldReason,
        target: Option<String>,
        working_dir: Option<String>,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            turn_id,
            session_id,
            request_id,
            messages,
            pending_tool_call,
            yield_reason,
            created_at,
            target,
            working_dir,
        }
    }

    /// Check if this state has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.created_at) > TURN_STATE_TTL_SECS
    }
}

/// Store for managing turn states with disk persistence
pub struct TurnStateStore {
    /// In-memory cache
    cache: Arc<RwLock<HashMap<String, TurnState>>>,
    /// Directory for persistent storage
    storage_dir: PathBuf,
}

impl TurnStateStore {
    /// Create a new store with the given storage directory
    pub fn new(storage_dir: PathBuf) -> Self {
        // Ensure directory exists
        let _ = fs::create_dir_all(&storage_dir);

        let store = Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
        };

        // Load existing states from disk
        store.load_from_disk();

        store
    }

    /// Create with default data directory
    pub fn with_default_dir() -> Self {
        let data_dir = std::env::var("BRAINPRO_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("brainpro")
            });
        Self::new(data_dir.join("turns"))
    }

    /// Get the storage path for a turn ID
    fn state_path(&self, turn_id: &str) -> PathBuf {
        self.storage_dir.join(format!("{}.json", turn_id))
    }

    /// Load existing states from disk
    fn load_from_disk(&self) {
        let mut cache = self.cache.write().unwrap();

        if let Ok(entries) = fs::read_dir(&self.storage_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        if let Ok(state) = serde_json::from_str::<TurnState>(&contents) {
                            if !state.is_expired() {
                                cache.insert(state.turn_id.clone(), state);
                            } else {
                                // Remove expired file
                                let _ = fs::remove_file(&path);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Save a turn state
    pub fn save(&self, state: TurnState) -> Result<(), std::io::Error> {
        let turn_id = state.turn_id.clone();
        let path = self.state_path(&turn_id);

        // Write to disk
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(&path, json)?;

        // Update cache
        let mut cache = self.cache.write().unwrap();
        cache.insert(turn_id, state);

        Ok(())
    }

    /// Get a turn state by ID
    pub fn get(&self, turn_id: &str) -> Option<TurnState> {
        let cache = self.cache.read().unwrap();
        cache.get(turn_id).cloned().filter(|s| !s.is_expired())
    }

    /// Remove a turn state
    pub fn remove(&self, turn_id: &str) -> Option<TurnState> {
        // Remove from disk
        let path = self.state_path(turn_id);
        let _ = fs::remove_file(&path);

        // Remove from cache
        let mut cache = self.cache.write().unwrap();
        cache.remove(turn_id)
    }

    /// Clean up expired states
    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.write().unwrap();
        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, s)| s.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        for turn_id in expired {
            cache.remove(&turn_id);
            let path = self.state_path(&turn_id);
            let _ = fs::remove_file(&path);
        }
    }

    /// Run cleanup in background
    pub fn start_cleanup_task(store: Arc<Self>) {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            store.cleanup_expired();
        });
    }
}

impl Default for TurnStateStore {
    fn default() -> Self {
        Self::with_default_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_turn_state_expiry() {
        let state = TurnState {
            turn_id: "test".to_string(),
            session_id: "s1".to_string(),
            request_id: "r1".to_string(),
            messages: vec![],
            pending_tool_call: PendingToolCall {
                tool_call_id: "tc1".to_string(),
                tool_name: "Bash".to_string(),
                tool_args: serde_json::json!({"command": "ls"}),
                policy_rule: Some("Bash".to_string()),
                questions: None,
            },
            yield_reason: YieldReason::AwaitingApproval,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            target: None,
            working_dir: None,
        };

        assert!(!state.is_expired());
    }

    #[test]
    fn test_turn_state_store() {
        let dir = tempdir().unwrap();
        let store = TurnStateStore::new(dir.path().to_path_buf());

        let state = TurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "req-1".to_string(),
            vec![serde_json::json!({"role": "user", "content": "hello"})],
            PendingToolCall {
                tool_call_id: "tc1".to_string(),
                tool_name: "Bash".to_string(),
                tool_args: serde_json::json!({"command": "ls"}),
                policy_rule: Some("Bash".to_string()),
                questions: None,
            },
            YieldReason::AwaitingApproval,
            None,
            None,
        );

        // Save
        store.save(state.clone()).unwrap();

        // Get
        let retrieved = store.get("turn-1").unwrap();
        assert_eq!(retrieved.turn_id, "turn-1");
        assert_eq!(retrieved.session_id, "session-1");

        // File exists
        assert!(dir.path().join("turn-1.json").exists());

        // Remove
        store.remove("turn-1");
        assert!(store.get("turn-1").is_none());
        assert!(!dir.path().join("turn-1.json").exists());
    }
}
