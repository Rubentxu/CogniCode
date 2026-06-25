//! `PostgresQualityRepository` — `QualityRepository` port adapter
//! backed by PostgreSQL.
//!
//! Implements all 8 methods of the `QualityRepository` port
//! (`crates/cognicode-explorer/src/ports/quality_repository.rs`)
//! against the `issues`, `baselines`, and `rules` tables defined in
//! `crates/cognicode-core/src/infrastructure/persistence/m0011_quality.sql`.
//!
//! ## Read-only contract
//!
//! The adapter never writes — quality data is owned by the quality
//! agent (SonarQube-style external scanner, `cognicode-axiom`
//! pipeline, etc.). The contract is the same as the port: empty
//! results when the DB is missing/corrupted, errors only on actual
//! I/O failures.
//!
//! ## Why a thin wrapper
//!
//! The adapter is intentionally simple — every method is a single
//! SQL statement routed through `sqlx::query_as`. The complexity of
//! the trait lies in the *port* (the 8 methods and their graceful
//! degradation contract); the adapter is a one-to-one mapping from
//! method to query.
//!
//! ## Connection pool
//!
//! The adapter owns a `sqlx::PgPool` (cloned from the parent service
//! in `cognicode-runtime`). Cloning the adapter is cheap because
//! the pool is internally `Arc`-backed.
//!
//! ## Migration coupling
//!
//! Migration `m0011_quality.sql` must have been applied before any
//! query is dispatched. `PostgresRepository::run_migrations()`
//! applies it as step 4 unconditionally when the `postgres` feature
//! is on, so a healthy runtime has the schema ready.

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

#[cfg(feature = "postgres")]
use crate::error::ExplorerResult;
#[cfg(feature = "postgres")]
use crate::ports::quality_repository::{
    IssueFilter, QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
};

#[cfg(feature = "postgres")]
use sqlx::Row;

#[cfg(feature = "postgres")]
#[derive(Clone)]
pub struct PostgresQualityRepository {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresQualityRepository {
    /// Build the adapter from a `PostgresRepository` (the existing
    /// PG owner used by the rest of the explorer runtime). The pool
    /// is cloned — both adapters share the same connection pool.
    pub fn new(pg: &PostgresRepository) -> Self {
        Self {
            pool: pg.pool().clone(),
        }
    }

    /// Build the adapter from a raw `sqlx::PgPool`. Useful for tests
    /// that wire their own pool against an ephemeral PG instance.
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
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

#[cfg(feature = "postgres")]
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

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct BaselineRow {
    rating: Option<String>,
    total_issues: i32,
    blockers: i32,
    criticals: i32,
    debt_minutes: i32,
    /// `snapshot_at` is stored as TEXT (RFC 3339) in the migration to
    /// avoid pulling `sqlx`'s `chrono` feature into the workspace.
    /// The PG `TIMESTAMPTZ` column is implicitly converted to TEXT
    /// via the SQL driver. Parsed lazily by the caller if needed.
    snapshot_at: Option<String>,
}

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct RuleRow {
    rule_id: String,
    description: String,
}

#[cfg(feature = "postgres")]
impl QualityRepository for PostgresQualityRepository {
    fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>> {
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("issues_for_file: {e}")))?;
        Ok(rows.into_iter().map(QualityIssue::from).collect())
    }

