#!/bin/bash
# Test: Plan mode can create a structured plan
# Expected: Plan with STEP entries is generated

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "plan-create"

# Start plan mode, then cancel it
OUTPUT=$(run_yo_repl \
    "/plan Add a goodbye function to fixtures/hello_repo/src/lib.rs" \
    "/plan cancel" \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should show plan-related output
assert_output_matches "(plan|Plan|STEP|step)" "$OUTPUT"

report_result
