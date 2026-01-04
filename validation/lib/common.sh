#!/bin/bash
# Common functions for yo validation tests

# Paths - use unique variable names to avoid conflicts
_COMMON_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VALIDATION_DIR="$(dirname "$_COMMON_SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$VALIDATION_DIR")"
YO_BIN="${PROJECT_ROOT}/target/release/yo"
FIXTURES_DIR="${PROJECT_ROOT}/fixtures"
SCRATCH_DIR="${FIXTURES_DIR}/scratch"

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

# Run yo in one-shot mode (-p)
# Usage: OUTPUT=$(run_yo_oneshot "prompt" [additional args...])
# Note: Always uses --yes to skip permission prompts for automated testing
run_yo_oneshot() {
    local prompt="$1"
    shift
    local args=("$@")

    echo "Command: $YO_BIN -p \"$prompt\" --yes ${args[*]}" >> "$TEST_LOG"

    # Run yo and capture both stdout and stderr
    # Always use --yes to auto-approve for testing
    local output
    output=$("$YO_BIN" -p "$prompt" --yes "${args[@]}" 2>&1)
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Run yo in REPL mode with piped commands
# Usage: OUTPUT=$(run_yo_repl "command1" "command2" ...)
run_yo_repl() {
    local commands=""
    for cmd in "$@"; do
        commands+="$cmd"$'\n'
    done

    echo "REPL Commands:" >> "$TEST_LOG"
    echo "$commands" >> "$TEST_LOG"

    local output
    output=$(echo "$commands" | "$YO_BIN" 2>&1)
    local exit_code=$?

    echo "Exit code: $exit_code" >> "$TEST_LOG"
    echo "Output:" >> "$TEST_LOG"
    echo "$output" >> "$TEST_LOG"
    echo "---" >> "$TEST_LOG"

    echo "$output"
    return $exit_code
}

# Run yo REPL with expect script (for complex interactions)
# Usage: OUTPUT=$(run_yo_repl_expect "script.exp")
run_yo_repl_expect() {
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
    rm -rf "$SCRATCH_DIR"
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

# Check if yo binary exists
check_yo_binary() {
    if [ ! -x "$YO_BIN" ]; then
        echo -e "${RED}ERROR${NC}: yo binary not found at $YO_BIN"
        echo "Run: cargo build --release"
        exit 1
    fi
}

# Check if API key is set
check_api_key() {
    if [ -z "$VENICE_API_KEY" ] && [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$OPENAI_API_KEY" ]; then
        echo -e "${YELLOW}WARNING${NC}: No API key found (VENICE_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY)"
        echo "Tests may fail without an API key"
    fi
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
