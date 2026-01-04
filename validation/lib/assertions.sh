#!/bin/bash
# Assertion functions for yo validation tests
# All assertions set TEST_PASSED=0 on failure

# Assert exit code equals expected
# Usage: assert_exit_code expected actual
assert_exit_code() {
    local expected="$1"
    local actual="$2"

    if [ "$actual" -ne "$expected" ]; then
        echo "  ASSERT FAILED: Expected exit code $expected, got $actual" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Exit code is $expected" >> "$TEST_LOG"
    return 0
}

# Assert output contains string (case-insensitive)
# Usage: assert_output_contains "needle" "$output"
assert_output_contains() {
    local needle="$1"
    local haystack="$2"

    if ! echo "$haystack" | grep -qi "$needle"; then
        echo "  ASSERT FAILED: Output does not contain '$needle'" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Output contains '$needle'" >> "$TEST_LOG"
    return 0
}

# Assert output does NOT contain string (case-insensitive)
# Usage: assert_output_not_contains "needle" "$output"
assert_output_not_contains() {
    local needle="$1"
    local haystack="$2"

    if echo "$haystack" | grep -qi "$needle"; then
        echo "  ASSERT FAILED: Output contains '$needle' but should not" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Output does not contain '$needle'" >> "$TEST_LOG"
    return 0
}

# Assert output matches regex (case-insensitive, extended regex)
# Usage: assert_output_matches "pattern" "$output"
assert_output_matches() {
    local pattern="$1"
    local haystack="$2"

    if ! echo "$haystack" | grep -qiE "$pattern"; then
        echo "  ASSERT FAILED: Output does not match pattern '$pattern'" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Output matches pattern '$pattern'" >> "$TEST_LOG"
    return 0
}

# Assert file exists
# Usage: assert_file_exists "/path/to/file"
assert_file_exists() {
    local filepath="$1"

    if [ ! -f "$filepath" ]; then
        echo "  ASSERT FAILED: File does not exist: $filepath" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: File exists: $filepath" >> "$TEST_LOG"
    return 0
}

# Assert file does NOT exist
# Usage: assert_file_not_exists "/path/to/file"
assert_file_not_exists() {
    local filepath="$1"

    if [ -f "$filepath" ]; then
        echo "  ASSERT FAILED: File should not exist: $filepath" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: File does not exist: $filepath" >> "$TEST_LOG"
    return 0
}

# Assert file contains string
# Usage: assert_file_contains "/path/to/file" "needle"
assert_file_contains() {
    local filepath="$1"
    local needle="$2"

    if [ ! -f "$filepath" ]; then
        echo "  ASSERT FAILED: File does not exist: $filepath" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi

    if ! grep -q "$needle" "$filepath"; then
        echo "  ASSERT FAILED: File '$filepath' does not contain '$needle'" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: File '$filepath' contains '$needle'" >> "$TEST_LOG"
    return 0
}

# Assert file does NOT contain string
# Usage: assert_file_not_contains "/path/to/file" "needle"
assert_file_not_contains() {
    local filepath="$1"
    local needle="$2"

    if [ -f "$filepath" ] && grep -q "$needle" "$filepath"; then
        echo "  ASSERT FAILED: File '$filepath' contains '$needle' but should not" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: File '$filepath' does not contain '$needle'" >> "$TEST_LOG"
    return 0
}

# Assert directory exists
# Usage: assert_dir_exists "/path/to/dir"
assert_dir_exists() {
    local dirpath="$1"

    if [ ! -d "$dirpath" ]; then
        echo "  ASSERT FAILED: Directory does not exist: $dirpath" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Directory exists: $dirpath" >> "$TEST_LOG"
    return 0
}

# Assert command succeeded (exit code 0)
# Usage: assert_success $?
assert_success() {
    local exit_code="$1"

    if [ "$exit_code" -ne 0 ]; then
        echo "  ASSERT FAILED: Command failed with exit code $exit_code" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Command succeeded" >> "$TEST_LOG"
    return 0
}

# Assert command failed (non-zero exit code)
# Usage: assert_failure $?
assert_failure() {
    local exit_code="$1"

    if [ "$exit_code" -eq 0 ]; then
        echo "  ASSERT FAILED: Command succeeded but should have failed" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Command failed as expected (exit code $exit_code)" >> "$TEST_LOG"
    return 0
}

# Assert two strings are equal
# Usage: assert_equals "expected" "actual"
assert_equals() {
    local expected="$1"
    local actual="$2"

    if [ "$expected" != "$actual" ]; then
        echo "  ASSERT FAILED: Expected '$expected', got '$actual'" | tee -a "$TEST_LOG"
        TEST_PASSED=0
        return 1
    fi
    echo "  OK: Values are equal" >> "$TEST_LOG"
    return 0
}

# Assert output contains at least one of multiple patterns
# Usage: assert_output_contains_any "$output" "pattern1" "pattern2" ...
assert_output_contains_any() {
    local haystack="$1"
    shift
    local patterns=("$@")

    for pattern in "${patterns[@]}"; do
        if echo "$haystack" | grep -qi "$pattern"; then
            echo "  OK: Output contains '$pattern'" >> "$TEST_LOG"
            return 0
        fi
    done

    echo "  ASSERT FAILED: Output does not contain any of: ${patterns[*]}" | tee -a "$TEST_LOG"
    TEST_PASSED=0
    return 1
}

# Assert tool was called (check for tool display format)
# Usage: assert_tool_called "Read" "$output"
# Note: This is a best-effort check. LLM output may not always explicitly name tools.
assert_tool_called() {
    local tool_name="$1"
    local output="$2"

    # Look for the tool display format: ⏺ ToolName( or just tool indicators
    # Also check for result lines like "Read 14 lines" or "⎿"
    if echo "$output" | grep -qE "(⏺ ${tool_name}|^${tool_name} [0-9]|⎿.*${tool_name})"; then
        echo "  OK: Tool '$tool_name' was called" >> "$TEST_LOG"
        return 0
    fi

    # Check if any tools were called (Tools: N where N > 0)
    if echo "$output" | grep -qE "Tools: [1-9]"; then
        echo "  OK: At least one tool was called (expected '$tool_name')" >> "$TEST_LOG"
        return 0
    fi

    echo "  ASSERT FAILED: Tool '$tool_name' was not called (no tools detected)" | tee -a "$TEST_LOG"
    TEST_PASSED=0
    return 1
}

# Assert multiple tools were called
# Usage: assert_tools_called "$output" "Glob" "Read" "Search"
# Note: This checks if at least the expected number of tools were called
assert_tools_called() {
    local output="$1"
    shift
    local tools=("$@")
    local expected_count=${#tools[@]}

    # Check if the expected number of tools (or more) were called
    if echo "$output" | grep -qE "Tools: ([${expected_count}-9]|[1-9][0-9])"; then
        echo "  OK: At least $expected_count tools were called" >> "$TEST_LOG"
        return 0
    fi

    echo "  ASSERT FAILED: Expected at least $expected_count tools to be called" | tee -a "$TEST_LOG"
    TEST_PASSED=0
    return 1
}
