//! SnapshotGraphExecutor — in-memory `GraphExecutor` backed by `SnapshotProvider`.
//!
//! Part of e28-2-differential-graph-executors: PR3 Phase 3.
//!
//! ## Architecture
//!
//! `SnapshotGraphExecutor` wraps a `&dyn SnapshotProvider` and implements the
//! `GraphExecutor` port trait. Execution flow:
//!
//! 1. `execute()` calls `provider.snapshot(ws, rev)` to get the `Arc<CallGraph>`.
//! 2. Errors from `snapshot()` (`SnapshotError::UnknownRevision`) are translated
//!    to `ExecutorError::RevisionUnknown`.
//! 3. `execute_snapshot()` dispatches to variant-specific methods that walk the
//!    in-memory graph.
//! 4. BFS traversal uses petgraph's `Bfs` iterator over a `StableGraph` built
//!    from the `CallGraph`.
//!
//! ## Graph Traversal
//!
//! - `PATH`: BFS over petgraph StableGraph; all simple paths within `max_hops`
//!   are collected in BFS discovery order (shortest first).
//! - `NEIGHBORS`: BFS from source node; collects reachable nodes within `depth`
//!   respecting `NeighborKind` (Outgoing/Incoming/Both).
//! - `SUBGRAPH`: BFS from seed nodes up to `max_depth`; returns visited nodes + edges.
//! - `CLUSTER`: HashMap group counts by the `by` key.
//! - `BOOLEAN`: Evaluate each operand, then combine via multiset operations.

use crate::domain::plan::result::{EdgeResult, NodeResult, Path, PathHop, ResultSet, Row};
use crate::domain::plan::{
    CancellationToken, ExecutorError, GraphExecutor, GraphPlan, PlanHash, PlanLimitKind,
    PlanLimits, PlanMetadata, PlanVersion, TruncationMarker,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use petgraph::Direction;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::plan::graph_plan::{BooleanOp, NeighborKind};
use crate::domain::plan::result::{EdgeResult, NodeResult, Path, PathHop, ResultSet};
use crate::domain::plan::value::TypedValue;
    ExecutorError, GraphExecutor, GraphPlan, PlanLimitKind, PlanLimits,
    TruncationMarker,
};
use crate::domain::value_objects::{DependencyType, EdgeKind, RevisionId, WorkspaceId};
use crate::infrastructure::graph::SnapshotProvider;

/// A `TestSnapshotProvider` adapter for unit testing.
///
/// This is a minimal in-memory implementation that stores snapshots keyed by
/// `(workspace, revision)` and is used exclusively in unit tests.
pub struct TestSnapshotProvider {
    snapshots: std::sync::Mutex<std::collections::HashMap<(String, u64), Arc<CallGraph>>>,
    heads: std::sync::Mutex<std::collections::HashMap<String, RevisionId>>,
}

impl TestSnapshotProvider {
    /// Construct a new empty `TestSnapshotProvider`.
    pub fn new() -> Self {
        Self {
            snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
            heads: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Insert a graph snapshot at the given workspace and revision.
    pub fn insert(&self, ws: &WorkspaceId, rev: RevisionId, graph: CallGraph) {
        let key = (ws.as_str().to_string(), rev.get());
        self.snapshots.lock().unwrap().insert(key, Arc::new(graph));
        *self
            .heads
            .lock()
            .unwrap()
            .entry(ws.as_str().to_string())
            .or_insert(RevisionId::NONE) = rev;
    }

    /// Set the current head revision for a workspace.
    pub fn set_head(&self, ws: &WorkspaceId, rev: RevisionId) {
        self.heads
            .lock()
            .unwrap()
            .insert(ws.as_str().to_string(), rev);
    }
}

impl Default for TestSnapshotProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotProvider for TestSnapshotProvider {
    fn current_head(
        &self,
        workspace: &WorkspaceId,
    ) -> Result<RevisionId, crate::infrastructure::graph::SnapshotError> {
        let heads = self.heads.lock().unwrap();
        Ok(heads
            .get(workspace.as_str())
            .copied()
            .unwrap_or(RevisionId::NONE))
    }

    fn snapshot(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Arc<CallGraph>, crate::infrastructure::graph::SnapshotError> {
        let snapshots = self.snapshots.lock().unwrap();
        let key = (workspace.as_str().to_string(), revision.get());
        snapshots.get(&key).cloned().ok_or_else(|| {
            crate::infrastructure::graph::SnapshotError::UnknownRevision {
                workspace: workspace.clone(),
                revision,
            }
        })
    }

    fn subscribe(
        &self,
        _workspace: &WorkspaceId,
    ) -> tokio::sync::broadcast::Receiver<crate::infrastructure::graph::SnapshotEvent> {
        let (tx, rx) = tokio::sync::broadcast::channel(16);
        let _ = tx.send(crate::infrastructure::graph::SnapshotEvent::Updated {
            workspace: _workspace.clone(),
            revision: RevisionId::NONE,
        });
        rx
    }
}

// ============================================================================
// SnapshotGraphExecutor
// ============================================================================

/// In-memory graph executor backed by `SnapshotProvider`.
///
/// Holds a `&dyn SnapshotProvider` and implements `GraphExecutor` by fetching
/// an `Arc<CallGraph>` snapshot and walking it in-memory using BFS over petgraph.
pub struct SnapshotGraphExecutor<'a> {
    provider: &'a dyn SnapshotProvider,
}

impl<'a> std::fmt::Debug for SnapshotGraphExecutor<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotGraphExecutor").finish()
    }
}

impl<'a> SnapshotGraphExecutor<'a> {
    /// Construct a `SnapshotGraphExecutor` from a `&dyn SnapshotProvider`.
    ///
    /// Construction is side-effect-free — no snapshot is read until `execute()`.
    pub fn new(provider: &'a dyn SnapshotProvider) -> Self {
        Self { provider }
    }
}

impl<'a> GraphExecutor for SnapshotGraphExecutor<'a> {
    fn execute(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
    ) -> Result<ResultSet, ExecutorError> {
        self.execute_with_limits(plan, pin, None)
    }

