#!/bin/bash
# Test: Plan mode performs exploration before planning
# Expected: Glob/Read calls visible during planning phase

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "plan-explore"

# Create a plan - exploration should happen first
OUTPUT=$(run_yo_repl \
    "/plan Understand the structure of fixtures/hello_repo and describe it" \
    "/plan cancel" \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should have used exploration tools
assert_output_matches "(Glob|Read|Search|Grep)" "$OUTPUT"

report_result
