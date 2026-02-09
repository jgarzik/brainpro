//! Unit tests for mock_webapp.

use mock_webapp::config::Config;
use mock_webapp::models::user::User;
use mock_webapp::models::session::Session;
use mock_webapp::utils::helpers;
use mock_webapp::utils::validation;

#[test]
fn test_config_default() {
    let config = Config::default();
    assert!(!config.database_url.is_empty());
}

#[test]
fn test_user_creation() {
    let user = User::new(1, "test".to_string(), "test@example.com".to_string(), 1609459200);
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "test");
    assert_eq!(user.email, "test@example.com");
}

#[test]
fn test_session_creation() {
    let session = Session::new("token123".to_string(), 1);
    assert_eq!(session.user_id, 1);
    assert!(!session.is_expired());
}

#[test]
fn test_format_timestamp() {
    let result = helpers::format_timestamp(60);
    assert_eq!(result, "60s ago");
}

#[test]
fn test_simple_hash() {
    let hash1 = helpers::simple_hash("test");
    let hash2 = helpers::simple_hash("test");
    assert_eq!(hash1, hash2);

    let hash3 = helpers::simple_hash("different");
    assert_ne!(hash1, hash3);
}

#[test]
fn test_truncate() {
    assert_eq!(helpers::truncate("hello", 10), "hello");
    assert_eq!(helpers::truncate("hello world", 5), "hello");
}

#[test]
fn test_validate_email_valid() {
    assert!(validation::validate_email("user@example.com"));
    assert!(validation::validate_email("test.name@domain.org"));
}

#[test]
fn test_validate_email_invalid() {
    assert!(!validation::validate_email("invalid"));
    assert!(!validation::validate_email("no-at-sign.com"));
}

#[test]
fn test_validate_username() {
    assert!(validation::validate_username("validuser"));
    assert!(!validation::validate_username("ab")); // too short
}

#[test]
fn test_validate_password() {
    assert!(validation::validate_password("longpassword123"));
    assert!(!validation::validate_password("short")); // too short
}

#[test]
fn test_sanitize_input() {
    assert_eq!(validation::sanitize_input("hello world"), "hello world");
    assert_eq!(validation::sanitize_input("hello<script>"), "helloscript");
    assert_eq!(validation::sanitize_input("test_name-123"), "test_name-123");
}

#[test]
fn test_validate_malformed_email() {
    // These malformed emails should NOT be valid
    assert!(!validation::validate_email("@."));
    assert!(!validation::validate_email("a@."));
    assert!(!validation::validate_email("@.b"));
}
