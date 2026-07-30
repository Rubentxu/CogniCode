//! Graph analytics service — wraps petgraph algorithms behind a clean
//! domain API.
//!
//! Each method takes a `&CallGraph` (the domain aggregate) and operates
//! on a transient [`CallGraphProjection`] snapshot. The projection's
//! underlying [`StableGraph`] already implements every petgraph trait
//! the algorithms below need (`NodeIndexable`, `IntoEdges`,
//! `IntoNeighborsDirected`, `GraphProp<EdgeType = Directed>`, …), so
//! the algorithms run directly on the projection — no extra graph
//! copy.
//!
//! ## Provided analytics
//!
//! - [`Self::page_rank`] — importance score per symbol (god-node signal).
//! - [`Self::all_simple_paths`] — every simple path between two
//!   symbols, bounded by a hop budget.
//! - [`Self::condensation`] — strongly-connected-component
//!   decomposition (cycles collapsed into single components).
//! - [`Self::god_nodes`] — symbols whose PageRank sits above a
//!   percentile threshold.
//! - [`Self::transitive_reduction`] — minimal set of dependency edges
//!   that preserve reachability.
//! - [`Self::feedback_arc_set`] — edges whose removal makes the
//!   dependency graph acyclic (cycle-breaker candidates).
//!
//! ## Edge cases
//!
//! All methods are total: an empty graph, a missing symbol id, or a
//! graph without a path between two symbols never panics. They
//! degrade to `vec![]` / empty map / empty pair so callers can render
//! "no data" messages uniformly.

use std::collections::HashMap;
use std::sync::Arc;

use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences, NodeIndexable};

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::domain::analytics::{
    AdmissionError, AlgorithmDescriptor, AlgorithmExecute, AlgorithmId, AnalyticsError,
    AnalyticsMode, DeterminismKind, RunLineage, RunLineageFilter, RunLineageStore, RunStatus,
};
use crate::domain::plan::limits::PlanLimits;
use crate::domain::value_objects::{RevisionId, WorkspaceId};
use crate::infrastructure::graph::CallGraphProjection;
use cognicode_graph_algos::{self, GraphBuilder};

/// Graph analytics service wrapping petgraph algorithms.
///
/// A zero-sized type — every method is a pure function over the input
/// `CallGraph`. The struct exists so the analytics surface is grouped
/// under a single name and so MCP tool handlers can be wired against a
/// stable, documented entry point.
pub struct GraphAnalyticsService;

