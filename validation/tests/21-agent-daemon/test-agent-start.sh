#!/bin/bash
# Test: Agent daemon starts and creates socket
set -e

SOCKET_PATH="/tmp/brainpro-test-$$.sock"
AGENT_BIN="${BRAINPRO_AGENT:-./target/release/brainpro-agent}"

# Start agent in background
timeout 5 "$AGENT_BIN" --socket "$SOCKET_PATH" &
AGENT_PID=$!
sleep 1

# Check socket exists
if [[ ! -S "$SOCKET_PATH" ]]; then
    echo "FAIL: Socket not created at $SOCKET_PATH"
    kill $AGENT_PID 2>/dev/null
    exit 1
fi

echo "PASS: Agent started, socket created at $SOCKET_PATH"

# Cleanup
kill $AGENT_PID 2>/dev/null
rm -f "$SOCKET_PATH"
exit 0
