#!/bin/bash
# Setup script for yo validation tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== yo Validation Setup ==="
echo ""

# Build yo release binary
echo "Building yo (release)..."
cd "$PROJECT_ROOT"
cargo build --release
echo "  Done: target/release/yo"

# Build MCP calc server (optional, for MCP tests)
if [ -d "$PROJECT_ROOT/fixtures/mcp_calc_server" ]; then
    echo "Building mcp-calc-server..."
    cd "$PROJECT_ROOT/fixtures/mcp_calc_server"
    cargo build --release 2>/dev/null || echo "  Warning: mcp-calc-server build failed (optional)"
    echo "  Done: fixtures/mcp_calc_server/target/release/mcp-calc"
fi

# Create directories
echo "Creating directories..."
mkdir -p "$SCRIPT_DIR/results"
mkdir -p "$PROJECT_ROOT/fixtures/scratch"
echo "  Done"

# Check API key
echo ""
echo "Checking API key..."
if [ -n "$VENICE_API_KEY" ]; then
    echo "  Found: VENICE_API_KEY"
elif [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "  Found: ANTHROPIC_API_KEY"
elif [ -n "$OPENAI_API_KEY" ]; then
    echo "  Found: OPENAI_API_KEY"
else
    echo "  WARNING: No API key found!"
    echo "  Set VENICE_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY"
fi

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Run tests with:"
echo "  ./validation/run-all.sh           # Run all tests"
echo "  ./validation/tests/01-tools/test-read.sh  # Run single test"
