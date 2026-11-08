# CogniCode MCP — Container image
#
# Build:
#   docker build -t cognicode-mcp .
#
# Run (single project):
#   docker run -e COGNICODE_MCP_AUTH_TOKEN=<secret> -p 9847:9847 -v /path/to/project:/workspace:ro cognicode-mcp
#
# Run (multi-project, with Explorer API):
#   docker run -e COGNICODE_MCP_AUTH_TOKEN=<secret> -p 9847:9847 -p 8010:8010 -v /home/projects:/workspaces:ro cognicode-mcp --multi

FROM rust:1.82-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build only cognicode-mcp and explorer-api (skip test/dev crates)
# Build all binaries: cognicode-mcp (stdio), cognicode-mcp-server (HTTP/SSE), explorer-api
RUN cargo build --release -p cognicode-mcp -p cognicode-runtime --bin explorer-api --bin cognicode-mcp-server \
    && strip target/release/cognicode-mcp target/release/explorer-api target/release/cognicode-mcp-server

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cognicode-mcp /usr/local/bin/
COPY --from=builder /build/target/release/cognicode-mcp-server /usr/local/bin/
COPY --from=builder /build/target/release/explorer-api /usr/local/bin/

# Default environment
ENV RUST_LOG=info

# Entrypoint: checks the auth token, then starts the server
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 8010 9847

ENTRYPOINT ["docker-entrypoint.sh"]
