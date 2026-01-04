#!/bin/bash
# Test: Read tool can read file contents
# Expected: Output contains content from fixtures/hello_repo/src/lib.rs

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "read-basic"

PROMPT='Read the file fixtures/hello_repo/src/lib.rs and tell me what function it defines'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "greet" "$OUTPUT"
assert_tool_called "Read" "$OUTPUT"

report_result
