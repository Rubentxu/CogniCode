#!/bin/bash
# CogniCode MCP — Docker entrypoint
# Starts PostgreSQL, runs migrations, then launches cognicode-mcp

set -e

echo "🐘 Starting PostgreSQL..."
su - postgres -c "pg_ctl -D $PGDATA -l /var/log/postgresql.log start" || {
    # First run: initialize the database
    su - postgres -c "pg_ctl -D $PGDATA initdb"
    su - postgres -c "pg_ctl -D $PGDATA -l /var/log/postgresql.log start"
}

# Wait for PG to be ready
until su - postgres -c "pg_isready" > /dev/null 2>&1; do
    echo "⏳ Waiting for PostgreSQL..."
    sleep 1
done

# Create database and user if needed
su - postgres -c "psql -tc \"SELECT 1 FROM pg_roles WHERE rolname='cognicode'\"" | grep -q 1 || {
    su - postgres -c "psql -c \"CREATE USER cognicode WITH PASSWORD 'cognicode';\""
    su - postgres -c "psql -c \"CREATE DATABASE cognicode OWNER cognicode;\""
    su - postgres -c "psql -c \"GRANT ALL PRIVILEGES ON DATABASE cognicode TO cognicode;\""
}
echo "✅ PostgreSQL ready"

# If --multi flag, start Explorer API (multi-project mode)
if [ "$1" = "--multi" ]; then
    echo "🚀 Starting Explorer API (multi-project) on :8010"
    exec /usr/local/bin/explorer-api --listen 0.0.0.0:8010 --cwd /workspaces &
fi

# Start cognicode-mcp-server (HTTP/SSE mode on port 9847)
PROJECT_DIR="${COGNICODE_PROJECT:-/workspace}"
echo "🚀 Starting CogniCode MCP HTTP/SSE Server on :9847 for $PROJECT_DIR"
exec /usr/local/bin/cognicode-mcp-server --cwd "$PROJECT_DIR" --listen 0.0.0.0:9847 --postgres "$DATABASE_URL"
