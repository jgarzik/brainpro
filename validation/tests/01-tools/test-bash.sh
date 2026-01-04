#!/bin/bash
# Test: Bash tool executes shell commands
# Expected: Command output is captured and returned

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "bash-basic"

PROMPT='Run the command "ls fixtures/hello_repo/src" and tell me what files are there'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "lib.rs" "$OUTPUT"
assert_output_contains "main.rs" "$OUTPUT"
assert_tool_called "Bash" "$OUTPUT"

report_result
