#!/bin/bash
# Test: EnterPlanMode and ExitPlanMode tools
# Expected: Agent can enter and exit plan mode via tools

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "plan-mode-tools"

PROMPT='Use the EnterPlanMode tool to enter planning mode, then describe the structure of fixtures/hello_repo, then use ExitPlanMode to exit.'

OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"
# Should call either tool or mention plan mode
assert_output_matches "(EnterPlanMode|ExitPlanMode|plan.?mode|planning)" "$OUTPUT"

report_result
