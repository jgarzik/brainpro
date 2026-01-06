//! Session persistence for resuming conversations.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Saved session state
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedSession {
    pub session_id: String,
    pub messages: Vec<Value>,
    pub turn_count: u32,
}

/// Get the sessions directory
fn sessions_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".yo")
        .join("sessions")
}

/// Get path for a specific session
fn session_path(session_id: &str) -> PathBuf {
    sessions_dir().join(format!("{}.json", session_id))
}

/// Save a session to disk
pub fn save_session(session_id: &str, messages: &[Value], turn_count: u32) -> Result<()> {
    let dir = sessions_dir();
    fs::create_dir_all(&dir)?;

    let session = SavedSession {
        session_id: session_id.to_string(),
        messages: messages.to_vec(),
        turn_count,
    };

    let path = session_path(session_id);
    let json = serde_json::to_string_pretty(&session)?;
    fs::write(&path, json)?;

    eprintln!("Session saved: {}", path.display());
    Ok(())
}

/// Load a session from disk
pub fn load_session(session_id: &str) -> Result<SavedSession> {
    let path = session_path(session_id);
    let json = fs::read_to_string(&path)?;
    let session: SavedSession = serde_json::from_str(&json)?;
    Ok(session)
}
