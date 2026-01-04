#!/bin/bash
# Test: Edit tool can modify existing files
# Expected: File contains modified content

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "edit-basic"
reset_scratch

# Create initial file by copying from fixture
cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

PROMPT='In fixtures/scratch/lib.rs, change the TODO comment to say "greeting implemented"'

OUTPUT=$(run_yo_oneshot "$PROMPT" --mode acceptEdits)
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_file_contains "$SCRATCH_DIR/lib.rs" "greeting implemented"
assert_file_not_contains "$SCRATCH_DIR/lib.rs" "TODO: add proper greeting"
assert_tool_called "Edit" "$OUTPUT"

report_result
