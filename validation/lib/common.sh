#!/bin/bash
# Common functions for brainpro validation tests

# Paths - use unique variable names to avoid conflicts
_COMMON_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VALIDATION_DIR="$(dirname "$_COMMON_SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$VALIDATION_DIR")"
BRAINPRO_BIN="${PROJECT_ROOT}/target/release/brainpro"
FIXTURES_DIR="${PROJECT_ROOT}/fixtures"
SCRATCH_DIR="${FIXTURES_DIR}/scratch"

# Gateway URL (set by run-all.sh when using Docker mode)
BRAINPRO_GATEWAY_URL="${BRAINPRO_GATEWAY_URL:-}"

# Results tracking
RESULTS_DIR="${VALIDATION_DIR}/results/$(date +%Y-%m-%d-%H%M%S)"
CURRENT_TEST=""
TEST_PASSED=1
TOTAL_PASSED=0
TOTAL_FAILED=0
TEST_LOG=""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Initialize a test
# Usage: setup_test "test-name"
setup_test() {
    CURRENT_TEST="$1"
    TEST_PASSED=1
    mkdir -p "$RESULTS_DIR"
    TEST_LOG="${RESULTS_DIR}/${CURRENT_TEST}.log"
    echo "=== Test: $CURRENT_TEST ===" | tee "$TEST_LOG"
    echo "Started: $(date)" >> "$TEST_LOG"
}

