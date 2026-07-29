-- Rollback m0020: Analytics Run Lineage and Descriptor Limits
--
-- Part of E28.4 Analytics Registry Cohort 1 — PR4 Lineage Persistence.
--
-- Drops the two tables created by m0020_analytics_lineage.sql:
--   1. analytics_run_lineage
--   2. descriptor_limits
--
-- CASCADE is used to drop any dependent objects (indexes are auto-dropped).

DROP TABLE IF EXISTS analytics_run_lineage CASCADE;
DROP TABLE IF EXISTS descriptor_limits CASCADE;
