//! PgGraphExecutor — PostgreSQL-backed `GraphExecutor` implementation.
//!
//! Part of e28-2-differential-graph-executors: PR2 Phase 2.
//!
//! ## Architecture
//!
//! `PgGraphExecutor` wraps a `PostgresRepository` and implements the
//! `GraphExecutor` port trait. Execution flow:
//!
//! 1. `execute()` calls `load_call_graph_ws` to verify the revision exists.
//! 2. `execute_pg()` dispatches to variant-specific methods.
//! 3. Each method runs SQL queries directly against `graph_nodes`/`graph_edges`.
//! 4. Errors from `load_call_graph_ws` (`RepositoryError::UnknownRevision`) are
//!    translated to `ExecutorError::RevisionUnknown`.
//!
//! ## SQL Patterns
//!
//! - `PATH`: `WITH RECURSIVE` CTE bounded by `max_hops ≤ 32`.
//! - `NEIGHBORS`: iterative CTE or `UNION ALL` per depth level.
//! - `SUBGRAPH`: BFS via recursive CTE starting from seed nodes.
//! - `CLUSTER`: `GROUP BY` aggregation on node properties.
//! - `BOOLEAN`: typed multiset operations — `INTERSECT`/`UNION ALL`/`EXCEPT`.
//! - `LIMIT` pushed into SQL for `max_result_rows` enforcement.

#[cfg(feature = "postgres")]
use std::collections::HashSet;
#[cfg(feature = "postgres")]
use std::str::FromStr;
#[cfg(feature = "postgres")]
use std::time::Instant;

#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};

#[cfg(feature = "postgres")]
use crate::domain::plan::graph_plan::{BooleanOp, NeighborKind};
#[cfg(feature = "postgres")]
use crate::domain::plan::result::{EdgeResult, NodeResult, Path, PathHop, ResultSet};
#[cfg(feature = "postgres")]
use crate::domain::plan::value::TypedValue;
#[cfg(feature = "postgres")]
use crate::domain::plan::{
    ExecutorError, GraphExecutor, GraphPlan, PlanHash, PlanLimits, PlanMetadata, PlanVersion,
    TruncationMarker,
};
#[cfg(feature = "postgres")]
use crate::domain::value_objects::{EdgeKind, RevisionId, WorkspaceId};
#[cfg(feature = "postgres")]
use crate::infrastructure::persistence::PostgresRepository;

// ============================================================================
// PgGraphExecutor
// ============================================================================

/// PostgreSQL-backed graph executor.
///
/// Holds a `PostgresRepository` and implements `GraphExecutor` by running
/// SQL queries directly against `graph_nodes` and `graph_edges`.
#[cfg(feature = "postgres")]
pub struct PgGraphExecutor {
    repo: PostgresRepository,
}

#[cfg(feature = "postgres")]
impl std::fmt::Debug for PgGraphExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgGraphExecutor").finish()
    }
}

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Construct a `PgGraphExecutor` from a `PostgresRepository`.
    pub fn new(repo: PostgresRepository) -> Self {
        Self { repo }
    }

    /// Returns a reference to the underlying repository.
    pub fn repo(&self) -> &PostgresRepository {
        &self.repo
    }

    /// Run an async closure on the current Tokio runtime via `block_in_place` +
    /// `Handle::current` + `tokio::spawn`. This is the proven working pattern
    /// (matches `snapshot_provider.rs::current_head` / `snapshot`) and avoids:
    ///
    /// - The `Handle::block_on` panic when called from a Tokio worker thread
    ///   (multi-thread runtime).
    /// - The PG connection-pool leak caused by spawning a fresh `Runtime` per
    ///   call: a new runtime's lifecycle interferes with the shared pool's
    ///   internal state (tokio handles, mutexes) that was initialized in the
    ///   caller runtime, leading to "pool timed out" errors on subsequent
    ///   acquire attempts.
    ///
    /// The caller must be inside a Tokio runtime (multi-thread or current_thread).
    /// `block_in_place` blocks the current thread (must be a worker in multi_thread
    /// or any thread where blocking is acceptable) while the async work runs on
    /// the same runtime via `tokio::spawn`.
    fn execute_async<T, F, Fut>(&self, f: F) -> Result<T, ExecutorError>
    where
        F: FnOnce(PostgresRepository) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, sqlx::Error>> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.repo.with_pool(|p| p.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let repo = PostgresRepository::from_pool(pool);
                let result = f(repo).await;
                let _ = tx.send(result);
            });
        });
        rx.recv()
            .expect("PG executor task panicked")
            .map_err(|e| ExecutorError::InternalError(format!("async query failed: {e}")))
    }
}

#[cfg(feature = "postgres")]
impl GraphExecutor for PgGraphExecutor {
    fn execute(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
    ) -> Result<ResultSet, ExecutorError> {
        // Execute with plan limits
        self.execute_with_limits(plan, pin, None)
    }

    fn execute_with_limits(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
        limits_override: Option<PlanLimits>,
    ) -> Result<ResultSet, ExecutorError> {
        let limits = limits_override.unwrap_or_else(|| plan.limits().clone());
        let pool = self.repo.with_pool(|p| p.clone());

        // First: verify the revision exists by calling load_call_graph_ws.
        // This is the "closed world" check — unknown revisions fail fast.
        //
        // Use the working `block_in_place` pattern (matches `snapshot_provider.rs`):
        // run the async load on the *current* Tokio runtime via `block_in_place +
        // Handle::current + handle.enter + tokio::spawn`. This avoids:
        //   - `Handle::block_on` panic from a Tokio worker thread (multi-thread runtime)
        //   - PG connection-pool leak from spawning a fresh `Runtime` per call
        //     (which would interfere with the shared pool's tokio state and cause
        //     "pool timed out" errors)
        let pin0 = pin.0.clone();
        let pin1 = pin.1;
        let pool_for_load = self.repo.with_pool(|p| p.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            let tx2 = tx.clone();
            tokio::spawn(async move {
                let repo = PostgresRepository::from_pool(pool_for_load);
                let result = repo.load_call_graph_ws(&pin0, pin1).await;
                let _ = tx2.send(result);
            });
        });
        let graph = rx.recv().expect("PG executor task panicked");

        let graph = match graph {
            Ok(Some(g)) => g,
            Ok(None) => {
                // Empty graph — return empty result set
                return Ok(ResultSet::empty());
            }
            Err(crate::domain::traits::repository::CallGraphStoreError::UnknownRevision {
                workspace,
                revision,
            }) => {
                let pin_str = format!("{}:{}", workspace.as_str(), revision.get());
                return Err(ExecutorError::RevisionUnknown(pin_str));
            }
            Err(e) => {
                return Err(ExecutorError::InternalError(format!(
                    "load_call_graph_ws failed: {e}"
                )));
            }
        };

