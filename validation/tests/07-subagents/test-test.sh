#!/bin/bash
# Test: Test subagent runs tests
# Expected: Test results reported

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "test-agent"

PROMPT='Use the test agent to run the tests for fixtures/hello_repo'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_matches "(pass|PASSED|ok|OK)" "$OUTPUT"

report_result
