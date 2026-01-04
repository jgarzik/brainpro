#!/bin/bash
# Test: Agent can find tests in codebase
# Expected: Output identifies test_greet

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "find-tests"

PROMPT='Find the tests defined in fixtures/hello_repo and list their names'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "test_greet" "$OUTPUT"

report_result
