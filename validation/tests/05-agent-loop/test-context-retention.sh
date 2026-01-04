#!/bin/bash
# Test: Agent retains context from earlier in conversation
# Expected: Agent remembers information from first turn in second turn

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "context-retention"

# Run REPL - first establish a fact, then ask about it
OUTPUT=$(run_yo_repl \
    "Read fixtures/hello_repo/src/lib.rs and remember what the test function is called" \
    "What was the name of the test function you just read?" \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should remember test_greet from the first read
assert_output_contains "test_greet" "$OUTPUT"

report_result
