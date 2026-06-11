-- Migration `m0010_spaces` — Federation (brain-federation) tables.
--
-- Additive DDL for the Space concept introduced by the
-- `brain-federation` slice. Two pieces:
--
-- 1. `spaces` — the new table. One row per registered federation
--    space (Repo / Docs / Issues). The `kind` column is constrained
--    to the three canonical values; unknown kinds are rejected at
--    insert time. The default row is seeded immediately so every
--    pre-federation node has a valid `space_id` to back-reference.
--
-- 2. `graph_nodes.space_id` — an additive column on the existing
--    multimodal `graph_nodes` table. NOT NULL DEFAULT 'default'
--    backfills every existing row to the reserved `"default"`
--    space. The btree index on `space_id` makes
--    `WHERE space_id = $1` lookups O(log n).
--
-- This file is loaded into the migration pipeline ONLY when the
-- `multimodal` Cargo feature is enabled:
--
--     #[cfg(all(feature = "postgres", feature = "multimodal"))]
--     const SCHEMA_SQL_SPACES: &str = include_str!("m0010_spaces.sql");
--
-- The DDL is `IF NOT EXISTS`-idempotent and additive — no ALTER on
-- the existing `symbols` / `call_edges` / `named_views` /
-- `graph_edges` tables, and no data migration on existing rows
-- beyond the `DEFAULT 'default'` backfill. Re-running migrations on
-- a populated DB is a no-op.

-- =============================================================================
-- spaces
-- =============================================================================
--
-- Federation unit. One row per Space registered in any session.
-- `id` is the opaque `SpaceId` (e.g. "default", "auth-repo",
-- "docs-2024"). `kind` is the `SpaceKind::as_str()` wire form
-- (`Repo` | `Docs` | `Issues`); unknown kinds are rejected by the
-- CHECK constraint. `config` is a free-form JSONB blob for
-- extractor-specific hints.
CREATE TABLE IF NOT EXISTS spaces (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('Repo','Docs','Issues')),
    source_path TEXT,
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Fast kind-based scans: "every Repo space", "every Docs space".
CREATE INDEX IF NOT EXISTS idx_spaces_kind ON spaces(kind);

-- Seed the reserved "default" space so every pre-federation node
-- has a valid `space_id` to point at. The `ON CONFLICT DO NOTHING`
-- makes the INSERT idempotent — re-running migrations on a
-- populated DB is a no-op.
INSERT INTO spaces (id, name, kind) VALUES ('default', 'default', 'Repo')
    ON CONFLICT (id) DO NOTHING;

-- =============================================================================
-- graph_nodes.space_id — additive column
-- =============================================================================
--
-- Every node in the Generic Graph Layer now belongs to exactly one
-- space. The column is `NOT NULL DEFAULT 'default'`, so the migration
-- backfills every pre-existing row to the reserved space
-- automatically — no explicit UPDATE is needed. The btree index
-- keeps the per-space scans fast as the table grows.
ALTER TABLE graph_nodes
    ADD COLUMN IF NOT EXISTS space_id TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_graph_nodes_space_id ON graph_nodes(space_id);
