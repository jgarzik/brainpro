# syntax=docker/dockerfile:1

# =============================================================================
# Stage 1: Chef - Install cargo-chef for dependency caching
# =============================================================================
FROM rust:1.88-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# =============================================================================
# Stage 2: Planner - Generate recipe.json (dependency manifest)
# =============================================================================
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Builder - Build dependencies (cached), then build application
# =============================================================================
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy recipe and build dependencies first (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build the application
COPY . .
RUN cargo build --release

# =============================================================================
# Stage 4: Runtime - Minimal image with supervisor for gateway+agent
# =============================================================================
FROM debian:bookworm-slim

# Create non-root user
RUN groupadd -r brainpro && useradd -r -g brainpro brainpro

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run /var/log/supervisor /app/data /app/logs /app/workspace /app/scratch /app/data/.brainpro \
    && chown -R brainpro:brainpro /app /run /var/log/supervisor

# Copy binaries from builder
COPY --from=builder /app/target/release/brainpro-gateway /usr/local/bin/
COPY --from=builder /app/target/release/brainpro-agent /usr/local/bin/
COPY --from=builder /app/target/release/brainpro /usr/local/bin/

# Copy supervisord config
COPY supervisord.conf /etc/supervisor/conf.d/brainpro.conf

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Expose gateway port
EXPOSE 18789

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:18789/health || exit 1

# Set working directory
WORKDIR /app

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["supervisord", "-n", "-c", "/etc/supervisor/supervisord.conf"]
