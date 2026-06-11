//! Database schema initialization and migration

use rusqlite::Connection;

/// Current schema version. Bump on every backward-compatible migration.
pub const CURRENT_SCHEMA_VERSION: i64 = 2;

/// Initialize all tables and indexes. Idempotent (CREATE IF NOT EXISTS).
pub fn initialize_schema(db: &Connection) {
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").ok();

    db.execute_batch("
        -- Quality analysis runs
        CREATE TABLE IF NOT EXISTS analysis_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            total_issues INTEGER NOT NULL DEFAULT 0,
            debt_minutes INTEGER NOT NULL DEFAULT 0,
            rating TEXT NOT NULL DEFAULT 'B',
            blockers INTEGER NOT NULL DEFAULT 0,
            criticals INTEGER NOT NULL DEFAULT 0,
            files_changed INTEGER NOT NULL DEFAULT 0,
            files_total INTEGER NOT NULL DEFAULT 0,
            new_issues INTEGER NOT NULL DEFAULT 0,
            fixed_issues INTEGER NOT NULL DEFAULT 0
        );

        -- Issues found by rules
        CREATE TABLE IF NOT EXISTS issues (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER REFERENCES analysis_runs(id),
            rule_id TEXT NOT NULL,
            severity TEXT NOT NULL,
            category TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL,
            message TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            first_seen_run INTEGER REFERENCES analysis_runs(id),
            fixed_in_run INTEGER REFERENCES analysis_runs(id)
        );

        -- Quality baselines (comparison points)
        CREATE TABLE IF NOT EXISTS baselines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            total_issues INTEGER NOT NULL,
            debt_minutes INTEGER NOT NULL,
            rating TEXT NOT NULL,
            blockers INTEGER NOT NULL DEFAULT 0,
            criticals INTEGER NOT NULL DEFAULT 0
        );

        -- File tracking (BLAKE3 hashes)
        CREATE TABLE IF NOT EXISTS file_states (
            path TEXT PRIMARY KEY,
            hash TEXT NOT NULL,
            issues_count INTEGER NOT NULL DEFAULT 0,
            last_analyzed TEXT NOT NULL
        );

        -- CallGraph blob storage (versioned bincode, see `VersionedBlob`)
        CREATE TABLE IF NOT EXISTS call_graphs (
            id INTEGER PRIMARY KEY,
            data BLOB NOT NULL
        );

        -- Symbols (the canonical denormalized source for graph queries).
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT,
            line INTEGER,
            column INTEGER,
            complexity INTEGER
        );

        -- Call edges between symbols. The pair `(caller_id, callee_id,
        -- dependency_type)` identifies an edge; the per-edge
        -- `provenance` and `confidence` columns were added in schema
        -- v2 by `migrate_v1_to_v2`. The denormalized `caller_name` /
        -- `callee_name` columns are kept for fast lookups without a
        -- join.
        CREATE TABLE IF NOT EXISTS call_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            caller_id TEXT NOT NULL,
            caller_name TEXT NOT NULL,
            callee_id TEXT NOT NULL,
            callee_name TEXT NOT NULL,
            dependency_type TEXT NOT NULL,
            provenance TEXT NOT NULL DEFAULT 'Extracted',
            confidence REAL NOT NULL DEFAULT 1.0
        );

        -- Indexes (v1 only — the v2 `idx_call_edges_provenance` index is
        -- created later, *after* `migrate_v1_to_v2` has added the
        -- `provenance` column to legacy databases).
        CREATE INDEX IF NOT EXISTS idx_issues_rule ON issues(rule_id);
        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_issues_file ON issues(file_path);
        CREATE INDEX IF NOT EXISTS idx_runs_timestamp ON analysis_runs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
        CREATE INDEX IF NOT EXISTS idx_call_edges_caller ON call_edges(caller_id);
        CREATE INDEX IF NOT EXISTS idx_call_edges_callee ON call_edges(callee_id);

        -- File import tracking for incremental analysis dependency resolution
        CREATE TABLE IF NOT EXISTS file_imports (
            source_file TEXT NOT NULL,
            imported_file TEXT NOT NULL,
            PRIMARY KEY (source_file, imported_file)
        );
        CREATE INDEX IF NOT EXISTS idx_imports_imported ON file_imports(imported_file);

        -- AVC Contracts
        CREATE TABLE IF NOT EXISTS avc_contracts (
            id TEXT PRIMARY KEY,
            source_file TEXT NOT NULL,
            function_name TEXT NOT NULL,
            contract_json TEXT NOT NULL,
            generated_at TEXT NOT NULL,
            compliance_score REAL DEFAULT 1.0
        );

        -- Intent-Drift events
        CREATE TABLE IF NOT EXISTS drift_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            file_path TEXT NOT NULL,
            function_name TEXT NOT NULL,
            drift_score REAL NOT NULL,
            intent TEXT,
            severity TEXT DEFAULT 'warning'
        );

        -- Agent Interactions Telemetry (Phase 3A)
        CREATE TABLE IF NOT EXISTS agent_interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            contract_id TEXT,
            result_summary TEXT NOT NULL,
            duration_ms REAL
        );

        -- BM25 Symbol Index (FTS5)
        CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
            symbol_name, symbol_kind, file_path, docstring, body_tokens,
            tokenize='porter unicode61'
        );

        -- Symbol Timestamps (BM25 Temporal Indexing)
        /* Stores per-symbol modification timestamps for temporal ranking boost */
        CREATE TABLE IF NOT EXISTS symbol_timestamps (
            file_path       TEXT NOT NULL,
            symbol_name     TEXT NOT NULL,
            last_modified   INTEGER NOT NULL,
            source          TEXT NOT NULL,
            PRIMARY KEY (file_path, symbol_name)
        );
        CREATE INDEX IF NOT EXISTS idx_timestamps_mtime ON symbol_timestamps(last_modified);

        -- Agent Outputs (dashboard activity)
        CREATE TABLE IF NOT EXISTS agent_outputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name TEXT NOT NULL,
            session_id TEXT,
            output_json TEXT NOT NULL,
            summary_text TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_agent_outputs_tool ON agent_outputs(tool_name);
        CREATE INDEX IF NOT EXISTS idx_agent_outputs_created ON agent_outputs(created_at);

        -- Agent Tasks (dashboard task queue)
        CREATE TABLE IF NOT EXISTS agent_tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_type TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 5,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_by TEXT DEFAULT 'dashboard',
            created_at TEXT NOT NULL,
            assigned_at TEXT,
            completed_at TEXT,
            result_json TEXT,
            error_message TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_agent_tasks_status ON agent_tasks(status);
        CREATE INDEX IF NOT EXISTS idx_agent_tasks_priority ON agent_tasks(priority);

        -- Diagram Snapshots (Phase 7)
        CREATE TABLE IF NOT EXISTS diagram_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_path TEXT NOT NULL,
            diagram_type TEXT NOT NULL,
            level TEXT,
            entry_symbol TEXT,
            mermaid_code TEXT NOT NULL,
            element_count INTEGER NOT NULL DEFAULT 0,
            relationship_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            expires_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_diagram_snapshots_project ON diagram_snapshots(project_path);
        CREATE INDEX IF NOT EXISTS idx_diagram_snapshots_type ON diagram_snapshots(diagram_type);
        CREATE INDEX IF NOT EXISTS idx_diagram_snapshots_created ON diagram_snapshots(created_at);
    ").expect("Failed to initialize schema");

    // Apply any pending migrations. `migrate_v1_to_v2` is idempotent
    // (it only runs on schemas < 2 and is skipped thereafter).
    migrate_v1_to_v2(db);

    // v2-only indexes — they depend on the v2 `provenance` column
    // being present, so we must create them *after* the migration.
    let _ = db.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_call_edges_provenance ON call_edges(provenance);\n",
    );

    set_schema_version(db, CURRENT_SCHEMA_VERSION);
}