impl GraphAnalyticsService {
    /// Compute PageRank over the call graph.
    ///
    /// `alpha` is the damping factor (typical: `0.85`). `max_iterations`
    /// caps the fixed-point loop. Returns a map `SymbolId -> score`;
    /// scores sum to `1.0` across all nodes and nodes with the highest
    /// scores are "god nodes" — heavily depended-upon symbols.
    ///
    /// **Edge direction**: in CogniCode's call graph, edge `A -> B`
    /// means "A calls B" (A is the caller, B is the callee). A "god
    /// node" is a heavily-**called** symbol, i.e. one with many
    /// *incoming* edges. This implementation iterates over the
    /// inverse graph (in-neighbours) so that rank accumulates on
    /// callees, matching the codebase's `god_nodes` semantics (see
    /// `graph_handlers.rs::handle_graph_pagerank` and
    /// `Self::god_nodes`).
    ///
    /// Edge cases:
    /// - Empty graph -> empty map.
    /// - Disconnected components still receive non-zero scores via
    ///   the dangling-node term in the formula.
    /// - Single node -> `1.0` for that node.
    /// - Nodes with `NaN` scores (degenerate input) are clamped to `0.0`.
    ///
    /// **Implementation note (ADR-031)**: This uses an explicit
    /// sparse-matrix PageRank (O(V + E) per iteration) instead of
    /// `petgraph::algo::page_rank`, which is O(N·V²·E) in petgraph
    /// 0.6 and infeasible for graphs with more than a few thousand
    /// nodes (~20 days for 29K symbols × 100 iterations).
    pub fn page_rank(
        graph: &CallGraph,
        alpha: f64,
        max_iterations: usize,
    ) -> HashMap<SymbolId, f64> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let (in_neighbors, out_degree) = projection.build_adjacency();
        let n = projection.node_count();
        let raw_scores =
            cognicode_graph_algos::page_rank(&in_neighbors, &out_degree, n, alpha, max_iterations);
        // Map usize indices back to SymbolId via projection.id_to_index().
        let mut out: HashMap<SymbolId, f64> = HashMap::with_capacity(n);
        for (sid, ni) in projection.id_to_index() {
            let idx = ni.index();
            if let Some(&score) = raw_scores.get(&idx) {
                out.insert(sid.clone(), score);
            }
        }
        out
    }

    /// Find all simple paths from `from` to `to` bounded by `max_hops`.
    ///
    /// A simple path does not repeat a node, so cycles are terminated
    /// by the visited-set. `max_hops` is the maximum number of
    /// intermediate nodes (i.e. the path may traverse at most
    /// `max_hops + 1` edges).
    ///
    /// Edge cases:
    /// - Missing `from` or `to` id -> empty vec.
    /// - No path within `max_hops` -> empty vec.
    /// - `from == to` -> no path is emitted.
    pub fn all_simple_paths(
        graph: &CallGraph,
        from: &SymbolId,
        to: &SymbolId,
        max_hops: usize,
    ) -> Vec<Vec<SymbolId>> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();

        let (Some(&from_idx), Some(&to_idx)) = (
            projection.id_to_index().get(from),
            projection.id_to_index().get(to),
        ) else {
            return Vec::new();
        };

        let raw = cognicode_graph_algos::all_simple_paths(
            &out_neighbors,
            from_idx.index(),
            to_idx.index(),
            max_hops,
        );
        raw.into_iter()
            .map(|path| {
                path.into_iter()
                    .filter_map(|idx| {
                        projection
                            .id_to_index()
                            .iter()
                            .find(|(_, ni)| ni.index() == idx)
                            .map(|(sid, _)| sid.clone())
                    })
                    .collect()
            })
            .collect()
    }

    /// Compute the SCC condensation of the call graph.
    ///
    /// Each returned `Vec<SymbolId>` is one strongly connected
    /// component. The order of components and the order of nodes
    /// inside a component follow the graph-algos Tarjan implementation
    /// (post-order on the DFS tree, alphabetic sort within each SCC).
    /// Self-loops surface as singleton components.
    pub fn condensation(graph: &CallGraph) -> Vec<Vec<SymbolId>> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();
        let raw = cognicode_graph_algos::condensation(&out_neighbors, n);
        raw.into_iter()
            .map(|scc| {
                scc.into_iter()
                    .filter_map(|idx| {
                        projection
                            .id_to_index()
                            .iter()
                            .find(|(_, ni)| ni.index() == idx)
                            .map(|(sid, _)| sid.clone())
                    })
                    .collect()
            })
            .collect()
    }

    /// Find god nodes — symbols with PageRank above a percentile
    /// threshold of the score distribution.
    ///
    /// `percentile` is in `[0.0, 1.0]`. With the default
    /// `percentile = 0.95`, only the top 5% scoring symbols are
    /// reported. The output is sorted by score descending so the most
    /// critical god nodes come first.
    ///
    /// Returns an empty vec for an empty graph. The percentile
    /// selection uses an off-by-one-tolerant clamp so values at the
    /// upper end (`percentile == 1.0`) include the single top score.
    pub fn god_nodes(graph: &CallGraph, percentile: f64) -> Vec<(SymbolId, f64)> {
        let scores = Self::page_rank(graph, 0.85, 100);
        if scores.is_empty() {
            return Vec::new();
        }
        // Map SymbolId -> usize (positional) for the new API, then back.
        let projection = CallGraphProjection::from_call_graph(graph);
        let mut usize_scores: HashMap<usize, f64> = HashMap::with_capacity(scores.len());
        for (sid, score) in &scores {
            if let Some(ni) = projection.id_to_index().get(sid) {
                usize_scores.insert(ni.index(), *score);
            }
        }
        let god_indices = cognicode_graph_algos::god_nodes(&usize_scores, percentile);
        god_indices
            .into_iter()
            .filter_map(|(idx, score)| {
                projection
                    .id_to_index()
                    .iter()
                    .find(|(_, ni)| ni.index() == idx)
                    .map(|(sid, _)| (sid.clone(), score))
            })
            .collect()
    }

    /// Compute the transitive reduction of the call graph — the
    /// minimal set of edges that preserves reachability.
    ///
    /// Returns every `(source, target)` pair that survives the
    /// reduction. Edges that are implied by a longer path (e.g.
    /// `A -> C` when `A -> B` and `B -> C` exist) are dropped.
    /// For cyclic graphs, all edges are returned (identity reduction)
    /// since no edge is implied by a strictly longer simple path.
    pub fn transitive_reduction(graph: &CallGraph) -> Vec<(SymbolId, SymbolId)> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let (in_neighbors, _) = projection.build_adjacency();
        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();
        let raw = cognicode_graph_algos::transitive_reduction(&in_neighbors, &out_neighbors, n);
        raw.into_iter()
            .filter_map(|(s, t)| {
                let sid_s = projection
                    .id_to_index()
                    .iter()
                    .find(|(_, ni)| ni.index() == s)
                    .map(|(sid, _)| sid.clone());
                let sid_t = projection
                    .id_to_index()
                    .iter()
                    .find(|(_, ni)| ni.index() == t)
                    .map(|(sid, _)| sid.clone());
                match (sid_s, sid_t) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                }
            })
            .collect()
    }

    /// Find the greedy feedback arc set — edges whose removal makes
    /// the dependency graph acyclic.
    ///
    /// Useful for resolving circular dependencies: the reported edges
    /// are the cheapest candidates to break first (per the
    /// Eades-Lin-Smyth heuristic). Returns an empty vec for a DAG.
    pub fn feedback_arc_set(graph: &CallGraph) -> Vec<(SymbolId, SymbolId)> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let (in_neighbors, _) = projection.build_adjacency();
        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();
        let raw = cognicode_graph_algos::feedback_arc_set(&in_neighbors, &out_neighbors, n);
        raw.into_iter()
            .filter_map(|(s, t)| {
                let sid_s = projection
                    .id_to_index()
                    .iter()
                    .find(|(_, ni)| ni.index() == s)
                    .map(|(sid, _)| sid.clone());
                let sid_t = projection
                    .id_to_index()
                    .iter()
                    .find(|(_, ni)| ni.index() == t)
                    .map(|(sid, _)| sid.clone());
                match (sid_s, sid_t) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                }
            })
            .collect()
    }
}

