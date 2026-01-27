//! brainpro-agent: Agent daemon that listens on Unix socket.
//!
//! This daemon handles LLM calls and tool execution, receiving requests
//! from the gateway via a Unix socket and streaming NDJSON events back.
//!
//! Usage:
//!   brainpro-agent [--socket /path/to/socket] [--gateway-mode] [--personality mrcode|mrbot]
//!
//! Environment variables:
//!   BRAINPRO_AGENT_SOCKET - Path to Unix socket (default: /run/brainpro.sock)
//!   BRAINPRO_GATEWAY_MODE - Enable gateway mode (yields on ask decisions)
//!   BRAINPRO_PERSONALITY - Personality to use (mrcode or mrbot, default: mrbot)

use brainpro::agent_service::server::run_with_personality;
use std::env;

fn main() {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    // Parse socket path from args or environment
    let socket_path = parse_socket_path();
    let gateway_mode = parse_gateway_mode();
    let personality = parse_personality();

    eprintln!("brainpro-agent starting...");
    eprintln!("Socket: {}", socket_path);
    eprintln!("Gateway mode: {}", gateway_mode);
    eprintln!("Personality: {}", personality);

    // Run the server
    let result = if gateway_mode {
        run_with_personality(&socket_path, true, &personality)
    } else {
        run_with_personality(&socket_path, false, &personality)
    };

    if let Err(e) = result {
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

fn parse_gateway_mode() -> bool {
    // Check command line args first
    let args: Vec<String> = env::args().collect();
    for arg in &args {
        if arg == "--gateway-mode" {
            return true;
        }
    }

    // Check environment variable
    if let Ok(val) = env::var("BRAINPRO_GATEWAY_MODE") {
        return val == "1" || val.to_lowercase() == "true";
    }

    // Default: gateway mode enabled (required for permission prompts)
    true
}

fn parse_personality() -> String {
    // Check command line args first
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--personality" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }

    // Check environment variable
    if let Ok(personality) = env::var("BRAINPRO_PERSONALITY") {
        return personality;
    }

    // Default: mrbot for gateway mode
    "mrbot".to_string()
}
