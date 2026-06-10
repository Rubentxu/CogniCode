-- Migration: graph upsert constraints
--
-- Adds the unique constraints required by the
-- `GraphRepository::upsert_nodes` and
-- `GraphRepository::upsert_edges` write methods (T4 — see
-- `openspec/changes/issue-tracker-adapter/specs/graph-repository-write`).
--
-- The natural key for nodes is `(id, kind)`: a Doc and a
-- Symbol may share an id (e.g. `auth`) without colliding on
-- the uniqueness check. The natural key for edges is
-- `(source, target, kind)`: edges don't carry an `id` field.
--
-- Both constraints use `IF NOT EXISTS` so the migration is
-- idempotent — re-running it on a partially-migrated database
-- is a no-op.
--
-- ROLLBACK:
--   ALTER TABLE graph_nodes DROP CONSTRAINT IF EXISTS graph_nodes_id_kind_unique;
--   ALTER TABLE graph_edges DROP CONSTRAINT IF EXISTS graph_edges_stk_unique;

ALTER TABLE graph_nodes
    ADD CONSTRAINT graph_nodes_id_kind_unique UNIQUE (id, kind);

ALTER TABLE graph_edges
    ADD CONSTRAINT graph_edges_stk_unique UNIQUE (source, target, kind);
