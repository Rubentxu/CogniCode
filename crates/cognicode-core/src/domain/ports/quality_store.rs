//! `QualityStore` — domain port for the quality lens.
//!
//! Surfaces the `issues`, `baselines`, and `rules` tables behind a
//! `Send + Sync` trait shape. The PG schema is owned by
//! `cognicode-core`'s persistence layer (see migration `m0011_quality.sql`);
//! the in-crate adapter `PostgresQualityStore` reads from it.
//!
//! # Read-only contract (preserve QualityRepository legacy)
//!
//! The 8 read methods must degrade gracefully when the underlying DB
//! is missing (return empty / zero, never an error). Errors are
//! reserved for actual I/O / parse failures (e.g. a corrupted DB
//! file).
//!
//! # Write contract (preserve QualityWritePort legacy)
//!
//! The 2 write methods DO error on I/O failure — a failed write is a
//! real error, not an empty result.
//!
//! # Phase 0 origin
//!
//! This trait was relocated from `cognicode-explorer::ports::quality_repository`
//! per ADR-028 to make the port importable from `cognicode_core` only.
//! The original split into two traits (`QualityRepository` +
//! `QualityWritePort`) is collapsed into a single 10-method surface
//! here so adapters ship one impl block; callers that need the
//! read-only invariant can use a `&dyn QualityStore` projection that
//! only exercises the read methods (see `WriteOnly` projection in a
//! future ADR if needed).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Insert DTO — mirror of [`QualityIssue`] minus `id` (DB-assigned), plus
/// explicit `workspace_id` (the read struct carries no workspace field
/// because reads are scoped by method arg, not by row ownership).
#[derive(Debug, Clone, Deserialize)]
pub struct NewIssue {
    pub workspace_id: String,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub file_path: String,
    pub line: u32,
    pub message: String,
    pub status: String,
}

/// Summary of an upsert operation — counts of inserted vs updated rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpsertSummary {
    pub inserted: usize,
    pub updated: usize,
}

/// A single quality finding, lifted from the `issues` table.
///
/// `file_path` is the struct-side name, matching the DB column
/// `issues.file_path`. The adapter does the column→field mapping so
/// callers do not have to know the SQL column name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualityIssue {
    pub id: i64,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    #[serde(rename = "file_path", alias = "file")]
    pub file_path: String,
    pub line: u32,
    pub message: String,
    pub status: String,
}

/// Compact summary of a single rule — its open count and a short
/// description. The `rules` table stores description + category; the
/// description defaults to the rule id when empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSummary {
    pub rule_id: String,
    pub description: String,
    pub open_count: usize,
}

/// Quality gate snapshot — the latest `baselines` row, plus a current
/// open-issue count. Used by file/scope views to surface a "score card"
/// without forcing the caller to issue multiple queries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QualityGateSummary {
    pub rating: Option<String>,
    pub total_issues: usize,
    pub blockers: usize,
    pub criticals: usize,
    pub debt_minutes: u64,
    pub last_run: Option<String>,
}

/// Optional filter applied to `issues_for_workspace`. All fields are
/// `AND`-combined; `None` means "no filter on this dimension".
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    pub severity: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    /// Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
    pub file_prefix: Option<String>,
    pub limit: Option<usize>,
}

/// Errors returned by [`QualityStore`] write operations.
///
/// Read methods do not return errors in graceful mode (they return
/// empty results on missing DB); only write operations surface
/// [`Store`] / [`Conflict`].
#[derive(Debug, Error)]
pub enum QualityError {
    #[error("quality store error: {0}")]
    Store(String),
    #[error("quality write conflict: {0}")]
    Conflict(String),
    #[error("quality issue not found: {0}")]
    NotFound(i64),
}

/// Read + write port for quality findings, rules, and gate state.
///
/// Implementations must be `Send + Sync` and `Arc`-friendly.
///
/// **Read methods (8)**: gracefully degrade when the underlying DB is
/// missing or empty — returning empty vectors or zero counts instead
/// of errors.
/// **Write methods (2)**: surface I/O errors directly. A failed write
/// is a real error, not an empty result.
pub trait QualityStore: Send + Sync {
    /// Every issue whose `file_path` matches `file` exactly.
    fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError>;

    /// Every issue whose `file_path` is `scope` or starts with `scope/`.
    /// Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
    fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError>;

    /// Every issue at exactly `(file, line)`.
    fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError>;

    /// Look up a single issue by its primary key. Returns `Ok(None)`
    /// when the id does not exist.
    fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError>;

    /// Compact summary of a single rule (open count + description).
    fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError>;

    /// Latest quality gate snapshot for the workspace (or all).
    fn quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError>;

    /// Total count of issues with `status = 'open'`.
    fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError>;