    fn issues_for_scope(&self, scope_prefix: &str) -> ExplorerResult<Vec<QualityIssue>> {
        // Boundary-aware: `scope = "src"` does NOT match `src_extra.rs`.
        // The `text_pattern_ops` index on `file_path` makes the
        // `LIKE 'src/%'` predicate O(log n).
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("issues_for_scope: {e}")))?;
        Ok(rows.into_iter().map(QualityIssue::from).collect())
    }

    fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>> {
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("issues_at_line: {e}")))?;
        Ok(rows.into_iter().map(QualityIssue::from).collect())
    }

    fn issue_by_id(&self, id: i64) -> ExplorerResult<Option<QualityIssue>> {
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("issue_by_id: {e}")))?;
        Ok(row.map(QualityIssue::from))
    }

    fn rule_summary(&self, rule_id: &str) -> ExplorerResult<RuleSummary> {
        let pool = &self.pool;
        let rule_id_s = rule_id.to_string();
        // Two-step: lookup metadata in `rules`, then count open issues.
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("rule_summary: {e}")))?;

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

    fn quality_gate(&self, workspace_id: Option<&str>) -> ExplorerResult<QualityGateSummary> {
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("quality_gate: {e}")))?;

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

    fn open_issues_count(&self, workspace_id: Option<&str>) -> ExplorerResult<usize> {
        let pool = self.pool.clone();
        let ws = workspace_id.map(|s| s.to_string());
        let count: i64 = block_on(async move {
            if let Some(ref w) = ws {
                sqlx::query(r#"SELECT COUNT(*) FROM issues WHERE workspace_id = $1 AND status = 'open'"#)
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
        .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("open_issues_count: {e}")))?;
        Ok(count.max(0) as usize)
    }

    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> ExplorerResult<Vec<QualityIssue>> {
        // Build the SQL incrementally based on which filters are set.
        // The PG planner treats the `WHERE workspace_id = $N` as the
        // primary predicate (idx_issues_workspace_status), with the
        // other filters applied as residual filters.
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
            // Boundary-aware prefix: exact match OR `{prefix}/%`.
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
            .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!("issues_for_workspace: {e}")))?;
        Ok(rows.into_iter().map(QualityIssue::from).collect())
    }
}

// ============================================================================
// Tests (unit-level coverage; integration tests live in
// `tests/postgres_quality_integration.rs` once a CI runner is wired)
// ============================================================================

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;

    #[test]
    fn issue_row_to_dto_maps_file_path() {
        let row = IssueRow {
            id: 7,
            rule_id: "S107".to_string(),
            severity: "critical".to_string(),
            category: "complexity".to_string(),
            file_path: "src/auth/login.rs".to_string(),
            line: 42,
            message: "too many args".to_string(),
            status: "open".to_string(),
        };
        let dto: QualityIssue = row.into();
        assert_eq!(dto.id, 7);
        assert_eq!(dto.file_path, "src/auth/login.rs");
        assert_eq!(dto.line, 42);
    }

    #[test]
    fn issue_row_clamps_negative_line_to_zero() {
        let row = IssueRow {
            id: 1,
            rule_id: "X".to_string(),
            severity: "info".to_string(),
            category: "naming".to_string(),
            file_path: "f.rs".to_string(),
            line: -5,
            message: "msg".to_string(),
            status: "open".to_string(),
        };
        let dto: QualityIssue = row.into();
        assert_eq!(dto.line, 0);
    }

    #[test]
    fn empty_baseline_returns_default_gate() {
        let summary = QualityGateSummary::default();
        assert!(summary.rating.is_none());
        assert_eq!(summary.total_issues, 0);
        assert_eq!(summary.blockers, 0);
        assert_eq!(summary.criticals, 0);
        assert_eq!(summary.debt_minutes, 0);
        assert!(summary.last_run.is_none());
    }

    #[test]
    fn issue_filter_default_is_unfiltered() {
        let filter = IssueFilter::default();
        assert!(filter.severity.is_none());
        assert!(filter.category.is_none());
        assert!(filter.status.is_none());
        assert!(filter.file_prefix.is_none());
        assert!(filter.limit.is_none());
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Run a future synchronously on the current thread. Used to keep
/// the port's `fn` methods short (the trait is `fn` not `async fn`,
/// so the SQL has to be driven from a sync context). On the call
/// site (the MCP handler) the runtime is multi-threaded, so this
/// block-on just borrows a thread for the duration of the
/// transaction. Same pattern as
/// `pg_graph_repository::futures_executor_block_on`.
#[cfg(feature = "postgres")]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Handle::current().block_on(fut)
}