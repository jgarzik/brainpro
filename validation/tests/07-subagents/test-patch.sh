#!/bin/bash
# Test: Patch subagent can edit files
# Expected: File is modified

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "patch-agent"
reset_scratch

cp "$FIXTURES_DIR/hello_repo/src/lib.rs" "$SCRATCH_DIR/lib.rs"

PROMPT='Use the patch agent to add a doc comment to the greet function in fixtures/scratch/lib.rs'

OUTPUT=$(run_yo_oneshot "$PROMPT" --mode acceptEdits --yes)
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should have added a doc comment (///)
assert_file_contains "$SCRATCH_DIR/lib.rs" "///"

report_result