        // Now dispatch to variant-specific executor
        let mut result = match plan {
            GraphPlan::Path {
                src,
                dst,
                quantifier,
                edge_kind_filter,
                ..
            } => {
                let max_hops = quantifier.max_hops.unwrap_or(32).min(32) as i32;
                self.execute_path(
                    &pool,
                    &pin.0,
                    src,
                    dst,
                    max_hops,
                    edge_kind_filter.as_deref(),
                    &limits,
                )
            }
            GraphPlan::Neighbors {
                src,
                kind,
                depth,
                edge_kind_filter,
                ..
            } => self.execute_neighbors(
                &pool,
                &pin.0,
                src,
                kind.clone(),
                *depth as i32,
                edge_kind_filter.as_deref(),
                &limits,
            ),
            GraphPlan::Subgraph { nodes, edges, .. } => {
                self.execute_subgraph(&pool, &pin.0, nodes, edges.as_ref(), &limits)
            }
            GraphPlan::Cluster { by, .. } => self.execute_cluster(&pool, &pin.0, by, &limits),
            GraphPlan::Explain { inner, .. } => {
                // EXPLAIN: return plan metadata without executing
                self.execute(inner, pin)
            }
            GraphPlan::BooleanComposition { op, operands, .. } => {
                self.execute_boolean(&pool, &pin.0, pin.1, *op, operands, &limits)
            }
        };

        // Apply soft limit truncation if result exceeds max_result_rows or max_path_count
        if let Ok(ref mut rs) = result {
            if let Some(max_rows) = limits.max_result_rows {
                let total_rows = rs.rows.len() + rs.nodes.len() + rs.edges.len();
                if total_rows as u64 > max_rows {
                    *rs = rs
                        .clone()
                        .with_truncation(TruncationMarker::ResultRowsLimit);
                }
            }
            if let Some(max_paths) = limits.max_path_count {
                if rs.paths.len() as u64 > max_paths {
                    // Mark as truncated; the executor already truncated at the SQL LIMIT.
                    *rs = rs.clone().with_truncation(TruncationMarker::PathCountLimit);
                    rs.paths.truncate(max_paths as usize);
                }
            }
        }

        result
    }
}

// ============================================================================
// Path execution — WITH RECURSIVE CTE
// ============================================================================

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Execute a shortest-path query using a `WITH RECURSIVE` CTE.
    ///
    /// The CTE starts from `src`, walks edges, and stops when `dst` is reached
    /// or `max_hops` is exhausted. Returns all simple paths (no cycles) in order
    /// of discovery (BFS order).
    ///
    /// Bounded by `max_hops ≤ 32` per spec.
    fn execute_path(
        &self,
        pool: &PgPool,
        workspace: &WorkspaceId,
        src: &str,
        dst: &str,
        max_hops: i32,
        edge_kind_filter: Option<&[crate::domain::value_objects::DependencyType]>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        // max_hops is already bounded to 32 by caller

        let sql = if limits.max_result_rows.is_some() {
            format!(
                r#"
                WITH RECURSIVE path_cte AS (
                    SELECT
                        n.id AS node_id,
                        ARRAY[n.id] AS path,
                        ARRAY[]::text[] AS edge_kinds,
                        0 AS depth,
                        false AS reached_dst
                    FROM graph_nodes n
                    WHERE n.id = $1 AND n.workspace_id = $2

                    UNION ALL

                    SELECT
                        e.target_id,
                        pc.path || e.target_id,
                        pc.edge_kinds || e.kind,
                        pc.depth + 1,
                        e.target_id = $3
                    FROM path_cte pc
                    JOIN graph_edges e ON e.source_id = pc.node_id
                    WHERE pc.depth < $4
                      AND NOT (e.target_id = ANY(pc.path))
                      AND e.workspace_id = $2
                      AND NOT pc.reached_dst
                      AND ($6::text[] IS NULL OR e.kind = ANY($6))
                )
                -- DISTINCT (path) deduplicates by full path sequence (the
                -- PG executor must agree with the snapshot executor which
                -- returns all simple paths; previously used DISTINCT ON
                -- (path[last]) which collapsed parallel paths per endpoint).
                SELECT DISTINCT path,
                    edge_kinds,
                    depth
                FROM path_cte
                WHERE path[array_upper(path, 1)] = $3
                ORDER BY depth ASC, path ASC
                LIMIT $5
                "#,
            )
        } else {
            format!(
                r#"
                WITH RECURSIVE path_cte AS (
                    SELECT
                        n.id AS node_id,
                        ARRAY[n.id] AS path,
                        ARRAY[]::text[] AS edge_kinds,
                        0 AS depth,
                        false AS reached_dst
                    FROM graph_nodes n
                    WHERE n.id = $1 AND n.workspace_id = $2

                    UNION ALL

                    SELECT
                        e.target_id,
                        pc.path || e.target_id,
                        pc.edge_kinds || e.kind,
                        pc.depth + 1,
                        e.target_id = $3
                    FROM path_cte pc
                    JOIN graph_edges e ON e.source_id = pc.node_id
                    WHERE pc.depth < $4
                      AND NOT (e.target_id = ANY(pc.path))
                      AND e.workspace_id = $2
                      AND NOT pc.reached_dst
                      AND ($5::text[] IS NULL OR e.kind = ANY($5))
                )
                SELECT DISTINCT path,
                    edge_kinds,
                    depth
                FROM path_cte
                WHERE path[array_upper(path, 1)] = $3
                ORDER BY depth ASC, path ASC
                "#,
            )
        };

        let start = Instant::now();

        // Run the async query on the current Tokio runtime via `block_in_place`
        // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
        // All data that needs to be used in the spawn closure must be cloned.
        let pool_clone = pool.clone();
        let sql_clone = sql.clone();
        let workspace_str = workspace.as_str().to_string();
        let src_clone = src.to_string();
        let dst_clone = dst.to_string();
        // Map DependencyType filter → DB string form ("dependency.calls", etc.)
        let edge_kind_db_filter: Option<Vec<String>> = edge_kind_filter
            .map(|kinds| kinds.iter().map(|k| format!("dependency.{}", k)).collect());
        // Capture owned max_result_rows to satisfy 'static.
        let max_result_rows_owned: Option<i64> = limits.max_result_rows.map(|v| v as i64);
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let mut query = sqlx::query(&sql_clone)
                    .bind(&src_clone)
                    .bind(&workspace_str)
                    .bind(&dst_clone)
                    .bind(max_hops);
                if let Some(max_rows) = max_result_rows_owned {
                    query = query.bind(max_rows).bind(edge_kind_db_filter.clone());
                } else {
                    query = query.bind(edge_kind_db_filter.clone());
                }
                let result = query.fetch_all(&pool_clone).await;
                let _ = tx.send(result);
            });
        });
        let rows = rx
            .recv()
            .expect("path query task panicked")
            .map_err(|e| ExecutorError::InternalError(format!("path query failed: {e}")))?;

        let mut paths = Vec::new();
        for row in rows {
            let path_arr: Vec<String> = row.get("path");
            let edge_kinds_arr: Vec<String> = row.get("edge_kinds");

            // Build Path with hops
            let hops: Vec<PathHop> = path_arr
                .iter()
                .enumerate()
                .map(|(i, node_id)| {
                    let edge_kind = if i == 0 {
                        None
                    } else {
                        let kind_str = edge_kinds_arr
                            .get(i.saturating_sub(1))
                            .cloned()
                            .unwrap_or_default();
                        let edge_kind = parse_edge_kind(&kind_str);
                        Some(edge_kind)
                    };
                    PathHop {
                        node_id: node_id.clone(),
                        edge_kind,
                    }
                })
                .collect();

            paths.push(Path { hops });
        }

        // Check time limit
        if let Some(time_ms) = limits.time_ms {
            if start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation {
            if token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }
        }

        Ok(ResultSet {
            rows: vec![],
            nodes: vec![],
            edges: vec![],
            paths,
            scalars: vec![],
            truncated: false,
            truncation: None,
        })
    }
}

