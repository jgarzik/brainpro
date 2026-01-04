#!/bin/bash
# Test: Scout subagent can explore codebase
# Expected: Returns file information

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "scout-agent"

PROMPT='Use the scout agent to find all the source files in fixtures/hello_repo and list them'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "lib.rs" "$OUTPUT"
assert_output_contains "main.rs" "$OUTPUT"

report_result
