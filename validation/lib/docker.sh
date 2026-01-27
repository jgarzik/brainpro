#!/bin/bash
# Docker lifecycle helpers for validation

GATEWAY_URL="ws://localhost:18789/ws"
HEALTH_URL="http://localhost:18789/health"

# Generate secrets files and override yaml for API keys
# Uses environment variables if set, otherwise uses existing secret files
setup_secrets() {
    local compose_file="${PROJECT_ROOT}/docker-compose.yml"
    local override_file="${PROJECT_ROOT}/docker-compose.override.yml"
    local secrets_dir="${PROJECT_ROOT}/secrets"
    mkdir -p "$secrets_dir"

    local gateway_token_file="$secrets_dir/brainpro_gateway_token.txt"
    if [ ! -s "$gateway_token_file" ]; then
        if command -v openssl >/dev/null 2>&1; then
            openssl rand -hex 32 > "$gateway_token_file"
        else
            date +%s%N > "$gateway_token_file"
        fi
        chmod 600 "$gateway_token_file"
    fi

    # Map of env var name -> secret file name
    declare -A api_keys=(
        ["VENICE_API_KEY"]="venice_api_key"
        ["OPENAI_API_KEY"]="openai_api_key"
        ["ANTHROPIC_API_KEY"]="anthropic_api_key"
    )

    local secrets_yaml=""
    local service_secrets=""

    for env_var in "${!api_keys[@]}"; do
        local secret_name="${api_keys[$env_var]}"
        local secret_file="$secrets_dir/${secret_name}.txt"
        local value="${!env_var}"

        # If env var is set, write it to secret file
        if [ -n "$value" ]; then
            echo -n "$value" > "$secret_file"
            chmod 600 "$secret_file"
        elif [ ! -f "$secret_file" ]; then
            : > "$secret_file"
            chmod 600 "$secret_file"
        fi

        # If secret file exists (from env var or pre-existing), add to override
        if [ -f "$secret_file" ]; then
            secrets_yaml+="  ${secret_name}:
    file: ./secrets/${secret_name}.txt
"
            service_secrets+="      - ${secret_name}
"
        fi
    done

    # Generate override file only if we have secrets
    if [ -n "$secrets_yaml" ]; then
        cat > "$override_file" <<EOF
services:
  brainpro:
    secrets:
${service_secrets}
secrets:
${secrets_yaml}
EOF
    else
        rm -f "$override_file"
    fi
}

# Wait for health endpoint
wait_for_health() {
    local timeout="${1:-60}"
    local start=$(date +%s)

    echo "Waiting for gateway health..."
    while true; do
        if curl -sf "$HEALTH_URL" > /dev/null 2>&1; then
            echo "Gateway is healthy"
            return 0
        fi

        local now=$(date +%s)
        if [ $((now - start)) -ge $timeout ]; then
            echo "Timeout waiting for gateway health"
            return 1
        fi
        sleep 1
    done
}

# Clean workspace directory (needed before each validation run)
clean_workspace() {
    local workspace_dir="${PROJECT_ROOT}/workspace"
    local scratch_dir="${PROJECT_ROOT}/fixtures/scratch"
    # Use docker to clean with proper permissions (files may be owned by container user)
    docker run --rm -v "$workspace_dir:/ws" -v "$scratch_dir:/scratch" alpine sh -c \
        "rm -rf /ws/* /ws/.[!.]* 2>/dev/null; chown -R 1000:1000 /ws; rm -rf /scratch/* /scratch/.[!.]* 2>/dev/null; chown -R 1000:1000 /scratch" 2>/dev/null || true
}

# Start Docker services
start_docker() {
    setup_secrets
    echo "Starting Docker services..."
    # Clean workspace before starting to ensure fresh state
    clean_workspace
    # Run from PROJECT_ROOT so docker compose auto-detects override file
    (cd "$PROJECT_ROOT" && docker compose up -d --build)
    wait_for_health 60
}

# Stop Docker services
stop_docker() {
    local override_file="${PROJECT_ROOT}/docker-compose.override.yml"
    echo "Stopping Docker services..."
    (cd "$PROJECT_ROOT" && docker compose down)
    rm -f "$override_file"
}
