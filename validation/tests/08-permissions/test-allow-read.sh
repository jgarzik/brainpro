#!/bin/bash
# Test: Read operations are allowed by default
# Expected: File content is returned

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "allow-read"

PROMPT='Read fixtures/hello_repo/src/lib.rs'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should return content without permission prompt
assert_output_contains "greet" "$OUTPUT"
assert_tool_called "Read" "$OUTPUT"

report_result
