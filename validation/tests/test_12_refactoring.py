"""Test 12: Refactoring (rename across files, find deprecated, modernize function)."""

import subprocess

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_output_contains_any,
    assert_file_contains,
    assert_file_not_contains,
    assert_git_dirty,
    assert_success,
)

# Path to mock_webapp_scratch (relative to project root for tool compatibility)
WEBAPP = "fixtures/mock_webapp_scratch"


class TestRefactoring:
    """Refactoring tests."""

    def test_rename_across_files(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to rename a struct across files."""
        prompt = (
            f"Rename the User struct to AppUser across {WEBAPP}. "
            "Update all imports, usages, and references. Make sure the code still compiles."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert files were modified
        assert_git_dirty(mock_webapp.path)

        # Assert AppUser now exists
        assert_file_contains(
            mock_webapp.path / "src" / "models" / "user.rs",
            "struct AppUser",
        )

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)

    def test_find_deprecated(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to find deprecated functions."""
        prompt = (
            f"Find all deprecated functions in {WEBAPP}. "
            "Look for #[deprecated] attributes or 'deprecated' comments."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro found the deprecated function
        assert_output_contains_any(
            result.output, "old_query", "deprecated", "database.rs"
        )

    def test_modernize_function(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to remove a deprecated function."""
        # Verify old_query exists initially
        assert_file_contains(
            mock_webapp.path / "src" / "services" / "database.rs",
            "old_query",
        )

        prompt = (
            f"The function old_query in {WEBAPP}/src/services/database.rs is deprecated. "
            "Remove it from the codebase. Make sure the code still compiles after removal."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert file was modified
        assert_git_dirty(mock_webapp.path)

        # Assert old_query is no longer in the file
        assert_file_not_contains(
            mock_webapp.path / "src" / "services" / "database.rs",
            "fn old_query",
        )

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)