    /// Workspace-wide issue scan with optional filters.
    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> Result<Vec<QualityIssue>, QualityError>;

    // --- Write methods ---

    /// Upsert a batch of issues; returns insert/update counts.
    fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError>;

    /// Delete a single issue by `(workspace_id, rule_id, file_path, line)`.
    /// Returns `Ok(true)` when a row was deleted, `Ok(false)` when no
    /// matching row existed.
    fn delete_issue(
        &self,
        workspace_id: &str,
        rule_id: &str,
        file_path: &str,
        line: u32,
    ) -> Result<bool, QualityError>;
}

// =============================================================================
// PostgresQualityStore adapter
// =============================================================================

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use crate::infrastructure::persistence::PostgresRepository;
    use sqlx::Row;

    use super::{
        IssueFilter, NewIssue, QualityError, QualityGateSummary, QualityIssue, QualityStore,
        RuleSummary, UpsertSummary,
    };

    /// `PostgresQualityStore` — [`QualityStore`] adapter.
    ///
    /// Implements all 10 methods of the [`QualityStore`] port against
    /// the `issues`, `baselines`, and `rules` tables defined in
    /// `crates/cognicode-core/src/infrastructure/persistence/m0011_quality.sql`.
    #[derive(Clone)]
    pub struct PostgresQualityStore {
        pool: sqlx::PgPool,
    }

    impl PostgresQualityStore {
        /// Build the adapter from a `PostgresRepository`.
        pub fn new(pg: &PostgresRepository) -> Self {
            Self {
                pool: pg.with_pool(|p| p.clone()),
            }
        }

        /// Build the adapter from a raw `sqlx::PgPool`. Useful for tests.
        pub fn from_pool(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }
    }

    #[derive(sqlx::FromRow)]
    struct IssueRow {
        id: i64,
        rule_id: String,
        severity: String,
        category: String,
        file_path: String,
        line: i32,
        message: String,
        status: String,
    }

    impl From<IssueRow> for QualityIssue {
        fn from(r: IssueRow) -> Self {
            QualityIssue {
                id: r.id,
                rule_id: r.rule_id,
                severity: r.severity,
                category: r.category,
                file_path: r.file_path,
                line: r.line.max(0) as u32,
                message: r.message,
                status: r.status,
            }
        }
    }

    #[derive(sqlx::FromRow)]
    struct BaselineRow {
        rating: Option<String>,
        total_issues: i32,
        blockers: i32,
        criticals: i32,
        debt_minutes: i32,
        /// `snapshot_at` is stored as TEXT (RFC 3339) in the migration to
        /// avoid pulling `sqlx`'s `chrono` feature into the workspace.
        snapshot_at: Option<String>,
    }

    #[derive(sqlx::FromRow)]
    struct RuleRow {
        rule_id: String,
        description: String,
    }

    /// Run a future synchronously. Same `block_in_place` pattern used by
    /// the other adapters in this workspace — keeps the port's `fn`
    /// methods synchronous while still driving the SQL through sqlx.
    fn block_on<F>(fut: F) -> F::Output
    where
        F: std::future::Future,
        F::Output: Send + 'static,
    {
        use tokio::runtime::Handle;
        if Handle::try_current().is_err() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("quality_store current-thread runtime");
            return rt.block_on(fut);
        }
        let handle = Handle::current();
        let outcome: std::sync::Mutex<Option<F::Output>> = std::sync::Mutex::new(None);
        tokio::task::block_in_place(|| {
            let out = handle.block_on(fut);
            *outcome.lock().unwrap() = Some(out);
        });
        outcome.into_inner().unwrap().expect("block_on outcome set")
    }

    impl QualityStore for PostgresQualityStore {
        fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError> {
            let pool = &self.pool;
            let file = file.to_string();
            let rows: Vec<IssueRow> = block_on(async move {
                sqlx::query_as::<_, IssueRow>(
                    r#"SELECT id, rule_id, severity, category, file_path, line, message, status
                       FROM issues
                       WHERE file_path = $1
                       ORDER BY id"#,
                )
                .bind(&file)
                .fetch_all(pool)
                .await
            })
            .map_err(|e| QualityError::Store(format!("issues_for_file: {e}")))?;
            Ok(rows.into_iter().map(QualityIssue::from).collect())
        }

        fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError> {
            let pool = &self.pool;
            let scope = scope_prefix.to_string();
            let boundary = format!("{}/%", scope_prefix.trim_end_matches('/'));
            let rows: Vec<IssueRow> = block_on(async move {
                sqlx::query_as::<_, IssueRow>(
                    r#"SELECT id, rule_id, severity, category, file_path, line, message, status
                       FROM issues
                       WHERE file_path = $1 OR file_path LIKE $2
                       ORDER BY file_path, line"#,
                )
                .bind(&scope)
                .bind(&boundary)
                .fetch_all(pool)
                .await
            })
            .map_err(|e| QualityError::Store(format!("issues_for_scope: {e}")))?;
            Ok(rows.into_iter().map(QualityIssue::from).collect())
        }

        fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError> {
            let pool = &self.pool;
            let file = file.to_string();
            let line_i = line as i32;
            let rows: Vec<IssueRow> = block_on(async move {
                sqlx::query_as::<_, IssueRow>(
                    r#"SELECT id, rule_id, severity, category, file_path, line, message, status
                       FROM issues
                       WHERE file_path = $1 AND line = $2
                       ORDER BY id"#,
                )
                .bind(&file)
                .bind(line_i)
                .fetch_all(pool)
                .await
            })
            .map_err(|e| QualityError::Store(format!("issues_at_line: {e}")))?;
            Ok(rows.into_iter().map(QualityIssue::from).collect())
        }

        fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError> {
            let pool = self.pool.clone();
            let row: Option<IssueRow> = block_on(async move {
                sqlx::query_as::<_, IssueRow>(
                    r#"SELECT id, rule_id, severity, category, file_path, line, message, status
                       FROM issues WHERE id = $1"#,
                )
                .bind(id)
                .fetch_optional(&pool)
                .await
            })
            .map_err(|e| QualityError::Store(format!("issue_by_id: {e}")))?;
            Ok(row.map(QualityIssue::from))
        }

        fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError> {
            let pool = &self.pool;
            let rule_id_s = rule_id.to_string();
            let (meta, count) = block_on(async move {
                let meta: Option<RuleRow> = sqlx::query_as::<_, RuleRow>(
                    r#"SELECT rule_id, description FROM rules WHERE rule_id = $1"#,
                )
                .bind(&rule_id_s)
                .fetch_optional(pool)
                .await?;
                let row = sqlx::query(
                    r#"SELECT COUNT(*) AS count FROM issues
                       WHERE rule_id = $1 AND status = 'open'"#,
                )
                .bind(&rule_id_s)
                .fetch_one(pool)
                .await?;
                let count: i64 = row.try_get::<i64, _>(0)?;
                Ok::<_, sqlx::Error>((meta, count))
            })
            .map_err(|e| QualityError::Store(format!("rule_summary: {e}")))?;

            let description = meta
                .map(|r| r.description)
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| rule_id.to_string());

            Ok(RuleSummary {
                rule_id: rule_id.to_string(),
                description,
                open_count: count.max(0) as usize,
            })
        }

        fn quality_gate(
            &self,
            workspace_id: Option<&str>,
        ) -> Result<QualityGateSummary, QualityError> {
            let pool = &self.pool;
            let ws = workspace_id.map(|s| s.to_string());
            let row: Option<BaselineRow> = block_on(async move {
                if let Some(ref w) = ws {
                    sqlx::query_as::<_, BaselineRow>(
                        r#"SELECT rating, total_issues, blockers, criticals, debt_minutes, snapshot_at
                           FROM baselines
                           WHERE workspace_id = $1
                           ORDER BY snapshot_at DESC
                           LIMIT 1"#,
                    )
                    .bind(w)
                    .fetch_optional(pool)
                    .await
                } else {
                    sqlx::query_as::<_, BaselineRow>(
                        r#"SELECT rating, total_issues, blockers, criticals, debt_minutes, snapshot_at
                           FROM baselines
                           ORDER BY snapshot_at DESC
                           LIMIT 1"#,
                    )
                    .fetch_optional(pool)
                    .await
                }
            })
            .map_err(|e| QualityError::Store(format!("quality_gate: {e}")))?;

            Ok(match row {
                Some(b) => QualityGateSummary {
                    rating: b.rating,
                    total_issues: b.total_issues.max(0) as usize,
                    blockers: b.blockers.max(0) as usize,
                    criticals: b.criticals.max(0) as usize,
                    debt_minutes: b.debt_minutes.max(0) as u64,
                    last_run: b.snapshot_at,
                },
                None => QualityGateSummary::default(),
            })
        }

        fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError> {
            let pool = self.pool.clone();
            let ws = workspace_id.map(|s| s.to_string());
            let count: i64 = block_on(async move {
                if let Some(ref w) = ws {
                    sqlx::query(
                        r#"SELECT COUNT(*) FROM issues WHERE workspace_id = $1 AND status = 'open'"#,
                    )
                    .bind(w)
                    .fetch_one(&pool)
                    .await?
                    .try_get::<i64, _>(0)
                } else {
                    sqlx::query(r#"SELECT COUNT(*) FROM issues WHERE status = 'open'"#)
                        .fetch_one(&pool)
                        .await?
                        .try_get::<i64, _>(0)
                }
            })
            .map_err(|e| QualityError::Store(format!("open_issues_count: {e}")))?;
            Ok(count.max(0) as usize)
        }

        fn issues_for_workspace(
            &self,
            workspace_id: Option<&str>,
            filter: &IssueFilter,
        ) -> Result<Vec<QualityIssue>, QualityError> {
            let mut sql = String::from(
                "SELECT id, rule_id, severity, category, file_path, line, message, status FROM issues WHERE 1=1",
            );
            let mut binds: Vec<String> = Vec::new();
            let mut idx = 1;

            if let Some(ws) = workspace_id {
                sql.push_str(&format!(" AND workspace_id = ${idx}"));
                binds.push(ws.to_string());
                idx += 1;
            }
            if let Some(sev) = &filter.severity {
                sql.push_str(&format!(" AND severity = ${idx}"));
                binds.push(sev.clone());
                idx += 1;
            }
            if let Some(cat) = &filter.category {
                sql.push_str(&format!(" AND category = ${idx}"));
                binds.push(cat.clone());
                idx += 1;
            }
            if let Some(st) = &filter.status {
                sql.push_str(&format!(" AND status = ${idx}"));
                binds.push(st.clone());
                idx += 1;
            }
            if let Some(prefix) = &filter.file_prefix {
                sql.push_str(&format!(
                    " AND (file_path = ${idx} OR file_path LIKE ${})",
                    idx + 1
                ));
                binds.push(prefix.clone());
                binds.push(format!("{}/%", prefix.trim_end_matches('/')));
                idx += 2;
            }
            sql.push_str(" ORDER BY file_path, line, id");

            let limit = filter.limit.unwrap_or(1000);
            sql.push_str(&format!(" LIMIT {limit}"));

            let mut query = sqlx::query_as::<_, IssueRow>(&sql);
            for b in &binds {
                query = query.bind(b);
            }
            let rows = block_on(async move { query.fetch_all(&self.pool).await })
                .map_err(|e| QualityError::Store(format!("issues_for_workspace: {e}")))?;
            Ok(rows.into_iter().map(QualityIssue::from).collect())
        }

        fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError> {
            if issues.is_empty() {
                return Ok(UpsertSummary::default());
            }
            let pool = self.pool.clone();
            let (inserted, updated): (usize, usize) = block_on(async move {
                let mut tx = pool.begin().await?;

                let mut qb = sqlx::QueryBuilder::new(
                    "INSERT INTO issues \
                     (workspace_id, rule_id, severity, category, file_path, line, message, status) ",
                );
                qb.push_values(issues.iter(), |mut b, i| {
                    b.push_bind(&i.workspace_id)
                        .push_bind(&i.rule_id)
                        .push_bind(&i.severity)
                        .push_bind(&i.category)
                        .push_bind(&i.file_path)
                        .push_bind(i.line as i32)
                        .push_bind(&i.message)
                        .push_bind(&i.status);
                });
                qb.push(
                    " ON CONFLICT (workspace_id, rule_id, file_path, line) DO UPDATE SET \
                         severity = EXCLUDED.severity, category = EXCLUDED.category, \
                         message = EXCLUDED.message, status = EXCLUDED.status, \
                         updated_at = now() \
                         RETURNING (xmax = 0) AS inserted",
                );

                let rows = qb.build().fetch_all(&mut *tx).await?;

                let mut inserted = 0usize;
                let mut updated = 0usize;
                for row in &rows {
                    let was_inserted: bool =
                        sqlx::Row::try_get::<bool, _>(row, "inserted").unwrap_or(false);
                    if was_inserted {
                        inserted += 1;
                    } else {
                        updated += 1;
                    }
                }
                tx.commit().await?;
                Ok::<_, sqlx::Error>((inserted, updated))
            })
            .map_err(|e| QualityError::Store(format!("insert_issues: {e}")))?;
            Ok(UpsertSummary { inserted, updated })
        }

        fn delete_issue(
            &self,
            workspace_id: &str,
            rule_id: &str,
            file_path: &str,
            line: u32,
        ) -> Result<bool, QualityError> {
            let pool = self.pool.clone();
            let ws = workspace_id.to_string();
            let rid = rule_id.to_string();
            let fp = file_path.to_string();
            let ln = line as i32;
            let deleted = block_on(async move {
                let res = sqlx::query(
                    "DELETE FROM issues \
                     WHERE workspace_id = $1 AND rule_id = $2 AND file_path = $3 AND line = $4",
                )
                .bind(&ws)
                .bind(&rid)
                .bind(&fp)
                .bind(ln)
                .execute(&pool)
                .await?;
                Ok::<_, sqlx::Error>(res.rows_affected() as i64)
            })
            .map_err(|e| QualityError::Store(format!("delete_issue: {e}")))?;
            Ok(deleted > 0)
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresQualityStore;
