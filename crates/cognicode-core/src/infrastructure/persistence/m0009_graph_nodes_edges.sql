-- Migration `m0009_graph_nodes_edges` — Generic Graph Layer tables.
--
-- This file is the canonical, additive DDL for the multimodal
-- (Generic Graph Layer) PG tables. It is loaded into the migration
-- pipeline ONLY when the `multimodal` Cargo feature is enabled:
-- see `postgres_repository.rs`:
--
--     #[cfg(all(feature = "postgres", feature = "multimodal"))]
--     const SCHEMA_SQL_MULTIMODAL: &str =
--         include_str!("m0009_graph_nodes_edges.sql");
--
-- Why a separate file: `include_str!` is unconditional — to gate the
-- DDL behind `#[cfg(feature = "multimodal")]`, the strings must live
-- in a file that is only included by the conditional constant. The
-- base schema (`schema_postgres.sql`) is intentionally left
-- unchanged so the default `postgres`-only build is byte-for-byte
-- the same as before this migration.
--
-- The DDL is `IF NOT EXISTS`-idempotent and additive — no ALTER on
-- the existing `symbols` / `call_edges` / `named_views` tables, and
-- no data migration. Re-running migrations on a populated DB is a
-- no-op.

-- =============================================================================
-- graph_nodes
-- =============================================================================
--
-- One row per node in the Generic Graph Layer. The `id` is the
-- canonical `NodeId` string (e.g. `doc:adr/0007.md#decision`,
-- `src/api/schema.rs:build_schema:10`). The `kind` column stores the
-- `Display` form of `NodeKind` (`symbol` | `decision` | `doc` |
-- `issue` | `evidence`). `properties` is an open key=value JSONB blob
-- the `DocsExtractor` uses for ADR status / heading anchors / etc.
CREATE TABLE IF NOT EXISTS graph_nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    label       TEXT NOT NULL,
    source_path TEXT,
    properties  JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Fast kind-based scans: `find_graph_nodes(Some(NodeKind::Doc), …)`.
CREATE INDEX IF NOT EXISTS idx_graph_nodes_kind
    ON graph_nodes(kind);

-- Fast provenance queries: "show me every node that came from
-- /docs/adr/".
CREATE INDEX IF NOT EXISTS idx_graph_nodes_source_path
    ON graph_nodes(source_path);

-- =============================================================================
-- graph_edges
-- =============================================================================
--
-- One row per directed, typed edge in the Generic Graph Layer.
-- `(source_id, target_id, kind)` is the natural key (mirrors the
-- design.md shape and the existing `call_edges` table's PK contract).
-- FK references to `graph_nodes(id)` enforce referential integrity at
-- insert time; edges whose endpoints have not yet been ingested
-- fail the FK and surface as a typed `RepositoryError`.
CREATE TABLE IF NOT EXISTS graph_edges (
    id         SERIAL PRIMARY KEY,
    source_id  TEXT NOT NULL REFERENCES graph_nodes(id),
    target_id  TEXT NOT NULL REFERENCES graph_nodes(id),
    kind       TEXT NOT NULL,
    provenance TEXT NOT NULL DEFAULT 'extracted',
    confidence REAL NOT NULL DEFAULT 0.5
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    metadata   JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Natural-key uniqueness: two edges with the same `(source, target,
-- kind)` collapse into one. Re-ingesting a doc must UPDATE the
-- existing row, not INSERT a duplicate.
CREATE UNIQUE INDEX IF NOT EXISTS uniq_graph_edges_source_target_kind
    ON graph_edges(source_id, target_id, kind);

-- Forward edge scans: `find_graph_edges(Some(source), …)`.
CREATE INDEX IF NOT EXISTS idx_graph_edges_source
    ON graph_edges(source_id);

-- Reverse edge scans: `find_graph_edges(None, Some(target), …)`.
CREATE INDEX IF NOT EXISTS idx_graph_edges_target
    ON graph_edges(target_id);

-- Kind-filtered scans: "every Cites edge" / "every Justifies edge".
CREATE INDEX IF NOT EXISTS idx_graph_edges_kind
    ON graph_edges(kind);
