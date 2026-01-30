# syntax=docker/dockerfile:1

# =============================================================================
# Stage 1: Chef - Install cargo-chef for dependency caching
# =============================================================================
FROM ubuntu:24.04 AS chef
RUN apt-get update && apt-get install -y \
    curl build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
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

# Copy recipe and build dependencies first (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build the application
COPY . .
RUN cargo build --release

# =============================================================================
# Stage 4: Runtime - Ubuntu 24.04 with full development environment
# =============================================================================
FROM ubuntu:24.04

# Create non-root user
RUN groupadd -r brainpro && useradd -r -g brainpro brainpro

# Install full development environment
RUN apt-get update && apt-get install -y \
    build-essential \
    git ssh wget curl vim jq \
    python3 python3-pip \
    nodejs npm \
    cmake gdb strace net-tools \
    supervisor ca-certificates \
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
