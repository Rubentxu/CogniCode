# ADR-020: Workspace-Scoped Schema + scan_manifest

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** Critical review — multi-workspace gap

## Context

The existing schema has `workspace_id` on `named_views` and `view_specs` but
**not** on `symbols`, `call_edges`, `graph_nodes`, or `graph_edges`. This
means the graph tables are global — only one workspace's graph can exist in
the database at a time. The ingest pipeline needs workspace isolation to
support multiple projects.

Additionally, the pipeline needs a `scan_manifest` table for incremental
change detection (ADR-017). This table must also be workspace-scoped.

## Decision

Add `workspace_id TEXT NOT NULL` to all graph tables and create the
`scan_manifest` table, all workspace-scoped.

```sql
-- Migration: add workspace_id to existing tables
ALTER TABLE graph_nodes ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE graph_edges ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
CREATE INDEX idx_graph_nodes_workspace ON graph_nodes(workspace_id);
CREATE INDEX idx_graph_edges_workspace ON graph_edges(workspace_id);

-- New table: scan_manifest
CREATE TABLE IF NOT EXISTS scan_manifest (
    workspace_id  TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    file_type     TEXT NOT NULL,
    language      TEXT,
    content_hash  TEXT NOT NULL,
    mtime         DOUBLE PRECISION NOT NULL,
    symbol_count  INTEGER NOT NULL DEFAULT 0,
    edge_count    INTEGER NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'ok',
    error_msg     TEXT,
    scanned_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, file_path)
);

CREATE INDEX idx_scan_manifest_workspace ON scan_manifest(workspace_id);
```

The `workspace_id` is a stable string derived from the workspace root path
(same `workspace_id()` function already in `facades/workspace.rs:77`).

## Rationale

- **Multi-workspace.** Multiple projects can share one PG database. Each
  workspace's graph is isolated by `workspace_id`. All queries filter by it.
- **`status` + `error_msg`** on `scan_manifest` support per-file error
  isolation (ADR-024). Failed files are tracked, not silently dropped.
- **`mtime` column** enables the mtime-first optimization (ADR-017) without
  an extra `stat()` round trip — the last-seen mtime is in the manifest.

## Consequences

- All graph queries must filter by `workspace_id`. The pipeline, Explorer, and
  MCP tools all receive the workspace context.
- The existing `save_call_graph()` (global delete-and-replace) is incompatible.
  It is replaced by the pipeline's per-file PgUpsert.
- Migration of existing data: the `DEFAULT 'default'` backfills existing rows.

## Alternatives Considered

- **Separate PG schemas per workspace:** more isolated but harder to query
  across workspaces and complicates connection pooling.
- **Separate PG databases per workspace:** maximum isolation but operationally
  expensive. Overkill for v1.
