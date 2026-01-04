#!/bin/bash
# Test: Agent can describe a codebase
# Expected: Output identifies it as a Rust project

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "describe-codebase"

PROMPT='Describe the project in fixtures/hello_repo. What kind of project is it?'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "Rust" "$OUTPUT"

report_result
