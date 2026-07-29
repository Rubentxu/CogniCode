//! PostgreSQL-backed implementation of the [`RunLineageStore`] trait.
//!
//! Provides durable storage for analytics run lineage records and descriptor
//! limit policies. Feature-gated behind `postgres`.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR4 Lineage Persistence.

#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use async_trait::async_trait;
#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "postgres")]
use crate::domain::analytics::{
    AnalyticsError, AnalyticsMode, RunLineage, RunLineageFilter,
    RunLineageStore, RunStatus, TruncationMarker, Uuid,
};
#[cfg(feature = "postgres")]
use crate::domain::plan::limits::PlanLimits;
#[cfg(feature = "postgres")]
use crate::domain::value_objects::{RevisionId, WorkspaceId};

#[cfg(feature = "postgres")]
use crate::infrastructure::persistence::PostgresRepository;

/// PostgreSQL-backed implementation of [`RunLineageStore`].
#[cfg(feature = "postgres")]
pub struct PostgresLineageStore {
    pool: PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresLineageStore {
    /// Create a new store from a PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new store from an existing [`PostgresRepository`].
    pub fn from_repo(repo: &Arc<PostgresRepository>) -> Self {
        Self::new(repo.pool().clone())
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl RunLineageStore for PostgresLineageStore {
    /// Insert a new run lineage record.
    ///
    /// Returns `Err(AnalyticsError::IdempotencyConflict)` if an existing
    /// record has the same idempotency_key but different parameters.
    async fn insert(&self, lineage: &RunLineage) -> Result<(), AnalyticsError> {
        let pool = &self.pool;

        // Check idempotency: if idempotency_key is set, check for conflicts
        let key_exists: bool = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(SELECT 1 FROM analytics_run_lineage WHERE idempotency_key = $1)"#,
        )
        .bind(lineage.idempotency_key.as_ref().map(|k| k.as_str()))
        .fetch_one(pool)
        .await
        .map_err(|e| AnalyticsError::Internal(format!("lineage insert check: {e}")))?;

        if key_exists {
            // Key exists — check if params match (idempotency)
            let existing_params: serde_json::Value = sqlx::query_scalar(
                r#"SELECT params FROM analytics_run_lineage WHERE idempotency_key = $1"#,
            )
            .bind(lineage.idempotency_key.as_ref().map(|k| k.as_str()))
            .fetch_one(pool)
            .await
            .map_err(|e| AnalyticsError::Internal(format!("lineage params fetch: {e}")))?;

            if existing_params != lineage.params {
                return Err(AnalyticsError::IdempotencyConflict);
            }
            // Params match — idempotent, no-op
            return Ok(());
        }

        // Insert new row
        let plan_hash_bytes: Vec<u8> = lineage.plan_hash.clone();
        let status_str = lineage.status.to_string();
        let mode_str = lineage.mode.to_string();

        sqlx::query(
            r#"
            INSERT INTO analytics_run_lineage (
                run_id, workspace_id, revision_id, algorithm_id, algorithm_version,
                plan_hash, params, seed, mode, status, started_at, finished_at,
                row_count, truncation_marker, idempotency_key, error_kind, error_message
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(lineage.run_id.to_string())
        .bind(lineage.workspace_id.as_str())
        .bind(lineage.revision_id.get() as i64)
        .bind(lineage.algorithm_id.as_str())
        .bind(&lineage.algorithm_version)
        .bind(&plan_hash_bytes)
        .bind(&lineage.params)
        .bind(lineage.seed.map(|s| s as i64))
        .bind(&mode_str)
        .bind(&status_str)
        .bind(&lineage.started_at.to_rfc3339())
        .bind(lineage.finished_at.map(|dt| dt.to_rfc3339()))
        .bind(lineage.row_count)
        .bind(lineage.truncation_marker.map(|t| t.to_string()))
        .bind(lineage.idempotency_key.as_ref().map(|k| k.as_str()))
        .bind(lineage.error_kind.as_ref().map(|k| k.as_str()))
        .bind(lineage.error_message.as_ref().map(|m| m.as_str()))
        .execute(pool)
        .await
        .map_err(|e| AnalyticsError::Internal(format!("lineage insert: {e}")))?;

        Ok(())
    }

    /// Get a run lineage record by run ID.
    async fn get(&self, run_id: Uuid) -> Result<RunLineage, AnalyticsError> {
        let pool = &self.pool;
        let run_id_str = run_id.to_string();

        let row: Option<AnalyticsRunLineageRow> = sqlx::query_as::<_, AnalyticsRunLineageRow>(
            r#"
            SELECT run_id, workspace_id, revision_id, algorithm_id, algorithm_version,
                   plan_hash, params, seed, mode, status, started_at, finished_at,
                   row_count, truncation_marker, idempotency_key, error_kind, error_message
            FROM analytics_run_lineage
            WHERE run_id = $1
            "#,
        )
        .bind(&run_id_str)
        .fetch_optional(pool)
        .await
        .map_err(|e| AnalyticsError::Internal(format!("lineage get: {e}")))?;

        row.ok_or_else(|| AnalyticsError::RunNotFound(run_id.to_string()))
            .map(|r| r.into_run_lineage())
    }

    /// Query lineage records by filter.
    async fn query(
        &self,
        filter: RunLineageFilter,
        limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError> {
        let pool = &self.pool;

        // Build query with dynamic filters
        let rows: Vec<AnalyticsRunLineageRow> = {
            let mut q = String::from(
                r#"
                SELECT run_id, workspace_id, revision_id, algorithm_id, algorithm_version,
                       plan_hash, params, seed, mode, status, started_at, finished_at,
                       row_count, truncation_marker, idempotency_key, error_kind, error_message
                FROM analytics_run_lineage
                WHERE 1=1
                "#,
            );

            let mut bind_idx = 1;

            if filter.workspace_id.is_some() {
                q.push_str(&format!(" AND workspace_id = ${}", bind_idx));
                bind_idx += 1;
            }
            if filter.revision_id.is_some() {
                q.push_str(&format!(" AND revision_id = ${}", bind_idx));
                bind_idx += 1;
            }
            if filter.algorithm_id.is_some() {
                q.push_str(&format!(" AND algorithm_id = ${}", bind_idx));
                bind_idx += 1;
            }
            if filter.status.is_some() {
                q.push_str(&format!(" AND status = ${}", bind_idx));
                bind_idx += 1;
            }

            q.push_str(" ORDER BY started_at DESC");

            if let Some(lim) = limit {
                q.push_str(&format!(" LIMIT {}", lim));
            }

            let mut query = sqlx::query_as::<_, AnalyticsRunLineageRow>(&q);

            if let Some(ref ws) = filter.workspace_id {
                query = query.bind(ws.as_str());
            }
            if let Some(ref rev) = filter.revision_id {
                query = query.bind(rev.get() as i64);
            }
            if let Some(ref algo) = filter.algorithm_id {
                query = query.bind(algo.as_str());
            }
            if let Some(ref status) = filter.status {
                query = query.bind(status.to_string());
            }

            query
                .fetch_all(pool)
                .await
                .map_err(|e| AnalyticsError::Internal(format!("lineage query: {e}")))?
        };

        Ok(rows.into_iter().map(|r| r.into_run_lineage()).collect())
    }

    /// Upsert descriptor limits for an algorithm version.
    async fn upsert_descriptor_limits(
        &self,
        algorithm_id: &crate::domain::analytics::AlgorithmId,
        version: &str,
        limits: &PlanLimits,
    ) -> Result<(), AnalyticsError> {
        let pool = &self.pool;
        let limits_json = serde_json::to_value(limits)
            .map_err(|e| AnalyticsError::Internal(format!("serialize limits: {e}")))?;

        sqlx::query(
            r#"
            INSERT INTO descriptor_limits (algorithm_id, algorithm_version, limits, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (algorithm_id) DO UPDATE SET
                algorithm_version = EXCLUDED.algorithm_version,
                limits = EXCLUDED.limits,
                updated_at = NOW()
            "#,
        )
        .bind(algorithm_id.as_str())
        .bind(version)
        .bind(&limits_json)
        .execute(pool)
        .await
        .map_err(|e| AnalyticsError::Internal(format!("upsert descriptor limits: {e}")))?;

        Ok(())
    }

    /// Get descriptor limits for an algorithm version.
    async fn get_descriptor_limits(
        &self,
        algorithm_id: &crate::domain::analytics::AlgorithmId,
        version: &str,
    ) -> Result<Option<PlanLimits>, AnalyticsError> {
        let pool = &self.pool;

        let row: Option<(String, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT algorithm_version, limits
            FROM descriptor_limits
            WHERE algorithm_id = $1 AND algorithm_version = $2
            "#,
        )
        .bind(algorithm_id.as_str())
        .bind(version)
        .fetch_optional(pool)
        .await
        .map_err(|e| AnalyticsError::Internal(format!("get descriptor limits: {e}")))?;

        match row {
            Some((_, limits_json)) => {
                let limits: PlanLimits = serde_json::from_value(limits_json)
                    .map_err(|e| AnalyticsError::Internal(format!("deserialize limits: {e}")))?;
                Ok(Some(limits))
            }
            None => Ok(None),
        }
    }
}

/// Internal row type for analytics_run_lineage queries.
#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow)]
struct AnalyticsRunLineageRow {
    run_id: String,
    workspace_id: String,
    revision_id: i64,
    algorithm_id: String,
    algorithm_version: String,
    plan_hash: Vec<u8>,
    params: serde_json::Value,
    seed: Option<i64>,
    mode: String,
    status: String,
    started_at: String,
    finished_at: Option<String>,
    row_count: Option<i64>,
    truncation_marker: Option<String>,
    idempotency_key: Option<String>,
    error_kind: Option<String>,
    error_message: Option<String>,
}

#[cfg(feature = "postgres")]
impl AnalyticsRunLineageRow {
    fn into_run_lineage(self) -> RunLineage {
        use crate::domain::analytics::AlgorithmId;

        RunLineage {
            run_id: Uuid::from_string(self.run_id),
            workspace_id: WorkspaceId::try_new(self.workspace_id.clone())
                .expect("valid workspace_id from DB"),
            revision_id: RevisionId::new(self.revision_id as u64),
            algorithm_id: AlgorithmId::from_string(self.algorithm_id.clone()),
            algorithm_version: self.algorithm_version,
            plan_hash: self.plan_hash,
            params: self.params,
            seed: self.seed.map(|s| s as u64),
            mode: match self.mode.as_str() {
                "stream" => AnalyticsMode::Stream,
                "stats" => AnalyticsMode::Stats,
                "annotate" => AnalyticsMode::Annotate,
                "persist" => AnalyticsMode::Persist,
                _ => AnalyticsMode::Stream,
            },
            status: match self.status.as_str() {
                "pending" => RunStatus::Pending,
                "running" => RunStatus::Running,
                "succeeded" => RunStatus::Succeeded,
                "truncated" => RunStatus::Truncated,
                "failed" => RunStatus::Failed,
                _ => RunStatus::Pending,
            },
            started_at: DateTime::parse_from_rfc3339(&self.started_at)
                .expect("valid timestamp from DB")
                .with_timezone(&Utc),
            finished_at: self.finished_at.map(|s|
                DateTime::parse_from_rfc3339(&s)
                    .expect("valid timestamp from DB")
                    .with_timezone(&Utc)
            ),
            row_count: self.row_count,
            truncation_marker: self.truncation_marker.map(|s| match s.as_str() {
                "ResultRowsLimit" => TruncationMarker::ResultRowsLimit,
                "PathCountLimit" => TruncationMarker::PathCountLimit,
                "VisitedNodesLimit" => TruncationMarker::VisitedNodesLimit,
                "VisitedEdgesLimit" => TruncationMarker::VisitedEdgesLimit,
                _ => TruncationMarker::ResultRowsLimit,
            }),
            idempotency_key: self.idempotency_key,
            error_kind: self.error_kind,
            error_message: self.error_message,
        }
    }
}
