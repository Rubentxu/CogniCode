-- Migration m0010: Pipeline Schema
--
-- This migration transforms the database from the old dual-table model
-- (symbols + call_edges) to the pipeline model (graph_nodes + graph_edges
-- as canonical, symbols + call_edges as SQL VIEWs).
--
-- It also adds:
--   - workspace_id to graph tables (ADR-020)
--   - scan_manifest table for incremental change detection (ADR-017/020)
--   - graph_reports table for cached analysis reports (Sprint 2)
--   - notify_graph_change() trigger for real-time Explorer updates (ADR-022)
--
-- The migration is fully idempotent: every statement uses IF NOT EXISTS
-- or checks information_schema before acting. Safe to run multiple times.

-- =============================================================================
-- 1. Ensure graph_nodes exists (move from m0009 multimodal gate to base)
-- =============================================================================
CREATE TABLE IF NOT EXISTS graph_nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    label       TEXT NOT NULL,
    source_path TEXT,
    properties  JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_kind ON graph_nodes(kind);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_source_path ON graph_nodes(source_path);

-- =============================================================================
-- 2. Ensure graph_edges exists
-- =============================================================================
CREATE TABLE IF NOT EXISTS graph_edges (
    id          SERIAL PRIMARY KEY,
    source_id   TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    kind        TEXT NOT NULL,
    provenance  TEXT NOT NULL DEFAULT 'Extracted',
    confidence  REAL NOT NULL DEFAULT 1.0
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    metadata    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- FK constraints: add only if they don't already exist (idempotent).
-- We use DO blocks because ADD CONSTRAINT doesn't support IF NOT EXISTS.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_source_id_fkey'
    ) THEN
        ALTER TABLE graph_edges
            ADD CONSTRAINT graph_edges_source_id_fkey
            FOREIGN KEY (source_id) REFERENCES graph_nodes(id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_target_id_fkey'
    ) THEN
        ALTER TABLE graph_edges
            ADD CONSTRAINT graph_edges_target_id_fkey
            FOREIGN KEY (target_id) REFERENCES graph_nodes(id);
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uniq_graph_edges_source_target_kind
    ON graph_edges(source_id, target_id, kind);
CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_kind ON graph_edges(kind);

-- =============================================================================
-- 3. Add workspace_id (ADR-020)
-- =============================================================================
ALTER TABLE graph_nodes
    ADD COLUMN IF NOT EXISTS workspace_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE graph_edges
    ADD COLUMN IF NOT EXISTS workspace_id TEXT NOT NULL DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_graph_nodes_workspace ON graph_nodes(workspace_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_workspace ON graph_edges(workspace_id);

-- =============================================================================
-- 4. Migrate existing data from symbols/call_edges (if they are still tables)
-- =============================================================================
DO $$
BEGIN
    -- Only migrate if 'symbols' is a BASE TABLE (not already a VIEW)
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'symbols' AND table_type = 'BASE TABLE'
    ) THEN
        -- Migrate symbols → graph_nodes
        -- FQN format: file_path:name:line (matches Symbol::fully_qualified_name)
        INSERT INTO graph_nodes (id, kind, label, source_path, properties, workspace_id)
        SELECT
            file_path || ':' || name || ':' || COALESCE(line::TEXT, '0'),
            'symbol.' || COALESCE(kind, 'unknown'),
            name,
            file_path,
            jsonb_build_object(
                'line', line,
                'column', "column",
                'complexity', complexity
            ),
            'default'
        FROM symbols
        ON CONFLICT (id) DO NOTHING;

        -- Migrate call_edges → graph_edges
        -- Only insert edges whose endpoints exist in graph_nodes
        INSERT INTO graph_edges (source_id, target_id, kind, provenance, confidence, workspace_id)
        SELECT
            ce.caller_id,
            ce.callee_id,
            'dependency.' || ce.dependency_type,
            COALESCE(ce.provenance, 'Extracted'),
            COALESCE(ce.confidence, 1.0),
            'default'
        FROM call_edges ce
        WHERE EXISTS (SELECT 1 FROM graph_nodes gn WHERE gn.id = ce.caller_id)
          AND EXISTS (SELECT 1 FROM graph_nodes gn WHERE gn.id = ce.callee_id)
        ON CONFLICT (source_id, target_id, kind) DO NOTHING;

        -- Drop the old tables
        DROP TABLE IF EXISTS call_edges;
        DROP TABLE IF EXISTS symbols;

        RAISE NOTICE 'Migrated symbols/call_edges → graph_nodes/graph_edges and dropped old tables';
    END IF;
