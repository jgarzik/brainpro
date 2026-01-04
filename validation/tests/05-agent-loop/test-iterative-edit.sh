#!/bin/bash
# Test: Agent can perform iterative edits across multiple turns
# Expected: File reflects both edits after completion

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "iterative-edit"
reset_scratch

# Create initial file
cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

# Run REPL with two edit requests
OUTPUT=$(run_yo_repl \
    "In fixtures/scratch/lib.rs, change the function name from greet to say_hello" \
    "Now add a new function called farewell that returns the string \"Goodbye!\"" \
    "/exit")
EXIT_CODE=$?

# Note: This test may fail if the LLM doesn't get permission or times out
# The key is that BOTH edits should be present

assert_exit_code 0 "$EXIT_CODE"
assert_file_contains "$SCRATCH_DIR/lib.rs" "say_hello"
assert_file_contains "$SCRATCH_DIR/lib.rs" "farewell"
assert_file_contains "$SCRATCH_DIR/lib.rs" "Goodbye"

report_result
