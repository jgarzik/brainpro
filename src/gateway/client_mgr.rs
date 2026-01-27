//! Client manager - tracks connected clients and their sessions.

use crate::protocol::client::{ClientCapabilities, ClientRole};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Information about a connected client
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id: String,
    pub role: ClientRole,
    pub device_id: String,
    pub caps: ClientCapabilities,
    pub session_id: Option<String>,
    pub connected_at: std::time::Instant,
}

/// Message to send to a client
#[derive(Debug, Clone)]
pub struct ClientMessage {
    pub json: String,
}

/// Handle for sending messages to a client
pub type ClientSender = mpsc::UnboundedSender<ClientMessage>;

/// Manager for all connected clients
pub struct ClientManager {
    /// Map of client ID to client info
    clients: DashMap<String, ClientInfo>,
    /// Map of client ID to message sender
    senders: DashMap<String, ClientSender>,
    /// Map of session ID to client IDs (for broadcasting)
    sessions: DashMap<String, Vec<String>>,
}

impl ClientManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            clients: DashMap::new(),
            senders: DashMap::new(),
            sessions: DashMap::new(),
        })
    }

    /// Register a new client
    pub fn register(
        &self,
        client_id: &str,
        role: ClientRole,
        device_id: &str,
        caps: ClientCapabilities,
        sender: ClientSender,
    ) {
        let info = ClientInfo {
            id: client_id.to_string(),
            role,
            device_id: device_id.to_string(),
            caps,
            session_id: None,
            connected_at: std::time::Instant::now(),
        };
        self.clients.insert(client_id.to_string(), info);
        self.senders.insert(client_id.to_string(), sender);
    }

    /// Unregister a client
    pub fn unregister(&self, client_id: &str) {
        // Remove from any sessions
        if let Some((_, info)) = self.clients.remove(client_id) {
            if let Some(session_id) = &info.session_id {
                self.sessions.alter(session_id, |_, mut clients| {
                    clients.retain(|id| id != client_id);
                    clients
                });
            }
        }
        self.senders.remove(client_id);
    }

    /// Associate a client with a session
    pub fn join_session(&self, client_id: &str, session_id: &str) {
        // Update client info
        if let Some(mut info) = self.clients.get_mut(client_id) {
            info.session_id = Some(session_id.to_string());
        }

        // Add to session's client list
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(client_id.to_string());
    }

    /// Send a message to a specific client
    pub fn send_to_client(&self, client_id: &str, message: &str) -> bool {
        if let Some(sender) = self.senders.get(client_id) {
            sender
                .send(ClientMessage {
                    json: message.to_string(),
                })
                .is_ok()
        } else {
            false
        }
    }

    /// Broadcast a message to all clients in a session
    pub fn broadcast_to_session(&self, session_id: &str, message: &str) {
        if let Some(client_ids) = self.sessions.get(session_id) {
            for client_id in client_ids.iter() {
                self.send_to_client(client_id, message);
            }
        }
    }

    /// Get client info
    pub fn get_client(&self, client_id: &str) -> Option<ClientInfo> {
        self.clients.get(client_id).map(|r| r.clone())
    }

    /// Get all connected client IDs
    pub fn list_clients(&self) -> Vec<String> {
        self.clients.iter().map(|r| r.key().clone()).collect()
    }

    /// Get count of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get operators only
    pub fn list_operators(&self) -> Vec<ClientInfo> {
        self.clients
            .iter()
            .filter(|r| r.role == ClientRole::Operator)
            .map(|r| r.clone())
            .collect()
    }

    /// Get nodes only
    pub fn list_nodes(&self) -> Vec<ClientInfo> {
        self.clients
            .iter()
            .filter(|r| r.role == ClientRole::Node)
            .map(|r| r.clone())
            .collect()
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self {
            clients: DashMap::new(),
            senders: DashMap::new(),
            sessions: DashMap::new(),
        }
    }
}
