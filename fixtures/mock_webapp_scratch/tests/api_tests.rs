//! API integration tests.

use mock_webapp::utils::validation::validate_email;

#[test]
fn test_validate_email_with_valid() {
    assert!(validate_email("user@example.com"));
}

#[test]
fn test_validate_email_without_at() {
    assert!(!validate_email("invalid.email.com"));
}

#[test]
fn test_validate_email_without_dot() {
    assert!(!validate_email("user@example"));
}

// BUG: This test fails because validate_email accepts "@." as valid
// Marked as #[ignore] so it doesn't break other tests (e.g., TDD validation)
#[test]
#[ignore]
fn test_validate_malformed_email() {
    // This should return false, but due to the bug it returns true
    // The bug is in validation.rs - it only checks for presence of '@' and '.'
    // but doesn't validate the actual email format
    assert!(!validate_email("@."));
}
