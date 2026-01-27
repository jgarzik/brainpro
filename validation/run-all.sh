#!/bin/bash
# Run all brainpro validation tests (Docker Mode)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source common functions
source "$SCRIPT_DIR/lib/common.sh"
source "$SCRIPT_DIR/lib/assertions.sh"
source "$SCRIPT_DIR/lib/docker.sh"

echo "=== brainpro Validation Suite (Docker Mode) ==="
echo ""

# Track Docker state for cleanup
DOCKER_STARTED=0

cleanup() {
    if [ $DOCKER_STARTED -eq 1 ]; then
        echo ""
        echo "Cleaning up Docker..."
        stop_docker
    fi
}
trap cleanup EXIT INT TERM

# Check prerequisites
check_brainpro_binary
check_api_key

# Start Docker (API keys passed via environment variables)
start_docker || { echo "Failed to start Docker"; exit 1; }
DOCKER_STARTED=1

# Export gateway URL for test functions
export BRAINPRO_GATEWAY_URL="ws://localhost:18789/ws"

echo "Results will be saved to: $RESULTS_DIR"
echo ""

# Categories to run (in order)
# Note: 20-gateway-basic and 21-agent-daemon removed - Docker handles startup
CATEGORIES=(
    "01-tools"
    "02-exploration"
    "03-editing"
    "04-build"
    "05-agent-loop"
    "06-plan-mode"
    "07-subagents"
    "08-permissions"
    "09-errors"
    "10-tdd"
    "11-debugging"
    "12-refactoring"
    "13-documentation"
    "14-git"
    "15-multi-file"
    "16-review"
    "18-new-tools"
)

# Option to run specific category
if [ -n "$1" ]; then
    CATEGORIES=("$1")
    echo "Running category: $1"
    echo ""
fi

# Track counts in this script (tests run in subshells)
LOCAL_PASSED=0
LOCAL_FAILED=0

# Run tests by category
for category in "${CATEGORIES[@]}"; do
    category_dir="$SCRIPT_DIR/tests/$category"

    if [ ! -d "$category_dir" ]; then
        echo "Category not found: $category"
        continue
    fi

    echo "=== Category: $category ==="

    # Find and run all test scripts
    for test_script in "$category_dir"/test-*.sh; do
        if [ -f "$test_script" ]; then
            # Make executable if not already
            chmod +x "$test_script"

            # Run the test and track result
            if bash "$test_script"; then
                ((LOCAL_PASSED++))
            else
                ((LOCAL_FAILED++))
            fi
        fi
    done

    echo ""
done

# Print summary
echo ""
echo "=========================================="
echo "Validation Summary"
echo "=========================================="
echo "Passed: $LOCAL_PASSED"
echo "Failed: $LOCAL_FAILED"
echo "Total:  $((LOCAL_PASSED + LOCAL_FAILED))"
echo ""
echo "Results saved to: $RESULTS_DIR"

# Write summary file
mkdir -p "$RESULTS_DIR"
{
    echo "Validation Run: $(date)"
    echo "Passed: $LOCAL_PASSED"
    echo "Failed: $LOCAL_FAILED"
    echo "Total:  $((LOCAL_PASSED + LOCAL_FAILED))"
} > "${RESULTS_DIR}/summary.txt"

# Exit with appropriate code
if [ $LOCAL_FAILED -gt 0 ]; then
    exit 1
fi
exit 0
