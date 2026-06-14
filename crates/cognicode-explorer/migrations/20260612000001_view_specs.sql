-- ViewSpec persistence table — Phase 2 of the Moldable View Runtime (ADR-008).
-- Co-exists with `named_views` in v1; each table is independent.
--
-- The unique index on `(workspace_id, owner, title)` prevents duplicate saved views
-- within the same scope (mirrors the named_views constraint so the UI can
-- enforce the same uniqueness invariant).
CREATE TABLE IF NOT EXISTS view_specs (
    id              TEXT NOT NULL,
    workspace_id    TEXT NOT NULL,
    owner          TEXT NOT NULL,
    title          TEXT NOT NULL,
    applies_to     TEXT NOT NULL,
    view_kind      TEXT NOT NULL,
    data_source    TEXT NOT NULL,  -- JSON-serialized DataSource
    transform      TEXT,           -- JSON-serialized Option<Transform>
    renderer_kind  TEXT NOT NULL,
    props          TEXT NOT NULL DEFAULT '{}',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_view_specs_scope_title
    ON view_specs (workspace_id, owner, title);

-- Composite index for list + lookup by scope
CREATE INDEX IF NOT EXISTS idx_view_specs_scope
    ON view_specs (workspace_id, owner);