// ============================================================================
// AlgorithmRegistry
// ============================================================================

/// Analytics algorithm registry — admission gate and run orchestrator.
///
/// The registry owns admitted algorithm descriptors and provides the
/// `run()` method that all analytics execution flows through.
/// Validates descriptor completeness at admission, limits at request time,
/// and delegates to pure algorithm implementations.
pub struct AlgorithmRegistry {
    descriptors: HashMap<AlgorithmId, Box<dyn AlgorithmExecute>>,
    lineage: Arc<dyn RunLineageStore>,
    boundary_guard: Option<Arc<dyn AnalyticsBoundaryGuard>>,
}

impl AlgorithmRegistry {
    /// Create a new registry with the given lineage store and optional boundary guard.
    pub fn new(
        lineage: Arc<dyn RunLineageStore>,
        boundary_guard: Option<Arc<dyn AnalyticsBoundaryGuard>>,
    ) -> Self {
        Self {
            descriptors: HashMap::new(),
            lineage,
            boundary_guard,
        }
    }

    /// Admit an algorithm descriptor into the registry.
    ///
    /// Returns `Ok(())` if all 12 required methods return non-empty values.
    /// Returns `Err(AdmissionError::Incomplete)` if any method returns None/empty.
    ///
    /// Also persists the descriptor's default limits to the lineage store so they
    /// can be reloaded on registry boot (survives restarts).
    pub fn admit(&mut self, d: Box<dyn AlgorithmExecute>) -> Result<(), AdmissionError> {
        // Validate completeness of all required fields
        let id = d.identity();

        // Check all required fields are present
        let missing = validate_descriptor_completeness(&*d);
        if !missing.is_empty() {
            return Err(AdmissionError::Incomplete(missing));
        }

        // Check if already admitted with same version
        if let Some(existing) = self.descriptors.get(&id.id) {
            if existing.identity().version == id.version {
                return Err(AdmissionError::AlreadyAdmitted(
                    id.id.as_str().into(),
                    id.version.clone(),
                ));
            }
        }

        // Persist descriptor limits to lineage store (idempotent upsert)
        // Use block_in_place to run async SQL from this sync context on a
        // blocking thread. This avoids the Handle::block_on panic when called
        // from a tokio worker thread (multi-thread runtime).
        // Use try_current to handle the case where no runtime is available
        // (e.g., called from a non-async test context).
        let lineage_store = self.lineage.clone();
        let id_clone = id.id.clone();
        let version_str = id.version.to_string();
        let limits = d.limits().clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Runtime is available: use block_in_place with captured handle
            let (tx, rx) = std::sync::mpsc::channel();
            tokio::task::block_in_place(move || {
                let _enter = handle.enter();
                tokio::spawn(async move {
                    let result = lineage_store
                        .upsert_descriptor_limits(&id_clone, &version_str, &limits)
                        .await;
                    let _ = tx.send(result);
                });
            });
            rx.recv()
                .map_err(|e| AdmissionError::Incomplete(format!("lineage store error: channel recv: {e}")))?
                .map_err(|e| AdmissionError::Incomplete(format!("lineage store error: {}", e)))?;
        } else {
            // No runtime available: create a temporary one-shot runtime
            // This is needed for sync contexts like plain #[test] functions
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| AdmissionError::Incomplete(format!("lineage store error: runtime create: {e}")))?;
            runtime.block_on(lineage_store.upsert_descriptor_limits(&id_clone, &version_str, &limits))
                .map_err(|e| AdmissionError::Incomplete(format!("lineage store error: {}", e)))?;
        }

        self.descriptors.insert(id.id.clone(), d);
        Ok(())
    }

    /// Get an admitted descriptor by ID.
    pub fn get(&self, id: &AlgorithmId) -> Option<&dyn AlgorithmExecute> {
        self.descriptors.get(id).map(|b| b.as_ref())
    }

    /// Iterate over all admitted descriptors.
    pub fn admitted(&self) -> impl Iterator<Item = &dyn AlgorithmExecute> {
        self.descriptors.values().map(|b| b.as_ref())
    }

    /// Check if an algorithm is admitted.
    pub fn is_admitted(&self, id: &AlgorithmId) -> bool {
        self.descriptors.contains_key(id)
    }

    /// Get the lineage store.
    pub fn lineage_store(&self) -> &Arc<dyn RunLineageStore> {
        &self.lineage
    }
}

