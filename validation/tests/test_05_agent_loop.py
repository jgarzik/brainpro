"""Test 05: Agent loop (multi-turn, tool chains, context retention, iterative edit)."""

import shutil
from pathlib import Path

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
    assert_output_matches,
    assert_file_contains,
    assert_tool_called,
)


class TestAgentLoop:
    """Agent loop tests."""

    def test_multi_turn(self, runner: BrainproRunner):
        """Multi-turn conversation works correctly."""
        result = runner.repl(
            "What is the name of the function in fixtures/hello_repo/src/lib.rs?",
            "What does that function return?",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)
        # First question should identify "greet"
        assert_output_contains("greet", result.output)
        # Second question should mention the return type or value
        assert_output_matches("(String|Hello|greeting)", result.output)

    def test_tool_chain(self, runner: BrainproRunner):
        """Agent can chain multiple tools to complete a task."""
        prompt = "Find all Rust files in fixtures/hello_repo, read them, and tell me how many functions are defined in total"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        # Should have used Glob or similar to find files
        assert_output_matches("(Glob|Search)", result.output)
        # Should have used Read to read files
        assert_tool_called("Read", result.output)
        # Should provide a count
        assert_output_matches("[0-9]+", result.output)

    def test_context_retention(self, runner: BrainproRunner):
        """Agent retains context from earlier in conversation."""
        result = runner.repl(
            "Read fixtures/hello_repo/src/lib.rs and remember what the test function is called",
            "What was the name of the test function you just read?",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)
        # Should remember test_greet from the first read
        assert_output_contains("test_greet", result.output)

    def test_iterative_edit(
        self, runner: BrainproRunner, fixtures_dir: Path
    ):
        """Agent can perform iterative edits across multiple turns."""
        # Copy lib.rs to fixtures/scratch (matching the prompt path)
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        result = runner.repl(
            "In fixtures/scratch/lib.rs, change the function name from greet to say_hello",
            'Now add a new function called farewell that returns the string "Goodbye!"',
            "/exit",
        )

        assert_exit_code(0, result.exit_code)
        assert_file_contains(dst_file, "say_hello")
        assert_file_contains(dst_file, "farewell")
        assert_file_contains(dst_file, "Goodbye")