// ============================================================================
// Neighbors execution
// ============================================================================

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Execute a neighbors query: all nodes reachable from `src` at `depth`
    /// with edge direction `kind`.
    fn execute_neighbors(
        &self,
        pool: &PgPool,
        workspace: &WorkspaceId,
        src: &str,
        kind: NeighborKind,
        depth: i32,
        edge_kind_filter: Option<&[crate::domain::value_objects::DependencyType]>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        let (edge_clause, join_clause) = match kind {
            NeighborKind::Both => (
                "".to_string(),
                "e.source_id = nc.id OR e.target_id = nc.id".to_string(),
            ),
            NeighborKind::Outgoing => (
                "AND e.source_id = nc.id".to_string(),
                "e.source_id = nc.id".to_string(),
            ),
            NeighborKind::Incoming => (
                "AND e.target_id = nc.id".to_string(),
                "e.target_id = nc.id".to_string(),
            ),
        };

        let sql = if limits.max_result_rows.is_some() {
            format!(
                r#"
                WITH RECURSIVE neighbors_cte AS (
                    SELECT n.id, n.kind, n.label, n.source_path, n.properties, 0 AS depth
                    FROM graph_nodes n
                    WHERE n.id = $1 AND n.workspace_id = $2

                    UNION

                    SELECT next_n.id, next_n.kind, next_n.label, next_n.source_path, next_n.properties, nc.depth + 1
                    FROM neighbors_cte nc
                    JOIN graph_edges e ON ({})
                    JOIN graph_nodes next_n ON (
                        (e.source_id = nc.id AND next_n.id = e.target_id)
                        OR (e.target_id = nc.id AND next_n.id = e.source_id)
                    )
                    WHERE nc.depth < $3
                      AND next_n.workspace_id = $2
                      AND ($5::text[] IS NULL OR e.kind = ANY($5))
                      {}
                )
                SELECT DISTINCT id, kind, label, source_path, properties
                FROM neighbors_cte
                WHERE depth > 0
                ORDER BY id ASC
                LIMIT $4
                "#,
                join_clause, edge_clause
            )
        } else {
            format!(
                r#"
                WITH RECURSIVE neighbors_cte AS (
                    SELECT n.id, n.kind, n.label, n.source_path, n.properties, 0 AS depth
                    FROM graph_nodes n
                    WHERE n.id = $1 AND n.workspace_id = $2

                    UNION

                    SELECT next_n.id, next_n.kind, next_n.label, next_n.source_path, next_n.properties, nc.depth + 1
                    FROM neighbors_cte nc
                    JOIN graph_edges e ON ({})
                    JOIN graph_nodes next_n ON (
                        (e.source_id = nc.id AND next_n.id = e.target_id)
                        OR (e.target_id = nc.id AND next_n.id = e.source_id)
                    )
                    WHERE nc.depth < $3
                      AND next_n.workspace_id = $2
                      AND ($4::text[] IS NULL OR e.kind = ANY($4))
                      {}
                )
                SELECT DISTINCT id, kind, label, source_path, properties
                FROM neighbors_cte
                WHERE depth > 0
                ORDER BY id ASC
                "#,
                join_clause, edge_clause
            )
        };

        let start = Instant::now();

        // Run the async query on the current Tokio runtime via `block_in_place`
        // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
        let pool_clone = pool.clone();
        let sql_clone = sql.clone();
        let workspace_str = workspace.as_str().to_string();
        let src_clone = src.to_string();
        let max_rows_opt = limits.max_result_rows;
        let edge_kind_db_filter: Option<Vec<String>> = edge_kind_filter
            .map(|kinds| kinds.iter().map(|k| format!("dependency.{}", k)).collect());
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let mut query = sqlx::query(&sql_clone)
                    .bind(&src_clone)
                    .bind(&workspace_str)
                    .bind(depth);
                if let Some(max_rows) = max_rows_opt {
                    query = query
                        .bind(max_rows as i64)
                        .bind(edge_kind_db_filter.clone());
                } else {
                    query = query.bind(edge_kind_db_filter.clone());
                }
                let result = query.fetch_all(&pool_clone).await;
                let _ = tx.send(result);
            });
        });
        let rows = rx
            .recv()
            .expect("neighbors query task panicked")
            .map_err(|e| ExecutorError::InternalError(format!("neighbors query failed: {e}")))?;

        let mut nodes = Vec::new();
        let mut seen = HashSet::new();
        for row in rows {
            let id: String = row.get("id");
            if seen.contains(&id) {
                continue;
            }
            seen.insert(id.clone());

            let kind_str: String = row.get("kind");
            let label: String = row.get("label");
            let labels = parse_node_labels(&kind_str);

            nodes.push(NodeResult {
                id,
                labels,
                properties: vec![],
            });
        }

        // Check time limit
        if let Some(time_ms) = limits.time_ms {
            if start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation {
            if token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }
        }

        let truncated = limits.max_result_rows.is_some()
            && nodes.len() as u64 >= limits.max_result_rows.unwrap();

        Ok(ResultSet {
            rows: vec![],
            nodes,
            edges: vec![],
            paths: vec![],
            scalars: vec![],
            truncated,
            truncation: if truncated {
                Some(TruncationMarker::ResultRowsLimit)
            } else {
                None
            },
        })
    }
}

