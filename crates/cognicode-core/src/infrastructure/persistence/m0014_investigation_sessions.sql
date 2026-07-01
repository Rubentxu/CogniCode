-- m0014: Link exploration sessions to investigations (ADR-005 INV-1)
--
-- Adds an optional FK from exploration_sessions to investigations, allowing
-- the UI to track which investigation (if any) was active during a session.
--
-- This is a pure ADD COLUMN — no data migration needed since existing rows
-- simply get NULL for their investigation_id (no active investigation).

ALTER TABLE exploration_sessions
    ADD COLUMN IF NOT EXISTS investigation_id TEXT;

-- Index for "find all sessions for an investigation" queries.
CREATE INDEX IF NOT EXISTS idx_exploration_sessions_investigation
    ON exploration_sessions (investigation_id)
    WHERE investigation_id IS NOT NULL;
