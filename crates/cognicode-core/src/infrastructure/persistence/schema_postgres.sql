-- PostgreSQL schema for cognicode-core's `symbols` and `call_edges`
-- query tables.
--
-- Strategy commitment: raw SQL loaded via `include_str!` in
-- `postgres_repository.rs`. We are intentionally NOT adopting
-- `refinery` / `sqlx-cli` for this 2-table slice. Switch to a
-- migration framework once the table count exceeds 3.
--
-- The DDL is column-for-column compatible with the SQLite `symbols`
-- and `call_edges` tables defined in
-- `crates/cognicode-db/src/schema.rs` so that query projections
-- are portable between backends. The `call_edges` column `dependency_type`
-- (NOT `dep_type`) is the canonical name shared with SQLite v2.

CREATE TABLE IF NOT EXISTS symbols (
    id          SERIAL PRIMARY KEY,
    file_path   TEXT NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT,
    line        INTEGER,
    column      INTEGER,
    complexity  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_pg_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_pg_symbols_file ON symbols(file_path);

-- Call edges between symbols. Column-for-column parity with the
-- SQLite v2 `call_edges` table: `caller_id`, `caller_name`,
-- `callee_id`, `callee_name`, `dependency_type`, `provenance`,
-- `confidence`. The pair `(caller_id, callee_id, dependency_type)`
-- identifies an edge; the per-edge `provenance` and `confidence`
-- columns are required (defaulted to `Extracted` / `1.0` for
-- freshly-inserted rows that omit them). The denormalized
-- `caller_name` / `callee_name` columns are kept for fast lookups
-- without a join.
CREATE TABLE IF NOT EXISTS call_edges (
    id              SERIAL PRIMARY KEY,
    caller_id       TEXT NOT NULL,
    caller_name     TEXT NOT NULL,
    callee_id       TEXT NOT NULL,
    callee_name     TEXT NOT NULL,
    dependency_type TEXT NOT NULL,
    provenance      TEXT NOT NULL DEFAULT 'Extracted',
    confidence      REAL NOT NULL DEFAULT 1.0
);

CREATE INDEX IF NOT EXISTS idx_pg_call_edges_caller ON call_edges(caller_id);
CREATE INDEX IF NOT EXISTS idx_pg_call_edges_callee ON call_edges(callee_id);

-- Named views: user-saved projections of the graph, addressed by
-- `(workspace_id, owner, name)`. The unique index prevents two
-- saves of the same name within the same scope (the spec's
-- "named_view_already_exists" error contract). The DDL is purely
-- additive — no ALTER on existing tables.
CREATE TABLE IF NOT EXISTS named_views (
    id            UUID PRIMARY KEY,
    workspace_id  TEXT NOT NULL,
    owner         TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    level         TEXT NOT NULL,
    lens          TEXT NOT NULL,
    focus_node    TEXT NOT NULL,
    max_depth     INTEGER NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_named_views_scope
    ON named_views (workspace_id, owner, name);
