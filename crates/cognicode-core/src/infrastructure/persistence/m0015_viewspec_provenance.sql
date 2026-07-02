-- Add provenance and applicability columns to view_specs table.
-- Part of viewspec-context-metadata-v1: seed_object_id, seed_view_id, applies_when.
-- m0015 is embedded via include_str! in postgres_repository.rs and executed
-- as step 8 of run_migrations().
-- Uses IF NOT EXISTS so it is idempotent against schema_postgres.sql (step 1)
-- which already creates these columns in the CREATE TABLE statement.
ALTER TABLE view_specs ADD COLUMN IF NOT EXISTS seed_object_id TEXT;
ALTER TABLE view_specs ADD COLUMN IF NOT EXISTS seed_view_id TEXT;
ALTER TABLE view_specs ADD COLUMN IF NOT EXISTS applies_when TEXT;
