#!/bin/bash
set -e

# Load Docker secrets and inject into supervisord config
SECRETS_ENV=""

for secret in VENICE_API_KEY OPENAI_API_KEY ANTHROPIC_API_KEY BRAINPRO_GATEWAY_TOKEN; do
    file="/run/secrets/$(echo $secret | tr '[:upper:]' '[:lower:]')"
    if [ -f "$file" ]; then
        value="$(cat $file)"
        export "$secret"="$value"
        # Build comma-separated env string for supervisord
        if [ -n "$SECRETS_ENV" ]; then
            SECRETS_ENV="${SECRETS_ENV},"
        fi
        SECRETS_ENV="${SECRETS_ENV}${secret}=\"${value}\""
    fi
done

# Inject secrets into agent's environment line in supervisord config
if [ -n "$SECRETS_ENV" ]; then
    sed -i "s|environment=HOME=\"/app\",BRAINPRO_DATA_DIR=\"/app/data\"|environment=HOME=\"/app\",BRAINPRO_DATA_DIR=\"/app/data\",${SECRETS_ENV}|" \
        /etc/supervisor/conf.d/brainpro.conf
fi

# Fix permissions for brainpro user
chown -R brainpro:brainpro /app /run /var/log/supervisor 2>/dev/null || true

exec "$@"
