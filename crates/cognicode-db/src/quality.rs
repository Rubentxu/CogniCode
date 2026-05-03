//! Quality analysis persistence (issues, runs, baselines)

use rusqlite::{Connection, params};
use crate::types::*;

pub struct QualityStore {
    db: Connection,
}

impl QualityStore {
    /// Open or create the database at the given project path
    pub fn open(project_root: &std::path::Path) -> Self {
        let db_dir = project_root.join(".cognicode");
        let _ = std::fs::create_dir_all(&db_dir);
        let db_path = db_dir.join("cognicode.db");
        let db = Connection::open(&db_path).expect("Failed to open SQLite database");
        crate::schema::initialize_schema(&db);
        Self { db }
    }

    // === Baseline ===
    
    pub fn set_baseline(&self, total_issues: usize, debt_minutes: u64, rating: &str, blockers: usize, criticals: usize) {
        self.db.execute(
            "INSERT INTO baselines (timestamp, total_issues, debt_minutes, rating, blockers, criticals) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chrono::Utc::now().to_rfc3339(), total_issues as i64, debt_minutes as i64, rating, blockers as i64, criticals as i64],
        ).ok();
    }

    pub fn get_baseline(&self) -> Option<QualityBaseline> {
        self.db.query_row(
            "SELECT timestamp, total_issues, debt_minutes, rating, blockers, criticals FROM baselines ORDER BY id DESC LIMIT 1",
            [], |row| Ok(QualityBaseline {
                timestamp: row.get(0)?, total_issues: row.get::<_, i64>(1)? as usize,
                debt_minutes: row.get::<_, i64>(2)? as u64, rating: row.get(3)?,
                blockers: row.get::<_, i64>(4)? as usize, criticals: row.get::<_, i64>(5)? as usize,
            })
        ).ok()
    }

    pub fn diff_vs_baseline(&self, current_issues: usize, current_debt: u64, current_rating: &str, current_blockers: usize) -> Option<BaselineDiff> {
        self.get_baseline().map(|b| BaselineDiff {
            baseline_timestamp: b.timestamp,
            issues_delta: current_issues as i64 - b.total_issues as i64,
            debt_delta: current_debt as i64 - b.debt_minutes as i64,
            rating_before: b.rating,
            rating_after: current_rating.to_string(),
            blockers_before: b.blockers,
            blockers_after: current_blockers,
        })
    }

    // === Runs / History ===

    pub fn add_run(&self, total_issues: usize, debt_minutes: u64, rating: &str, files_changed: usize, new_issues: usize, fixed_issues: usize) -> i64 {
        self.db.execute(
            "INSERT INTO analysis_runs (timestamp, total_issues, debt_minutes, rating, files_changed, new_issues, fixed_issues) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![chrono::Utc::now().to_rfc3339(), total_issues as i64, debt_minutes as i64, rating, files_changed as i64, new_issues as i64, fixed_issues as i64],
        ).ok();
        self.db.last_insert_rowid()
    }

    pub fn get_run_history(&self, limit: usize) -> Vec<QualitySnapshot> {
        let mut stmt = self.db.prepare(
            "SELECT timestamp, total_issues, debt_minutes, rating, files_changed, new_issues, fixed_issues FROM analysis_runs ORDER BY id DESC LIMIT ?1"
        ).unwrap();
        stmt.query_map(params![limit as i64], |row| Ok(QualitySnapshot {
            timestamp: row.get(0)?, total_issues: row.get::<_, i64>(1)? as usize,
            debt_minutes: row.get::<_, i64>(2)? as u64, rating: row.get(3)?,
            files_changed: row.get::<_, i64>(4)? as usize,
            new_issues: row.get::<_, i64>(5)? as usize, fixed_issues: row.get::<_, i64>(6)? as usize,
        })).unwrap().filter_map(|r| r.ok()).collect()
    }

    /// Get the latest run ID
    pub fn get_latest_run_id(&self) -> Option<i64> {
        self.db.query_row("SELECT MAX(id) FROM analysis_runs", [], |row| row.get(0)).ok()
    }

    // === Issues ===

    pub fn insert_issues(&self, run_id: i64, issues: &[serde_json::Value]) {
        for issue in issues {
            self.db.execute(
                "INSERT INTO issues (run_id, rule_id, severity, category, file_path, line, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![run_id, 
                    issue["rule_id"].as_str().unwrap_or(""),
                    issue["severity"].as_str().unwrap_or("Minor"),
                    issue["category"].as_str().unwrap_or("CodeSmell"),
                    issue["file"].as_str().unwrap_or(""),
                    issue["line"].as_i64().unwrap_or(0),
                    issue["message"].as_str().unwrap_or(""),
                ],
            ).ok();
        }
    }

    pub fn get_open_issues_count(&self) -> usize {
        self.db.query_row("SELECT COUNT(*) FROM issues WHERE status='open'", [], |row| row.get::<_, i64>(0)).unwrap_or(0) as usize
    }
}