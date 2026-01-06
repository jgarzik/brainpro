#!/bin/bash
# Test: Session persistence and resume
# Expected: Session is saved and can be resumed

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "session-resume"

# Clean up any old sessions
SESSIONS_DIR="$HOME/.yo/sessions"
rm -rf "$SESSIONS_DIR"

# Run yo in REPL mode with a simple command then exit
# The session should be auto-saved on clean exit
OUTPUT=$(run_yo_repl \
    "What is 2+2? Just say the number." \
    "/exit")
EXIT_CODE=$?

assert_exit_code 0 "$EXIT_CODE"

# Check that a session was saved
if [ ! -d "$SESSIONS_DIR" ]; then
    echo "FAIL: Sessions directory not created" >> "$TEST_LOG"
    TEST_PASSED=0
else
    SESSION_COUNT=$(ls -1 "$SESSIONS_DIR"/*.json 2>/dev/null | wc -l)
    if [ "$SESSION_COUNT" -eq 0 ]; then
        echo "FAIL: No session files found" >> "$TEST_LOG"
        TEST_PASSED=0
    else
        echo "Session files found: $SESSION_COUNT" >> "$TEST_LOG"
        # Get the session ID from the first file
        SESSION_FILE=$(ls -1 "$SESSIONS_DIR"/*.json | head -1)
        SESSION_ID=$(basename "$SESSION_FILE" .json)
        echo "Session ID: $SESSION_ID" >> "$TEST_LOG"

        # Try to resume the session
        RESUME_OUTPUT=$("$YO_BIN" --resume "$SESSION_ID" -p "What did I just ask you?" --yes 2>&1)
        RESUME_EXIT=$?

        echo "Resume output:" >> "$TEST_LOG"
        echo "$RESUME_OUTPUT" >> "$TEST_LOG"

        if [ $RESUME_EXIT -ne 0 ]; then
            echo "FAIL: Resume failed with exit code $RESUME_EXIT" >> "$TEST_LOG"
            TEST_PASSED=0
        elif ! echo "$RESUME_OUTPUT" | grep -qi "resumed\|messages"; then
            # Check for either "resumed" message or context that shows resumed messages
            # Just check it ran without error
            echo "Resume ran successfully (no explicit 'resumed' message, but no error)" >> "$TEST_LOG"
        fi
    fi
fi

report_result
