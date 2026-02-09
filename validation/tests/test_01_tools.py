"""Test 01: Basic tool operations (Read, Write, Edit, Bash, Glob, Grep, Patch)."""

import shutil
from pathlib import Path

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
    assert_file_exists,
    assert_file_contains,
    assert_file_not_contains,
    assert_tool_called,
)


class TestTools:
    """Basic tool operation tests."""

    def test_read_basic(self, runner: BrainproRunner):
        """Read tool can read file contents."""
        prompt = "Read the file fixtures/hello_repo/src/lib.rs and tell me what function it defines"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("greet", result.output)
        assert_tool_called("Read", result.output)

    def test_write_basic(self, runner: BrainproRunner, fixtures_dir: Path):
        """Write tool can create new files."""
        prompt = 'Create a file at fixtures/scratch/test.txt containing exactly the text "validation test passed"'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_file_exists(fixtures_dir / "scratch" / "test.txt")
        assert_file_contains(fixtures_dir / "scratch" / "test.txt", "validation")
        assert_tool_called("Write", result.output)

    def test_edit_basic(self, runner: BrainproRunner, fixtures_dir: Path):
        """Edit tool can modify existing files."""
        # Create initial file by copying from fixture
        src_file = fixtures_dir / "hello_repo" / "src" / "lib.rs"
        dst_file = fixtures_dir / "scratch" / "lib.rs"
        shutil.copy(src_file, dst_file)

        prompt = 'In fixtures/scratch/lib.rs, change the TODO comment to say "greeting implemented"'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_file_contains(dst_file, "greeting implemented")
        assert_file_not_contains(dst_file, "TODO: add proper greeting")
        assert_tool_called("Edit", result.output)

    def test_bash_basic(self, runner: BrainproRunner):
        """Bash tool executes shell commands."""
        prompt = 'Run the command "ls fixtures/hello_repo/src" and tell me what files are there'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("lib.rs", result.output)
        assert_output_contains("main.rs", result.output)
        assert_tool_called("Bash", result.output)

    def test_glob_basic(self, runner: BrainproRunner):
        """Glob tool finds files by pattern."""
        prompt = "List all Rust source files (*.rs) in fixtures/hello_repo"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("lib.rs", result.output)
        assert_output_contains("main.rs", result.output)
        assert_tool_called("Glob", result.output)

    def test_grep_basic(self, runner: BrainproRunner):
        """Grep/Search tool searches file contents."""
        prompt = 'Search for the word "greet" in fixtures/hello_repo/src'

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("lib.rs", result.output)
        # Accept either Grep or Search tool (Search is the primary search tool)
        assert "Grep" in result.output or "Search" in result.output, \
            f"Neither Grep nor Search tool was called\n\nOutput:\n{result.output[:2000]}"

    def test_patch_basic(self, runner: BrainproRunner, fixtures_dir: Path):
        """Patch tool can apply unified diffs."""
        # Create initial file
        example_file = fixtures_dir / "scratch" / "example.txt"
        example_file.write_text("line 1\nline 2\nline 3\nline 4\n")

        prompt = '''Read fixtures/scratch/changes.patch and apply its contents to the target file using the Patch tool. The patch file contains:

--- a/fixtures/scratch/example.txt
+++ b/fixtures/scratch/example.txt
@@ -1,4 +1,5 @@
 line 1
+inserted line
 line 2
 line 3
 line 4

Use the Patch tool with this exact patch content and path "fixtures/scratch/example.txt"'''

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_file_contains(example_file, "inserted line")
        assert_tool_called("Patch", result.output)