// ============================================================================
// Subgraph execution — BFS via recursive CTE
// ============================================================================

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Execute a subgraph query: all nodes reachable from `nodes` within `depth`
    /// hops, including the edges between them.
    fn execute_subgraph(
        &self,
        pool: &PgPool,
        workspace: &WorkspaceId,
        seed_nodes: &[String],
        _edges_filter: Option<&Vec<EdgeResult>>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        let depth = limits.max_depth.unwrap_or(5) as i32;
        let depth = depth.min(32);

        if seed_nodes.is_empty() {
            return Ok(ResultSet::empty());
        }

        // Build the seed node list for SQL
        let seed_list: Vec<String> = seed_nodes
            .iter()
            .map(|s| format!("'{}'", s.replace('\'', "''")))
            .collect();
        let seed_sql = seed_list.join(", ");

        let sql = format!(
            r#"
            WITH RECURSIVE subgraph_cte AS (
                SELECT n.id, n.kind, n.label, n.source_path, n.properties, 0 AS depth
                FROM graph_nodes n
                WHERE n.id IN ({seed_sql}) AND n.workspace_id = $1

                UNION

                SELECT next_n.id, next_n.kind, next_n.label, next_n.source_path, next_n.properties, sc.depth + 1
                FROM subgraph_cte sc
                JOIN graph_edges e ON e.source_id = sc.id OR e.target_id = sc.id
                JOIN graph_nodes next_n ON (
                    (e.source_id = sc.id AND next_n.id = e.target_id)
                    OR (e.target_id = sc.id AND next_n.id = e.source_id)
                )
                WHERE sc.depth < $2
                  AND next_n.workspace_id = $1
            )
            SELECT DISTINCT id, kind, label, source_path, properties
            FROM subgraph_cte
            "#,
        );

        let start = Instant::now();

        // Run the async query on the current Tokio runtime via `block_in_place`
        // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
        let pool_clone = pool.clone();
        let sql_clone = sql.clone();
        let workspace_str = workspace.as_str().to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let result = sqlx::query(&sql_clone)
                    .bind(&workspace_str)
                    .bind(depth)
                    .fetch_all(&pool_clone)
                    .await;
                let _ = tx.send(result);
            });
        });
        let rows = rx
            .recv()
            .expect("subgraph query task panicked")
            .map_err(|e| ExecutorError::InternalError(format!("subgraph query failed: {e}")))?;

        let mut nodes = Vec::new();
        let mut seen_ids = HashSet::new();
        for row in rows {
            let id: String = row.get("id");
            if seen_ids.contains(&id) {
                continue;
            }
            seen_ids.insert(id.clone());

            let kind_str: String = row.get("kind");
            let label: String = row.get("label");
            let labels = parse_node_labels(&kind_str);

            nodes.push(NodeResult {
                id,
                labels,
                properties: vec![],
            });
        }

        // Now fetch the edges between visited nodes (limit to reasonable size)
        let edges =
            if nodes.len() <= 100 && limits.max_result_rows.unwrap_or(1000) >= nodes.len() as u64 {
                let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();

                let edge_sql = if let Some(max_rows) = limits.max_result_rows {
                    format!(
                        r#"
                    SELECT e.source_id, e.target_id, e.kind, e.provenance, e.confidence
                    FROM graph_edges e
                    WHERE e.workspace_id = $1
                      AND e.source_id = ANY($2)
                      AND e.target_id = ANY($2)
                    LIMIT $3
                    "#,
                    )
                } else {
                    r#"
                SELECT e.source_id, e.target_id, e.kind, e.provenance, e.confidence
                FROM graph_edges e
                WHERE e.workspace_id = $1
                  AND e.source_id = ANY($2)
                  AND e.target_id = ANY($2)
                "#
                    .to_string()
                };

                let mut query = sqlx::query(&edge_sql)
                    .bind(workspace.as_str())
                    .bind(&node_ids);

                if let Some(max_rows) = limits.max_result_rows {
                    query = query.bind(max_rows as i64);
                }

                // Run the async query on the current Tokio runtime via `block_in_place`
                // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
                let pool_clone = pool.clone();
                let edge_sql_clone = edge_sql.clone();
                let node_ids_clone = node_ids.clone();
                let workspace_str = workspace.as_str().to_string();
                let max_rows_opt = limits.max_result_rows;
                let (tx, rx) = std::sync::mpsc::channel();
                tokio::task::block_in_place(move || {
                    let handle = tokio::runtime::Handle::current();
                    let _enter = handle.enter();
                    tokio::spawn(async move {
                        let mut query = sqlx::query(&edge_sql_clone)
                            .bind(&workspace_str)
                            .bind(&node_ids_clone);
                        if let Some(max_rows) = max_rows_opt {
                            query = query.bind(max_rows as i64);
                        }
                        let result = query.fetch_all(&pool_clone).await;
                        let _ = tx.send(result);
                    });
                });
                let edge_rows = rx
                    .recv()
                    .expect("subgraph edges query task panicked")
                    .map_err(|e| {
                        ExecutorError::InternalError(format!("subgraph edges query failed: {e}"))
                    })?;

                edge_rows
                    .iter()
                    .map(|row| {
                        let src: String = row.get("source_id");
                        let dst: String = row.get("target_id");
                        let kind: String = row.get("kind");
                        let provenance: String = row.get("provenance");
                        let confidence: f32 = row.get("confidence");

                        EdgeResult {
                            id: format!("{}->{}", src, dst),
                            src,
                            dst,
                            label: kind.clone(),
                            properties: vec![
                                TypedValue::String(provenance),
                                TypedValue::Float(confidence as f64),
                            ],
                        }
                    })
                    .collect()
            } else {
                vec![]
            };

        // Check time limit
        if let Some(time_ms) = limits.time_ms {
            if start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation {
            if token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }
        }

        Ok(ResultSet {
            rows: vec![],
            nodes,
            edges,
            paths: vec![],
            scalars: vec![],
            truncated: false,
            truncation: None,
        })
    }
}

// ============================================================================
// Cluster execution — GROUP BY
// ============================================================================

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Execute a cluster query: group nodes by `by` properties and count them.
    fn execute_cluster(
        &self,
        pool: &PgPool,
        workspace: &WorkspaceId,
        by: &[String],
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        if by.is_empty() {
            return Ok(ResultSet::empty());
        }

        // Use the first grouping key
        let group_key = &by[0];

        // Map group_key to actual column: "Kind" -> "kind", otherwise -> "label"
        let group_col = match group_key.as_str() {
            "Kind" => "kind",
            _ => "label",
        };

        let sql = if let Some(max_rows) = limits.max_result_rows {
            format!(
                r#"
                SELECT {group_col} AS group_key, COUNT(*) AS count
                FROM graph_nodes
                WHERE workspace_id = $1
                GROUP BY {group_col}
                ORDER BY count DESC
                LIMIT $2
                "#,
            )
        } else {
            format!(
                r#"
                SELECT {group_col} AS group_key, COUNT(*) AS count
                FROM graph_nodes
                WHERE workspace_id = $1
                GROUP BY {group_col}
                ORDER BY count DESC
                "#,
            )
        };

        let start = Instant::now();

        // Run the async query on the current Tokio runtime via `block_in_place`
        // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
        let pool_clone = pool.clone();
        let sql_clone = sql.clone();
        let workspace_str = workspace.as_str().to_string();
        let max_rows_opt = limits.max_result_rows;
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let mut query = sqlx::query(&sql_clone).bind(&workspace_str);
                if let Some(max_rows) = max_rows_opt {
                    query = query.bind(max_rows as i64);
                }
                let result = query.fetch_all(&pool_clone).await;
                let _ = tx.send(result);
            });
        });
        let rows = rx
            .recv()
            .expect("cluster query task panicked")
            .map_err(|e| ExecutorError::InternalError(format!("cluster query failed: {e}")))?;

        let mut scalars = Vec::new();
        for row in rows {
            let count: i64 = row.get("count");
            scalars.push(TypedValue::Int(count));
        }

        // Check time limit
        if let Some(time_ms) = limits.time_ms {
            if start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation {
            if token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: crate::domain::plan::PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }
        }

        Ok(ResultSet {
            rows: vec![],
            nodes: vec![],
            edges: vec![],
            paths: vec![],
            scalars,
            truncated: false,
            truncation: None,
        })
    }
}

// ============================================================================
// Boolean composition — typed multiset: INTERSECT / UNION ALL / EXCEPT
// ============================================================================

