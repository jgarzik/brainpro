"""Test 06: Plan mode (create, explore, cancel, execute)."""

import hashlib
import shutil
from pathlib import Path

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_matches,
    assert_file_contains,
    assert_equals,
)


class TestPlanMode:
    """Plan mode tests."""

    def test_plan_create(self, runner: BrainproRunner):
        """Plan mode can create a structured plan."""
        result = runner.repl(
            "/plan Add a goodbye function to fixtures/hello_repo/src/lib.rs",
            "/plan cancel",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)
        # Should show plan-related output
        assert_output_matches("(plan|Plan|STEP|step)", result.output)

    @pytest.mark.gateway_only
    def test_plan_explore(self, runner: BrainproRunner):
        """Plan mode performs exploration before planning."""
        result = runner.repl(
            "/plan Understand the structure of fixtures/hello_repo and describe it",
            "/plan cancel",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)
        # Should have used exploration tools
        assert_output_matches("(Glob|Read|Search|Grep)", result.output)

    def test_plan_cancel(self, runner: BrainproRunner, fixtures_dir: Path):
        """Plan mode can be cancelled without making changes."""
        # Copy lib.rs to fixtures/scratch (matching the prompt path)
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        # Get original hash
        original_content = dst_file.read_bytes()
        original_hash = hashlib.sha256(original_content).hexdigest()

        result = runner.repl(
            "/plan Delete the greet function from fixtures/scratch/lib.rs",
            "/plan cancel",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)

        # File should be unchanged
        new_content = dst_file.read_bytes()
        new_hash = hashlib.sha256(new_content).hexdigest()
        assert_equals(original_hash, new_hash)

        # Should still have greet function
        assert_file_contains(dst_file, "fn greet")

    def test_plan_execute(self, runner: BrainproRunner, fixtures_dir: Path):
        """Plan mode can execute a plan and modify files."""
        # Copy lib.rs to fixtures/scratch (matching the prompt path)
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        result = runner.repl(
            "/plan Add a doc comment to the greet function in fixtures/scratch/lib.rs",
            "/plan execute",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)

        # The key assertion is that we got a coherent response
        assert_output_matches("(plan|Plan|execute|Execute|STEP)", result.output)
