-- Migration m0020: Analytics Run Lineage and Descriptor Limits
--
-- Part of E28.4 Analytics Registry Cohort 1 — PR4 Lineage Persistence.
--
-- Creates two tables:
--   1. analytics_run_lineage  — immutable run records (workspace, revision, plan hash,
--       versions, params, seed, mode, status, timestamps, truncation, errors)
--   2. descriptor_limits       — per-descriptor PlanLimits persistence
--
-- Both tables are additive (IF NOT EXISTS) and idempotent.
-- No canonical graph data is touched.

-- =============================================================================
-- analytics_run_lineage
-- =============================================================================

CREATE TABLE IF NOT EXISTS analytics_run_lineage (
    run_id              UUID PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    revision_id         BIGINT NOT NULL,
    algorithm_id        TEXT NOT NULL,          -- e.g. "pagerank"
    algorithm_version   TEXT NOT NULL,          -- e.g. "v1.0.0"
    plan_hash          BYTEA NOT NULL,         -- SHA-256 of canonicalized plan
    params             JSONB NOT NULL,
    seed               BIGINT,                 -- nullable; required iff determinism = Seeded
    mode               TEXT NOT NULL,          -- 'stream' | 'stats' | 'annotate' | 'persist'
    status             TEXT NOT NULL,          -- 'pending' | 'running' | 'succeeded' | 'failed' | 'truncated'
    started_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at        TIMESTAMPTZ,            -- nullable; set when run completes
    row_count          BIGINT,                 -- nullable; set on success/truncation
    truncation_marker   TEXT,                   -- nullable; 'ResultRowsLimit' | 'PathCountLimit' | 'VisitedNodesLimit' | 'VisitedEdgesLimit'
    idempotency_key    TEXT UNIQUE,           -- ensures 'persist' is idempotent
    error_kind         TEXT,                   -- nullable; 'LimitExceeded(Memory)' | 'CanonicalWriteViolation' | ...
    error_message      TEXT                    -- nullable
);

-- Index for querying by workspace + revision (history)
CREATE INDEX IF NOT EXISTS idx_lineage_workspace_revision
    ON analytics_run_lineage(workspace_id, revision_id);

-- Index for querying by algorithm (catalog debug)
CREATE INDEX IF NOT EXISTS idx_lineage_algorithm
    ON analytics_run_lineage(algorithm_id);

-- Index for stable newest-first listing
CREATE INDEX IF NOT EXISTS idx_lineage_started_at
    ON analytics_run_lineage(started_at DESC);

-- =============================================================================
-- descriptor_limits
-- =============================================================================

CREATE TABLE IF NOT EXISTS descriptor_limits (
    algorithm_id        TEXT PRIMARY KEY,
    algorithm_version   TEXT NOT NULL,
    limits             JSONB NOT NULL,        -- serialized PlanLimits
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE analytics_run_lineage IS
    'Immutable run records for every admitted analytics execution. '
    'Queryable by workspace, revision, algorithm, or time range. '
    'Idempotency key ensures persist mode is deduplicated.';

COMMENT ON TABLE descriptor_limits IS
    'Per-descriptor PlanLimits storage for analytics registry. '
    'Allows limit policies to survive registry restarts.';
