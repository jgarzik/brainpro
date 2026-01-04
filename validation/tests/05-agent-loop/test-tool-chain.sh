#!/bin/bash
# Test: Agent can chain multiple tools to complete a task
# Expected: Glob -> Read -> summarize workflow

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "tool-chain"

# Task requiring multiple tools
PROMPT='Find all Rust files in fixtures/hello_repo, read them, and tell me how many functions are defined in total'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should have used Glob or similar to find files
assert_output_matches "(Glob|Search)" "$OUTPUT"
# Should have used Read to read files
assert_tool_called "Read" "$OUTPUT"
# Should provide a count (at least greet function)
assert_output_matches "[0-9]+" "$OUTPUT"

report_result
