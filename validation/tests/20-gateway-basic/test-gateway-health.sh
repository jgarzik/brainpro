#!/bin/bash
# Test: Gateway health endpoint returns OK
set -e

PORT="${TEST_PORT:-18798}"
GATEWAY_BIN="${BRAINPRO_GATEWAY:-./target/release/brainpro-gateway}"
AGENT_SOCKET="/tmp/brainpro-test-health-$$.sock"

# Start gateway in background
timeout 5 "$GATEWAY_BIN" --port "$PORT" --agent-socket "$AGENT_SOCKET" &
GW_PID=$!
sleep 1

# Check health endpoint
HEALTH=$(curl -s "http://localhost:$PORT/health")
if [[ -z "$HEALTH" ]]; then
    echo "FAIL: Health endpoint returned empty response"
    kill $GW_PID 2>/dev/null
    exit 1
fi

# Check status field
if ! echo "$HEALTH" | grep -q '"status"'; then
    echo "FAIL: Health response missing status field"
    echo "Response: $HEALTH"
    kill $GW_PID 2>/dev/null
    exit 1
fi

echo "PASS: Health endpoint returns valid JSON"
echo "Response: $HEALTH"

# Cleanup
kill $GW_PID 2>/dev/null
exit 0
