# yo Manual Validation Framework

A comprehensive manual validation suite for testing `yo` against common Claude Code usage patterns.

## Overview

This framework validates `yo` functionality by testing **outcomes** rather than exact LLM outputs. Since LLM responses are non-deterministic, tests focus on:

- File existence and content (did the expected file get created?)
- Tool invocation (was the right tool called?)
- Exit codes and error handling
- Semantic content (does the output contain expected keywords?)

## Prerequisites

1. **Build yo**:
   ```bash
   cargo build --release
   ```

2. **Set API key** (Venice is default):
   ```bash
   export VENICE_API_KEY="your-key-here"
   # Or: export ANTHROPIC_API_KEY="..."
   # Or: export OPENAI_API_KEY="..."
   ```

3. **Run setup** (optional - run-all.sh does this automatically):
   ```bash
   ./validation/setup.sh
   ```

## Running Tests

### Run All Tests
```bash
./validation/run-all.sh
```

### Run Specific Category
```bash
./validation/run-all.sh 01-tools
./validation/run-all.sh 05-agent-loop
./validation/run-all.sh 06-plan-mode
```

### Run Single Test
```bash
./validation/tests/01-tools/test-read.sh
```

## Test Categories

| Category | Description | Tests | Est. Cost |
|----------|-------------|-------|-----------|
| 01-tools | Basic tool operations | 6 | ~$0.05 |
| 02-exploration | Codebase understanding | 3 | ~$0.03 |
| 03-editing | File creation/modification | 2 | ~$0.04 |
| 04-build | cargo build/test | 2 | ~$0.03 |
| **05-agent-loop** | **CORE: Multi-turn REPL** | 4 | ~$0.15 |
| **06-plan-mode** | **CORE: Plan workflow** | 4 | ~$0.12 |
| 07-subagents | Task delegation | 3 | ~$0.10 |
| 08-permissions | Policy enforcement | 2 | ~$0.02 |
| 09-errors | Error handling | 2 | ~$0.02 |

**Total estimated cost: ~$0.56 per full run**

## Test Priority

For quick validation, run in this order:

1. `01-tools` - Verify basic tool operations work
2. `05-agent-loop` - Validate multi-turn conversations (CORE)
3. `06-plan-mode` - Validate plan mode workflow (CORE)

## Writing New Tests

### Test Structure
```bash
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "my-test-name"

# Optional: reset scratch directory
reset_scratch

# Run yo
PROMPT='Your prompt here'
OUTPUT=$(run_yo_oneshot "$PROMPT")
EXIT_CODE=$?

# Assertions
assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "expected text" "$OUTPUT"
assert_file_exists "$SCRATCH_DIR/created-file.txt"

report_result
```

### Available Functions

**common.sh:**
- `run_yo_oneshot "prompt" [args...]` - Run yo in -p mode
- `run_yo_repl "cmd1" "cmd2" ...` - Run yo REPL with piped input
- `reset_scratch` - Clean fixtures/scratch directory
- `setup_test "name"` - Initialize test
- `report_result` - Print PASS/FAIL

**assertions.sh:**
- `assert_exit_code expected actual`
- `assert_output_contains "needle" "$output"` (case-insensitive)
- `assert_output_matches "regex" "$output"`
- `assert_output_not_contains "needle" "$output"`
- `assert_file_exists "/path/to/file"`
- `assert_file_not_exists "/path/to/file"`
- `assert_file_contains "/path" "needle"`
- `assert_file_not_contains "/path" "needle"`
- `assert_tool_called "ToolName" "$output"`
- `assert_tools_called "$output" "Tool1" "Tool2" ...`

## Results

Results are saved to `validation/results/YYYY-MM-DD-HHMMSS/`:
- `summary.txt` - Pass/fail counts
- `test-name.log` - Individual test logs with full output

## Design Principles

1. **Bound inputs tightly**: Use specific fixtures and prompts
2. **Bound outcomes loosely**: Check for presence, not exact match
3. **Case-insensitive**: LLM capitalization varies
4. **Multiple valid patterns**: Use regex alternation `(pass|ok|success)`
5. **Idempotent**: Each test resets its scratch state

## Fixtures

- `fixtures/hello_repo/` - Simple Rust project for testing
- `fixtures/scratch/` - Ephemeral directory for test artifacts (reset each test)
- `fixtures/agents/` - Subagent configurations
- `fixtures/mcp_calc_server/` - MCP server for integration tests

## Troubleshooting

### Test fails with "yo binary not found"
```bash
cargo build --release
```

### Test fails with "No API key"
```bash
export VENICE_API_KEY="your-key"
```

### Test times out
Some tests may take 30-60 seconds due to LLM response time.

### Test is flaky
LLM outputs are non-deterministic. If a test fails:
1. Check the log file in `results/`
2. Consider loosening assertions (add more valid patterns)
3. Ensure the prompt is specific enough
