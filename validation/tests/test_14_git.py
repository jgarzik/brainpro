"""Test 14: Git operations (create commit, create branch, git status)."""

import subprocess

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_output_contains_any,
    assert_git_has_commits,
    assert_git_clean,
)

# Path to mock_webapp_scratch (relative to project root for tool compatibility)
WEBAPP = "fixtures/mock_webapp_scratch"


class TestGit:
    """Git operation tests."""

    def test_create_commit(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to create a commit."""
        # Make a change
        main_rs = mock_webapp.path / "src" / "main.rs"
        content = main_rs.read_text()
        main_rs.write_text(content + "\n// Added for testing git commit\n")

        prompt = (
            f"In the git repository at {WEBAPP}, stage the changes to src/main.rs and create a commit "
            "with an appropriate message describing the change. Run the git commands from within that directory."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert a new commit was created (more than initial commit)
        assert_git_has_commits(mock_webapp.path, 2)

        # Assert working tree is now clean
        assert_git_clean(mock_webapp.path)

    def test_create_branch(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to create a feature branch."""
        prompt = f"In the git repository at {WEBAPP}, create a new branch called 'feature/auth-improvements' and switch to it. Run the git commands from within that directory."

        result = webapp_runner.oneshot(prompt)

        # Check current branch
        branch_result = subprocess.run(
            ["git", "branch", "--show-current"],
            capture_output=True,
            text=True,
            cwd=mock_webapp.path,
        )
        current_branch = branch_result.stdout.strip()

        assert (
            current_branch == "feature/auth-improvements"
        ), f"Expected branch 'feature/auth-improvements', got '{current_branch}'"

    def test_git_status(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Ask brainpro to check git status and summarize."""
        # Make a change to create a dirty state
        main_rs = mock_webapp.path / "src" / "main.rs"
        content = main_rs.read_text()
        main_rs.write_text(content + "\n// Test change\n")

        prompt = f"Run 'git status' in the {WEBAPP} directory and tell me what files have been modified."

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro mentions the modified file
        assert_output_contains_any(result.output, "main.rs", "modified", "changed")
