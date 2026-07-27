-- Migration m0017: Graph Revisions Table
--
-- Part of e28-0-canonical-graph-revisions: PR1 Foundation.
-- Creates the `graph_revisions` table that tracks monotonic revision IDs
-- per workspace. Every ingest commit opens a new revision.
--
-- Idempotent: uses IF NOT EXISTS / DO blocks guarded by information_schema checks.
--
-- Schema:
--   graph_revisions(workspace_id TEXT, revision_id BIGINT, created_at TIMESTAMPTZ, head_of BOOLEAN, PK(workspace_id, revision_id))
--   idx_graph_revisions_head (UNIQUE WHERE head_of=true)

-- =============================================================================
-- 1. Create graph_revisions table
-- =============================================================================
CREATE TABLE IF NOT EXISTS graph_revisions (
    workspace_id  TEXT NOT NULL DEFAULT 'default',
    revision_id  BIGINT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    head_of      BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (workspace_id, revision_id)
);

-- Index for efficient head lookup by workspace
CREATE INDEX IF NOT EXISTS idx_graph_revisions_workspace
    ON graph_revisions(workspace_id);

-- =============================================================================
-- 2. Partial unique index: at most one head revision per workspace
-- Only one row with head_of=true per workspace_id.
-- Uses a unique index with a WHERE clause to enforce this constraint.
-- =============================================================================
-- First, drop the old index if it exists (idempotent).
DROP INDEX IF EXISTS idx_graph_revisions_head;
-- Then create the partial unique index.
-- The index entry is only added when head_of=true, so at most one such entry
-- can exist per workspace_id.
CREATE UNIQUE INDEX idx_graph_revisions_head
    ON graph_revisions(workspace_id)
    WHERE head_of = true;