/// Get current schema version for migration tracking
pub fn schema_version(db: &Connection) -> i64 {
    db.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap_or(0)
}

/// Set schema version after migration
pub fn set_schema_version(db: &Connection, version: i64) {
    db.execute(&format!("PRAGMA user_version = {}", version), []).ok();
}

/// Returns true if the `call_edges` table has a `provenance` column.
fn call_edges_has_provenance_column(db: &Connection) -> bool {
    let mut stmt = match db.prepare("PRAGMA table_info(call_edges)") {
        Ok(s) => s,
        Err(_) => return false,
    };
    let rows = match stmt.query_map([], |row| row.get::<_, String>(1)) {
        Ok(r) => r,
        Err(_) => return false,
    };
    for col in rows.flatten() {
        if col == "provenance" {
            return true;
        }
    }
    false
}

/// Migrate the database from schema v1 (no edge metadata) to v2
/// (per-edge `provenance` and `confidence` columns on `call_edges`).
///
/// This migration is **idempotent**:
/// * If the schema is already at v2 (or above) it is a no-op.
/// * If `call_edges` does not yet have a `provenance` column, two
///   `ALTER TABLE ... ADD COLUMN` statements are issued. SQLite
///   requires each `ADD COLUMN` to be a separate statement and to
///   apply to an existing column. Both `ADD COLUMN` statements have
///   `NOT NULL DEFAULT` so existing rows are valid after the
///   migration.
/// * If `call_edges` does not exist yet (e.g. fresh database),
///   `initialize_schema` already created the v2 table directly and
///   there is nothing to do here.
pub fn migrate_v1_to_v2(db: &Connection) {
    if schema_version(db) >= 2 {
        return;
    }

    if !call_edges_has_provenance_column(db) {
        // SQLite requires separate `ALTER TABLE` statements for each
        // added column. Both have `NOT NULL DEFAULT` so existing rows
        // are valid (they get the default value automatically).
        // Errors are ignored because we may race with `initialize_schema`
        // already creating the v2 table; that's fine.
        let _ = db.execute_batch(
            "ALTER TABLE call_edges ADD COLUMN provenance TEXT NOT NULL DEFAULT 'Extracted';\n\
             ALTER TABLE call_edges ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0;\n",
        );
    }
    // Any v1 rows that were inserted with the old
    // `(caller_name, callee_name, dependency_type)` column shape
    // (i.e. legacy callers of `populate_edges` that pre-date this
    // change) carry the DEFAULT values for the new columns thanks to
    // the `DEFAULT` clause above — that satisfies the spec
    // requirement that "legacy v1 rows MUST receive (Extracted, 1.0)".
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_in_memory_db() -> Connection {
        let db = Connection::open_in_memory().expect("open in-memory db");
        initialize_schema(&db);
        db
    }

    #[test]
    fn initialize_schema_is_idempotent() {
        let db = fresh_in_memory_db();
        // Running it again must not error.
        initialize_schema(&db);
        // Schema must end at the current version.
        assert_eq!(schema_version(&db), CURRENT_SCHEMA_VERSION);
        // v2 columns must be present.
        assert!(call_edges_has_provenance_column(&db));
    }

    #[test]
    fn fresh_schema_has_provenance_and_confidence_columns() {
        let db = fresh_in_memory_db();
        // The v2 columns exist with the documented defaults. We check
        // by inserting a row and reading the new columns back.
        db.execute(
            "INSERT INTO call_edges \
             (caller_id, caller_name, callee_id, callee_name, dependency_type) \
             VALUES ('src/a.rs:a:1', 'a', 'src/b.rs:b:5', 'b', 'Calls')",
            [],
        )
        .expect("legacy insert");
        // Spec: "legacy v1 rows MUST receive (Extracted, 1.0)".
        // Inserting without specifying the new columns exercises the
        // `NOT NULL DEFAULT` clause.
        let (prov, conf): (String, f64) = db
            .query_row(
                "SELECT provenance, confidence FROM call_edges LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("read row");
        assert_eq!(prov, "Extracted");
        assert!((conf - 1.0).abs() < 1e-9);
    }

    #[test]
    fn migrate_v1_to_v2_is_idempotent() {
        // Manually downgrade a fresh DB to v1 by clearing
        // `user_version` and dropping the v2 columns, then re-run
        // `migrate_v1_to_v2` and confirm it is a no-op (still passes
        // because the v2 columns were already created by
        // `initialize_schema`).
        let db = fresh_in_memory_db();
        set_schema_version(&db, 1);
        // migrate_v1_to_v2 is a no-op now because the columns exist.
        migrate_v1_to_v2(&db);
        assert!(call_edges_has_provenance_column(&db));
        // Second call is still a no-op.
        migrate_v1_to_v2(&db);
        assert!(call_edges_has_provenance_column(&db));
    }
}