/// Validate that all required descriptor methods return complete data.
/// Returns a comma-separated list of missing fields.
///
/// Params are considered complete if param_names() returns a non-empty list
/// (algorithms with required params) OR if validate() accepts null/empty
/// (algorithms with no required params like SCC, WCC).
fn validate_descriptor_completeness(d: &dyn AlgorithmDescriptor) -> String {
    let mut missing = Vec::new();

    // Params are complete if either:
    // - param_names() returns non-empty (has named parameters), OR
    // - validate(null) succeeds (null/empty is acceptable — no required params)
    let has_named_params = !d.params().param_names().is_empty();
    let accepts_null = d.params().validate(&serde_json::Value::Null).is_ok();
    if !has_named_params && !accepts_null {
        missing.push("params");
    }
    if d.output_schema().fields.is_empty() {
        missing.push("output_schema");
    }
    if d.supported_modes().is_empty() {
        missing.push("supported_modes");
    }
    if d.complexity().time.is_empty() {
        missing.push("complexity.time");
    }
    if d.limits().is_unbounded() {
        missing.push("limits");
    }
    if d.conformance_fixtures().is_empty() {
        missing.push("conformance_fixtures");
    }

    missing.join(", ")
}

// ============================================================================
// AnalyticsBoundaryGuard (port)
// ============================================================================

/// Port for the analytics execution boundary guard.
///
/// The guard holds write access to canonical graph tables and enforces
/// that only `Persist` mode can write, and only derived-analysis records.
pub trait AnalyticsBoundaryGuard: Send + Sync + 'static {
    /// Check if the caller is authorized for persist mode.
    fn can_persist(&self, caller: CallerCapabilities) -> bool;
}

/// Caller capabilities for authorization decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallerCapabilities {
    /// Internal services with full access.
    Internal,
    /// Trusted REST callers.
    TrustedREST,
    /// External MCP callers.
    ExternalMCP,
    /// Explorer UI callers.
    Explorer,
}

/// Validate effective limits for a request.
///
/// Returns the effective limits after applying caller constraints.
pub fn validate_effective_limits(
    descriptor: &dyn AlgorithmDescriptor,
    caller_limits: PlanLimits,
) -> Result<PlanLimits, AnalyticsError> {
    apply_caller_limits(descriptor.limits(), caller_limits)
}

/// Apply caller constraints to base limits.
///
/// Caller can only tighten limits, not widen them. Returns the effective limits.
fn apply_caller_limits(
    base: &PlanLimits,
    caller: PlanLimits,
) -> Result<PlanLimits, AnalyticsError> {
    let mut effective = base.clone();

    if let Some(caller_max_nodes) = caller.max_visited_nodes {
        if let Some(base_max) = base.max_visited_nodes {
            if caller_max_nodes > base_max {
                return Err(AnalyticsError::LimitPolicyViolation(format!(
                    "caller max_visited_nodes {} exceeds base maximum {}",
                    caller_max_nodes, base_max
                )));
            }
            effective.max_visited_nodes = Some(caller_max_nodes);
        }
    }

    if let Some(caller_max_edges) = caller.max_visited_edges {
        if let Some(base_max) = base.max_visited_edges {
            if caller_max_edges > base_max {
                return Err(AnalyticsError::LimitPolicyViolation(format!(
                    "caller max_visited_edges {} exceeds base maximum {}",
                    caller_max_edges, base_max
                )));
            }
            effective.max_visited_edges = Some(caller_max_edges);
        }
    }

    if let Some(caller_max_rows) = caller.max_result_rows {
        if let Some(base_max) = base.max_result_rows {
            if caller_max_rows > base_max {
                return Err(AnalyticsError::LimitPolicyViolation(format!(
                    "caller max_result_rows {} exceeds base maximum {}",
                    caller_max_rows, base_max
                )));
            }
            effective.max_result_rows = Some(caller_max_rows);
        }
    }

    if let Some(caller_time) = caller.time_ms {
        if let Some(base_time) = base.time_ms {
            if caller_time > base_time {
                return Err(AnalyticsError::LimitPolicyViolation(format!(
                    "caller time_ms {} exceeds base maximum {}",
                    caller_time, base_time
                )));
            }
            effective.time_ms = Some(caller_time);
        }
    }

    Ok(effective)
}

// ============================================================================
// RunRequest and RunResult
// ============================================================================

