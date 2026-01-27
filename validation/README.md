# brainpro Manual Validation Framework

A comprehensive manual validation suite for testing `brainpro` functionality.

## Overview

This framework validates `brainpro` functionality by testing **outcomes** rather than exact LLM outputs. Since LLM responses are non-deterministic, tests focus on:

- File existence and content (did the expected file get created?)
- Tool invocation (was the right tool called?)
- Exit codes and error handling
- Semantic content (does the output contain expected keywords?)

## Prerequisites

1. **Build brainpro**:
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
| 10-tdd | Test-driven development | 3 | ~$0.10 |
| 11-debugging | Bug fixing & security review | 3 | ~$0.08 |
| 12-refactoring | Code modernization | 3 | ~$0.08 |
| 13-documentation | Doc generation | 3 | ~$0.08 |
| 14-git | Git operations | 3 | ~$0.08 |
| 15-multi-file | Cross-file changes | 3 | ~$0.12 |
| 16-review | Code review | 3 | ~$0.08 |
| 18-new-tools | TodoWrite, Session, PlanMode | 3 | ~$0.08 |

**Total estimated cost: ~$1.25 per full run** (52 tests)

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

# Run brainpro
PROMPT='Your prompt here'
OUTPUT=$(run_brainpro_oneshot "$PROMPT")
EXIT_CODE=$?

# Assertions
assert_exit_code 0 "$EXIT_CODE"
assert_output_contains "expected text" "$OUTPUT"
assert_file_exists "$SCRATCH_DIR/created-file.txt"

report_result
```

### Available Functions

**common.sh:**
- `run_brainpro_oneshot "prompt" [args...]` - Run brainpro in -p mode
- `run_brainpro_repl "cmd1" "cmd2" ...` - Run brainpro REPL with piped input
- `run_brainpro_in_mock_webapp "prompt" [args...]` - Run brainpro in mock_webapp_scratch
- `reset_scratch` - Clean fixtures/scratch directory
- `reset_mock_webapp` - Fresh copy of mock_webapp to scratch
- `cleanup_mock_webapp` - Remove mock_webapp_scratch
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
- `assert_cargo_test_passes "/path/to/project"`
- `assert_cargo_test_fails "/path/to/project"`
- `assert_single_test_passes "/path" "test_name"`
- `assert_single_test_fails "/path" "test_name"`
- `assert_git_clean "/path/to/repo"`
- `assert_git_dirty "/path/to/repo"`
- `assert_git_has_commits "/path" min_count`

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

- `fixtures/hello_repo/` - Simple Rust project for basic tests
- `fixtures/mock_webapp/` - Full web app with intentional issues for advanced tests:
  - Security issue: hardcoded API key in `auth.rs`
  - Deprecated function: `old_query()` in `database.rs`
  - Validation bug: `@.` passes email validation
  - Undocumented functions in `handlers.rs`
  - One intentionally failing test
- `fixtures/mock_webapp_scratch/` - Ephemeral copy for tests that mutate the webapp
- `fixtures/scratch/` - Ephemeral directory for test artifacts (reset each test)
- `fixtures/agents/` - Subagent configurations
- `fixtures/mcp_calc_server/` - MCP server for integration tests

## Troubleshooting

### Test fails with "brainpro binary not found"
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
