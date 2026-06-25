-- Migration `m0011_quality` — Quality data tables for the
-- `QualityRepository` port in `cognicode-explorer`.
--
-- Backstory: the v1 schema (when the workspace-internal SQLite-backed
-- persistence layer was alive) stored quality issues in SQLite.
-- The `postgres-canonical` cleanup (`verify-report` archived as
-- engram obs #1829, 2026-06-13) removed that layer and its
-- SQLite-backed adapters. The `QualityRepository` port survived in
-- `cognicode-explorer` with a stale doc-comment claiming ownership by
-- the removed layer — that comment is the source of confusion flagged
-- in the 2026-06-25 architecture review (see candidate 2: cleanup of
-- the 8 dead references to the retired persistence layer).
--
-- This migration restores the quality tables in PostgreSQL so the
-- port can be backed by a real adapter (`PostgresQualityRepository`,
-- introduced alongside this migration in PR #54). The DDL is
-- idempotent (`IF NOT EXISTS`) and additive — no ALTER on
-- existing tables. Re-running migrations on a populated DB is a
-- no-op.
--
-- Three tables:
--
-- 1. `issues` — one row per quality finding. The column set mirrors
--    the `QualityIssue` DTO in
--    `cognicode-explorer/src/ports/quality_repository.rs`:
--    `(rule_id, severity, category, file_path, line, message,
--    status)`. `file_path` matches the DB column; the DTO's `file`
--    field is renamed in candidate 5 to `file_path` for symmetry.
--
-- 2. `baselines` — point-in-time snapshots of the workspace quality
--    gate. The column set mirrors `QualityGateSummary`:
--    `(rating, total_issues, blockers, criticals, debt_minutes,
--    snapshot_at)`. `quality_gate()` returns the most recent row.
--
-- 3. `rules` — metadata for each rule (description, category). The
--    v1 `rule_summary()` returns the description (defaulting to the
--    rule_id when empty) and the open count from `issues`.
--
-- Indexing strategy:
-- - `issues.file_path` is the dominant lookup (per-file view).
-- - `issues(workspace_id, status)` for the workspace summary.
-- - `issues.severity` for severity-filtered scans.
-- - `issues.rule_id` for the rule summary aggregation.
-- - `issues.file_path text_pattern_ops` makes `LIKE 'src/auth/%'`
--   lookups O(log n) on the scope scan.
-- - `baselines(workspace_id, snapshot_at DESC)` makes
--   `ORDER BY snapshot_at DESC LIMIT 1` the workspace summary a
--   single index seek.

-- =============================================================================
-- issues
-- =============================================================================

CREATE TABLE IF NOT EXISTS issues (
    id            BIGSERIAL PRIMARY KEY,
    rule_id       TEXT NOT NULL,
    severity      TEXT NOT NULL CHECK (severity IN ('blocker', 'critical', 'major', 'minor', 'info')),
    category      TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    line          INTEGER NOT NULL CHECK (line >= 0),
    message       TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved', 'reopened', 'closed')),
    workspace_id  TEXT NOT NULL DEFAULT 'default',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_issues_file_path ON issues(file_path);
CREATE INDEX IF NOT EXISTS idx_issues_workspace_status ON issues(workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_issues_severity ON issues(severity);
CREATE INDEX IF NOT EXISTS idx_issues_rule_id ON issues(rule_id);
-- text_pattern_ops enables efficient LIKE-prefix queries for scope scans.
CREATE INDEX IF NOT EXISTS idx_issues_file_prefix
    ON issues(file_path text_pattern_ops);

-- =============================================================================
-- baselines
-- =============================================================================

CREATE TABLE IF NOT EXISTS baselines (
    id              BIGSERIAL PRIMARY KEY,
    workspace_id    TEXT NOT NULL DEFAULT 'default',
    rating          TEXT NOT NULL CHECK (rating IN ('A', 'B', 'C', 'D', 'E')),
    total_issues    INTEGER NOT NULL DEFAULT 0,
    blockers        INTEGER NOT NULL DEFAULT 0,
    criticals       INTEGER NOT NULL DEFAULT 0,
    debt_minutes    INTEGER NOT NULL DEFAULT 0,
    snapshot_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    notes           TEXT
);

CREATE INDEX IF NOT EXISTS idx_baselines_workspace_snapshot
    ON baselines(workspace_id, snapshot_at DESC);

-- =============================================================================
-- rules
-- =============================================================================

CREATE TABLE IF NOT EXISTS rules (
    rule_id     TEXT PRIMARY KEY,
    description TEXT NOT NULL DEFAULT '',
    category    TEXT,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);