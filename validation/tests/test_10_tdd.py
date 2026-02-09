"""Test 10: TDD workflow (write failing test, implement to pass, full cycle)."""

import subprocess

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_file_contains,
    assert_single_test_passes,
    assert_cargo_test_passes,
)

# Path to mock_webapp_scratch (relative to project root for tool compatibility)
WEBAPP = "fixtures/mock_webapp_scratch"


class TestTDD:
    """TDD workflow tests."""

    def test_write_failing_test(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Ask brainpro to write a test for a new feature."""
        prompt = (
            f"Write a test called test_validate_phone_number in {WEBAPP}/tests/unit_tests.rs "
            "that tests a validate_phone_number function. The function should accept "
            "numbers like '555-1234' and '+1-555-555-1234'. Just write the test, "
            "don't implement the function yet."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert the test was added
        assert_file_contains(
            mock_webapp.path / "tests" / "unit_tests.rs",
            "test_validate_phone_number",
        )

    def test_implement_to_pass(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """First create a failing test, then ask brainpro to implement the function."""
        # Add a test that will fail
        test_file = mock_webapp.path / "tests" / "unit_tests.rs"
        test_content = test_file.read_text()
        test_content += """

#[test]
fn test_is_valid_username_char() {
    use mock_webapp::utils::validation::is_valid_username_char;
    assert!(is_valid_username_char('a'));
    assert!(is_valid_username_char('Z'));
    assert!(is_valid_username_char('5'));
    assert!(is_valid_username_char('_'));
    assert!(!is_valid_username_char('@'));
    assert!(!is_valid_username_char(' '));
}
"""
        test_file.write_text(test_content)

        # Ask brainpro to implement the function
        prompt = (
            f"There's a test called test_is_valid_username_char in {WEBAPP}/tests/unit_tests.rs "
            f"that's failing because the function doesn't exist. Implement is_valid_username_char "
            f"in {WEBAPP}/src/utils/validation.rs to make the test pass."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert the function was added
        assert_file_contains(
            mock_webapp.path / "src" / "utils" / "validation.rs",
            "is_valid_username_char",
        )

        # Assert the test now passes
        assert_single_test_passes(mock_webapp.path, "test_is_valid_username_char")

    def test_tdd_cycle(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Full TDD cycle - write test, see it fail, implement, see it pass."""
        prompt = (
            f"Follow TDD to add a new feature: a function called 'is_strong_password' "
            f"in {WEBAPP}/src/utils/validation.rs that returns true if a password has at least 8 chars, "
            f"contains a number, and contains an uppercase letter. First write the test in "
            f"{WEBAPP}/tests/unit_tests.rs, then implement the function to make it pass."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert the test exists
        assert_file_contains(
            mock_webapp.path / "tests" / "unit_tests.rs",
            "strong_password",
        )

        # Assert the function exists
        assert_file_contains(
            mock_webapp.path / "src" / "utils" / "validation.rs",
            "is_strong_password",
        )

        # Assert the new test passes (don't run full suite - pre-existing intentional failures)
        assert_single_test_passes(mock_webapp.path, "test_is_strong_password")