# Run brainpro in one-shot mode (-p)
# Usage: OUTPUT=$(run_brainpro_oneshot "prompt" [additional args...])
# Note: Always uses --yes to skip permission prompts for automated testing
run_brainpro_oneshot() {
    local prompt="$1"
    shift
    local args=("$@")

    # Run brainpro and capture both stdout and stderr
    # Always use --yes to auto-approve for testing
    local output
    if [ -n "$BRAINPRO_GATEWAY_URL" ]; then
        echo "Command: $BRAINPRO_BIN --gateway $BRAINPRO_GATEWAY_URL -p \"$prompt\" --yes ${args[*]}" >> "$TEST_LOG"
        output=$("$BRAINPRO_BIN" --gateway "$BRAINPRO_GATEWAY_URL" -p "$prompt" --yes "${args[@]}" 2>&1)
    else
        echo "Command: $BRAINPRO_BIN -p \"$prompt\" --yes ${args[*]}" >> "$TEST_LOG"
        output=$("$BRAINPRO_BIN" -p "$prompt" --yes "${args[@]}" 2>&1)
    fi
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Run brainpro in REPL mode with piped commands
# Usage: OUTPUT=$(run_brainpro_repl "command1" "command2" ...)
# Note: Always uses --yes to skip permission prompts for automated testing
run_brainpro_repl() {
    local commands=""
    for cmd in "$@"; do
        commands+="$cmd"$'\n'
    done

    echo "REPL Commands:" >> "$TEST_LOG"
    echo "$commands" >> "$TEST_LOG"

    local output
    if [ -n "$BRAINPRO_GATEWAY_URL" ]; then
        echo "Command: $BRAINPRO_BIN --gateway $BRAINPRO_GATEWAY_URL --yes (REPL)" >> "$TEST_LOG"
        output=$(echo "$commands" | "$BRAINPRO_BIN" --gateway "$BRAINPRO_GATEWAY_URL" --yes 2>&1)
    else
        echo "Command: $BRAINPRO_BIN --yes (REPL)" >> "$TEST_LOG"
        output=$(echo "$commands" | "$BRAINPRO_BIN" --yes 2>&1)
    fi
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Run brainpro REPL with expect script (for complex interactions)
# Usage: OUTPUT=$(run_brainpro_repl_expect "script.exp")
run_brainpro_repl_expect() {
    local script="$1"

    if ! command -v expect &> /dev/null; then
        echo "SKIP: expect not installed" >> "$TEST_LOG"
        echo "SKIP: expect not installed"
        return 77  # Skip exit code
    fi

    echo "Expect script: $script" >> "$TEST_LOG"

    local output
    output=$(expect "$script" 2>&1)
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Reset the scratch directory
reset_scratch() {
    # Use docker to clean files that may be owned by container user (uid 999)
    docker run --rm -v "$SCRATCH_DIR:/scratch" alpine sh -c \
        "rm -rf /scratch/* /scratch/.[!.]* 2>/dev/null; chown -R 1000:1000 /scratch" 2>/dev/null || true
    rm -rf "$SCRATCH_DIR" 2>/dev/null || true
    mkdir -p "$SCRATCH_DIR"
    echo "Scratch directory reset: $SCRATCH_DIR" >> "$TEST_LOG"
}

# Report test result
report_result() {
    echo "" >> "$TEST_LOG"
    if [ $TEST_PASSED -eq 1 ]; then
        echo -e "${GREEN}PASS${NC}: $CURRENT_TEST"
        echo "Result: PASS" >> "$TEST_LOG"
        ((TOTAL_PASSED++))
        return 0
    else
        echo -e "${RED}FAIL${NC}: $CURRENT_TEST (see $TEST_LOG)"
        echo "Result: FAIL" >> "$TEST_LOG"
        ((TOTAL_FAILED++))
        return 1
    fi
}

# Print final summary
print_summary() {
    echo ""
    echo "=========================================="
    echo "Validation Summary"
    echo "=========================================="
    echo "Passed: $TOTAL_PASSED"
    echo "Failed: $TOTAL_FAILED"
    echo "Total:  $((TOTAL_PASSED + TOTAL_FAILED))"
    echo ""
    echo "Results saved to: $RESULTS_DIR"

    # Write summary file
    {
        echo "Validation Run: $(date)"
        echo "Passed: $TOTAL_PASSED"
        echo "Failed: $TOTAL_FAILED"
        echo "Total:  $((TOTAL_PASSED + TOTAL_FAILED))"
    } > "${RESULTS_DIR}/summary.txt"

    if [ $TOTAL_FAILED -gt 0 ]; then
        return 1
    fi
    return 0
}

# Check if brainpro binary exists
check_brainpro_binary() {
    if [ ! -x "$BRAINPRO_BIN" ]; then
        echo -e "${RED}ERROR${NC}: brainpro binary not found at $BRAINPRO_BIN"
        echo "Run: cargo build --release"
        exit 1
    fi
}

# Check if API key is set (env var or secrets file)
check_api_key() {
    local secrets_dir="${PROJECT_ROOT}/secrets"
    # Check environment variables
    if [ -n "$VENICE_API_KEY" ] || [ -n "$ANTHROPIC_API_KEY" ] || [ -n "$OPENAI_API_KEY" ]; then
        return 0
    fi
    # Check secrets files
    if [ -s "$secrets_dir/venice_api_key.txt" ] || \
       [ -s "$secrets_dir/anthropic_api_key.txt" ] || \
       [ -s "$secrets_dir/openai_api_key.txt" ]; then
        return 0
    fi
    echo -e "${YELLOW}WARNING${NC}: No API key found (VENICE_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY)"
    echo "Tests may fail without an API key"
}

# Mock webapp fixture directory
MOCK_WEBAPP_DIR="${FIXTURES_DIR}/mock_webapp"
MOCK_WEBAPP_SCRATCH="${FIXTURES_DIR}/mock_webapp_scratch"

# Reset the mock_webapp scratch directory
# Creates a fresh git repo copy of mock_webapp for testing
reset_mock_webapp() {
    rm -rf "$MOCK_WEBAPP_SCRATCH"
    cp -r "$MOCK_WEBAPP_DIR" "$MOCK_WEBAPP_SCRATCH"

    # Initialize a git repo in the scratch copy for testing
    (
        cd "$MOCK_WEBAPP_SCRATCH" && \
        git init -q && \
        git add . && \
        git commit -q -m "Initial commit" && \
        git config user.email "test@example.com" && \
        git config user.name "Test User"
    ) >> "$TEST_LOG" 2>&1

    echo "Mock webapp scratch reset: $MOCK_WEBAPP_SCRATCH" >> "$TEST_LOG"
}

# Clean up mock_webapp scratch directory
cleanup_mock_webapp() {
    rm -rf "$MOCK_WEBAPP_SCRATCH"
    echo "Mock webapp scratch cleaned up" >> "$TEST_LOG"
}

# Run brainpro in the mock_webapp scratch directory
# Usage: OUTPUT=$(run_brainpro_in_mock_webapp "prompt" [additional args...])
run_brainpro_in_mock_webapp() {
    local prompt="$1"
    shift
    local args=("$@")

    local output
    if [ -n "$BRAINPRO_GATEWAY_URL" ]; then
        echo "Command: cd $MOCK_WEBAPP_SCRATCH && $BRAINPRO_BIN --gateway $BRAINPRO_GATEWAY_URL -p \"$prompt\" --yes ${args[*]}" >> "$TEST_LOG"
        output=$(cd "$MOCK_WEBAPP_SCRATCH" && "$BRAINPRO_BIN" --gateway "$BRAINPRO_GATEWAY_URL" -p "$prompt" --yes "${args[@]}" 2>&1)
    else
        echo "Command: cd $MOCK_WEBAPP_SCRATCH && $BRAINPRO_BIN -p \"$prompt\" --yes ${args[*]}" >> "$TEST_LOG"
        output=$(cd "$MOCK_WEBAPP_SCRATCH" && "$BRAINPRO_BIN" -p "$prompt" --yes "${args[@]}" 2>&1)
    fi
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Wait for user confirmation (for expensive tests)
confirm_run() {
    local cost="$1"
    echo -e "${YELLOW}This test will cost approximately $cost${NC}"
    read -p "Continue? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Skipped by user"
        return 1
    fi
    return 0
}
