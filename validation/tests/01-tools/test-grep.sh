#!/bin/bash
# Test: Grep tool searches file contents
# Expected: Output shows matches for pattern in fixtures/hello_repo

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "grep-basic"

PROMPT='Search for the word "greet" in fixtures/hello_repo/src'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "lib.rs" "$OUTPUT"
# Verify a tool was used
assert_tool_called "Grep" "$OUTPUT"

report_result
