#!/bin/bash
# Test: Default mode blocks writes without --yes
# Expected: File is NOT created

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "deny-write"
reset_scratch

# Run WITHOUT --yes flag, pipe 'n' to decline permission
OUTPUT=$(echo "n" | run_yo_oneshot "Write 'test' to fixtures/scratch/blocked.txt")
EXIT_CODE=$?

# File should NOT exist (user declined or permission blocked)
assert_file_not_exists "$SCRATCH_DIR/blocked.txt"

report_result
