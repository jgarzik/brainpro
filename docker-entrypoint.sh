#!/bin/bash
set -e

# Load Docker secrets into environment (12-factor app pattern)
# supervisor -n runs in foreground, inherits our exported env
for secret in VENICE_API_KEY OPENAI_API_KEY ANTHROPIC_API_KEY BRAINPRO_GATEWAY_TOKEN; do
    file="/run/secrets/$(echo $secret | tr '[:upper:]' '[:lower:]')"
    [ -f "$file" ] && export "$secret"="$(cat $file)"
done

# Fix permissions for workspace - allow brainpro user to write
# (bind mount from host may have different ownership)
if [ -d /app/workspace ]; then
    chown -R brainpro:brainpro /app/workspace 2>/dev/null || true
fi

# Fix permissions for scratch directory (mounted at /app/scratch)
if [ -d /app/scratch ]; then
    chown -R brainpro:brainpro /app/scratch 2>/dev/null || true
fi

# Create fixtures directory structure in workspace with symlinks
# This allows paths like "fixtures/scratch/file.txt" to work correctly
# while scratch is mounted separately for write access
rm -rf /app/workspace/fixtures 2>/dev/null || true
mkdir -p /app/workspace/fixtures

# Link read-only fixture dirs
for dir in agents hello_repo mock_webapp mock_webapp_scratch; do
    ln -sf /app/fixtures/$dir /app/workspace/fixtures/$dir 2>/dev/null || true
done

# Link scratch to writable mount at /app/scratch
ln -sf /app/scratch /app/workspace/fixtures/scratch

# Fix ownership of the fixtures directory structure we just created
chown -h brainpro:brainpro /app/workspace/fixtures /app/workspace/fixtures/* 2>/dev/null || true

exec "$@"