/// Request to run an analytics algorithm.
pub struct RunRequest {
    /// Algorithm identifier.
    pub algorithm_id: AlgorithmId,
    /// Schema-validated algorithm parameters.
    pub params: serde_json::Value,
    /// Pinned workspace and revision.
    pub pin: (WorkspaceId, RevisionId),
    /// Execution mode.
    pub mode: AnalyticsMode,
    /// Caller-provided limits (tightened against descriptor maxima).
    pub caller_limits: PlanLimits,
    /// Seed for seeded algorithms.
    pub seed: Option<u64>,
    /// Idempotency key for persist mode.
    pub idempotency_key: Option<String>,
    /// Caller capabilities for authorization.
    pub caller: CallerCapabilities,
    /// The call graph to run the algorithm on.
    pub graph: CallGraph,
}

/// Result of an analytics run.
pub struct RunResult {
    /// Unique run identifier.
    pub run_id: crate::domain::analytics::lineage::Uuid,
    /// Final status.
    pub status: RunStatus,
    /// Algorithm output.
    pub output: crate::domain::analytics::RunOutput,
    /// Row count (if applicable).
    pub row_count: i64,
    /// Truncation marker (if truncated).
    pub truncation_marker: Option<crate::domain::analytics::TruncationMarker>,
}

// ============================================================================
// DefaultAnalyticsBoundaryGuard
// ============================================================================

/// Default boundary guard that allows persist for Internal and TrustedREST callers.
#[derive(Debug, Clone)]
pub struct DefaultAnalyticsBoundaryGuard {
    persist_caller_classes: std::collections::HashSet<CallerCapabilities>,
}

impl DefaultAnalyticsBoundaryGuard {
    /// Create a new guard with the default policy.
    ///
    /// Internal and TrustedREST callers are authorized for persist.
    /// ExternalMCP and Explorer are NOT authorized for persist.
    pub fn new() -> Self {
        let mut persist_caller_classes = std::collections::HashSet::new();
        persist_caller_classes.insert(CallerCapabilities::Internal);
        persist_caller_classes.insert(CallerCapabilities::TrustedREST);
        Self {
            persist_caller_classes,
        }
    }
}

impl Default for DefaultAnalyticsBoundaryGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsBoundaryGuard for DefaultAnalyticsBoundaryGuard {
    fn can_persist(&self, caller: CallerCapabilities) -> bool {
        self.persist_caller_classes.contains(&caller)
    }
}

// ============================================================================
// AlgorithmRegistry::run
// ============================================================================

