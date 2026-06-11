//! SQLite-backed [`QualityRepository`] adapter.
//!
//! Opens its own short-lived read-only `rusqlite::Connection` against
//! the workspace's `.cognicode/cognicode.db` file, queries the
//! `issues` / `baselines` tables, then drops the connection. This
//! mirrors the `Fts5SearchAdapter` pattern exactly — both adapters
//! are read-only and never hold a long-lived DB connection.
//!
//! The schema column names are taken verbatim from
//! `cognicode_db::schema::initialize_schema` (see that file for the
//! canonical DDL). If that schema changes, this adapter must be
//! updated in lockstep — there is no migration layer here.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{ExplorerError, ExplorerResult};
use crate::ports::quality_repository::{
    QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
};

/// Adapter that surfaces the `issues` and `baselines` tables behind
/// the [`QualityRepository`] port.
pub struct SqliteQualityAdapter {
    db_path: PathBuf,
}

impl SqliteQualityAdapter {
    /// Build an adapter targeting the workspace's `.cognicode/cognicode.db`
    /// file. The file does not need to exist — the adapter degrades to
    /// empty results when it is missing.
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// Borrow the underlying DB path. Exposed for diagnostics / tests.
    #[allow(dead_code)]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Open a fresh read-only connection, or return `Ok(None)` when the
    /// DB file is missing. The connection is closed as soon as the
    /// caller drops it — no long-lived state is held.
    fn open(&self) -> ExplorerResult<Option<Connection>> {
        if !self.db_path.exists() {
            return Ok(None);
        }
        let conn = Connection::open(&self.db_path)
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        Ok(Some(conn))
    }
}

/// Map a raw `&Row` to a `QualityIssue`. The column order matches the
/// `SELECT` list in every adapter method below.
fn row_to_issue(row: &rusqlite::Row<'_>) -> rusqlite::Result<QualityIssue> {
    Ok(QualityIssue {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        severity: row.get(2)?,
        category: row.get(3)?,
        file: row.get(4)?,
        line: {
            let n: i64 = row.get(5)?;
            n.max(0) as u32
        },
        message: row.get(6)?,
        status: row.get(7)?,
    })
}

/// Standard `SELECT` column list for the `issues` table — kept in one
/// place so every method issues a row with the same column order, and
/// `row_to_issue` can be a single fn.
const ISSUE_COLUMNS: &str = "id, rule_id, severity, category, file_path, line, message, status";

