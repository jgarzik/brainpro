#!/bin/bash
# Test: Agent can perform multiple edits in one request
# Expected: Both changes present in file

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "multi-edit"
reset_scratch

cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

PROMPT='In fixtures/scratch/lib.rs: 1) Rename the function from "greet" to "hello" 2) Add a new function called "farewell" that returns "Goodbye, World!"'

OUTPUT=$(run_yo_oneshot "$PROMPT" --mode acceptEdits --yes)
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_file_contains "$SCRATCH_DIR/lib.rs" "hello"
assert_file_contains "$SCRATCH_DIR/lib.rs" "farewell"
assert_file_contains "$SCRATCH_DIR/lib.rs" "Goodbye"

report_result
