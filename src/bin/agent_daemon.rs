//! brainpro-agent: Agent daemon that listens on Unix socket.
//!
//! This daemon handles LLM calls and tool execution, receiving requests
//! from the gateway via a Unix socket and streaming NDJSON events back.
//!
//! Usage:
//!   brainpro-agent [--socket /path/to/socket] [--gateway-mode] [--persona mrcode|mrbot]
//!
//! Environment variables:
//!   BRAINPRO_AGENT_SOCKET - Path to Unix socket (default: /run/brainpro.sock)
//!   BRAINPRO_GATEWAY_MODE - Enable gateway mode (yields on ask decisions)
//!   BRAINPRO_PERSONA - Persona to use (mrcode or mrbot, default: mrbot)

use brainpro::agent_service::server::run_with_persona;
use std::env;

fn main() {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    // Parse socket path from args or environment
    let socket_path = parse_socket_path();
    let gateway_mode = parse_gateway_mode();
    let persona = parse_persona();

    eprintln!("brainpro-agent starting...");
    eprintln!("Socket: {}", socket_path);
    eprintln!("Gateway mode: {}", gateway_mode);
    eprintln!("Persona: {}", persona);

    // Run the server
    let result = if gateway_mode {
        run_with_persona(&socket_path, true, &persona)
    } else {
        run_with_persona(&socket_path, false, &persona)
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

fn parse_persona() -> String {
    // Check command line args first
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--persona" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }

    // Check environment variable
    if let Ok(persona) = env::var("BRAINPRO_PERSONA") {
        return persona;
    }

    // Default: mrbot for gateway mode
    "mrbot".to_string()
}