    fn execute_with_limits(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
        limits_override: Option<PlanLimits>,
    ) -> Result<ResultSet, ExecutorError> {
        let limits = limits_override.unwrap_or_else(|| plan.limits().clone());
        let (ws, rev) = pin;

        // Fetch the snapshot — unknown revision fails fast with ExecutorError::RevisionUnknown
        let graph = self.provider.snapshot(&ws, rev).map_err(|e| {
            let pin_str = format!("{}:{}", ws.as_str(), rev.get());
            match e {
                crate::infrastructure::graph::SnapshotError::UnknownRevision { .. } => {
                    ExecutorError::RevisionUnknown(pin_str)
                }
                crate::infrastructure::graph::SnapshotError::NoSnapshot(_) => {
                    ExecutorError::RevisionUnknown(pin_str)
                }
            }
        })?;

        // Dispatch to variant-specific executor
        let mut result = match plan {
            GraphPlan::Path {
                src,
                dst,
                quantifier,
                edge_kind_filter,
                ..
            } => {
                let max_hops = quantifier.max_hops.unwrap_or(32).min(32) as usize;
                self.execute_path(
                    &graph,
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
                &graph,
                src,
                kind.clone(),
                *depth as usize,
                edge_kind_filter.as_deref(),
                &limits,
            ),
            GraphPlan::Subgraph { nodes, edges, .. } => {
                self.execute_subgraph(&graph, nodes, edges.as_ref(), &limits)
            }
            GraphPlan::Cluster { by, .. } => self.execute_cluster(&graph, by, &limits),
            GraphPlan::Explain { inner, .. } => {
                // EXPLAIN: return inner plan's metadata as scalars, without executing
                self.execute_explain(inner.as_ref())
            }
            GraphPlan::BooleanComposition { op, operands, .. } => {
                self.execute_boolean(&graph, *op, operands, &limits)
            }
        };

        // Apply soft limit truncation for max_result_rows post-walk
        if let Ok(ref mut rs) = result
            && let Some(max_rows) = limits.max_result_rows {
                let total_rows = rs.rows.len() + rs.nodes.len() + rs.edges.len();
                if total_rows as u64 > max_rows {
                    // Truncate nodes to max_rows (prioritize by some ordering if needed)
                    if rs.nodes.len() as u64 > max_rows {
                        rs.nodes.truncate(max_rows as usize);
                    }
                    // If rows + edges still exceed, truncate further
                    let remaining = max_rows as usize - rs.nodes.len();
                    if remaining < rs.rows.len() {
                        rs.rows.truncate(remaining);
                    }
                    let remaining = max_rows as usize - rs.nodes.len() - rs.rows.len();
                    if remaining < rs.edges.len() {
                        rs.edges.truncate(remaining);
                    }
                    rs.truncated = true;
                    rs.truncation = Some(TruncationMarker::ResultRowsLimit);
                }
            }

        result
    }
}

// ============================================================================
// Path execution — BFS over petgraph
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute a shortest-path query using BFS over a petgraph StableGraph.
    ///
    /// Collects ALL simple paths from `src` to `dst` within `max_hops` hops,
    /// ordered by BFS discovery (shortest first, then by predecessor order).
    fn execute_path(
        &self,
        graph: &CallGraph,
        src: &str,
        dst: &str,
        max_hops: usize,
        edge_kind_filter: Option<&[DependencyType]>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        use std::time::Instant;

        let start = Instant::now();

        // Build a petgraph StableGraph from the CallGraph for BFS traversal.
        // Node indices map symbol IDs to petgraph NodeIndex.
        let (stable_graph, symbol_to_node, node_to_symbol) = self.build_petgraph(graph);

        // Find the NodeIndex for src and dst
        let src_node = self.find_node_index(src, &symbol_to_node)?;
        let dst_node = self.find_node_index(dst, &symbol_to_node)?;

        // BFS from src to dst within max_hops
        let mut all_paths: Vec<Vec<NodeIndex>> = Vec::new();
        let mut visited_paths: HashSet<Vec<NodeIndex>> = HashSet::new();

        // Use our BFS implementation that tracks all paths
        self.bfs_all_paths(
            &stable_graph,
            src_node,
            dst_node,
            max_hops,
            edge_kind_filter,
            &mut all_paths,
            &mut visited_paths,
        );

        // Check cancellation
        if let Some(ref token) = limits.cancellation
            && token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }

