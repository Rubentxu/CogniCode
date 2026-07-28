-- Migration m0019: Unique Index on (workspace_id, id) for FK Subset Reference
--
-- Part of e28-0-canonical-graph-revisions: PR3 Correction Cycle 1.
--
-- PROBLEM: m0018 added composite FKs on graph_edges:
--   FOREIGN KEY (workspace_id, source_id) REFERENCES graph_nodes(workspace_id, id)
--   FOREIGN KEY (workspace_id, target_id) REFERENCES graph_nodes(workspace_id, id)
--
-- But graph_nodes PK is (workspace_id, id, kind) — 3 columns. PostgreSQL
-- requires the FK referenced columns to have a matching UNIQUE constraint.
-- A 2-column FK can reference a 3-column PK only if there's a UNIQUE INDEX
-- on the FK's exact column subset (workspace_id, id) in graph_nodes.
--
-- Without this index, any fresh DB run fails at m0018 with:
--   ERROR: there is no unique constraint matching given keys for referenced table
--
-- FIX: Create a unique index on (workspace_id, id) so the FK subset is valid.
-- This index is logically redundant with the PK (which covers all 3 columns)
-- but PostgreSQL's FK mechanism requires an exact-match unique constraint
-- for subset references.
--
-- Idempotent: uses DO block guarded by information_schema check.

DO $$
BEGIN
    -- Only create if not exists — idempotent guard
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_graph_nodes_workspace_id'
    ) THEN
        CREATE UNIQUE INDEX idx_graph_nodes_workspace_id
            ON graph_nodes(workspace_id, id);
        RAISE NOTICE 'Created unique index idx_graph_nodes_workspace_id on graph_nodes(workspace_id, id)';
    ELSE
        RAISE NOTICE 'Index idx_graph_nodes_workspace_id already exists — skipping';
    END IF;
END $$;
