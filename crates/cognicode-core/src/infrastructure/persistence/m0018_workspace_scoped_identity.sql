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
DO $$
BEGIN
    -- Only proceed if the old unique index exists
    IF EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'uniq_graph_edges_source_target_kind'
          AND tablename = 'graph_edges'
    ) THEN
        -- Drop old unique index
        DROP INDEX IF EXISTS uniq_graph_edges_source_target_kind;

        -- Create new unique index with workspace_id
        CREATE UNIQUE INDEX uniq_graph_edges_ws_source_target_kind
            ON graph_edges(workspace_id, source_id, target_id, kind);

        RAISE NOTICE 'Changed graph_edges unique index to (workspace_id, source_id, target_id, kind)';
    END IF;
END $$;
