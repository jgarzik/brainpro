//! Input validation utilities.

/// Validate an email address format.
pub fn validate_email(email: &str) -> bool {
    // Basic email validation
    // BUG: Only checks for '@' and '.' presence, not proper format
    // This accepts invalid emails like "@." or "a@." or "@.b"
    email.contains('@') && email.contains('.')
}

/// Validate a username.
pub fn validate_username(username: &str) -> bool {
    username.len() >= 3 && username.len() <= 50
}

/// Validate a password meets requirements.
pub fn validate_password(password: &str) -> bool {
    password.len() >= 8
}

/// Check if a character is valid for a username.
pub fn is_valid_username_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Sanitize user input by removing dangerous characters.
pub fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '_' || *c == '-')
        .collect()
}
