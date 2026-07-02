-- Add provenance and applicability columns to view_specs table.
-- Part of viewspec-context-metadata-v1: seed_object_id, seed_view_id, applies_when.
-- m0015 is embedded via include_str! in postgres_repository.rs and executed
-- as step 8 of run_migrations().
ALTER TABLE view_specs
    ADD COLUMN seed_object_id TEXT,
    ADD COLUMN seed_view_id TEXT,
    ADD COLUMN applies_when TEXT;
