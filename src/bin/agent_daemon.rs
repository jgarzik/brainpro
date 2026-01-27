//! brainpro-agent: Agent daemon that listens on Unix socket.
//!
//! This daemon handles LLM calls and tool execution, receiving requests
//! from the gateway via a Unix socket and streaming NDJSON events back.
//!
//! Usage:
//!   brainpro-agent [--socket /path/to/socket]
//!
//! Environment variables:
//!   BRAINPRO_AGENT_SOCKET - Path to Unix socket (default: /run/brainpro.sock)

use brainpro::agent_service::server::run_with_socket;
use std::env;

fn main() {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    // Parse socket path from args or environment
    let socket_path = parse_socket_path();

    eprintln!("brainpro-agent starting...");
    eprintln!("Socket: {}", socket_path);

    // Run the server
    if let Err(e) = run_with_socket(&socket_path) {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn parse_socket_path() -> String {
    // Check command line args first
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--socket" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }

    // Check environment variable
    if let Ok(path) = env::var("BRAINPRO_AGENT_SOCKET") {
        return path;
    }

    // Default
    "/run/brainpro.sock".to_string()
}
