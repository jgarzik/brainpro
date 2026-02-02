//! Messaging channel integrations for Telegram and Discord.
//!
//! This module provides a plugin system for integrating external messaging
//! platforms as gateway clients. Each platform (Telegram, Discord) is
//! implemented as a `ChannelPlugin` that handles:
//!
//! - Receiving messages and forwarding to the agent
//! - Streaming agent responses back to users
//! - Interactive approval buttons for tool calls
//! - Per-chat session management
//! - Pairing-based authorization
//!
//! # Architecture
//!
//! ```text
//!                     ┌─────────────────────────────────────────┐
//!                     │         Gateway (Axum + Channels)        │
//!    TG Bot ─────────►│  ┌──────────┐    ┌─────────────────┐   │
//!                     │  │ Channel  │───►│ AsyncAgentConn  │───┼──► Agent Daemon
//! Discord Bot ──────►│  │ Manager  │◄───│ (Unix Socket)   │   │
//!                     │  └──────────┘    └─────────────────┘   │
//!    WebSocket ──────►│  │                                      │
//!                     │  ├─► ChannelSessionMap                  │
//!                     │  └─► ChannelAuthManager                 │
//!                     └─────────────────────────────────────────┘
//! ```

pub mod auth;
pub mod config;
pub mod manager;
pub mod plugin;
pub mod routes;
pub mod session_map;

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

// Re-export commonly used types (public API, may not be used internally)
#[allow(unused_imports)]
pub use config::ChannelsConfig;
#[allow(unused_imports)]
pub use manager::ChannelManager;
#[allow(unused_imports)]
pub use plugin::{
    ApprovalRequest, ApprovalResponse, ChannelContext, ChannelEvent, ChannelPlugin, ChannelTarget,
    InboundMessage, MessageId, OutboundMessage,
};
#[allow(unused_imports)]
pub use session_map::ChannelSessionMap;

/// Create channel HTTP routes
pub fn channel_routes() -> axum::Router<std::sync::Arc<manager::ChannelManager>> {
    routes::routes()
}
