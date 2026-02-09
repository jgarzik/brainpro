"""Test 03: Code editing (create functions, multi-edit)."""

import shutil
from pathlib import Path

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_file_exists,
    assert_file_contains,
)


class TestEditing:
    """Code editing tests."""

    def test_create_function(self, runner: BrainproRunner, fixtures_dir: Path):
        """Agent can create a new file with a function."""
        prompt = 'Create a new Rust file at fixtures/scratch/math.rs with a function called "add" that takes two i32 arguments and returns their sum'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_file_exists(fixtures_dir / "scratch" / "math.rs")
        assert_file_contains(fixtures_dir / "scratch" / "math.rs", "fn add")
        assert_file_contains(fixtures_dir / "scratch" / "math.rs", "i32")

    def test_multi_edit(self, runner: BrainproRunner, fixtures_dir: Path):
        """Agent can perform multiple edits in one request."""
        # Copy lib.rs to scratch
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        prompt = 'In fixtures/scratch/lib.rs: 1) Rename the function from "greet" to "hello" 2) Add a new function called "farewell" that returns "Goodbye, World!"'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_file_contains(dst_file, "hello")
        assert_file_contains(dst_file, "farewell")
        assert_file_contains(dst_file, "Goodbye")