END $$;

-- =============================================================================
-- 5. Create symbols/call_edges as VIEWs (ADR-019)
-- =============================================================================
CREATE OR REPLACE VIEW symbols AS
SELECT
    gn.id,
    gn.source_path AS file_path,
    gn.label AS name,
    -- Extract SymbolKind from NodeKind: 'symbol.function' → 'function'
    REPLACE(gn.kind, 'symbol.', '') AS kind,
    (gn.properties->>'line')::INTEGER AS line,
    (gn.properties->>'column')::INTEGER AS "column",
    (gn.properties->>'complexity')::INTEGER AS complexity
FROM graph_nodes gn
WHERE gn.kind LIKE 'symbol.%';

CREATE OR REPLACE VIEW call_edges AS
SELECT
    ge.source_id AS caller_id,
    src.label AS caller_name,
    ge.target_id AS callee_id,
    tgt.label AS callee_name,
    -- Extract DependencyType from EdgeKind: 'dependency.calls' → 'calls'
    REPLACE(ge.kind, 'dependency.', '') AS dependency_type,
    ge.provenance,
    ge.confidence
FROM graph_edges ge
JOIN graph_nodes src ON src.id = ge.source_id
JOIN graph_nodes tgt ON tgt.id = ge.target_id
WHERE ge.kind LIKE 'dependency.%';

-- =============================================================================
-- 6. scan_manifest table (ADR-017/020)
-- =============================================================================
CREATE TABLE IF NOT EXISTS scan_manifest (
    workspace_id  TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    file_type     TEXT NOT NULL DEFAULT 'code',
    language      TEXT,
    content_hash  TEXT NOT NULL,
    mtime         DOUBLE PRECISION NOT NULL DEFAULT 0,
    symbol_count  INTEGER NOT NULL DEFAULT 0,
    edge_count    INTEGER NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'ok',
    error_msg     TEXT,
    scanned_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, file_path)
);
CREATE INDEX IF NOT EXISTS idx_scan_manifest_workspace ON scan_manifest(workspace_id);

-- =============================================================================
-- 7. graph_reports table (Sprint 2 — created now for forward-compat)
-- =============================================================================
CREATE TABLE IF NOT EXISTS graph_reports (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    report        JSONB NOT NULL,
    symbol_count  INTEGER NOT NULL DEFAULT 0,
    edge_count    INTEGER NOT NULL DEFAULT 0,
    health_score  REAL
);
CREATE INDEX IF NOT EXISTS idx_graph_reports_workspace
    ON graph_reports(workspace_id, created_at DESC);

-- =============================================================================
-- 8. notify_graph_change trigger (ADR-022)
-- =============================================================================
CREATE OR REPLACE FUNCTION notify_graph_change() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('graph_updated', json_build_object(
        'workspace_id', COALESCE(NEW.workspace_id, OLD.workspace_id),
        'source_path', COALESCE(NEW.source_path, OLD.source_path),
        'action', TG_OP,
        'timestamp', extract(epoch from now())
    )::text);
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS graph_nodes_notify ON graph_nodes;
CREATE TRIGGER graph_nodes_notify
    AFTER INSERT OR UPDATE OR DELETE ON graph_nodes
    FOR EACH ROW EXECUTE FUNCTION notify_graph_change();