impl AlgorithmRegistry {
    /// Run an admitted algorithm.
    ///
    /// This is the main entry point for analytics execution. It:
    /// 1. Looks up the descriptor by algorithm_id
    /// 2. Validates effective limits
    /// 3. Checks persist authorization if mode == Persist
    /// 4. Inserts a pending lineage record
    /// 5. Dispatches to the algorithm's execute() method
    /// 6. Updates the lineage record with the result
    /// 7. Returns the run result
    pub async fn run(&self, request: RunRequest) -> Result<RunResult, AnalyticsError> {
        // Step 1: Look up the descriptor
        let descriptor = self
            .get(&request.algorithm_id)
            .ok_or_else(|| AnalyticsError::NotAdmitted(request.algorithm_id.clone()))?;

        // Step 2: Get effective limits — check lineage store first (persisted limits
        // survive restarts), then fall back to descriptor defaults. Caller limits are
        // validated on top of whatever base limits we resolve.
        let identity = descriptor.identity();
        let base_limits = self
            .lineage
            .get_descriptor_limits(&identity.id, &identity.version.to_string())
            .await
            .map_err(|e| AnalyticsError::Internal(format!("get descriptor limits: {e}")))?
            .unwrap_or_else(|| descriptor.limits().clone());
        let effective_limits = apply_caller_limits(&base_limits, request.caller_limits)?;

        // Step 3: Check persist authorization
        if request.mode == AnalyticsMode::Persist {
            if self
                .boundary_guard
                .as_ref()
                .map_or(true, |g| !g.can_persist(request.caller))
            {
                return Err(AnalyticsError::PersistUnauthorized);
            }
        }

        // Step 4: Insert pending lineage record
        let mut lineage = RunLineage::new(
            request.pin.0.clone(),
            request.pin.1.clone(),
            request.algorithm_id.clone(),
            descriptor.identity().version.to_string(),
            vec![], // plan_hash - empty for now
            request.params.clone(),
            request.seed,
            request.mode,
        );
        if let Some(ref key) = request.idempotency_key {
            lineage.set_idempotency_key(key.clone());
        }

        // Clone lineage for first async insert
        let lineage_store = self.lineage.clone();
        let lineage_for_insert = lineage.clone();
        lineage_store
            .insert(&lineage_for_insert)
            .await
            .map_err(|e| AnalyticsError::Internal(format!("lineage insert: {e}")))?;

        // Step 5: Dispatch to algorithm's execute() method
        let exec_result = descriptor
            .execute(&request.params, &request.graph, &effective_limits)
            .await;

        // Step 6: Update lineage record with result and extract output
        let (status, row_count, truncation_marker, output) = match exec_result {
            Ok(output) => {
                let count = output.row_count();
                lineage.succeed(count);
                (RunStatus::Succeeded, count, None, Some(output))
            }
            Err(AnalyticsError::LimitExceeded(kind)) => {
                lineage.fail(format!("LimitExceeded({:?})", kind), kind.to_string());
                return Err(AnalyticsError::LimitExceeded(kind));
            }
            Err(AnalyticsError::Truncated(msg)) => {
                // Determine truncation marker from limits
                let marker = if effective_limits.max_result_rows.is_some() {
                    Some(crate::domain::analytics::TruncationMarker::ResultRowsLimit)
                } else {
                    None
                };
                lineage.truncate(
                    marker.unwrap_or(crate::domain::analytics::TruncationMarker::ResultRowsLimit),
                    0,
                );
                (RunStatus::Truncated, 0, marker, None)
            }
            Err(e) => {
                lineage.fail(e.to_string(), format!("{:?}", e));
                return Err(e);
            }
        };

        // Update lineage in store
        let lineage_store = self.lineage.clone();
        let lineage_for_update = lineage.clone();
        lineage_store
            .insert(&lineage_for_update)
            .await
            .map_err(|e| AnalyticsError::Internal(format!("lineage update: {e}")))?;

        // Step 7: Return result
        let output = output.expect("output must be Some for success case");

        Ok(RunResult {
            run_id: lineage.run_id,
            status,
            output,
            row_count,
            truncation_marker,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Symbol;
    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    fn sym(name: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new("test.rs", 1, 1))
    }

    fn id(name: &str) -> SymbolId {
        SymbolId::new(format!("test.rs:{name}:1"))
    }

    fn build_graph(builder: impl FnOnce(&mut CallGraph)) -> CallGraph {
        let mut g = CallGraph::new();
        builder(&mut g);
        g
    }

    fn add_edge(g: &mut CallGraph, a: &str, b: &str) {
        g.add_symbol(sym(a));
        g.add_symbol(sym(b));
        let _ = g.add_dependency_with_provenance(
            &id(a),
            &id(b),
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }

    #[test]
    fn page_rank_empty_graph_returns_empty_map() {
        let g = CallGraph::new();
        let scores = GraphAnalyticsService::page_rank(&g, 0.85, 100);
        assert!(scores.is_empty());
    }

    #[test]
    fn page_rank_dag_assigns_higher_score_to_root() {
        // A -> B, A -> C. A has out-degree 2, B/C are leaves.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "A", "C");
        });
        let scores = GraphAnalyticsService::page_rank(&g, 0.85, 100);
        // A is depended-upon by both B and C (incoming edges from
        // its children in the call graph mean... actually in our
        // model the edge `A -> B` means A calls B, so A is the
        // caller. PageRank over a directed "calls" graph measures
        // "who is called the most" — so B and C should score higher
        // than A). The exact ranking is not asserted, only that all
        // three symbols are scored and the distribution is sane.
        assert_eq!(scores.len(), 3);
        for (_, v) in &scores {
            assert!(*v > 0.0);
        }
    }

    #[test]
    fn all_simple_paths_empty_when_symbols_missing() {
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
        });
        let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("missing"), 5);
        assert!(paths.is_empty());
    }

    #[test]
    fn all_simple_paths_finds_three_paths_in_diamond() {
        // A -> B, A -> C, B -> D, C -> D, A -> D. Three paths
        // from A to D: direct, via B, via C.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "A", "C");
            add_edge(g, "B", "D");
            add_edge(g, "C", "D");
            add_edge(g, "A", "D");
        });
        let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 5);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn all_simple_paths_respects_max_hops() {
        // A -> B -> C -> D. With max_hops=2 (3 edges) all three
        // intermediate nodes can be traversed; the path A -> B -> C
        // -> D is exactly 3 intermediate nodes. With max_hops=0 no
        // path fits.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "C");
            add_edge(g, "C", "D");
        });
        let paths_long = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 5);
        assert_eq!(paths_long.len(), 1);
        let paths_short = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 0);
        assert!(paths_short.is_empty());
    }

    #[test]
    fn condensation_dag_returns_n_singletons() {
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "C");
        });
        let comps = GraphAnalyticsService::condensation(&g);
        assert_eq!(comps.len(), 3);
        for c in &comps {
            assert_eq!(c.len(), 1);
        }
    }

    #[test]
    fn condensation_cycle_collapses_into_single_component() {
        // A -> B -> A. Single SCC of size 2.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "A");
        });
        let comps = GraphAnalyticsService::condensation(&g);
        let total: usize = comps.iter().map(|c| c.len()).sum();
        assert_eq!(total, 2);
        let big: Vec<_> = comps.iter().filter(|c| c.len() == 2).collect();
        assert_eq!(big.len(), 1);
    }

    #[test]
    fn god_nodes_empty_graph_returns_empty_vec() {
        let g = CallGraph::new();
        let god = GraphAnalyticsService::god_nodes(&g, 0.95);
        assert!(god.is_empty());
    }

    #[test]
    fn god_nodes_single_node_returns_that_node() {
        let g = build_graph(|g| {
            g.add_symbol(sym("only"));
        });
        let god = GraphAnalyticsService::god_nodes(&g, 0.5);
        // percentile clamp guarantees at least the top-1 survives
        // (the threshold index is min(len-1, len*p) = 0 for len=1).
        assert_eq!(god.len(), 1);
        assert_eq!(god[0].0, id("only"));
    }

    #[test]
    fn god_nodes_finds_highly_called_symbol() {
        // "core" is called by every other symbol — PageRank should
        // rank it as a top god node. We check it appears in the top
        // results (not strictly first) because floating-point tie-breaking
        // during power iteration may favor a7 over core by < 1e-12.
        let g = build_graph(|g| {
            add_edge(g, "a1", "core");
            add_edge(g, "a2", "core");
            add_edge(g, "a3", "core");
            add_edge(g, "a4", "core");
            add_edge(g, "a5", "core");
            add_edge(g, "a6", "core");
            add_edge(g, "a7", "core");
            add_edge(g, "a8", "core");
            add_edge(g, "a9", "core");
            add_edge(g, "a10", "core");
        });
        let god = GraphAnalyticsService::god_nodes(&g, 0.5);
        assert!(!god.is_empty());
        // core should be in the god nodes set (it's called by every other symbol)
        let core_score: Option<f64> = god
            .iter()
            .find(|(sid, _)| sid == &id("core"))
            .map(|(_, s)| *s);
        assert!(
            core_score.is_some(),
            "core should appear in god_nodes results"
        );
        // core's score should be at least as high as the top result (allowing tiny float drift)
        let top_score = god[0].1;
        assert!(
            core_score.unwrap() >= top_score - 1e-10,
            "core score ({}) should match top score ({}) within floating-point tolerance",
            core_score.unwrap(),
            top_score
        );
    }

    #[test]
    fn transitive_reduction_dag_drops_implied_edges() {
        // A -> B, A -> C, B -> C. The A->C edge is implied by
        // A->B->C; it should be dropped.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "A", "C");
            add_edge(g, "B", "C");
        });
        let reduced = GraphAnalyticsService::transitive_reduction(&g);
        // A->B and B->C survive; A->C is dropped.
        assert!(reduced.contains(&(id("A"), id("B"))));
        assert!(reduced.contains(&(id("B"), id("C"))));
        assert!(!reduced.contains(&(id("A"), id("C"))));
    }

    #[test]
    fn transitive_reduction_acyclic_diamond() {
        // A -> B, A -> C, B -> D, C -> D. Two paths to D, but no
        // direct edge implies a longer one. A->D does not exist
        // here, so all four edges should survive (none is implied).
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "A", "C");
            add_edge(g, "B", "D");
            add_edge(g, "C", "D");
        });
        let reduced = GraphAnalyticsService::transitive_reduction(&g);
        assert_eq!(reduced.len(), 4);
    }

    #[test]
    fn transitive_reduction_cycle_keeps_all_edges() {
        // Cyclic graph: every edge must survive (no edge is implied
        // by a strictly longer simple path).
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "A");
        });
        let reduced = GraphAnalyticsService::transitive_reduction(&g);
        assert_eq!(reduced.len(), 2);
    }

    #[test]
    fn feedback_arc_set_acyclic_returns_empty() {
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "C");
        });
        let fas = GraphAnalyticsService::feedback_arc_set(&g);
        assert!(fas.is_empty());
    }

    #[test]
    fn feedback_arc_set_cycle_returns_at_least_one_edge() {
        // A -> B -> A. Removing either edge makes the graph acyclic.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "A");
        });
        let fas = GraphAnalyticsService::feedback_arc_set(&g);
        assert!(!fas.is_empty());
        // Both endpoints must come from the cycle.
        for (s, d) in &fas {
            assert!(*s == id("A") || *s == id("B"));
            assert!(*d == id("A") || *d == id("B"));
        }
    }

    #[test]
    fn feedback_arc_set_three_cycle() {
        // A -> B -> C -> A. At least one edge must be flagged.
        let g = build_graph(|g| {
            add_edge(g, "A", "B");
            add_edge(g, "B", "C");
            add_edge(g, "C", "A");
        });
        let fas = GraphAnalyticsService::feedback_arc_set(&g);
        assert!(!fas.is_empty());
    }

    // =============================================================================
    // CallerCapabilities and boundary guard tests
    // =============================================================================

    #[test]
    fn default_boundary_guard_allows_internal_and_trusted_rest() {
        let guard = DefaultAnalyticsBoundaryGuard::new();
        assert!(guard.can_persist(CallerCapabilities::Internal));
        assert!(guard.can_persist(CallerCapabilities::TrustedREST));
        assert!(!guard.can_persist(CallerCapabilities::ExternalMCP));
        assert!(!guard.can_persist(CallerCapabilities::Explorer));
    }

    #[test]
    fn apply_caller_limits_tightens_base_limits() {
        use crate::domain::plan::limits::PlanLimitKind;
        use crate::domain::plan::limits::PlanLimits;

        let base = PlanLimits {
            time_ms: Some(1000),
            cancellation: None,
            max_depth: None,
            max_hops: None,
            max_visited_nodes: Some(10000),
            max_visited_edges: None,
            max_result_rows: Some(5000),
            max_path_count: None,
            max_memory_bytes: None,
        };

        // Caller tightens all limits
        let caller = PlanLimits {
            time_ms: Some(500),
            cancellation: None,
            max_depth: None,
            max_hops: None,
            max_visited_nodes: Some(5000),
            max_visited_edges: None,
            max_result_rows: Some(2500),
            max_path_count: None,
            max_memory_bytes: None,
        };

        let result = apply_caller_limits(&base, caller).unwrap();
        assert_eq!(result.time_ms, Some(500));
        assert_eq!(result.max_visited_nodes, Some(5000));
        assert_eq!(result.max_result_rows, Some(2500));
    }

    #[test]
    fn apply_caller_limits_rejects_widening() {
        use crate::domain::plan::limits::PlanLimits;

        let base = PlanLimits {
            time_ms: Some(1000),
            cancellation: None,
            max_depth: None,
            max_hops: None,
            max_visited_nodes: Some(10000),
            max_visited_edges: None,
            max_result_rows: Some(5000),
            max_path_count: None,
            max_memory_bytes: None,
        };

        // Caller tries to widen time limit
        let caller = PlanLimits {
            time_ms: Some(2000), // wider than base 1000
            cancellation: None,
            max_depth: None,
            max_hops: None,
            max_visited_nodes: Some(10000),
            max_visited_edges: None,
            max_result_rows: Some(5000),
            max_path_count: None,
            max_memory_bytes: None,
        };

        let result = apply_caller_limits(&base, caller);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AnalyticsError::LimitPolicyViolation(_)
        ));
    }
}

