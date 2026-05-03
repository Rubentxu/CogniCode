//! Database schema initialization and migration

use rusqlite::Connection;

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
        
        -- CallGraph blob storage (bincode serialized)
        CREATE TABLE IF NOT EXISTS call_graphs (
            id INTEGER PRIMARY KEY,
            data BLOB NOT NULL
        );

        -- Future: cognicode-mcp tables (symbols, call_edges)
        -- Defined here for schema completeness, not yet used
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT,
            line INTEGER,
            column INTEGER,
            complexity INTEGER
        );
        
        CREATE TABLE IF NOT EXISTS call_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            caller_id INTEGER REFERENCES symbols(id),
            callee_id INTEGER REFERENCES symbols(id),
            dependency_type TEXT
        );
        
        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_issues_rule ON issues(rule_id);
        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_issues_file ON issues(file_path);
        CREATE INDEX IF NOT EXISTS idx_runs_timestamp ON analysis_runs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);

        -- File import tracking for incremental analysis dependency resolution
        CREATE TABLE IF NOT EXISTS file_imports (
            source_file TEXT NOT NULL,
            imported_file TEXT NOT NULL,
            PRIMARY KEY (source_file, imported_file)
        );
        CREATE INDEX IF NOT EXISTS idx_imports_imported ON file_imports(imported_file);
    ").expect("Failed to initialize schema");
}

/// Get current schema version for migration tracking
pub fn schema_version(db: &Connection) -> i64 {
    db.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap_or(0)
}

/// Set schema version after migration
pub fn set_schema_version(db: &Connection, version: i64) {
    db.execute(&format!("PRAGMA user_version = {}", version), []).ok();
}