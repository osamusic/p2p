# Build stage
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev sqlite-dev

# Create app directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY tests ./tests

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache ca-certificates sqlite

# Create non-root user
RUN addgroup -g 1000 p2psync && \
    adduser -D -u 1000 -G p2psync p2psync

# Copy the binary from builder
COPY --from=builder /app/target/release/p2p-sync /usr/local/bin/p2p-sync

# Create data and config directories
RUN mkdir -p /data /config && \
    chown p2psync:p2psync /data /config

# Copy documentation
COPY --chown=p2psync:p2psync README.md LICENSE /app/
COPY --chown=p2psync:p2psync docs /app/docs/

# Add health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD p2p-sync --version || exit 1

# Switch to non-root user
USER p2psync

# Set data directory
WORKDIR /data

# Expose default ports
EXPOSE 4001/tcp
EXPOSE 4001/udp

# Default command
ENTRYPOINT ["p2p-sync"]
CMD ["start"]