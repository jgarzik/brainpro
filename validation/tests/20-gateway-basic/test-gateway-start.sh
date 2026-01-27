#!/bin/bash
# Test: Gateway starts and binds to port
set -e

PORT="${TEST_PORT:-18799}"
GATEWAY_BIN="${BRAINPRO_GATEWAY:-./target/release/brainpro-gateway}"
AGENT_SOCKET="/tmp/brainpro-test-gw-$$.sock"

# Start gateway in background (no agent needed for basic start test)
timeout 5 "$GATEWAY_BIN" --port "$PORT" --agent-socket "$AGENT_SOCKET" &
GW_PID=$!
sleep 1

# Check port is listening
if ! nc -z localhost "$PORT" 2>/dev/null; then
    echo "FAIL: Gateway not listening on port $PORT"
    kill $GW_PID 2>/dev/null
    exit 1
fi

echo "PASS: Gateway listening on port $PORT"

# Cleanup
kill $GW_PID 2>/dev/null
exit 0
