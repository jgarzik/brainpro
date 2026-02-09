"""Test 07: Subagents (scout, patch, test)."""

import shutil
from pathlib import Path

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
    assert_output_matches,
    assert_file_contains,
)


class TestSubagents:
    """Subagent tests."""

    def test_scout_agent(self, runner: BrainproRunner):
        """Scout subagent can explore codebase."""
        prompt = "Use the scout agent to find all the source files in fixtures/hello_repo and list them"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("lib.rs", result.output)
        assert_output_contains("main.rs", result.output)

    def test_patch_agent(self, runner: BrainproRunner, fixtures_dir: Path):
        """Patch subagent can edit files."""
        # Copy lib.rs to fixtures/scratch (matching the prompt path)
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        prompt = "Use the patch agent to add a doc comment to the greet function in fixtures/scratch/lib.rs"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        # Should have added a doc comment
        assert_file_contains(dst_file, "///")

    def test_test_agent(self, runner: BrainproRunner):
        """Test subagent runs tests."""
        prompt = "Use the test agent to run the tests for fixtures/hello_repo"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_matches("(pass|PASSED|ok|OK)", result.output)
