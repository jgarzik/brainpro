// Validation utilities

/// Checks if a password is strong.
/// 
/// A strong password must:
/// - Be at least 8 characters long
/// - Contain at least one uppercase letter
/// - Contain at least one digit
/// 
/// # Arguments
/// * `password` - The password string to validate
/// 
/// # Returns
/// * `true` if the password meets all criteria, `false` otherwise
pub fn is_strong_password(password: &str) -> bool {
    if password.len() < 8 {
        return false;
    }
    
    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    
    has_uppercase && has_digit
}
