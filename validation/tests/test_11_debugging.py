"""Test 11: Debugging (trace error, fix failing test, find security issue)."""

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_output_contains_any,
    assert_single_test_passes,
    assert_cargo_test_passes,
    assert_cargo_test_fails,
    assert_single_test_fails,
)

# Path to mock_webapp_scratch (relative to project root for tool compatibility)
WEBAPP = "fixtures/mock_webapp_scratch"


class TestDebugging:
    """Debugging tests."""

    def test_trace_error(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Give brainpro a simulated error and ask it to diagnose."""
        error_msg = (
            "Error: Email validation accepted invalid input '@.' - "
            "validation should have rejected this malformed email address"
        )

        prompt = (
            f"I'm getting this error in production: '{error_msg}'. "
            f"Find where this bug is in {WEBAPP} and explain what's wrong."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro traced it to validation.rs
        assert_output_contains_any(
            result.output,
            "validation.rs",
            "validate_email",
            "contains('@')",
            "only checks",
        )

    def test_fix_failing_test(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Ask brainpro to find and fix a failing test."""
        # Verify test is failing initially
        assert_cargo_test_fails(mock_webapp.path)
        assert_single_test_fails(mock_webapp.path, "test_validate_malformed_email")

        prompt = (
            f"Run cargo test in {WEBAPP} to find the failing test. Then fix the bug in the source "
            "code so the test passes. The test is correct - the source code has a bug."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro identified the issue
        assert_output_contains_any(
            result.output, "validate_email", "validation.rs", "malformed", "@."
        )

        # Assert the test now passes
        assert_single_test_passes(mock_webapp.path, "test_validate_malformed_email")

        # Verify all tests pass
        assert_cargo_test_passes(mock_webapp.path)

    def test_find_security_issue(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to find security issues in auth.rs."""
        prompt = f"Review {WEBAPP}/src/services/auth.rs for security issues. Report any problems you find."

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro found the hardcoded API key issue
        assert_output_contains_any(
            result.output,
            "hardcoded",
            "api_key",
            "secret",
            "sk-secret",
            "credential",
        )
