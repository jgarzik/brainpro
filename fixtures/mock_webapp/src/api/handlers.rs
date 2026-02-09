//! HTTP request handlers.

use crate::models::user::User;
use crate::services::auth::AuthService;

/// Retrieves a user by their unique ID.
/// Returns `Some(User)` if found, `None` otherwise.
pub fn get_user(user_id: u64) -> Option<User> {
    // Simulate database lookup
    if user_id == 1 {
        Some(User::new(1, "admin".to_string(), "admin@example.com".to_string(), 1609459200))
    } else {
        None
    }
}

/// Creates a new user with the provided name and email.
/// Returns `Ok(User)` on success, `Err(message)` on validation failure.
pub fn create_user(name: String, email: String) -> Result<User, String> {
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    Ok(User::new(0, name, email, 1609459200))
}

/// Authenticates a user with email and password.
/// Returns `Ok(token)` with an authentication token on success, `Err(message)` on failure.
pub fn login(email: &str, password: &str, auth: &AuthService) -> Result<String, String> {
    if auth.verify_credentials(email, password) {
        Ok(auth.generate_token(email))
    } else {
        Err("Invalid credentials".to_string())
    }
}

/// Returns a list of all users in the system.
/// This is a mock implementation with static data.
pub fn list_users() -> Vec<User> {
    vec![
        User::new(1, "admin".to_string(), "admin@example.com".to_string(), 1609459200),
        User::new(2, "user".to_string(), "user@example.com".to_string(), 1609459300),
    ]
}

/// Deletes a user by their ID.
/// Returns `true` if deletion was successful (simulated), `false` otherwise.
pub fn delete_user(user_id: u64) -> bool {
    // Simulate deletion
    user_id > 0
}
