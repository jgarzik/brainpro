"""Test 15: Multi-file operations (extract module, add field everywhere, dependency chain)."""

import subprocess

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_file_exists,
    assert_file_contains,
    assert_success,
)

# Path to mock_webapp_scratch (relative to project root for tool compatibility)
WEBAPP = "fixtures/mock_webapp_scratch"


class TestMultiFile:
    """Multi-file operation tests."""

    def test_extract_module(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to extract code into a new module."""
        prompt = (
            f"Create a new module {WEBAPP}/src/errors.rs that defines an AppError enum with "
            f"variants NotFound, InvalidInput, and Unauthorized. Add it to {WEBAPP}/src/lib.rs. "
            "Make sure the code compiles."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert the new file exists
        assert_file_exists(mock_webapp.path / "src" / "errors.rs")

        # Assert it contains the enum
        assert_file_contains(mock_webapp.path / "src" / "errors.rs", "AppError")
        assert_file_contains(mock_webapp.path / "src" / "errors.rs", "NotFound")

        # Assert lib.rs was updated
        assert_file_contains(mock_webapp.path / "src" / "lib.rs", "errors")

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)

    def test_add_field_everywhere(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to add a field to a struct and update all usages."""
        prompt = (
            f"Add a new field 'created_at: u64' to the User struct in {WEBAPP}/src/models/user.rs. "
            "Update the User::new() function to accept this new parameter. "
            "Make sure the code compiles after the change."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert the field was added
        assert_file_contains(
            mock_webapp.path / "src" / "models" / "user.rs",
            "created_at",
        )

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)

    def test_dependency_chain(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to add a config option and thread it through layers."""
        prompt = (
            f"Add a new config option 'max_sessions: u32' with a default value of 100 "
            f"to the Config struct in {WEBAPP}/src/config.rs. Then add a method to AuthService "
            f"in {WEBAPP}/src/services/auth.rs that uses this config value. Make sure the code compiles."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert config was updated
        assert_file_contains(mock_webapp.path / "src" / "config.rs", "max_sessions")

        # Assert auth service uses it
        assert_file_contains(
            mock_webapp.path / "src" / "services" / "auth.rs",
            "max_sessions",
        )

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)
