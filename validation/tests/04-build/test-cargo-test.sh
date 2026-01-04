#!/bin/bash
# Test: Agent can run cargo test
# Expected: Tests pass

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "cargo-test"

PROMPT='Run the tests for the project in fixtures/hello_repo and tell me if they pass'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_matches "(pass|PASSED|ok|OK)" "$OUTPUT"
assert_tool_called "Bash" "$OUTPUT"

report_result
