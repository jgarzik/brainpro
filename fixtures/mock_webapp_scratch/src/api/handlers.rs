//! HTTP request handlers.

use crate::models::user::User;
use crate::services::auth::AuthService;

// Handler for getting user by ID
pub fn get_user(user_id: u64) -> Option<User> {
    // Simulate database lookup
    if user_id == 1 {
        Some(User::new(1, "admin".to_string(), "admin@example.com".to_string()))
    } else {
        None
    }
}

// Handler for creating a new user
pub fn create_user(name: String, email: String) -> Result<User, String> {
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    Ok(User::new(0, name, email))
}

// Handler for user login
pub fn login(email: &str, password: &str, auth: &AuthService) -> Result<String, String> {
    if auth.verify_credentials(email, password) {
        Ok(auth.generate_token(email))
    } else {
        Err("Invalid credentials".to_string())
    }
}

// Handler for listing all users
pub fn list_users() -> Vec<User> {
    vec![
        User::new(1, "admin".to_string(), "admin@example.com".to_string()),
        User::new(2, "user".to_string(), "user@example.com".to_string()),
    ]
}

// Handler for deleting a user
pub fn delete_user(user_id: u64) -> bool {
    // Simulate deletion
    user_id > 0
}
