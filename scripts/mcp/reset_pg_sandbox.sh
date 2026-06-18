#!/bin/bash
# Reset PG sandbox schema for CogniCode
# Drops legacy tables and applies full new schema (m0010_pipeline_schema.sql)
#
# Usage: ./reset_pg_sandbox.sh [db_name] [db_user] [container_name]
# Defaults: db_name=cognicode, db_user=cognicode, container_name=cognicode-postgres
#
# Prereq: psql must be available in the container (postgres base image has it)
set -e

DB_NAME="${1:-cognicode}"
DB_USER="${2:-cognicode}"
CONTAINER_NAME="${3:-cognicode-postgres}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA_FILE="$SCRIPT_DIR/../../crates/cognicode-core/src/infrastructure/persistence/m0010_pipeline_schema.sql"

echo "=== Resetting PG sandbox ==="
echo "Database: $DB_NAME, User: $DB_USER, Container: $CONTAINER_NAME"

# Step 1: Drop legacy tables (if they exist as tables, not views)
echo "--- Dropping legacy tables ---"
podman exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
DROP TABLE IF EXISTS call_edges CASCADE;
DROP TABLE IF EXISTS symbols CASCADE;
DROP TABLE IF EXISTS graph_edges CASCADE;
DROP TABLE IF EXISTS graph_nodes CASCADE;
DROP TABLE IF EXISTS scan_manifest CASCADE;
DROP TABLE IF EXISTS graph_reports CASCADE;
DROP VIEW IF EXISTS call_edges CASCADE;
DROP VIEW IF EXISTS symbols CASCADE;
DROP TRIGGER IF EXISTS graph_nodes_notify ON graph_nodes;
DROP FUNCTION IF EXISTS notify_graph_change();
" 2>&1

echo "--- Applying new schema (m0010_pipeline_schema.sql) ---"
podman exec -i "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" < "$SCHEMA_FILE" 2>&1

echo "--- Verifying ---"
podman exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -c '\dt' 2>&1

echo "=== Schema reset complete ==="