impl QualityRepository for SqliteQualityAdapter {
    fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>> {
        let Some(conn) = self.open()? else {
            return Ok(Vec::new());
        };
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ISSUE_COLUMNS} FROM issues WHERE file_path = ?1 ORDER BY line, id"
            ))
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let rows = stmt
            .query_map([file], row_to_issue)
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let mut out: Vec<QualityIssue> = rows.filter_map(|r| r.ok()).collect();
        // Defensive stable sort — the DB already orders by (line, id)
        // but we re-sort here so the public contract is testable.
        out.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.id.cmp(&b.id)));
        Ok(out)
    }

    fn issues_for_scope(&self, scope_prefix: &str) -> ExplorerResult<Vec<QualityIssue>> {
        let Some(conn) = self.open()? else {
            return Ok(Vec::new());
        };
        // Boundary-aware: an issue with `file_path = scope_prefix`
        // (e.g. a file that IS the scope, like a `mod.rs`) is included
        // alongside anything under `scope_prefix/`. The `LIKE` pattern
        // is anchored on `/` so `src` does not match `src_extra.rs`.
        let like_pattern = format!("{}/%", scope_prefix);
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ISSUE_COLUMNS} FROM issues \
                 WHERE file_path = ?1 OR file_path LIKE ?2 \
                 ORDER BY file_path, line, id"
            ))
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let rows = stmt
            .query_map(rusqlite::params![scope_prefix, like_pattern], row_to_issue)
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let out: Vec<QualityIssue> = rows.filter_map(|r| r.ok()).collect();
        Ok(out)
    }

    fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>> {
        let Some(conn) = self.open()? else {
            return Ok(Vec::new());
        };
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ISSUE_COLUMNS} FROM issues \
                 WHERE file_path = ?1 AND line = ?2 ORDER BY id"
            ))
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let rows = stmt
            .query_map(rusqlite::params![file, line as i64], row_to_issue)
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn issue_by_id(&self, id: i64) -> ExplorerResult<Option<QualityIssue>> {
        let Some(conn) = self.open()? else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ISSUE_COLUMNS} FROM issues WHERE id = ?1 LIMIT 1"
            ))
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let mut rows = stmt
            .query_map([id], row_to_issue)
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        match rows.next() {
            Some(Ok(issue)) => Ok(Some(issue)),
            Some(Err(e)) => Err(ExplorerError::Anyhow(anyhow::anyhow!(e))),
            None => Ok(None),
        }
    }

    fn rule_summary(&self, rule_id: &str) -> ExplorerResult<RuleSummary> {
        let Some(conn) = self.open()? else {
            return Ok(RuleSummary {
                rule_id: rule_id.to_string(),
                description: rule_id.to_string(),
                open_count: 0,
            });
        };
        // Open count: matches the same definition as `open_issues_count`
        // (status = 'open'). The schema does not have a `rules` metadata
        // table, so the description is the rule id itself.
        let open_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM issues WHERE rule_id = ?1 AND status = 'open'",
                [rule_id],
                |row| {
                    let n: i64 = row.get(0)?;
                    Ok(n.max(0) as usize)
                },
            )
            .unwrap_or(0);
        Ok(RuleSummary {
            rule_id: rule_id.to_string(),
            description: rule_id.to_string(),
            open_count,
        })
    }

    fn quality_gate(&self) -> ExplorerResult<QualityGateSummary> {
        let Some(conn) = self.open()? else {
            return Ok(QualityGateSummary::default());
        };
        // Latest baseline (most recent `baselines` row). The current
        // schema does not store a `timestamp` column on `baselines`
        // (only `id` is implicit), so we return the most recently
        // inserted baseline by `id DESC`.
        let mut summary = QualityGateSummary::default();
        let baseline = conn.query_row(
            "SELECT timestamp, total_issues, debt_minutes, rating, blockers, criticals \
             FROM baselines ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        );
        if let Ok((timestamp, total, debt, rating, blockers, criticals)) = baseline {
            summary.last_run = Some(timestamp);
            summary.rating = Some(rating);
            summary.total_issues = total.max(0) as usize;
            summary.debt_minutes = debt.max(0) as u64;
            summary.blockers = blockers.max(0) as usize;
            summary.criticals = criticals.max(0) as usize;
        }
        Ok(summary)
    }

    fn open_issues_count(&self) -> ExplorerResult<usize> {
        let Some(conn) = self.open()? else {
            return Ok(0);
        };
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM issues WHERE status = 'open'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(n.max(0) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_db::schema::initialize_schema;
    use rusqlite::Connection;

    /// Build an in-memory SQLite database pre-loaded with the
    /// `cognicode_db` schema and a hand-crafted fixture of issues.
    /// Each test owns its own in-memory DB so there is no state bleed.
    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        initialize_schema(&conn);

        // Seed: 1 analysis run + 5 issues across 2 files + 1 baseline.
        conn.execute(
            "INSERT INTO analysis_runs (timestamp, total_issues, debt_minutes, rating) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["2026-06-06T10:00:00Z", 5i64, 120i64, "B"],
        )
        .expect("insert run");
        let run_id: i64 = conn
            .query_row("SELECT MAX(id) FROM analysis_runs", [], |r| r.get(0))
            .unwrap();

        let issues: &[(i64, &str, &str, &str, &str, i64, &str, &str)] = &[
            // (run_id, rule_id, severity, category, file_path, line, message, status)
            (
                run_id,
                "rust:S100",
                "Blocker",
                "Naming",
                "src/foo.rs",
                10,
                "rename `Foo` to snake_case",
                "open",
            ),
            (
                run_id,
                "rust:S101",
                "Critical",
                "Complexity",
                "src/foo.rs",
                20,
                "function too long",
                "open",
            ),
            (
                run_id,
                "rust:S102",
                "Minor",
                "Style",
                "src/foo.rs",
                30,
                "missing docs",
                "fixed",
            ),
            (
                run_id,
                "rust:S100",
                "Blocker",
                "Naming",
                "src/bar/baz.rs",
                5,
                "rename `Baz` to snake_case",
                "open",
            ),
            (
                run_id,
                "rust:S200",
                "Major",
                "Bug",
                "src/bar/baz.rs",
                15,
                "off-by-one",
                "open",
            ),
        ];
        for (run_id, rule_id, severity, category, file_path, line, message, status) in issues {
            conn.execute(
                "INSERT INTO issues (run_id, rule_id, severity, category, file_path, line, message, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![run_id, rule_id, severity, category, file_path, line, message, status],
            )
            .expect("insert issue");
        }

        conn.execute(
            "INSERT INTO baselines (timestamp, total_issues, debt_minutes, rating, blockers, criticals) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["2026-06-06T10:00:00Z", 5i64, 120i64, "B", 2i64, 1i64],
        )
        .expect("insert baseline");

        conn
    }

    /// Persist a connection to a temp file and return the path, so the
    /// adapter (which holds a `PathBuf` and opens its own connection)
    /// can read it. We avoid `Connection::open_in_memory` because
    /// the adapter's `Connection::open` is the actual production path.
    fn temp_db_path() -> PathBuf {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cognicode.db");
        // Hold the dir alive for the test by leaking it — tempdir's
        // destructor would remove the file before the adapter sees it.
        std::mem::forget(dir);
        let conn = fixture();
        conn.execute_batch(&format!(
            "VACUUM INTO '{}';",
            path.display().to_string().replace('\'', "''")
        ))
        .expect("vacuum into path");
        path
    }

    #[test]
    fn missing_db_returns_empty_for_all_methods() {
        let adapter = SqliteQualityAdapter::new("/tmp/__definitely_missing_quality__.db");
        assert!(adapter.issues_for_file("src/foo.rs").unwrap().is_empty());
        assert!(adapter.issues_for_scope("src").unwrap().is_empty());
        assert!(adapter.issues_at_line("src/foo.rs", 10).unwrap().is_empty());
        assert!(adapter.issue_by_id(1).unwrap().is_none());
        let r = adapter.rule_summary("rust:S100").unwrap();
        assert_eq!(r.open_count, 0);
        assert_eq!(r.description, "rust:S100");
        let g = adapter.quality_gate().unwrap();
        assert_eq!(g.rating, None);
        assert_eq!(g.total_issues, 0);
        assert_eq!(adapter.open_issues_count().unwrap(), 0);
    }

    #[test]
    fn issues_for_file_returns_only_matching_file() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        let issues = adapter.issues_for_file("src/foo.rs").expect("ok");
        assert_eq!(
            issues.len(),
            3,
            "expected 3 issues in src/foo.rs, got {issues:?}"
        );
        for i in &issues {
            assert_eq!(i.file, "src/foo.rs");
        }
        // Stable sort: by line asc, then id asc.
        let lines: Vec<u32> = issues.iter().map(|i| i.line).collect();
        assert_eq!(lines, vec![10, 20, 30]);
    }

    #[test]
    fn issues_for_scope_includes_exact_and_prefix_matches() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        // "src" matches src/foo.rs (prefix), src/bar/baz.rs (prefix)
        let issues = adapter.issues_for_scope("src").expect("ok");
        assert_eq!(
            issues.len(),
            5,
            "all 5 issues live under src/, got {issues:?}"
        );
        let files: std::collections::BTreeSet<&str> =
            issues.iter().map(|i| i.file.as_str()).collect();
        assert!(files.contains("src/foo.rs"));
        assert!(files.contains("src/bar/baz.rs"));
    }

    #[test]
    fn issues_for_scope_is_boundary_aware() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        // "src/bar" matches "src/bar/baz.rs" (2 issues) but not
        // "src/foo.rs" (no issues under "src/bar/") nor "srcbar.rs"
        // (boundary test — no such file in the fixture).
        let issues = adapter.issues_for_scope("src/bar").expect("ok");
        assert_eq!(issues.len(), 2, "expected 2 issues in src/bar scope");
        for i in &issues {
            assert!(
                i.file == "src/bar/baz.rs" || i.file == "src/bar",
                "unexpected file in src/bar scope: {}",
                i.file
            );
        }
    }

    #[test]
    fn issues_at_line_filters_by_exact_line() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        let issues = adapter.issues_at_line("src/foo.rs", 10).expect("ok");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 10);
        assert_eq!(issues[0].rule_id, "rust:S100");
    }

    #[test]
    fn issue_by_id_returns_some_for_known_and_none_for_unknown() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        let first = adapter.issue_by_id(1).expect("ok").expect("id=1 exists");
        assert_eq!(first.id, 1);
        assert_eq!(first.rule_id, "rust:S100");
        assert_eq!(first.file, "src/foo.rs");
        assert_eq!(first.status, "open");

        let missing = adapter.issue_by_id(99_999).expect("ok");
        assert!(missing.is_none());
    }

    #[test]
    fn rule_summary_counts_only_open_issues() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        // rust:S100 has 2 open (one in src/foo.rs, one in src/bar/baz.rs).
        let r = adapter.rule_summary("rust:S100").expect("ok");
        assert_eq!(r.rule_id, "rust:S100");
        assert_eq!(r.open_count, 2);

        // rust:S101: 1 open.
        let r = adapter.rule_summary("rust:S101").expect("ok");
        assert_eq!(r.open_count, 1);

        // rust:S102: 0 open (the only one is 'fixed').
        let r = adapter.rule_summary("rust:S102").expect("ok");
        assert_eq!(r.open_count, 0);

        // Unknown rule: 0 open, description defaults to rule id.
        let r = adapter.rule_summary("rust:UNKNOWN").expect("ok");
        assert_eq!(r.open_count, 0);
        assert_eq!(r.description, "rust:UNKNOWN");
    }

    #[test]
    fn quality_gate_returns_latest_baseline() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        let g = adapter.quality_gate().expect("ok");
        assert_eq!(g.rating.as_deref(), Some("B"));
        assert_eq!(g.total_issues, 5);
        assert_eq!(g.debt_minutes, 120);
        assert_eq!(g.blockers, 2);
        assert_eq!(g.criticals, 1);
        assert!(g.last_run.is_some());
    }

    #[test]
    fn open_issues_count_excludes_fixed() {
        let path = temp_db_path();
        let adapter = SqliteQualityAdapter::new(path);
        // 4 open + 1 fixed = 5 total, open count is 4.
        assert_eq!(adapter.open_issues_count().expect("ok"), 4);
    }
}
