use crate::config::{BackendConfig, Config};
use crate::llm::{Client, StreamingClient};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Registry of backends with lazy-loaded clients.
/// API keys are stored securely using secrecy::Secret.
pub struct BackendRegistry {
    backends: HashMap<String, BackendConfig>,
    clients: HashMap<String, Client>,
    streaming_clients: HashMap<String, StreamingClient>,
}

impl BackendRegistry {
    /// Create a new registry from config
    pub fn new(config: &Config) -> Self {
        Self {
            backends: config.backends.clone(),
            clients: HashMap::new(),
            streaming_clients: HashMap::new(),
        }
    }

    /// Get or create a client for a backend.
    /// API keys are resolved and wrapped in Secret for secure memory handling.
    pub fn get_client(&mut self, backend: &str) -> Result<&Client> {
        if !self.clients.contains_key(backend) {
            let config = self
                .backends
                .get(backend)
                .ok_or_else(|| anyhow!("Unknown backend: {}", backend))?;

            // Resolve API key - returns Secret<String> for secure handling
            let api_key = config.resolve_api_key().map_err(|_| {
                anyhow!(
                    "No API key for backend '{}'. Set {} or configure api_key in config.",
                    backend,
                    config
                        .api_key_env
                        .as_deref()
                        .unwrap_or("the appropriate env var")
                )
            })?;

            // Create client with the secret API key
            let client = Client::new(&config.base_url, api_key, config.api_format.clone());
            self.clients.insert(backend.to_string(), client);
        }

        Ok(self.clients.get(backend).unwrap())
    }

    /// Get or create a streaming client for a backend.
    pub fn get_streaming_client(&mut self, backend: &str) -> Result<&StreamingClient> {
        if !self.streaming_clients.contains_key(backend) {
            let config = self
                .backends
                .get(backend)
                .ok_or_else(|| anyhow!("Unknown backend: {}", backend))?;

            let api_key = config.resolve_api_key().map_err(|_| {
                anyhow!(
                    "No API key for backend '{}'. Set {} or configure api_key in config.",
                    backend,
                    config
                        .api_key_env
                        .as_deref()
                        .unwrap_or("the appropriate env var")
                )
            })?;

            let client = StreamingClient::new(&config.base_url, api_key, config.api_format.clone());
            self.streaming_clients.insert(backend.to_string(), client);
        }

        Ok(self.streaming_clients.get(backend).unwrap())
    }

    /// List all configured backends
    pub fn list_backends(&self) -> Vec<(&String, &BackendConfig)> {
        self.backends.iter().collect()
    }

    /// Check if a backend exists
    pub fn has_backend(&self, name: &str) -> bool {
        self.backends.contains_key(name)
    }
}
