//! User model.

/// Represents a user in the system.
#[derive(Debug, Clone)]
pub struct User {
    /// Unique user identifier
    pub id: u64,
    /// User's display name
    pub name: String,
    /// User's email address
    pub email: String,
    // TODO: Add created_at timestamp field
}

impl User {
    /// Create a new user.
    pub fn new(id: u64, name: String, email: String) -> Self {
        Self { id, name, email }
    }

    /// Check if the user is an admin.
    pub fn is_admin(&self) -> bool {
        self.name == "admin"
    }
}
