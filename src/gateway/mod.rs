//! Gateway module - WebSocket server that connects clients to agent daemon.
//! Used by brainpro-gateway binary.

#![allow(dead_code)]

pub mod agent_conn;
pub mod channels;
pub mod client_mgr;
pub mod lanes;
pub mod server;