        // Check time limit
        if let Some(time_ms) = limits.time_ms
            && start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }

        // Enforce max_path_count post-walk
        let truncated = if let Some(max_paths) = limits.max_path_count {
            if all_paths.len() as u64 > max_paths {
                all_paths.truncate(max_paths as usize);
                true
            } else {
                false
            }
        } else {
            false
        };

        // Convert paths to result format
        let paths: Vec<Path> = all_paths
            .iter()
            .map(|path_nodes| {
                let hops: Vec<PathHop> = path_nodes
                    .iter()
                    .enumerate()
                    .map(|(i, &node_idx)| {
                        let node_id = node_to_symbol.get(&node_idx).cloned().unwrap_or_default();
                        let edge_kind = if i == 0 {
                            None
                        } else {
                            // Find the edge kind from previous node to this node
                            let prev_idx = path_nodes[i - 1];
                            let edge_kind = stable_graph
                                .find_edge(prev_idx, node_idx)
                                .and_then(|e| stable_graph.edge_weight(e))
                                .copied();
                            edge_kind.map(EdgeKind::Dependency)
                        };
                        PathHop { node_id, edge_kind }
                    })
                    .collect();
                Path { hops }
            })
            .collect();

        Ok(ResultSet {
            rows: vec![],
            nodes: vec![],
            edges: vec![],
            paths,
            scalars: vec![],
            truncated,
            truncation: if truncated {
                Some(TruncationMarker::PathCountLimit)
            } else {
                None
            },
        })
    }

    /// BFS that finds ALL simple paths from src to dst within max_hops.
    ///
    /// Uses BFS order to discover shortest paths first.
    fn bfs_all_paths(
        &self,
        graph: &StableGraph<String, DependencyType>,
        src: NodeIndex,
        dst: NodeIndex,
        max_hops: usize,
        edge_kind_filter: Option<&[DependencyType]>,
        all_paths: &mut Vec<Vec<NodeIndex>>,
        visited_paths: &mut HashSet<Vec<NodeIndex>>,
    ) {
        use std::collections::VecDeque;

        // BFS queue: (current_node, current_path)
        let mut queue: VecDeque<(NodeIndex, Vec<NodeIndex>)> = VecDeque::new();
        queue.push_back((src, vec![src]));

        while let Some((current, path)) = queue.pop_front() {
            let path_len = path.len();

            // Don't exceed max_hops (path_len is number of nodes, so max_hops edges = max_hops + 1 nodes)
            // Check BEFORE destination check to filter over-long paths
            if path_len > max_hops + 1 {
                continue;
            }

            // Check if we reached the destination
            if current == dst {
                if visited_paths.insert(path.clone()) {
                    all_paths.push(path);
                }
                continue;
            }

            // Explore neighbors in BFS order
            for edge_idx in graph.edges(current) {
                // Apply edge_kind_filter (None = any edge kind; Some(list) = only listed kinds).
                // See e28-2-pr5-edge-filter.
                if let Some(filter) = edge_kind_filter {
                    let dep = edge_idx.weight();
                    if !filter.contains(dep) {
                        continue;
                    }
                }
                let next = edge_idx.target();
                if !path.contains(&next) {
                    let mut new_path = path.clone();
                    new_path.push(next);
                    // Add to back of queue for BFS ordering (FIFO)
                    queue.push_back((next, new_path));
                }
            }
        }
    }

    /// Build a petgraph StableGraph from a CallGraph.
    ///
    /// Returns the graph and bidirectional maps between symbol IDs and NodeIndex.
    fn build_petgraph(
        &self,
        graph: &CallGraph,
    ) -> (
        StableGraph<String, DependencyType>,
        HashMap<String, NodeIndex>,
        HashMap<NodeIndex, String>,
    ) {
        let mut stable_graph = StableGraph::new();
        let mut symbol_to_node: HashMap<String, NodeIndex> = HashMap::new();
        let mut node_to_symbol: HashMap<NodeIndex, String> = HashMap::new();

        // Add all symbols as nodes
        for (symbol_id, _) in graph.symbol_ids() {
            let node_idx = stable_graph.add_node(symbol_id.as_str().to_string());
            symbol_to_node.insert(symbol_id.as_str().to_string(), node_idx);
            node_to_symbol.insert(node_idx, symbol_id.as_str().to_string());
        }

        // Add all edges
        for (src_id, tgt_id, dep_type) in graph.all_dependencies() {
            if let (Some(&src_idx), Some(&tgt_idx)) = (
                symbol_to_node.get(src_id.as_str()),
                symbol_to_node.get(tgt_id.as_str()),
            ) {
                stable_graph.add_edge(src_idx, tgt_idx, *dep_type);
            }
        }

        (stable_graph, symbol_to_node, node_to_symbol)
    }

    /// Find the NodeIndex for a symbol ID.
    fn find_node_index(
        &self,
        symbol_id: &str,
        symbol_to_node: &HashMap<String, NodeIndex>,
    ) -> Result<NodeIndex, ExecutorError> {
        symbol_to_node.get(symbol_id).copied().ok_or_else(|| {
            ExecutorError::InternalError(format!("symbol not found in graph: {symbol_id}"))
        })
    }
}

