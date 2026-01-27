//! Application configuration.

/// Application configuration settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Database connection string
    pub database_url: String,
    /// Server port
    pub port: u16,
    /// Enable debug mode
    pub debug: bool,
}

impl Config {
    /// Load configuration from environment.
    pub fn load() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite::memory:".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            debug: std::env::var("DEBUG").is_ok(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::load()
    }
}
