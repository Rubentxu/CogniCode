-- Investigation entity persistence — ADR-005 Phase INV-1.
--
-- Three tables: investigations (parent), investigation_evidence (pinned
-- objects), investigation_artifacts (generated diagrams/markdown).
--
-- The `status` enum maps to InvestigationStatus in Rust:
--   'draft'       -> InvestigationStatus::Draft
--   'active'      -> InvestigationStatus::Active
--   'completed'   -> InvestigationStatus::Completed
--   'archived'    -> InvestigationStatus::Archived
--
-- The `related_adrs` column is JSONB for ergonomic PG + serde_json
-- round-tripping. Each entry is a string ADR identifier (e.g. "ADR-005").
--
-- Cascade delete: removing an investigation removes its evidence and
-- artifacts automatically (enforced at DB level).

CREATE TABLE IF NOT EXISTS investigations (
    id              TEXT NOT NULL,
    workspace_id     TEXT NOT NULL,
    title           TEXT NOT NULL,
    goal            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'draft',
    entry_point     TEXT,
    panes           JSONB NOT NULL DEFAULT '[]',
    narrative       TEXT NOT NULL DEFAULT '',
    related_adrs    JSONB NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_investigations_workspace
    ON investigations (workspace_id);

CREATE INDEX IF NOT EXISTS idx_investigations_status
    ON investigations (status);

-- Evidence items pinned to an investigation.
-- An evidence item is a reference to a code object (symbol, file, etc.)
-- that the user has marked as relevant to the investigation.
CREATE TABLE IF NOT EXISTS investigation_evidence (
    id              TEXT NOT NULL,
    investigation_id TEXT NOT NULL,
    object_id       TEXT NOT NULL,
    view_id         TEXT,
    note            TEXT NOT NULL DEFAULT '',
    pinned_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (id),
    CONSTRAINT fk_investigation_evidence_investigation
        FOREIGN KEY (investigation_id)
        REFERENCES investigations(id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_investigation_evidence_investigation
    ON investigation_evidence (investigation_id);

-- Artifacts attached to an investigation.
-- An artifact is generated content: Mermaid diagrams, draw.io exports,
-- SVG snapshots, or markdown notes produced during the investigation.
CREATE TABLE IF NOT EXISTS investigation_artifacts (
    id              TEXT NOT NULL,
    investigation_id TEXT NOT NULL,
    kind            TEXT NOT NULL,
    title           TEXT NOT NULL,
    content         TEXT NOT NULL,
    generated_from  TEXT,

    PRIMARY KEY (id),
    CONSTRAINT fk_investigation_artifacts_investigation
        FOREIGN KEY (investigation_id)
        REFERENCES investigations(id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_investigation_artifacts_investigation
    ON investigation_artifacts (investigation_id);
