# Brainpro multi-stage Docker build
# Runs both gateway and agent daemons via supervisord

FROM rust:1.83-slim-bookworm as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build release binaries
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run /var/log/supervisor

# Copy binaries from builder
COPY --from=builder /app/target/release/brainpro-gateway /usr/local/bin/
COPY --from=builder /app/target/release/brainpro-agent /usr/local/bin/
COPY --from=builder /app/target/release/brainpro /usr/local/bin/

# Copy supervisord config
COPY supervisord.conf /etc/supervisor/conf.d/brainpro.conf

# Expose gateway port
EXPOSE 18789

# Set working directory for agent
WORKDIR /workspace

# Run supervisord
CMD ["supervisord", "-n", "-c", "/etc/supervisor/supervisord.conf"]
