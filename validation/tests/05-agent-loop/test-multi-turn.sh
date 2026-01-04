#!/bin/bash
# Test: Multi-turn conversation works correctly
# Expected: Agent maintains context across multiple turns

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "multi-turn"

# Run REPL with two related questions
OUTPUT=$(run_yo_repl \
    "What is the name of the function in fixtures/hello_repo/src/lib.rs?" \
    "What does that function return?" \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# First question should identify "greet"
assert_output_contains "greet" "$OUTPUT"
# Second question should mention the return type or value (String, Hello)
assert_output_matches "(String|Hello|greeting)" "$OUTPUT"

report_result
