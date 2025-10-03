# Multi-stage build for optimized image size
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary
RUN cargo build --release --bin cardano-node

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create cardano user
RUN useradd -r -s /bin/false -u 1000 cardano

# Create directories
RUN mkdir -p /data/db /data/config /data/logs && \
    chown -R cardano:cardano /data

# Copy binary from builder
COPY --from=builder /app/target/release/cardano-node /usr/local/bin/cardano-node

# Set permissions
RUN chmod +x /usr/local/bin/cardano-node

# Switch to cardano user
USER cardano

# Set working directory
WORKDIR /data

# Expose ports
# 3001: Node-to-node communication
# 12798: Prometheus metrics (EKG)
EXPOSE 3001 12798

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
  CMD curl -f http://localhost:12798/metrics || exit 1

# Default command
ENTRYPOINT ["/usr/local/bin/cardano-node"]
CMD ["run", "--config", "/data/config/config.json", "--topology", "/data/config/topology.json", "--database-path", "/data/db"]
