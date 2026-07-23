-- Add provenance JSONB column to investigation_artifacts table.
-- Part of e24-diagram-artifacts: ADR-010 R1–R4 closure (E24.1 slice).
-- m0016 is embedded via include_str! in postgres_repository.rs and executed
-- as step 9 of run_migrations().
-- Uses IF NOT EXISTS so it is idempotent against m0013 (step 6)
-- which already creates the investigation_artifacts table.
-- The column is nullable so pre-migration rows deserialize with provenance = None.
ALTER TABLE investigation_artifacts ADD COLUMN IF NOT EXISTS provenance JSONB;
