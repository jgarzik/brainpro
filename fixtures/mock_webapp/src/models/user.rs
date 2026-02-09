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
    /// Timestamp when the user was created
    pub created_at: u64,
}

impl User {
    /// Create a new user.
    pub fn new(id: u64, name: String, email: String, created_at: u64) -> Self {
        Self { id, name, email, created_at }
    }

    /// Check if the user is an admin.
    pub fn is_admin(&self) -> bool {
        self.name == "admin"
    }
}
