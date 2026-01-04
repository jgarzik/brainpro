#!/bin/bash
# Test: Agent can understand file structure
# Expected: Output identifies greet function

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "understand-structure"

PROMPT='What functions are defined in fixtures/hello_repo/src/lib.rs?'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "greet" "$OUTPUT"

report_result