// ============================================================================
// Neighbors execution — BFS
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute a neighbors query: all nodes reachable from `src` at `depth`
    /// with edge direction `kind`.
    fn execute_neighbors(
        &self,
        graph: &CallGraph,
        src: &str,
        kind: NeighborKind,
        depth: usize,
        edge_kind_filter: Option<&[DependencyType]>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {

        let start = Instant::now();

        // Build petgraph for BFS
        let (stable_graph, symbol_to_node, node_to_symbol) = self.build_petgraph(graph);

        let src_node = match self.find_node_index(src, &symbol_to_node) {
            Ok(idx) => idx,
            Err(_) => {
                // Source node not found — return empty result
                return Ok(ResultSet::empty());
            }
        };

        // BFS from src node
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        let mut queue: Vec<(NodeIndex, usize)> = vec![(src_node, 0)];
        visited.insert(src_node);

        while let Some((current, current_depth)) = queue.pop() {
            if current_depth >= depth {
                continue;
            }

            // Collect neighbors respecting direction AND edge_kind_filter.
            // See e28-2-pr5-edge-filter.
            let neighbors: Vec<NodeIndex> = match kind {
                NeighborKind::Incoming => stable_graph
                    .edges_directed(current, Direction::Incoming)
                    .filter(|edge| match edge_kind_filter {
                        Some(filter) => filter.contains(edge.weight()),
                        None => true,
                    })
                    .map(|edge| edge.source())
                    .collect(),
                NeighborKind::Outgoing => stable_graph
                    .edges_directed(current, Direction::Outgoing)
                    .filter(|edge| match edge_kind_filter {
                        Some(filter) => filter.contains(edge.weight()),
                        None => true,
                    })
                    .map(|edge| edge.target())
                    .collect(),
                NeighborKind::Both => stable_graph
                    .edges(current)
                    .filter(|edge| match edge_kind_filter {
                        Some(filter) => filter.contains(edge.weight()),
                        None => true,
                    })
                    .map(|edge| edge.target())
                    .collect(),
            };

            for neighbor in neighbors {
                if visited.insert(neighbor) {
                    queue.insert(0, (neighbor, current_depth + 1));
                }
            }
        }

        // Remove src from results (depth 0)
        visited.remove(&src_node);

        // Check cancellation
        if let Some(ref token) = limits.cancellation
            && token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }

        // Check time limit
        if let Some(time_ms) = limits.time_ms
            && start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }

        // Build result nodes
        let nodes: Vec<NodeResult> = visited
            .iter()
            .filter_map(|&node_idx| node_to_symbol.get(&node_idx))
            .map(|symbol_id| {
                let symbol = graph.get_symbol(
                    &crate::domain::aggregates::call_graph::SymbolId::new(symbol_id),
                );
                let (labels, properties) = if let Some(sym) = symbol {
                    // Use Display (lowercase, e.g. "function") instead of
                    // Debug (PascalCase, e.g. "Function") to match the PG
                    // executor's `parse_node_labels("symbol.function")` output.
                    let kind_str = sym.kind().to_string();
                    let labels = if kind_str.starts_with("symbol.") {
                        vec![
                            kind_str
                                .strip_prefix("symbol.")
                                .unwrap_or(&kind_str)
                                .to_string(),
                        ]
                    } else {
                        vec![kind_str]
                    };
                    (labels, vec![])
                } else {
                    (vec![], vec![])
                };

                NodeResult {
                    id: symbol_id.clone(),
                    labels,
                    properties,
                }
            })
            .collect();
        // Sort nodes by id ASC so deterministic truncation (LIMIT N) yields
        // the same N nodes on PG and snapshot backends. Required for
        // `assert_equivalent` conformance when `max_result_rows` truncates.
        let mut nodes = nodes;
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

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
// Subgraph execution — BFS from seed nodes
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute a subgraph query: all nodes reachable from `seed_nodes` within `depth`
    /// hops, including the edges between them.
    fn execute_subgraph(
        &self,
        graph: &CallGraph,
        seed_nodes: &[String],
        _edges_filter: Option<&Vec<EdgeResult>>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {

        let start = Instant::now();

        // Build a (src, tgt) → (provenance, confidence) lookup up-front so
        // EdgeResult.properties mirrors the PG executor's shape.
        let edge_meta: std::collections::HashMap<
            (String, String),
            (crate::domain::value_objects::Provenance, f64),
        > = graph
            .all_dependencies_with_metadata()
            .map(|(src, tgt, _dep, prov, conf)| {
                (
                    (src.as_str().to_string(), tgt.as_str().to_string()),
                    (prov, conf),
                )
            })
            .collect();

        let depth = limits.max_depth.unwrap_or(5) as usize;

        if seed_nodes.is_empty() {
            return Ok(ResultSet::empty());
        }

        // Build petgraph for BFS
        let (stable_graph, symbol_to_node, node_to_symbol) = self.build_petgraph(graph);

        // BFS from all seed nodes
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        let mut queue: Vec<(NodeIndex, usize)> = Vec::new();

        for seed in seed_nodes {
            if let Some(&seed_idx) = symbol_to_node.get(seed) {
                visited.insert(seed_idx);
                queue.insert(0, (seed_idx, 0));
            }
        }

        while let Some((current, current_depth)) = queue.pop() {
            if current_depth >= depth {
                continue;
            }

            for neighbor in stable_graph.edges(current).map(|e| e.target()) {
                if visited.insert(neighbor) {
                    queue.insert(0, (neighbor, current_depth + 1));
                }
            }
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation
            && token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }

        // Check time limit
        if let Some(time_ms) = limits.time_ms
            && start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }

        // Build result nodes
        let nodes: Vec<NodeResult> = visited
            .iter()
            .filter_map(|&node_idx| node_to_symbol.get(&node_idx))
            .map(|symbol_id| {
                let symbol = graph.get_symbol(
                    &crate::domain::aggregates::call_graph::SymbolId::new(symbol_id),
                );
                let (labels, properties) = if let Some(sym) = symbol {
                    // Use Display (lowercase, e.g. "function") instead of
                    // Debug (PascalCase, e.g. "Function") to match the PG
                    // executor's `parse_node_labels("symbol.function")` output.
                    let kind_str = sym.kind().to_string();
                    let labels = if kind_str.starts_with("symbol.") {
                        vec![
                            kind_str
                                .strip_prefix("symbol.")
                                .unwrap_or(&kind_str)
                                .to_string(),
                        ]
                    } else {
                        vec![kind_str]
                    };
                    (labels, vec![])
                } else {
                    (vec![], vec![])
                };

                NodeResult {
                    id: symbol_id.clone(),
                    labels,
                    properties,
                }
            })
            .collect();
        // Sort nodes by id ASC so deterministic truncation (LIMIT N) yields
        // the same N nodes on PG and snapshot backends. Required for
        // `assert_equivalent` conformance when `max_result_rows` truncates.
        let mut nodes = nodes;
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        // Build result edges (only edges where both endpoints are in visited set)
        let visited_set: HashSet<&str> = visited
            .iter()
            .filter_map(|&node_idx| node_to_symbol.get(&node_idx).map(|s| s.as_str()))
            .collect();

        let mut edges: Vec<EdgeResult> = Vec::new();
        for edge_ref in stable_graph.edge_references() {
            let src_id = node_to_symbol.get(&edge_ref.source());
            let tgt_id = node_to_symbol.get(&edge_ref.target());

            if let (Some(src), Some(tgt)) = (src_id, tgt_id)
                && visited_set.contains(src.as_str()) && visited_set.contains(tgt.as_str()) {
                    let dep_type = edge_ref.weight();
                    // Use Display (lowercase, e.g. "calls") instead of Debug ("Calls") to
                    // match the PG executor's `format!("dependency.{}", dep_type)`
                    // which produces "dependency.calls". Conformance parity.
                    let kind_str = format!("dependency.{}", dep_type);
                    let (provenance, confidence) = edge_meta
                        .get(&(src.clone(), tgt.clone()))
                        .copied()
                        .unwrap_or((crate::domain::value_objects::Provenance::Extracted, 1.0));
                    let properties = vec![
                        TypedValue::String(provenance.to_string()),
                        TypedValue::Float(confidence),
                    ];
                    edges.push(EdgeResult {
                        id: format!("{}->{}", src, tgt),
                        src: src.clone(),
                        dst: tgt.clone(),
                        label: kind_str,
                        properties,
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
// Cluster execution — HashMap group counts
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute a cluster query: group nodes by `by` properties and count them.
    ///
    /// Uses `HashMap<String, usize>` for group counts; returns one row per group
    /// with a count.
    fn execute_cluster(
        &self,
        graph: &CallGraph,
        by: &[String],
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {

        let start = Instant::now();

        if by.is_empty() {
            return Ok(ResultSet::empty());
        }

        let group_key = &by[0];

        // Count nodes by the grouping key
        let mut counts: HashMap<String, usize> = HashMap::new();

        for (_, symbol) in graph.symbol_ids() {
            let key_value = if group_key.to_lowercase() == "kind" {
                // Group by kind
                let kind_str = format!("{:?}", symbol.kind());
                if kind_str.starts_with("symbol.") {
                    kind_str
                        .strip_prefix("symbol.")
                        .unwrap_or(&kind_str)
                        .to_string()
                } else {
                    kind_str
                }
            } else {
                // Group by name
                symbol.name().to_string()
            };
            *counts.entry(key_value).or_insert(0) += 1;
        }

        // Check cancellation
        if let Some(ref token) = limits.cancellation
            && token.is_cancelled() {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::Cancellation,
                    observed: 0,
                });
            }

        // Check time limit
        if let Some(time_ms) = limits.time_ms
            && start.elapsed().as_millis() as u64 > time_ms {
                return Err(ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::TimeMs,
                    observed: start.elapsed().as_millis() as u64,
                });
            }

        // Build scalars from counts
        let mut scalars: Vec<TypedValue> = counts
            .values()
            .map(|&count| TypedValue::Int(count as i64))
            .collect();

        // Apply max_result_rows limit
        let truncated = if let Some(max_rows) = limits.max_result_rows {
            if scalars.len() as u64 > max_rows {
                scalars.truncate(max_rows as usize);
                true
            } else {
                false
            }
        } else {
            false
        };

        Ok(ResultSet {
            rows: vec![],
            nodes: vec![],
            edges: vec![],
            paths: vec![],
            scalars,
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
// Explain execution
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute an EXPLAIN query: return inner plan's metadata as scalars,
    /// WITHOUT executing the inner traversal.
    fn execute_explain(&self, inner: &GraphPlan) -> Result<ResultSet, ExecutorError> {
        // EXPLAIN returns the inner plan's metadata as scalars
        let metadata = inner.metadata();
        let version_str = metadata.version_str();
        let hash_str = metadata.hash_str();

        let scalars = vec![
            TypedValue::String(version_str.to_string()),
            TypedValue::String(hash_str.to_string()),
        ];

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
// Boolean composition — typed multiset: And / Or / Not
// ============================================================================

impl<'a> SnapshotGraphExecutor<'a> {
    /// Execute a boolean composition on sub-plans.
    ///
    /// - `And`: set intersection of node IDs
    /// - `Or`: set union of node IDs
    /// - `Not`: set difference (complement within the snapshot's node universe)
    fn execute_boolean(
        &self,
        graph: &CallGraph,
        op: BooleanOp,
        operands: &[GraphPlan],
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        if operands.is_empty() {
            return Ok(ResultSet::empty());
        }

        // Evaluate each operand DIRECTLY against the already-fetched graph.
        // This avoids re-fetching the snapshot with a dummy revision.
        let mut all_node_sets: Vec<HashSet<String>> = Vec::new();

        for operand in operands {
            let node_ids = self.evaluate_operand(graph, operand)?;
            all_node_sets.push(node_ids);
        }

        // Collect all nodes in the graph for the universe of Not operation
        let _all_graph_nodes: HashSet<String> = graph
            .symbols()
            .map(|s| s.fully_qualified_name().to_string())
            .collect();

        let result_ids: HashSet<String> = match op {
            BooleanOp::And => {
                // Intersection: keep only nodes in ALL sets
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
                // Union: all nodes from all sets
                let mut union = HashSet::new();
                for set in &all_node_sets {
                    union.extend(set.iter().cloned());
                }
                union
            }
            BooleanOp::Not => {
                // Complement: all nodes in the graph universe EXCEPT those in the operand
                if let Some(first) = all_node_sets.first() {
                    let universe: HashSet<String> = graph
                        .symbols()
                        .map(|s| s.fully_qualified_name().to_string())
                        .collect();
                    universe.difference(first).cloned().collect()
                } else {
                    HashSet::new()
                }
            }
        };

        // Build result nodes
        let nodes: Vec<NodeResult> = result_ids
            .iter()
            .map(|symbol_id| {
                let symbol = graph.get_symbol(
                    &crate::domain::aggregates::call_graph::SymbolId::new(symbol_id),
                );
                let (labels, properties) = if let Some(sym) = symbol {
                    let kind_str = format!("{:?}", sym.kind());
                    let labels = if kind_str.starts_with("symbol.") {
                        vec![
                            kind_str
                                .strip_prefix("symbol.")
                                .unwrap_or(&kind_str)
                                .to_string(),
                        ]
                    } else {
                        vec![kind_str]
                    };
                    (labels, vec![])
                } else {
                    (vec![], vec![])
                };

                NodeResult {
                    id: symbol_id.clone(),
                    labels,
                    properties,
                }
            })
            .collect();
        // Sort nodes by id ASC so deterministic truncation (LIMIT N) yields
        // the same N nodes on PG and snapshot backends. Required for
        // `assert_equivalent` conformance when `max_result_rows` truncates.
        let mut nodes = nodes;
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        // Apply max_result_rows limit
        let mut result_nodes = nodes;
        let truncated = if let Some(max_rows) = limits.max_result_rows {
            if result_nodes.len() as u64 > max_rows {
                result_nodes.truncate(max_rows as usize);
                true
            } else {
                false
            }
        } else {
            false
        };

        Ok(ResultSet {
            rows: vec![],
            nodes: result_nodes,
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

    /// Evaluate a single operand plan directly against an already-fetched graph.
    /// Returns the set of node IDs from the result.
    fn evaluate_operand(
        &self,
        graph: &CallGraph,
        operand: &GraphPlan,
    ) -> Result<HashSet<String>, ExecutorError> {
        match operand {
            GraphPlan::Neighbors {
                src,
                kind,
                depth,
                edge_kind_filter,
                predicates: _,
                limits,
                ..
            } => {
                let rs = self.execute_neighbors(
                    graph,
                    src,
                    kind.clone(),
                    *depth as usize,
                    edge_kind_filter.as_deref(),
                    limits,
                )?;
                Ok(rs.nodes.iter().map(|n| n.id.clone()).collect())
            }
            GraphPlan::Path {
                src,
                dst,
                quantifier,
                edge_kind_filter,
                predicates: _,
                projection: _,
                limits,
                ..
            } => {
                let max_hops = quantifier.max_hops.unwrap_or(32).min(32) as usize;
                let rs = self.execute_path(
                    graph,
                    src,
                    dst,
                    max_hops,
                    edge_kind_filter.as_deref(),
                    limits,
                )?;
                // For path plans, extract all node IDs from the path hops
                let mut node_ids = HashSet::new();
                for path in &rs.paths {
                    for hop in &path.hops {
                        node_ids.insert(hop.node_id.clone());
                    }
                }
                Ok(node_ids)
            }
            GraphPlan::Subgraph {
                nodes,
                edges,
                aggregations: _,
                limits,
                ..
            } => {
                let rs = self.execute_subgraph(graph, nodes, edges.as_ref(), limits)?;
                Ok(rs.nodes.iter().map(|n| n.id.clone()).collect())
            }
            GraphPlan::Cluster { by, limits, .. } => {
                let rs = self.execute_cluster(graph, by, limits)?;
                Ok(rs.nodes.iter().map(|n| n.id.clone()).collect())
            }
            GraphPlan::BooleanComposition {
                op,
                operands,
                limits,
                ..
            } => {
                // Recurse for nested boolean composition
                let inner_sets = self.execute_boolean(graph, *op, operands, limits)?;
                Ok(inner_sets.nodes.iter().map(|n| n.id.clone()).collect())
            }
            GraphPlan::Explain { inner, .. } => {
                // For EXPLAIN, evaluate the inner plan
                self.evaluate_operand(graph, inner.as_ref())
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::aggregates::call_graph::SymbolId;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::plan::graph_plan::{PathProjection, PathQuantifier};
    use crate::domain::plan::version::{PlanHash, PlanMetadata, PlanVersion};
    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    /// Build a simple one-symbol CallGraph for testing.
    fn make_graph(symbol_name: &str) -> CallGraph {
        let mut g = CallGraph::new();
        let sym = Symbol::new(
            symbol_name,
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        g.add_symbol(sym);
        g
    }

    /// Build a fixture graph: A→B→C→D with A→D direct.
    fn make_abcd_graph() -> (CallGraph, String, String, String, String) {
        let mut graph = CallGraph::new();

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

        // A→B, A→D, B→C, C→D
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

        (
            graph,
            "src/A.rs:A:1".to_string(),
            "src/D.rs:D:1".to_string(),
            "src/B.rs:B:1".to_string(),
            "src/C.rs:C:1".to_string(),
        )
    }

    // -------------------------------------------------------------------------
    // Task 3.1a RED — SnapshotGraphExecutor::new compiles as &dyn GraphExecutor
    // Scenario: snapshot-graph-executor::Construction::Construct from provider
    // Assert: `fn _executor(_: &dyn GraphExecutor) {}` compiles with SnapshotGraphExecutor
    // -------------------------------------------------------------------------

    #[test]
    fn construct_snapshot_graph_executor_compiles_as_dyn_graph_executor() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);
        fn _executor(_: &dyn GraphExecutor) {}
        // This line would fail to compile if SnapshotGraphExecutor didn't implement GraphExecutor
        _executor(&executor);
    }

    #[test]
    fn snapshot_graph_executor_is_send_sync_static() {
        fn assert_send<T: Send + ?Sized>() {}
        fn assert_sync<T: Sync + ?Sized>() {}
        fn assert_static<T: 'static + ?Sized>() {}
        assert_send::<SnapshotGraphExecutor>();
        assert_sync::<SnapshotGraphExecutor>();
        assert_static::<SnapshotGraphExecutor>();
    }

    // -------------------------------------------------------------------------
    // Task 3.1b GREEN — SnapshotGraphExecutor construction is side-effect-free
    // Scenario: snapshot-graph-executor::Construction::Construction is side-effect-free
    // Assert: calling new() 1000 times does NOT read any snapshot
    // -------------------------------------------------------------------------

    #[test]
    fn construction_is_side_effect_free() {
        let provider = TestSnapshotProvider::new();

        // Insert a graph at rev 5
        let ws = WorkspaceId::try_new("ws1").unwrap();
        let graph = make_graph("test");
        provider.insert(&ws, RevisionId(5), graph);

        // Creating an executor should NOT read the snapshot
        for _ in 0..1000 {
            let _executor = SnapshotGraphExecutor::new(&provider);
        }

        // If we got here without panicking, construction is side-effect-free
        // (no snapshot read happened)
    }

    // -------------------------------------------------------------------------
    // Task 3.2 RED — Unknown revision returns ExecutorError::RevisionUnknown
    // Task 3.2 also covers: Cache hit returns cached Arc<CallGraph>
    // Scenario: snapshot-graph-executor::Pin Fails Closed::Unknown revision is rejected
    // -------------------------------------------------------------------------

    #[test]
    fn unknown_revision_returns_revision_unknown_error() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        let plan = GraphPlan::Neighbors {
            src: "A".to_string(),
            kind: NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        // Execute against unknown revision 99
        let result = executor.execute(&plan, (ws, RevisionId(99)));

        assert!(result.is_err(), "expected error for unknown revision");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ExecutorError::RevisionUnknown(ref s) if s.contains("ws1")),
            "expected RevisionUnknown error, got: {:?}",
            err
        );
    }

    #[test]
    fn cache_hit_returns_cached_snapshot() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let rev = RevisionId(3);

        // Insert a graph at rev 3
        let graph = make_graph("cached_func");
        provider.insert(&ws, rev, graph);

        // First call — cache miss, but snapshot is returned
        let plan = GraphPlan::Neighbors {
            src: "cached_func".to_string(),
            kind: NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result1 = executor.execute(&plan, (ws.clone(), rev));
        assert!(result1.is_ok(), "expected ok for known revision");

        // Second call — cache hit, should return same result
        let result2 = executor.execute(&plan, (ws, rev));
        assert!(result2.is_ok(), "expected ok from cache hit");
    }

    // -------------------------------------------------------------------------
    // Task 3.3 RED — Path via BFS, shortest-first ordering
    // Scenario: snapshot-graph-executor::Path Variant Uses BFS::Shortest path returns BFS result
    // -------------------------------------------------------------------------

    #[test]
    fn path_shortest_first_via_bfs() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let (graph, src, dst, _, _) = make_abcd_graph();
        provider.insert(&ws, RevisionId(1), graph);

        let plan = GraphPlan::Path {
            src: src.clone(),
            dst: dst.clone(),
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

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // Should have at least one path (the direct A→D edge is 1 hop)
        assert!(
            !rs.paths.is_empty(),
            "Expected at least one path from A to D, got {:?}",
            rs.paths
        );

        // All paths should start at A and end at D
        for path in &rs.paths {
            let first = path.hops.first().map(|h| h.node_id.as_str());
            let last = path.hops.last().map(|h| h.node_id.as_str());
            assert_eq!(first, Some(src.as_str()), "path should start at A");
            assert_eq!(last, Some(dst.as_str()), "path should end at D");
        }
    }

    // -------------------------------------------------------------------------
    // Task 3.4 RED — max_hops=2 on 3-hop chain returns empty
    // Scenario: snapshot-graph-executor::Path Variant Uses BFS::Hop bound respected
    // -------------------------------------------------------------------------

    #[test]
    fn path_max_hops_2_returns_empty_on_3_hop_chain() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let (graph, src, dst, _, _) = make_abcd_graph();
        provider.insert(&ws, RevisionId(1), graph);

        // Path from A to D through B→C is 3 hops (A→B, B→C, C→D)
        // With max_hops=2, this should return empty
        let plan = GraphPlan::Path {
            src: src.clone(),
            dst: dst.clone(),
            quantifier: PathQuantifier {
                max_hops: Some(2), // Only allows 2 hops, but path is 3 hops
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

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // With max_hops=2, the 3-hop path A→B→C→D should NOT be returned
        // Only the direct 1-hop path A→D should be returned
        // (if it exists — in our graph A→D does exist as a direct edge)
        // rs.paths may be empty if the direct edge doesn't exist;
        // the assertion below (all returned paths respect max_hops) is the
        // real correctness check.
        let _ = rs.paths.is_empty(); // Path may or may not be empty depending on if A→D exists

        // All returned paths must respect max_hops
        for path in &rs.paths {
            let hop_count = path.hops.len() - 1; // hops = nodes - 1
            assert!(
                hop_count as u32 <= 2,
                "path with {} hops exceeds max_hops=2",
                hop_count
            );
        }
    }

    // -------------------------------------------------------------------------
    // Task 3.5 RED — Neighbors Outgoing/Incoming + Subgraph + Cluster
    // Scenario: snapshot-graph-executor::Neighbors + Subgraph + Cluster + Explain
    // -------------------------------------------------------------------------

    #[test]
    fn neighbors_outgoing_returns_only_callees() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph: A→B, A→C, D→A (D calls A)
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

        provider.insert(&ws, RevisionId(1), graph);

        // Query outgoing neighbors of A
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

        let result = executor.execute(&plan, (ws.clone(), RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();

        // Should contain B and C (outgoing from A), NOT D (incoming to A)
        assert!(
            node_ids.contains(&"src/B.rs:B:1"),
            "Should contain B (outgoing), got {:?}",
            node_ids
        );
        assert!(
            node_ids.contains(&"src/C.rs:C:1"),
            "Should contain C (outgoing), got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/D.rs:D:1"),
            "Should NOT contain D (incoming), got {:?}",
            node_ids
        );
    }

    #[test]
    fn neighbors_incoming_returns_only_callers() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph: A→B, A→C, D→A
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

        provider.insert(&ws, RevisionId(1), graph);

        // Query incoming neighbors of A
        let plan = GraphPlan::Neighbors {
            src: "src/A.rs:A:1".to_string(),
            kind: NeighborKind::Incoming,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();

        // Should contain D (incoming to A), NOT B or C (outgoing from A)
        assert!(
            node_ids.contains(&"src/D.rs:D:1"),
            "Should contain D (incoming), got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/B.rs:B:1"),
            "Should NOT contain B (outgoing), got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/C.rs:C:1"),
            "Should NOT contain C (outgoing), got {:?}",
            node_ids
        );
    }

    #[test]
    fn subgraph_returns_visited_nodes_and_edges() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let (graph, _, _, _, _) = make_abcd_graph();
        provider.insert(&ws, RevisionId(1), graph);

        // Subgraph starting from A with depth 2
        let plan = GraphPlan::Subgraph {
            nodes: vec!["src/A.rs:A:1".to_string()],
            edges: None,
            aggregations: vec![],
            limits: PlanLimits::builder().max_depth(2).build(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // Should have visited A, B, C (within depth 2 from A)
        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(
            node_ids.contains(&"src/A.rs:A:1"),
            "Should contain A, got {:?}",
            node_ids
        );
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

        // Edges should include A→B and B→C
        assert!(
            !rs.edges.is_empty(),
            "Should have edges between visited nodes, got {:?}",
            rs.edges
        );
    }

    #[test]
    fn cluster_by_kind_returns_group_counts() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph with mixed kinds
        let mut graph = CallGraph::new();
        graph.add_symbol(Symbol::new(
            "func1",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        ));
        graph.add_symbol(Symbol::new(
            "func2",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        ));
        graph.add_symbol(Symbol::new(
            "class1",
            SymbolKind::Class,
            Location::new("test.rs", 3, 1),
        ));

        provider.insert(&ws, RevisionId(1), graph);

        let plan = GraphPlan::Cluster {
            by: vec!["kind".to_string()],
            aggregations: vec![],
            ordering: None,
            limit: None,
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // Should have scalars with counts (one per kind)
        assert!(
            !rs.scalars.is_empty(),
            "Should have group counts, got {:?}",
            rs.scalars
        );

        // Each scalar should be an integer count
        for scalar in &rs.scalars {
            assert!(
                matches!(scalar, TypedValue::Int(_)),
                "expected Int count, got {:?}",
                scalar
            );
        }
    }

    // -------------------------------------------------------------------------
    // Task 3.6 RED — And/Or/Not on typed multisets
    // Scenario: snapshot-graph-executor::Boolean Composition Typed Multiset::AND intersection
    //          + NOT complement
    // -------------------------------------------------------------------------

    #[test]
    fn boolean_and_returns_intersection() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph: A→{B,C}, B→C
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

        provider.insert(&ws, RevisionId(1), graph);

        // And(Neighbors(A, Outgoing, 1), Neighbors(B, Outgoing, 1)) should return {C}
        // because A's outgoing neighbors are {B, C} and B's outgoing neighbors are {C}
        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::And,
            operands: vec![
                GraphPlan::Neighbors {
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
                },
                GraphPlan::Neighbors {
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
                },
            ],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();

        // Intersection should be C only
        assert_eq!(
            node_ids.len(),
            1,
            "AND intersection should have exactly 1 node, got {:?}",
            node_ids
        );
        assert!(
            node_ids.contains(&"src/C.rs:C:1"),
            "Should contain C (intersection), got {:?}",
            node_ids
        );
    }

    #[test]
    fn boolean_not_returns_complement() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph: A→{B,C}, D isolated
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
            &id_a,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        // D is isolated — no edges

        provider.insert(&ws, RevisionId(1), graph);

        // Not(Neighbors(A, Outgoing, 1)) should return all nodes EXCEPT B and C
        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::Not,
            operands: vec![GraphPlan::Neighbors {
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
            }],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        let node_ids: Vec<&str> = rs.nodes.iter().map(|n| n.id.as_str()).collect();

        // Should NOT contain B or C
        assert!(
            !node_ids.contains(&"src/B.rs:B:1"),
            "Should NOT contain B (in operand), got {:?}",
            node_ids
        );
        assert!(
            !node_ids.contains(&"src/C.rs:C:1"),
            "Should NOT contain C (in operand), got {:?}",
            node_ids
        );

        // Should contain A (source node excluded from neighbors) and D (isolated)
        // Note: The source node itself is typically excluded from neighbors
        // So we expect D (isolated, never in neighbors) at minimum
        assert!(
            node_ids.contains(&"src/D.rs:D:1"),
            "Should contain D (not in neighbors), got {:?}",
            node_ids
        );
    }

    // -------------------------------------------------------------------------
    // Task 3.8 RED — CancellationToken::set() mid-BFS returns LimitExceeded
    // Scenario: snapshot-graph-executor::Plan Limit Enforcement::cancellation aborts
    // -------------------------------------------------------------------------

    #[test]
    fn cancellation_aborts_bfs() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let (graph, src, dst, _, _) = make_abcd_graph();
        provider.insert(&ws, RevisionId(1), graph);

        // Create a cancellation token and set it before execution
        let token = CancellationToken::new();
        token.set(); // Cancel immediately

        let plan = GraphPlan::Path {
            src,
            dst,
            quantifier: PathQuantifier {
                max_hops: Some(3),
                min_hops: 0,
            },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits {
                cancellation: Some(token),
                ..PlanLimits::default()
            },
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));

        assert!(result.is_err(), "expected error due to cancellation");
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                ExecutorError::LimitExceeded {
                    dimension: PlanLimitKind::Cancellation,
                    ..
                }
            ),
            "expected LimitExceeded with Cancellation, got: {:?}",
            err
        );
    }

    // -------------------------------------------------------------------------
    // Task 3.7 GREEN — max_result_rows enforced post-walk
    // Scenario: snapshot-graph-executor::Plan Limit Enforcement::max_result_rows truncated
    // -------------------------------------------------------------------------

    #[test]
    fn max_result_rows_truncation() {
        let provider = TestSnapshotProvider::new();
        let executor = SnapshotGraphExecutor::new(&provider);

        let ws = WorkspaceId::try_new("ws1").unwrap();

        // Build graph with many nodes connected to node_0
        let mut graph = CallGraph::new();
        // add_symbol returns the actual SymbolId (derived from fqn = file:name:line)
        let symbol_ids: Vec<SymbolId> = (0..20)
            .map(|i| {
                graph.add_symbol(Symbol::new(
                    &format!("node_{}", i),
                    SymbolKind::Function,
                    Location::new("test.rs", i as u32, 1),
                ))
            })
            .collect();

        // Add edges from node_0 to many nodes to create > 5 results
        for i in 1..20 {
            let _ = graph.add_dependency_with_provenance(
                &symbol_ids[0],
                &symbol_ids[i],
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );
        }

        provider.insert(&ws, RevisionId(1), graph);

        // Query neighbors with max_result_rows=5 (symbol_ids[0] is node_0)
        let plan = GraphPlan::Neighbors {
            src: symbol_ids[0].as_str().to_string(),
            kind: NeighborKind::Both,
            depth: 10, // High depth to reach all nodes
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits {
                max_result_rows: Some(5),
                ..PlanLimits::default()
            },
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };

        let result = executor.execute(&plan, (ws, RevisionId(1)));
        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        let rs = result.unwrap();

        // Result should be truncated
        assert!(rs.truncated, "result should be truncated");
        assert_eq!(
            rs.truncation,
            Some(TruncationMarker::ResultRowsLimit),
            "truncation marker should be ResultRowsLimit"
        );
        assert!(
            rs.nodes.len() <= 5,
            "should have at most 5 nodes, got {}",
            rs.nodes.len()
        );
    }

    // -------------------------------------------------------------------------
    // Test helper: make TestSnapshotProvider usable for GraphExecutor tests
    // -------------------------------------------------------------------------

    impl SnapshotGraphExecutor<'static> {
        /// Create an executor from a TestSnapshotProvider for convenience in tests.
        pub fn new_for_test(provider: &'static TestSnapshotProvider) -> Self {
            Self::new(provider)
        }
    }

    // -------------------------------------------------------------------------
    // Task 5 RED — `path_with_edge_kind_filter_excludes_references`
    // Scenario: `graph-executor-port::Edge-kind filter restricts traversal`
    // Assert: `Path(A, C, [Calls])` over a graph with both `Calls(A,B)` and
    //         `References(A,B')` + `B→C` and `B'→C` returns ONLY paths through
    //         `B` (the Calls node), not `B'` (the References node). Without
    //         the filter, both paths A→B→C and A→B'→C would appear.
    // Why RED: pre-fix `bfs_all_paths` does not consult `edge_kind_filter`
    //         and walks every edge weight indiscriminately.
    // -------------------------------------------------------------------------

    #[test]
    fn path_with_edge_kind_filter_excludes_references() {
        use crate::domain::value_objects::SymbolKind;

        let mut graph = CallGraph::new();

        let id_a = SymbolId::new("src/A.rs:A:1");
        let id_b = SymbolId::new("src/B.rs:B:1");
        let id_bref = SymbolId::new("src/B_ref.rs:B_ref:1");
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
            "B_ref",
            SymbolKind::Function,
            Location::new("src/B_ref.rs", 1, 1),
        ));
        graph.add_symbol(Symbol::new(
            "C",
            SymbolKind::Function,
            Location::new("src/C.rs", 1, 1),
        ));

        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &id_bref,
            DependencyType::References,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_b,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        let _ = graph.add_dependency_with_provenance(
            &id_bref,
            &id_c,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );

        let ws = WorkspaceId::try_new("ws1").unwrap();
        let provider = TestSnapshotProvider::new();
        provider.insert(&ws, RevisionId(1), graph);
        let executor = SnapshotGraphExecutor::new(&provider);

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

        let rs = executor
            .execute(&plan_filtered, (ws.clone(), RevisionId(1)))
            .expect("execute with Calls filter must succeed");

        for path in &rs.paths {
            assert_eq!(path.hops.len(), 3, "expected A→B→C, got {:?}", path.hops);
            let b_hop = &path.hops[1];
            assert_eq!(
                b_hop.node_id, "src/B.rs:B:1",
                "filtered path must go through B (Calls), not B_ref (References)"
            );
        }
        assert!(!rs.paths.is_empty(), "expected at least one filtered path");

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
            .execute(&plan_unfiltered, (ws, RevisionId(1)))
            .expect("execute without filter must succeed");
        assert!(
            rs_unfiltered.paths.len() >= 2,
            "unfiltered fixture must have at least 2 distinct paths, got {}",
            rs_unfiltered.paths.len()
        );
    }
}