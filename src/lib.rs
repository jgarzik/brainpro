//! Brainpro - An agentic coding assistant
//!
//! This library provides the core functionality for the brainpro CLI,
//! gateway, and agent daemon.

pub mod agent;
mod agent_impl;
pub mod agent_policy;
pub mod claude_api;
pub mod claude_auth;
pub mod agent_service;
pub mod backend;
pub mod circuit_breaker;
pub mod cli;
pub mod commands;
pub mod compact;
pub mod config;
pub mod context_factory;
pub mod cost;
pub mod events;
pub mod gateway;
pub mod gateway_client;
pub mod hooks;
pub mod llm;
pub mod metrics;
pub mod model_routing;
pub mod persona;
pub mod plan;
pub mod policy;
pub mod privacy;
pub mod protocol;
pub mod provider_health;
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