#[cfg(feature = "postgres")]
impl PgGraphExecutor {
    /// Execute a boolean composition on sub-plans.
    ///
    /// - `And`: INTERSECT (set intersection of node IDs)
    /// - `Or`: UNION ALL (set union, keeping duplicates)
    /// - `Not`: EXCEPT (set difference — all nodes except those in operand)
    fn execute_boolean(
        &self,
        pool: &PgPool,
        workspace: &WorkspaceId,
        revision: RevisionId,
        op: BooleanOp,
        operands: &[GraphPlan],
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        if operands.is_empty() {
            return Ok(ResultSet::empty());
        }

        // For boolean ops, we collect node IDs from each operand
        let mut all_node_sets: Vec<HashSet<String>> = Vec::new();

        for operand in operands {
            let result = self.execute(operand, (workspace.clone(), revision))?;
            let node_ids: HashSet<String> = result.nodes.iter().map(|n| n.id.clone()).collect();
            all_node_sets.push(node_ids);
        }

        // Compute the universe (all node IDs in the graph) for `Not`.
        // We issue a direct SQL query because `Subgraph { nodes: [] }` is
        // documented as a no-op (`seed_nodes.is_empty()` returns early).
        let universe: HashSet<String> = if matches!(op, BooleanOp::Not) {
            let pool_clone = pool.clone();
            let workspace_owned = workspace.as_str().to_string();
            let universe_ids: Vec<String> = tokio::task::block_in_place(move || {
                let handle = tokio::runtime::Handle::current();
                let _enter = handle.enter();
                let (tx, rx) = std::sync::mpsc::channel();
                tokio::spawn(async move {
                    let result: Result<Vec<String>, sqlx::Error> = sqlx::query_scalar(
                        "SELECT id FROM graph_nodes WHERE workspace_id = $1 ORDER BY id",
                    )
                    .bind(&workspace_owned)
                    .fetch_all(&pool_clone)
                    .await;
                    let _ = tx.send(result);
                });
                rx.recv()
                    .expect("universe query task panicked")
                    .unwrap_or_default()
            });
            universe_ids.into_iter().collect()
        } else {
            HashSet::new()
        };

        let result_ids: HashSet<String> = match op {
            BooleanOp::And => {
                // INTERSECT: keep only nodes in ALL sets
                if let Some(first) = all_node_sets.first() {
                    let mut intersection = first.clone();
                    for set in all_node_sets.iter().skip(1) {
                        intersection = intersection.intersection(set).cloned().collect();
                    }
                    intersection
                } else {
                    HashSet::new()
                }
            }
            BooleanOp::Or => {
                // UNION ALL: all nodes from all sets
                let mut union = HashSet::new();
                for set in &all_node_sets {
                    union.extend(set.iter().cloned());
                }
                union
            }
            BooleanOp::Not => {
                // EXCEPT: universe minus the first operand set. Not is unary.
                if let Some(first) = all_node_sets.first() {
                    let mut difference = universe.clone();
                    for v in first {
                        difference.remove(v);
                    }
                    difference
                } else {
                    universe.clone()
                }
            }
        };

        // Now fetch the actual node details
        if result_ids.is_empty() {
            return Ok(ResultSet::empty());
        }

        let node_ids: Vec<String> = result_ids.into_iter().collect();
        let limit_sql = if let Some(max_rows) = limits.max_result_rows {
            format!("LIMIT {}", max_rows as i64)
        } else {
            String::new()
        };

        let sql = format!(
            r#"
            SELECT id, kind, label, source_path, properties
            FROM graph_nodes
            WHERE workspace_id = $1 AND id = ANY($2)
            {limit_sql}
            "#,
        );

        // Run the async query on the current Tokio runtime via `block_in_place`
        // + `tokio::spawn` (avoids pool leak from per-call `Runtime::new()`).
        let pool_clone = pool.clone();
        let sql_clone = sql.clone();
        let node_ids_clone = node_ids.clone();
        let workspace_str = workspace.as_str().to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let result = sqlx::query(&sql_clone)
                    .bind(&workspace_str)
                    .bind(&node_ids_clone)
                    .fetch_all(&pool_clone)
                    .await;
                let _ = tx.send(result);
            });
        });
        let rows = rx
            .recv()
            .expect("boolean result nodes query task panicked")
            .map_err(|e| {
                ExecutorError::InternalError(format!("boolean result nodes query failed: {e}"))
            })?;

        let nodes: Vec<NodeResult> = rows
            .iter()
            .map(|row| {
                let id: String = row.get("id");
                let kind_str: String = row.get("kind");
                let label: String = row.get("label");
                let labels = parse_node_labels(&kind_str);

                NodeResult {
                    id,
                    labels,
                    properties: vec![],
                }
            })
            .collect();

        Ok(ResultSet {
            rows: vec![],
            nodes,
            edges: vec![],
            paths: vec![],
            scalars: vec![],
            truncated: false,
            truncation: None,
        })
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Parse an edge kind string like "dependency.calls" into an `EdgeKind`.
#[cfg(feature = "postgres")]
fn parse_edge_kind(kind: &str) -> EdgeKind {
    use crate::domain::value_objects::DependencyType;
    let stripped = kind.strip_prefix("dependency.").unwrap_or(kind);
    let dep_type = DependencyType::from_str(stripped).unwrap_or(DependencyType::Calls);
    EdgeKind::Dependency(dep_type)
}

