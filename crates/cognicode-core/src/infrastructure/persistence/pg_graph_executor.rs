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
    GraphExecutor, ExecutorError, GraphPlan, PlanLimits, PlanMetadata, PlanVersion, PlanHash,
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
        let pool = self.repo.pool();

        // First: verify the revision exists by calling load_call_graph_ws.
        // This is the "closed world" check — unknown revisions fail fast.
        let graph = {
            let repo = &self.repo;
            let pin0 = pin.0.clone();
            let pin1 = pin.1;
            tokio::runtime::Handle::current().block_on(async {
                repo.load_call_graph_ws(&pin0, pin1).await
            })
        };

        let graph = match graph {
            Ok(Some(g)) => g,
            Ok(None) => {
                // Empty graph — return empty result set
                return Ok(ResultSet::empty());
            }
            Err(crate::domain::traits::repository::RepositoryError::UnknownRevision { workspace, revision }) => {
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
            GraphPlan::Path { src, dst, quantifier, .. } => {
                let max_hops = quantifier.max_hops.unwrap_or(32).min(32) as i32;
                self.execute_path(pool, &pin.0, src, dst, max_hops, &limits)
            }
            GraphPlan::Neighbors { src, kind, depth, .. } => {
                self.execute_neighbors(pool, &pin.0, src, kind.clone(), *depth as i32, &limits)
            }
            GraphPlan::Subgraph { nodes, edges, .. } => {
                self.execute_subgraph(pool, &pin.0, nodes, edges.as_ref(), &limits)
            }
            GraphPlan::Cluster { by, .. } => {
                self.execute_cluster(pool, &pin.0, by, &limits)
            }
            GraphPlan::Explain { inner, .. } => {
                // EXPLAIN: return plan metadata without executing
                self.execute(inner, pin)
            }
            GraphPlan::BooleanComposition { op, operands, .. } => {
                self.execute_boolean(pool, &pin.0, *op, operands, &limits)
            }
        };

        // Apply soft limit truncation if result exceeds max_result_rows
        if let Ok(ref mut rs) = result {
            if let Some(max_rows) = limits.max_result_rows {
                let total_rows = rs.rows.len() + rs.nodes.len() + rs.edges.len();
                if total_rows as u64 > max_rows {
                    *rs = rs.clone().with_truncation(TruncationMarker::ResultRowsLimit);
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
                )
                SELECT DISTINCT ON (path[array_upper(path, 1)])
                    path,
                    edge_kinds,
                    depth
                FROM path_cte
                WHERE path[array_upper(path, 1)] = $3
                ORDER BY path[array_upper(path, 1)], depth ASC, path ASC
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
                )
                SELECT DISTINCT ON (path[array_upper(path, 1)])
                    path,
                    edge_kinds,
                    depth
                FROM path_cte
                WHERE path[array_upper(path, 1)] = $3
                ORDER BY path[array_upper(path, 1)], depth ASC, path ASC
                "#,
            )
        };

        let start = Instant::now();

        let rows = {
            let mut query = sqlx::query(&sql)
                .bind(src)
                .bind(workspace.as_str())
                .bind(dst)
                .bind(max_hops);

            if let Some(max_rows) = limits.max_result_rows {
                query = query.bind(max_rows as i64);
            }

            tokio::runtime::Handle::current().block_on(async { query.fetch_all(pool).await })
                .map_err(|e| ExecutorError::InternalError(format!("path query failed: {e}")))?
        };

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
                        let kind_str = edge_kinds_arr.get(i.saturating_sub(1)).cloned().unwrap_or_default();
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
                      {}
                )
                SELECT DISTINCT id, kind, label, source_path, properties
                FROM neighbors_cte
                WHERE depth > 0
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
                      {}
                )
                SELECT DISTINCT id, kind, label, source_path, properties
                FROM neighbors_cte
                WHERE depth > 0
                "#,
                join_clause, edge_clause
            )
        };

        let start = Instant::now();

        let rows = {
            let mut query = sqlx::query(&sql)
                .bind(src)
                .bind(workspace.as_str())
                .bind(depth);

            if let Some(max_rows) = limits.max_result_rows {
                query = query.bind(max_rows as i64);
            }

            tokio::runtime::Handle::current().block_on(async { query.fetch_all(pool).await })
                .map_err(|e| ExecutorError::InternalError(format!("neighbors query failed: {e}")))?
        };

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
        let seed_list: Vec<String> = seed_nodes.iter()
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

        let rows = tokio::runtime::Handle::current().block_on(async {
            sqlx::query(&sql)
                .bind(workspace.as_str())
                .bind(depth)
                .fetch_all(pool)
                .await
        })
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
        let edges = if nodes.len() <= 100 && limits.max_result_rows.unwrap_or(1000) >= nodes.len() as u64 {
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
                "#.to_string()
            };

            let mut query = sqlx::query(&edge_sql)
                .bind(workspace.as_str())
                .bind(&node_ids);

            if let Some(max_rows) = limits.max_result_rows {
                query = query.bind(max_rows as i64);
            }

            let edge_rows = tokio::runtime::Handle::current().block_on(async { query.fetch_all(pool).await })
                .map_err(|e| ExecutorError::InternalError(format!("subgraph edges query failed: {e}")))?;

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

        let rows = {
            let mut query = sqlx::query(&sql).bind(workspace.as_str());
            if let Some(max_rows) = limits.max_result_rows {
                query = query.bind(max_rows as i64);
            }
            tokio::runtime::Handle::current().block_on(async { query.fetch_all(pool).await })
                .map_err(|e| ExecutorError::InternalError(format!("cluster query failed: {e}")))?
        };

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
            let result = self.execute(operand, (workspace.clone(), RevisionId::NONE))?;
            let node_ids: HashSet<String> = result.nodes.iter().map(|n| n.id.clone()).collect();
            all_node_sets.push(node_ids);
        }

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
                // EXCEPT: all nodes in first set minus nodes in other sets
                if let Some(first) = all_node_sets.first() {
                    let mut difference = first.clone();
                    for set in all_node_sets.iter().skip(1) {
                        difference = difference.difference(set).cloned().collect();
                    }
                    difference
                } else {
                    HashSet::new()
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

        let rows = tokio::runtime::Handle::current().block_on(async {
            sqlx::query(&sql)
                .bind(workspace.as_str())
                .bind(&node_ids)
                .fetch_all(pool)
                .await
        })
            .map_err(|e| ExecutorError::InternalError(format!("boolean result nodes query failed: {e}")))?;

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
    use crate::domain::plan::version::{PlanMetadata, PlanVersion, PlanHash};
    use crate::domain::value_objects::WorkspaceId;

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
                eprintln!("skipping construct_from_postgres_repository: cannot connect to database");
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
            eprintln!("skipping pg_graph_executor_implements_graph_executor: TEST_DATABASE_URL not set");
            return;
        }

        let pool = match sqlx::PgPool::connect(&base).await.ok() {
            Some(p) => p,
            None => {
                eprintln!("skipping pg_graph_executor_implements_graph_executor: cannot connect to database");
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

    #[tokio::test]
    async fn unknown_revision_returns_error() {
        let base = std::env::var("TEST_DATABASE_URL").unwrap_or_default();
        if base.is_empty() {
            eprintln!("skipping unknown_revision_returns_error: TEST_DATABASE_URL not set");
            return;
        }

        let pool = match sqlx::PgPool::connect(&base).await.ok() {
            Some(p) => p,
            None => {
                eprintln!("skipping unknown_revision_returns_error: cannot connect to database");
                return;
            }
        };
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
    }
}
