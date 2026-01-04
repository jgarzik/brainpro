#!/bin/bash
# Test: Glob tool finds files by pattern
# Expected: Output lists .rs files from fixtures/hello_repo

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "glob-basic"

PROMPT='List all Rust source files (*.rs) in fixtures/hello_repo'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "lib.rs" "$OUTPUT"
assert_output_contains "main.rs" "$OUTPUT"
assert_tool_called "Glob" "$OUTPUT"

report_result
