//! Brainpro - An agentic coding assistant
//!
//! This library provides the core functionality for the brainpro CLI,
//! gateway, and agent daemon.

pub mod agent;
pub mod agent_service;
pub mod backend;
pub mod cli;
pub mod commands;
pub mod compact;
pub mod config;
pub mod cost;
pub mod gateway;
pub mod gateway_client;
pub mod hooks;
pub mod llm;
pub mod model_routing;
pub mod plan;
pub mod policy;
pub mod protocol;
pub mod session;
pub mod skillpacks;
pub mod subagent;
pub mod tool_display;
pub mod tool_filter;
pub mod tools;
pub mod transcript;
pub mod vendors;

// Re-export Args for the binaries
pub use cli::Args;
