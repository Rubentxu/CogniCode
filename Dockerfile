# CogniCode MCP — Container image
#
# Build:
#   docker build -t cognicode-mcp .
#
# Run (single project):
#   docker run -e COGNICODE_MCP_AUTH_TOKEN=<secret> -p 9847:9847 -v /path/to/project:/workspace:ro cognicode-mcp
#
# The Explorer API is not part of this image: it lives in
# Rubentxu/cognicode-explorer, which builds its own container.

FROM rust:1.96.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build only the CogniCode MCP binaries (skip test/dev crates):
# cognicode-mcp (stdio) and cognicode-mcp-server (HTTP/SSE)
RUN cargo build --release -p cognicode-mcp --bin cognicode-mcp --bin cognicode-mcp-server \
    && strip target/release/cognicode-mcp target/release/cognicode-mcp-server

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cognicode-mcp /usr/local/bin/
COPY --from=builder /build/target/release/cognicode-mcp-server /usr/local/bin/

# Default environment
ENV RUST_LOG=info

# Entrypoint: checks the auth token, then starts the server
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 9847

ENTRYPOINT ["docker-entrypoint.sh"]
