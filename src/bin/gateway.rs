//! brainpro-gateway: WebSocket gateway server.
//!
//! This server handles client connections via WebSocket, manages sessions,
//! and routes requests to the agent daemon.
//!
//! Usage:
//!   brainpro-gateway [--port 18789] [--agent-socket /path/to/socket]
//!
//! Environment variables:
//!   BRAINPRO_GATEWAY_PORT - Port to listen on (default: 18789)
//!   BRAINPRO_AGENT_SOCKET - Path to agent Unix socket (default: /run/brainpro.sock)
//!   BRAINPRO_GATEWAY_TOKEN - Auth token for client connections (optional)

use brainpro::gateway::server::{run, GatewayConfig};
use std::env;

#[tokio::main]
async fn main() {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    // Parse configuration
    let config = parse_config();

    eprintln!("brainpro-gateway starting...");
    eprintln!("Port: {}", config.port);
    eprintln!("Agent socket: {}", config.agent_socket);

    // Run the server
    if let Err(e) = run(config).await {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn parse_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                if let Ok(port) = args[i + 1].parse() {
                    config.port = port;
                }
                i += 2;
            }
            "--agent-socket" if i + 1 < args.len() => {
                config.agent_socket = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    // Environment variable overrides
    if let Ok(port) = env::var("BRAINPRO_GATEWAY_PORT") {
        if let Ok(p) = port.parse() {
            config.port = p;
        }
    }

    if let Ok(socket) = env::var("BRAINPRO_AGENT_SOCKET") {
        config.agent_socket = socket;
    }

    if let Ok(token) = env::var("BRAINPRO_GATEWAY_TOKEN") {
        config.auth_token = Some(token);
    }

    config
}
