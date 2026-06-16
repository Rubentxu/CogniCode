# CogniCode MCP — Self-contained container image (Mode B)
#
# Build:
#   docker build -t cognicode-mcp .
#
# Run (single project):
#   docker run -p 5432:5432 -v /path/to/project:/workspace:ro cognicode-mcp
#
# Run (multi-project, with Explorer API):
#   docker run -p 5432:5432 -p 8010:8010 -v /home/projects:/workspaces:ro cognicode-mcp --multi

FROM rust:1.82-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build only cognicode-mcp and explorer-api (skip test/dev crates)
RUN cargo build --release -p cognicode-mcp -p cognicode-runtime --bin explorer-api \
    && strip target/release/cognicode-mcp target/release/explorer-api

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-16 ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cognicode-mcp /usr/local/bin/
COPY --from=builder /build/target/release/explorer-api /usr/local/bin/

# PostgreSQL data directory
ENV PGDATA=/var/lib/postgresql/data
RUN mkdir -p "$PGDATA" && chown -R postgres:postgres "$PGDATA"

# Default environment
ENV DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode
ENV RUST_LOG=info

# Entrypoint: start PG, run migrations, then start both binaries
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 5432 8010

ENTRYPOINT ["docker-entrypoint.sh"]
