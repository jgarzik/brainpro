#!/bin/bash
set -e

# Load Docker secrets into environment (12-factor app pattern)
# supervisor -n runs in foreground, inherits our exported env
for secret in VENICE_API_KEY OPENAI_API_KEY ANTHROPIC_API_KEY BRAINPRO_GATEWAY_TOKEN; do
    file="/run/secrets/$(echo $secret | tr '[:upper:]' '[:lower:]')"
    [ -f "$file" ] && export "$secret"="$(cat $file)"
done

exec "$@"
