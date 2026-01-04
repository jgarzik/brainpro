#!/bin/bash
# Test: Agent can create a new file with a function
# Expected: File exists with function definition

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "create-function"
reset_scratch

PROMPT='Create a new Rust file at fixtures/scratch/math.rs with a function called "add" that takes two i32 arguments and returns their sum'

OUTPUT=$(run_yo_oneshot "$PROMPT" --mode acceptEdits --yes)
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_file_exists "$SCRATCH_DIR/math.rs"
assert_file_contains "$SCRATCH_DIR/math.rs" "fn add"
assert_file_contains "$SCRATCH_DIR/math.rs" "i32"

report_result
