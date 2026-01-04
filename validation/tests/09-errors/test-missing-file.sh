#!/bin/bash
# Test: Missing file is handled gracefully
# Expected: Error message, no crash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "missing-file"

PROMPT='Read the file fixtures/hello_repo/nonexistent.rs'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

# Should complete without crash
assert_exit_code 0 "$EXIT_CODE"
# Should indicate file not found
assert_output_matches "(not found|doesn't exist|error|Error)" "$OUTPUT"

report_result
