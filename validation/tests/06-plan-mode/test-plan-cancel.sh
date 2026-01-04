#!/bin/bash
# Test: Plan mode can be cancelled without making changes
# Expected: No modifications made when cancelled

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "plan-cancel"
reset_scratch

# Create a copy to work on
cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

# Get original content hash
ORIGINAL_HASH=$(shasum "$SCRATCH_DIR/lib.rs" | cut -d' ' -f1)

# Create a plan but cancel it
OUTPUT=$(run_yo_repl \
    "/plan Delete the greet function from fixtures/scratch/lib.rs" \
    "/plan cancel" \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"

# File should be unchanged
NEW_HASH=$(shasum "$SCRATCH_DIR/lib.rs" | cut -d' ' -f1)
assert_equals "$ORIGINAL_HASH" "$NEW_HASH"

# Should still have greet function
assert_file_contains "$SCRATCH_DIR/lib.rs" "fn greet"

report_result
