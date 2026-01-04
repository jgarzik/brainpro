#!/bin/bash
# Test: Agent can run cargo build
# Expected: Build succeeds

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "cargo-build"

PROMPT='Build the Rust project in fixtures/hello_repo using cargo build --release'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_not_contains "error\[E" "$OUTPUT"  # No compiler errors
assert_tool_called "Bash" "$OUTPUT"

report_result
