#!/bin/bash
# Test: Write tool can create new files
# Expected: File exists with expected content

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "write-basic"
reset_scratch

PROMPT='Create a file at fixtures/scratch/test.txt containing exactly the text "validation test passed"'

OUTPUT=$(run_yo_oneshot "$PROMPT" --mode acceptEdits)
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_file_exists "$SCRATCH_DIR/test.txt"
assert_file_contains "$SCRATCH_DIR/test.txt" "validation"
assert_tool_called "Write" "$OUTPUT"

report_result
