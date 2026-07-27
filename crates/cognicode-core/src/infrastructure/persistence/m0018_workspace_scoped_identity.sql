-- Migration m0018: Workspace-Scoped Identity
--
-- Part of e28-0-canonical-graph-revisions: PR1 Foundation.
-- Changes the primary key of `graph_nodes` and the unique index of
-- `graph_edges` to include `workspace_id`, ensuring that homonymous
-- nodes across workspaces do not collide.
--
-- WARNING: This migration changes existing constraints on a populated database.
-- It is safe to run on an empty schema (fresh install). On a populated DB,
-- the ALTER TABLE statements require an ACCESS EXCLUSIVE lock which may block
-- reads/writes briefly.
--
-- Idempotent: uses DO blocks guarded by information_schema checks.
--
-- Old: graph_nodes PRIMARY KEY (id) — workspace_id just a column
-- New: graph_nodes PRIMARY KEY (workspace_id, id, kind)
--
-- Old: graph_edges UNIQUE INDEX (source_id, target_id, kind)
-- New: graph_edges UNIQUE INDEX (workspace_id, source_id, target_id, kind)
--
-- Old: graph_edges FKs referencing graph_nodes(id) — replaced with composite FKs
-- New: graph_edges FKs referencing graph_nodes(workspace_id, id, kind)

-- =============================================================================
-- 1. Change graph_nodes PRIMARY KEY to include workspace_id
-- =============================================================================
DO $$
BEGIN
    -- Only proceed if the old PK exists and new PK does not
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_nodes_pkey'
          AND table_name = 'graph_nodes'
    )
    AND NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_nodes_pkey_ws'
          AND table_name = 'graph_nodes'
    ) THEN
        -- Drop old PK
        ALTER TABLE graph_nodes DROP CONSTRAINT graph_nodes_pkey;

        -- Add new PK with workspace_id
        ALTER TABLE graph_nodes
            ADD CONSTRAINT graph_nodes_pkey_ws
            PRIMARY KEY (workspace_id, id, kind);

        RAISE NOTICE 'Changed graph_nodes PK to (workspace_id, id, kind)';
    END IF;
END $$;

-- =============================================================================
-- 2. Change graph_edges unique index to include workspace_id
-- =============================================================================
-- Idempotent: unconditionally drop the old index (if present) and create
-- the new workspace-scoped index. Using IF EXISTS on the DROP prevents errors
-- if the old index was already absent (e.g., a fresh database that skipped m0018).
-- The new index is always created so this migration is safe in ALL starting states.
DROP INDEX IF EXISTS uniq_graph_edges_source_target_kind;
CREATE UNIQUE INDEX uniq_graph_edges_ws_source_target_kind
    ON graph_edges(workspace_id, source_id, target_id, kind);

-- =============================================================================
-- 3. Replace old single-column FKs with workspace-scoped composite FKs
-- =============================================================================
-- The old FKs graph_edges_source_id_fkey / graph_edges_target_id_fkey reference
-- graph_nodes(id). After changing graph_nodes PK to (workspace_id, id, kind),
-- we must replace them with composite FKs that reference the new PK.
-- Using IF EXISTS / IF NOT EXISTS guards for idempotency.
DO $$
BEGIN
    -- Drop old source FK if it exists
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_source_id_fkey'
          AND table_name = 'graph_edges'
    ) THEN
        ALTER TABLE graph_edges DROP CONSTRAINT graph_edges_source_id_fkey;
    END IF;

    -- Add new composite FK for source
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_ws_source_fkey'
          AND table_name = 'graph_edges'
    ) THEN
        ALTER TABLE graph_edges
            ADD CONSTRAINT graph_edges_ws_source_fkey
            FOREIGN KEY (workspace_id, source_id, kind)
            REFERENCES graph_nodes(workspace_id, id, kind);
    END IF;
END $$;

DO $$
BEGIN
    -- Drop old target FK if it exists
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_target_id_fkey'
          AND table_name = 'graph_edges'
    ) THEN
        ALTER TABLE graph_edges DROP CONSTRAINT graph_edges_target_id_fkey;
    END IF;

    -- Add new composite FK for target
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'graph_edges_ws_target_fkey'
          AND table_name = 'graph_edges'
    ) THEN
        ALTER TABLE graph_edges
            ADD CONSTRAINT graph_edges_ws_target_fkey
            FOREIGN KEY (workspace_id, target_id, kind)
            REFERENCES graph_nodes(workspace_id, id, kind);
    END IF;
END $$;
