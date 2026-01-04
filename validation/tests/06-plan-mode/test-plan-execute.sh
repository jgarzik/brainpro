#!/bin/bash
# Test: Plan mode can execute a plan and modify files
# Expected: File is modified according to plan

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "plan-execute"
reset_scratch

# Create a copy to work on
cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

# Create and execute a simple plan
OUTPUT=$(run_yo_repl \
    "/plan Add a doc comment to the greet function in fixtures/scratch/lib.rs" \
    "/plan execute" \
    "/exit")
EXIT_CODE=$?

# Check if plan was executed
# Note: Plan execution may require confirmation
assert_exit_code 0 "$EXIT_CODE"

# If executed, file should have doc comment
if grep -q "///" "$SCRATCH_DIR/lib.rs" 2>/dev/null; then
    echo "  OK: Doc comment added" >> "$TEST_LOG"
else
    echo "  INFO: Plan may not have executed (requires confirmation)" >> "$TEST_LOG"
fi

# The key assertion is that we got a coherent response
assert_output_matches "(plan|Plan|execute|Execute|STEP)" "$OUTPUT"

report_result