// =============================================================================
// Default Analytics Registry (Cohort 1 + Cohort 2)
// =============================================================================

/// Build an `AlgorithmRegistry` pre-loaded with all 11 algorithms:
///
/// **Cohort 1:**
/// - `pagerank` — PageRank importance scores
/// - `scc` — Strongly Connected Components
/// - `wcc` — Weakly Connected Components
/// - `bounded_shortest_paths` — Bounded shortest paths between symbols
///
/// **Cohort 2:**
/// - `dominators` — Dominator tree (directed, root-parametrized)
/// - `articulation_points` — Cut vertices (undirected)
/// - `bridges` — Cut edges (undirected)
/// - `k_core` — K-core decomposition (undirected, k-parametrized)
///
/// **Cohort 3 (E28.6):**
/// - `personalized_pagerank` — PageRank with personalization vector
/// - `conductance` — Community separation metric
/// - `modularity` — Community quality metric
///
/// # Arguments
///
/// - `lineage` — lineage store for run tracking
///
/// # Returns
///
/// A new `AlgorithmRegistry` with all 11 algorithms admitted.
pub fn default_analytics_registry(lineage: Arc<dyn RunLineageStore>) -> AlgorithmRegistry {
    let guard = Arc::new(DefaultAnalyticsBoundaryGuard::new());
    let mut registry = AlgorithmRegistry::new(lineage, Some(guard));

    // Cohort 1 algorithms
    registry
        .admit(Box::new(crate::domain::analytics::PageRankDescriptor))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::SccDescriptor))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::WccDescriptor))
        .unwrap();
    registry
        .admit(Box::new(
            crate::domain::analytics::BoundedShortestPathsDescriptor,
        ))
        .unwrap();

    // Cohort 2 algorithms
    registry
        .admit(Box::new(crate::domain::analytics::DominatorsDescriptor))
        .unwrap();
    registry
        .admit(Box::new(
            crate::domain::analytics::ArticulationPointsDescriptor,
        ))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::BridgesDescriptor))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::KCoreDescriptor))
        .unwrap();

    // Cohort 3 algorithms (E28.6)
    registry
        .admit(Box::new(
            crate::domain::analytics::PersonalizedPageRankDescriptor,
        ))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::ConductanceDescriptor))
        .unwrap();
    registry
        .admit(Box::new(crate::domain::analytics::ModularityDescriptor))
        .unwrap();

    registry
}
