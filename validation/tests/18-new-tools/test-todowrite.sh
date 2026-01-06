#!/bin/bash
# Test: TodoWrite tool creates task list
# Expected: Agent uses TodoWrite tool to track tasks

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "todowrite-basic"

PROMPT='Create a todo list with exactly 3 tasks for adding a greeting function to fixtures/hello_repo. Use the TodoWrite tool.'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
assert_tool_called "TodoWrite" "$OUTPUT"
# Should show task display box
assert_output_matches "(Tasks|pending|in_progress|completed)" "$OUTPUT"

report_result