/// Parse node labels from a kind string like "symbol.function" -> ["function"].
#[cfg(feature = "postgres")]
fn parse_node_labels(kind: &str) -> Vec<String> {
    if kind.starts_with("symbol.") {
        vec![kind.strip_prefix("symbol.").unwrap_or(kind).to_string()]
    } else {
        vec![kind.to_string()]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[cfg(feature = "postgres")]
mod tests {
    use super::*;
    use crate::domain::plan::graph_plan::{PathProjection, PathQuantifier};
    use crate::domain::plan::version::{PlanHash, PlanMetadata, PlanVersion};
    use crate::domain::value_objects::WorkspaceId;
    use sqlx::PgPool;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique counter for test database names
    static UNIQ: AtomicU64 = AtomicU64::new(0);

    /// Replace the database segment in a `postgres://...` URL
    fn rewrite_db_name(url: &str, new_name: &str) -> String {
        if let Some(at_idx) = url.rfind('@') {
            let (head, tail) = url.split_at(at_idx);
            if let Some(slash_idx) = tail.find('/') {
                let (host, _) = tail.split_at(slash_idx);
                return format!("{head}{host}/{new_name}");
            }
        }
        url.trim_end_matches('/').to_string() + "/" + new_name
    }

    /// Create a fresh test database with migrations run
    async fn fresh_pool() -> Option<PgPool> {
        use crate::infrastructure::persistence::PostgresRepository;

        let base = std::env::var("TEST_DATABASE_URL").ok()?;
        let n = UNIQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let db_name = format!("cognicode_test_{pid}_{n}");
        let admin_url = base.clone();
        let test_url = rewrite_db_name(&admin_url, &db_name);

        // Create the unique DB (idempotent: drop first if it somehow lingers)
        let admin = sqlx::PgPool::connect(&admin_url).await.ok()?;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
            .execute(&admin)
            .await;
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin)
            .await
            .ok()?;

        // Connect to the new DB and run migrations
        let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
        PostgresRepository::from_pool(pool.clone())
            .run_migrations()
            .await
            .ok()?;

        Some(pool)
    }

    /// pg_test macro for pg_graph_executor tests. Uses `flavor = "multi_thread"`
    /// because `PgGraphExecutor::execute_with_limits` calls
    /// `tokio::task::block_in_place`, which requires a multi-thread runtime.
    /// Without this, tests panic with "can call blocking only when running
    /// on the multi-threaded runtime". Required since v0.70.1 (pool fix).
    macro_rules! pg_test {
        ($name:ident, |$pool:ident: PgPool| $body:tt) => {
            #[tokio::test(flavor = "multi_thread")]
            async fn $name() {
                let Some($pool) = fresh_pool().await else {
                    eprintln!("skipping {}: TEST_DATABASE_URL not set", stringify!($name));
                    return;
                };
                async fn inner($pool: PgPool) {
                    $body
                }
                inner($pool).await
            }
        };
    }

    // -------------------------------------------------------------------------
    // Task 2.5a RED — Path shortest path succeeds (A→D with max_hops=3)
    // Scenario: pg-graph-executor::Path Variant Materializes Paths::Shortest path succeeds
    // Assert: ResultSet.paths is non-empty, starts at A, ends at D, hop count ≤ 3
    // -------------------------------------------------------------------------

    pg_test!(path_shortest_succeeds, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→B→C→D with A→D direct
        let mut graph = CallGraph::new();

        // Add symbols A, B, C, D
        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_c = SymbolId::new("src/C.rs:C:1");
        let id_d = SymbolId::new("src/D.rs:D:1");

        let sym_a = Symbol::new("A", SymbolKind::Function, Location::new("src/A.rs", 1, 0));
        let sym_b = Symbol::new("B", SymbolKind::Function, Location::new("src/B.rs", 1, 0));
        let sym_c = Symbol::new("C", SymbolKind::Function, Location::new("src/C.rs", 1, 0));
        let sym_d = Symbol::new("D", SymbolKind::Function, Location::new("src/D.rs", 1, 0));

        graph.add_symbol(sym_a);
        graph.add_symbol(sym_b);
        graph.add_symbol(sym_c);
        graph.add_symbol(sym_d);

        // Add edges: A→B, A→D, B→C, C→D
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_d,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_b,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_c,
            &id_d,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        // Save the graph
        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        // Execute path query: A → D with max_hops=3
        let executor = PgGraphExecutor::new(repo);
        let plan = GraphPlan::Path {
            src: "src/A.rs:A:1".to_string(),
            dst: "src/D.rs:D:1".to_string(),
            quantifier: PathQuantifier {
                max_hops: Some(3),
                min_hops: 0,
            },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws.clone(), rev));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // (debug print removed)

        // Should have at least one path
        assert!(
            !rs.paths.is_empty(),
            "Expected at least one path from A to D, got {:?}",
            rs.paths
        );

        // All paths should start at A and end at D
        for path in &rs.paths {
            let first = path.hops.first().map(|h| h.node_id.as_str());
            let last = path.hops.last().map(|h| h.node_id.as_str());
            assert_eq!(first, Some("src/A.rs:A:1"), "path should start at A");
            assert_eq!(last, Some("src/D.rs:D:1"), "path should end at D");
            // `path.hops.len()` counts nodes; max_hops counts edges. With
            // max_hops=3 and self-loop-free paths, the longest valid path
            // has 4 nodes (= 3 edges).
            assert!(
                path.hops.len() as i32 <= 4,
                "node count should be ≤ max_hops + 1 (got {} hops)",
                path.hops.len()
            );
        }
    });

    // -------------------------------------------------------------------------
    // Task 2.6 RED — Unreachable destination returns empty
    // Scenario: pg-graph-executor::Path Variant Materializes Paths::Unreachable destination returns empty
    // Assert: ResultSet.paths is empty
    // -------------------------------------------------------------------------

    pg_test!(path_unreachable_returns_empty, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→B only (no path to Z)
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Function,
            Location::new("src/B.rs", 1, 0),
        ));
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);
        let plan = GraphPlan::Path {
            src: "src/A.rs:A:1".to_string(),
            dst: "src/Z.rs:Z:1".to_string(), // Doesn't exist
            quantifier: PathQuantifier {
                max_hops: Some(5),
                min_hops: 0,
            },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();
        assert!(
            rs.paths.is_empty(),
            "Expected empty paths for unreachable destination"
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.7 RED — Outgoing neighbors at depth 1
    // Scenario: pg-graph-executor::Neighbors + Subgraph + Cluster + Explain::Outgoing neighbors at depth 1
    // Assert: ResultSet.rows contains {B, C}, NOT D
    // -------------------------------------------------------------------------

    pg_test!(neighbors_outgoing_depth_1, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::plan::graph_plan::NeighborKind;
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→B, A→C, D→A
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_c = SymbolId::new("src/C.rs:C:1");
        let id_d = SymbolId::new("src/D.rs:D:1");

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Function,
            Location::new("src/B.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "D",
            SymbolKind::Function,
            Location::new("src/D.rs", 1, 0),
        ));

        // A→B, A→C, D→A
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_d,
            &id_a,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);
        let plan = GraphPlan::Neighbors {
            src: "src/A.rs:A:1".to_string(),
            kind: NeighborKind::Outgoing,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();

        // Should have B and C, not D
        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(
            node_ids.contains(&"src/B.rs:B:1"),
            "Should contain B, got {:?}",
            node_ids
        );
        assert!(
            node_ids.contains(&"src/C.rs:C:1"),
            "Should contain C, got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/D.rs:D:1"),
            "Should NOT contain D (incoming)"
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.8 RED — Subgraph returns visited nodes
    // Scenario: pg-graph-executor::Neighbors + Subgraph + Cluster + Explain::Subgraph returns visited nodes
    // Assert: ResultSet.nodes contains {A, B, C}, edges {A→B, B→C}
    // -------------------------------------------------------------------------

    pg_test!(subgraph_returns_visited_nodes, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→B→C→D
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_c = SymbolId::new("src/C.rs:C:1");
        let id_d = SymbolId::new("src/D.rs:D:1");

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Function,
            Location::new("src/B.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "D",
            SymbolKind::Function,
            Location::new("src/D.rs", 1, 0),
        ));

        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_b,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_c,
            &id_d,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);
        let plan = GraphPlan::Subgraph {
            nodes: vec![
                "src/A.rs:A:1".to_string(),
                "src/B.rs:B:1".to_string(),
                "src/C.rs:C:1".to_string(),
            ],
            edges: None, // None means include all edges between specified nodes
            aggregations: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();

        // Should have A, B, C (the nodes we asked for)
        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(node_ids.contains(&"src/A.rs:A:1"), "Should contain A");
        assert!(node_ids.contains(&"src/B.rs:B:1"), "Should contain B");
        assert!(node_ids.contains(&"src/C.rs:C:1"), "Should contain C");

        // Should have edges A→B and B→C
        let edge_ids: Vec<&str> = rs.edges.iter().map(|e| e.id.as_str()).collect();
        assert!(
            edge_ids.contains(&"src/A.rs:A:1->src/B.rs:B:1"),
            "Should contain edge A->B, got {:?}",
            edge_ids
        );
        assert!(
            edge_ids.contains(&"src/B.rs:B:1->src/C.rs:C:1"),
            "Should contain edge B->C, got {:?}",
            edge_ids
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.9 RED — Cluster by kind
    // Scenario: pg-graph-executor::Neighbors + Subgraph + Cluster + Explain::Cluster by kind
    // Assert: One row per kind with count >= 1
    // -------------------------------------------------------------------------

    pg_test!(cluster_by_kind, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol};
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: mixed kinds
        let mut graph = CallGraph::new();

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Class,
            Location::new("src/B.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 0),
        ));

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);
        let plan = GraphPlan::Cluster {
            by: vec!["Kind".to_string()],
            aggregations: vec![],
            ordering: None,
            limit: None,
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();

        // Should have scalars with counts (one per kind)
        assert!(!rs.scalars.is_empty(), "Expected at least one cluster row");

        // Each scalar should be an Int with count >= 1
        for scalar in &rs.scalars {
            if let crate::domain::plan::value::TypedValue::Int(count) = scalar {
                assert!(*count >= 1, "count should be >= 1");
            }
        }
    });

    // -------------------------------------------------------------------------
    // Task 2.11 RED — AND intersection
    // Scenario: pg-graph-executor::Boolean Composition Typed Multiset::AND intersection
    // Assert: And(Neighbors(A,Out,1), Neighbors(B,Out,1)) → {C}
    // -------------------------------------------------------------------------

    pg_test!(bool_and_intersection, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::plan::graph_plan::{BooleanOp, NeighborKind};
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→{B,C}, B→C
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_c = SymbolId::new("src/C.rs:C:1");

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Function,
            Location::new("src/B.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 0),
        ));

        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_b,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);

        let neighbors_a = GraphPlan::Neighbors {
            src: "src/A.rs:A:1".to_string(),
            kind: NeighborKind::Outgoing,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let neighbors_b = GraphPlan::Neighbors {
            src: "src/B.rs:B:1".to_string(),
            kind: NeighborKind::Outgoing,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::And,
            operands: vec![neighbors_a, neighbors_b],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // AND intersection should give C (common neighbor of A and B)
        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            node_ids,
            vec!["src/C.rs:C:1"],
            "AND intersection should return {{C}}, got {:?}",
            node_ids
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.12 RED — NOT complement
    // Scenario: pg-graph-executor::Boolean Composition Typed Multiset::NOT complement
    // Assert: Not(Neighbors(A,Out,1)) returns all nodes EXCEPT {B, C}
    // -------------------------------------------------------------------------

    pg_test!(bool_not_complement, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::plan::graph_plan::{BooleanOp, NeighborKind};
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→{B,C}, D isolated
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_c = SymbolId::new("src/C.rs:C:1");
        let id_d = SymbolId::new("src/D.rs:D:1");

        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "B",
            SymbolKind::Function,
            Location::new("src/B.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 0),
        ));
        graph.add_symbol(Symbol::new(
            "D",
            SymbolKind::Function,
            Location::new("src/D.rs", 1, 0),
        ));

        // A→B, A→C, D is isolated
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);

        let neighbors_a = GraphPlan::Neighbors {
            src: "src/A.rs:A:1".to_string(),
            kind: NeighborKind::Outgoing,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::Not,
            operands: vec![neighbors_a],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();

        // NOT complement should return all nodes EXCEPT B and C (the neighbors of A)
        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(
            !node_ids.contains(&"src/B.rs:B:1"),
            "Should NOT contain B, got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/C.rs:C:1"),
            "Should NOT contain C, got {:?}",
            node_ids
        );
        assert!(
            node_ids.contains(&"src/A.rs:A:1"),
            "Should contain A (A is not in Neighbors(A))"
        );
        assert!(
            node_ids.contains(&"src/D.rs:D:1"),
            "Should contain D (isolated node)"
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.14 RED — max_result_rows truncated at SQL boundary
    // Scenario: pg-graph-executor::Plan Limit Enforcement::max_result_rows truncated at SQL boundary
    // Assert: ResultSet.rows.len() == 10, truncated: true, truncation: Some(ResultRowsLimit)
    // -------------------------------------------------------------------------

    pg_test!(max_result_rows_truncated, |pool: PgPool| {
        use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
        use crate::domain::plan::graph_plan::NeighborKind;
        use crate::domain::services::ExtractionContext;
        use crate::domain::traits::repository::CallGraphStore;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
        use crate::infrastructure::persistence::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("test_ws").unwrap();

        // Build fixture: A→{B01, B02, ..., B50} (50 neighbors)
        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        graph.add_symbol(Symbol::new(
            "A",
            SymbolKind::Function,
            Location::new("src/A.rs", 1, 0),
        ));

        for i in 0..50 {
            let name = format!("B{:02}", i);
            let fqn = format!("src/{}.rs:{}:1", name, name);
            let sym_id = SymbolId::new(&fqn);
            graph.add_symbol(Symbol::new(
                &name,
                SymbolKind::Function,
                Location::new(&format!("src/{}.rs", name), 1, 0),
            ));
            let _ = graph.add_dependency_with_provenance(
                &id_a,
                &sym_id,
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );
        }

        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save should succeed");

        let executor = PgGraphExecutor::new(repo);

        // Query with max_result_rows = 10
        let plan = GraphPlan::Neighbors {
            src: "src/A.rs:A:1".to_string(),
            kind: NeighborKind::Outgoing,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits {
                max_result_rows: Some(10),
                ..Default::default()
            },
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok(), "execute should succeed");
        let rs = result.unwrap();

        // Should be truncated to 10 nodes
        assert_eq!(
            rs.nodes.len(),
            10,
            "Should have exactly 10 nodes due to limit"
        );
        assert!(rs.truncated, "ResultSet should be marked as truncated");
        assert_eq!(
            rs.truncation,
            Some(TruncationMarker::ResultRowsLimit),
            "TruncationMarker should be ResultRowsLimit"
        );
    });

    // -------------------------------------------------------------------------
    // Task 2.1a RED — PgGraphExecutor::new(&PostgresRepository) constructor test
    // Scenario: pg-graph-executor::Construction::Construct from pool
    // Assert: `&dyn GraphExecutor` compiles
    // -------------------------------------------------------------------------

    /// `PgGraphExecutor::new` constructs from a `PostgresRepository`.
    #[tokio::test]
    async fn construct_from_postgres_repository() {
        let base = std::env::var("TEST_DATABASE_URL").unwrap_or_default();
        if base.is_empty() {
            eprintln!("skipping construct_from_postgres_repository: TEST_DATABASE_URL not set");
            return;
        }

        // Create a pool and repository
        let pool = match sqlx::PgPool::connect(&base).await.ok() {
            Some(p) => p,
            None => {
                eprintln!(
                    "skipping construct_from_postgres_repository: cannot connect to database"
                );
                return;
            }
        };
        let repo = PostgresRepository::from_pool(pool);
        let executor = PgGraphExecutor::new(repo);

        // Verify it implements GraphExecutor (can be called with a &dyn GraphExecutor)
        fn _assert_executor(_: &dyn GraphExecutor) {}
        _assert_executor(&executor);
    }

    // -------------------------------------------------------------------------
    // Task 2.1b GREEN — PgGraphExecutor struct + impl GraphExecutor
    // -------------------------------------------------------------------------

    /// `PgGraphExecutor` implements `GraphExecutor` and can be used via `&dyn GraphExecutor`.
    #[tokio::test]
    async fn pg_graph_executor_implements_graph_executor() {
        let base = std::env::var("TEST_DATABASE_URL").unwrap_or_default();
        if base.is_empty() {
            eprintln!(
                "skipping pg_graph_executor_implements_graph_executor: TEST_DATABASE_URL not set"
            );
            return;
        }

        let pool = match sqlx::PgPool::connect(&base).await.ok() {
            Some(p) => p,
            None => {
                eprintln!(
                    "skipping pg_graph_executor_implements_graph_executor: cannot connect to database"
                );
                return;
            }
        };
        let repo = PostgresRepository::from_pool(pool);
        let executor = PgGraphExecutor::new(repo);

        // Verify GraphExecutor trait is implemented
        fn _assert_executor(_: &dyn GraphExecutor) {}
        _assert_executor(&executor);
    }

    // -------------------------------------------------------------------------
    // Task 2.2 RED — unknown revision rejection
    // Scenario: pg-graph-executor::RevisionHandling::Unknown workspace:revision
    // Assert: ExecutorError::RevisionUnknown is returned
    // -------------------------------------------------------------------------

    /// Regression test for e28-2-pr2-pool-connection-release: the previous
    /// dedicated-OS-thread + `Runtime::new()` approach leaked PG pool
    /// connections on the first SELECT inside `load_call_graph_ws` for an
    /// unknown revision, leading to "pool timed out". After switching to the
    /// `block_in_place + Handle::current + tokio::spawn` pattern (which keeps
    /// the async SQL work on the caller's Tokio runtime), this scenario must
    /// return `ExecutorError::RevisionUnknown` instead of an InternalError.
    pg_test!(unknown_revision_returns_error, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let executor = PgGraphExecutor::new(repo);

        // Create a Path plan with a known source but non-existent revision
        let plan = GraphPlan::Path {
            src: "test_node".to_string(),
            dst: "other_node".to_string(),
            quantifier: PathQuantifier {
                max_hops: Some(3),
                min_hops: 0,
            },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let ws = WorkspaceId::try_new("test_ws").unwrap();
        let rev = RevisionId(999999); // Non-existent revision

        // Execute and verify we get RevisionUnknown error
        let result = executor.execute(&plan, (ws, rev));
        match result {
            Err(ExecutorError::RevisionUnknown(pin)) => {
                // Verify pin format is "workspace:revision"
                assert!(pin.contains("999999"), "pin should contain revision id");
            }
            other => panic!("expected ExecutorError::RevisionUnknown, got {:?}", other),
        }
    });

    // -------------------------------------------------------------------------
    // Task 5 RED — `path_with_edge_kind_filter_eliminates_only_path` (PG side)
    // Scenario: `pg-graph-executor::Plan restricts traversal to listed edge kinds`
    // Assert: `Path(A, C, [Calls])` over a graph where the ONLY route from A
    //         to C requires a `References` edge must return an empty
    //         `ResultSet::paths`. Without the filter, the path exists.
    // Why RED: pre-fix PG recursive CTE does not filter by `e.kind` and walks
    //         every edge indiscriminately. The conformance spec requires that
    //         reachability changes when the filter is applied.
    //
    //   Fixture: A → B (References); B → C (Calls).
    //     Without filter → A→B→C exists.
    //     With filter=[Calls] → A has no Calls outgoing edge → empty paths.
    // -------------------------------------------------------------------------

    pg_test!(
        path_with_edge_kind_filter_eliminates_only_path,
        |pool: PgPool| {
            use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
            use crate::domain::services::ExtractionContext;
            use crate::domain::traits::repository::CallGraphStore;
            use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
            use crate::infrastructure::persistence::PostgresRepository;

            let repo = PostgresRepository::from_pool(pool);
            let ws = WorkspaceId::try_new("ws_edge_filter").unwrap();

            let mut graph = CallGraph::new();
            let id_a = SymbolId::new("src/A.rs:A:1");
            let id_b = SymbolId::new("src/B.rs:B:1");
            let id_c = SymbolId::new("src/C.rs:C:1");
            graph.add_symbol(Symbol::new(
                "A",
                SymbolKind::Function,
                Location::new("src/A.rs", 1, 1),
            ));
            graph.add_symbol(Symbol::new(
                "B",
                SymbolKind::Function,
                Location::new("src/B.rs", 1, 1),
            ));
            graph.add_symbol(Symbol::new(
                "C",
                SymbolKind::Function,
                Location::new("src/C.rs", 1, 1),
            ));
            let _ = graph.add_dependency_with_provenance(
                &id_a,
                &id_b,
                DependencyType::References,
                ExtractionContext::DirectExtraction,
            );
            let _ = graph.add_dependency_with_provenance(
                &id_b,
                &id_c,
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );

            let rev = repo
                .save_call_graph_ws(&graph, &ws)
                .await
                .expect("save should succeed");
            let executor = PgGraphExecutor::new(repo);

            // Sanity: unfiltered plan must yield a path.
            let plan_unfiltered = GraphPlan::Path {
                src: "src/A.rs:A:1".to_string(),
                dst: "src/C.rs:C:1".to_string(),
                quantifier: PathQuantifier {
                    max_hops: Some(3),
                    min_hops: 0,
                },
                edge_kind_filter: None,
                predicates: vec![],
                projection: PathProjection::default(),
                limits: PlanLimits::default(),
                metadata: PlanMetadata::new(
                    PlanVersion::new("1.0.0").unwrap(),
                    PlanHash::compute(&0u32),
                ),
            };
            let rs_unfiltered = executor
                .execute(&plan_unfiltered, (ws.clone(), rev))
                .expect("unfiltered execute must succeed");
            assert!(
                !rs_unfiltered.paths.is_empty(),
                "unfiltered fixture must have at least one path A→B→C, got empty"
            );

            // Filtered: A has no Calls outgoing edge → empty paths.
            let plan_filtered = GraphPlan::Path {
                src: "src/A.rs:A:1".to_string(),
                dst: "src/C.rs:C:1".to_string(),
                quantifier: PathQuantifier {
                    max_hops: Some(3),
                    min_hops: 0,
                },
                edge_kind_filter: Some(vec![DependencyType::Calls]),
                predicates: vec![],
                projection: PathProjection::default(),
                limits: PlanLimits::default(),
                metadata: PlanMetadata::new(
                    PlanVersion::new("1.0.0").unwrap(),
                    PlanHash::compute(&0u32),
                ),
            };
            let rs_filtered = executor
                .execute(&plan_filtered, (ws, rev))
                .expect("filtered execute must succeed (not error)");
            assert!(
                rs_filtered.paths.is_empty(),
                "filtered plan must return empty paths (no Calls edge from A), got {:?}",
                rs_filtered.paths
            );
        }
    );
}
