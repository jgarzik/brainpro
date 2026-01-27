//! Database service.

use crate::config::Config;
use crate::models::user::User;

/// Database connection handler.
pub struct Database {
    connection_string: String,
}

impl Database {
    /// Create a new database connection.
    pub fn new(config: &Config) -> Self {
        Self {
            connection_string: config.database_url.clone(),
        }
    }

    /// Execute a query and return results.
    pub fn query(&self, sql: &str) -> Vec<User> {
        // Simulated query execution
        if sql.contains("SELECT") {
            vec![User::new(1, "test".to_string(), "test@example.com".to_string())]
        } else {
            vec![]
        }
    }

    /// Execute a query using the old API.
    ///
    /// # Deprecated
    /// This function is deprecated. Use `query()` instead.
    #[deprecated(since = "0.1.0", note = "Use query() instead")]
    pub fn old_query(&self, sql: &str) -> Vec<User> {
        // Old implementation - less efficient
        let mut results = Vec::new();
        if sql.contains("SELECT") {
            results.push(User::new(1, "test".to_string(), "test@example.com".to_string()));
        }
        results
    }

    /// Insert a new record.
    pub fn insert(&self, _table: &str, _data: &User) -> Result<u64, String> {
        // Simulated insert
        Ok(1)
    }

    /// Check database connection.
    pub fn is_connected(&self) -> bool {
        !self.connection_string.is_empty()
    }
}
