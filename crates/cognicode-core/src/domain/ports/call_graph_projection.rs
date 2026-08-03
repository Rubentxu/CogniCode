//! `CallGraphProjectionPort` — domain port for read-side call-graph projections.
//!
//! Analytics descriptors and application services run graph algorithms
//! over a petgraph-backed snapshot of the [`CallGraph`] aggregate. That
//! snapshot is produced by the infrastructure adapter
//! (`crate::infrastructure::graph::CallGraphProjection`); domain and
//! application code depend on this trait instead of the concrete
//! adapter so the hexagonal dependency direction holds
//! (domain → ports, never domain → infrastructure).
//!
//! ## Construction
//!
//! Construction stays in the infrastructure adapter. The object-safe
//! factory [`project_call_graph`] bridges the domain surface: consumers
//! call it and receive an `Arc<dyn CallGraphProjectionPort>` without
//! importing `crate::infrastructure::*`.
//!
//! ## Symbol indexing
//!
//! [`CallGraphProjectionPort::symbol_index`] exposes the `SymbolId →
//! NodeIndex` map that translates algorithm results (index-based) back
//! to domain-level `SymbolId`s. It replaces the old
//! `id_to_index().get(id).index()` pattern at consumer call sites.

use std::collections::HashMap;
use std::sync::Arc;

use petgraph::graph::NodeIndex;

use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
use crate::domain::value_objects::DependencyType;

/// Errors that a projection algorithm can return.
///
/// Currently only [`ProjectionError::CycleDetected`] is reachable from
/// [`CallGraphProjectionPort::topological_sort`]. Additional variants are kept
/// private to the implementation so the public surface is the minimum
/// required by the spec.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    /// The graph contains a directed cycle and a topological ordering is
    /// therefore impossible.
    #[error("cycle detected in graph")]
    CycleDetected,
}

/// Direction selector for [`CallGraphProjectionPort::extract_subgraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubgraphDirection {
    /// Walk outgoing edges (successors of the root).
    Outgoing,
    /// Walk incoming edges (predecessors of the root).
    Incoming,
    /// Walk both outgoing and incoming edges (BFS treats them as one
    /// unified frontier; the BFS depth still increases by 1 per
    /// traversal step, regardless of direction).
    Both,
}

/// A typed edge in a [`SubgraphView`]: carries the symbol endpoints,
/// the [`DependencyType`] and the sanitized confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphEdge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub dependency_type: DependencyType,
    pub confidence: f64,
}

/// A neighborhood subgraph of a projection: the set of nodes reachable
/// from `root` within `max_depth` hops in the chosen direction, plus
/// every edge traversed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubgraphView {
    pub nodes: Vec<SymbolId>,
    pub edges: Vec<SubgraphEdge>,
}

/// A single hop on an explanation path: the (from, to) symbols, the
/// edge's [`DependencyType`] and confidence, plus a human-readable
/// `rationale` string (e.g. `"calls"`, `"inherits from"`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExplanationHop {
    pub from: SymbolId,
    pub to: SymbolId,
    pub dependency_type: DependencyType,
    pub confidence: f64,
    pub rationale: String,
}

/// A complete explanation: ordered list of hops plus the sum of edge
/// costs along the chosen path.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplanationView {
    pub hops: Vec<ExplanationHop>,
    pub total_cost: f64,
}

/// Build a read-side projection from an existing [`CallGraph`].
///
/// Object-safe factory: the petgraph-backed construction logic lives in
/// the infrastructure adapter (`CallGraphProjection::from_call_graph`);
/// this free function bridges the domain surface so analytics descriptors
/// and services can construct a projection without importing
/// `crate::infrastructure::*`. It returns an `Arc` trait object because a
/// no-receiver static method on the trait would break `dyn` compatibility.
pub fn project_call_graph(cg: &CallGraph) -> Arc<dyn CallGraphProjectionPort> {
    Arc::new(crate::infrastructure::graph::CallGraphProjection::from_call_graph(cg))
}

/// Read-side algorithmic projection over the canonical [`CallGraph`] aggregate.
///
/// The trait abstracts the projection operations needed by analytics
/// descriptors and services. Implementations snapshot the node/edge set
/// at construction time and answer algorithmic queries from that
/// snapshot without mutating the underlying domain aggregate.
///
/// Construction is provided by the module-level [`project_call_graph`]
/// factory (object-safe), not by a trait method.
pub trait CallGraphProjectionPort: Send + Sync {
    /// Number of nodes in the projection.
    fn node_count(&self) -> usize;

    /// Number of edges in the projection.
    fn edge_count(&self) -> usize;

    /// Number of symbols known to the source `CallGraph`.
    fn symbol_count(&self) -> usize;

    /// Look up the [`Symbol`] for a given `SymbolId` from the side-lookup.
    fn resolve_symbol(&self, id: &SymbolId) -> Option<&Symbol>;

    /// Access the `SymbolId` → `NodeIndex` mapping (read-only).
    ///
    /// Used to translate algorithm results (which use `NodeIndex`) back
    /// to the domain-level `SymbolId` for MCP/tool output.
    fn symbol_index(&self) -> &HashMap<SymbolId, NodeIndex>;

    /// Build incoming adjacency: `(in_neighbors, out_degree)`.
    ///
    /// `in_neighbors[u]` lists every caller of `u`; `out_degree[u]` is
    /// the number of outgoing edges from `u`.
    fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>);

    /// Build outgoing adjacency (each node's targets).
    ///
    /// `out_neighbors[u]` lists every `v` with edge `u → v`.
    fn build_out_neighbors(&self) -> Vec<Vec<usize>>;

    /// Build undirected adjacency (union of incoming and outgoing edges).
    fn build_undirected_neighbors(&self) -> Vec<Vec<usize>>;

    /// Compute a topological ordering of the nodes.
    ///
    /// - Returns `Ok(vec![])` for an empty graph.
    /// - Returns `Err(ProjectionError::CycleDetected)` if a cycle is present.
    fn topological_sort(&self) -> Result<Vec<SymbolId>, ProjectionError>;

    /// Partition the graph into strongly connected components.
    fn strongly_connected_components(&self) -> Vec<Vec<SymbolId>>;

    /// Partition the graph into connected components under the
    /// **undirected** interpretation. Isolated nodes appear as singletons.
    fn connected_components(&self) -> Vec<Vec<SymbolId>>;

    /// Return `true` if the graph contains a directed cycle.
    ///
    /// An empty graph returns `false`. A self-loop on a single node
    /// counts as a cycle.
    fn detect_cycles(&self) -> bool;

    /// Return `true` if there is a directed path from `from` to `to`.
    ///
    /// Returns `false` (no panic) if either id is unknown. The trivial
    /// self-path `A → A` returns `true` when `A` is present in the graph.
    fn has_path(&self, from: &SymbolId, to: &SymbolId) -> bool;

    /// Compute the lowest-cost path from `from` to `to`.
    ///
    /// Cost per edge is `1.0 - sanitize_confidence(confidence)`. Returns
    /// `None` if either id is unknown or `to` is unreachable from `from`.
    fn dijkstra(&self, from: &SymbolId, to: &SymbolId) -> Option<(Vec<SymbolId>, f64)>;

    /// Compute the *reverse* impact radius of `root`: the set of
    /// predecessors of `root` reachable within `max_depth` incoming hops.
    ///
    /// Returns `vec![]` (no panic) when `root` is missing, `max_depth == 0`,
    /// or the projection is empty.
    fn find_impact_radius(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>;

    /// Compute the *forward* reach of `root`: the set of successors of
    /// `root` reachable within `max_depth` outgoing hops.
    ///
    /// Symmetric counterpart of [`Self::find_impact_radius`].
    /// `max_depth == usize::MAX` is a sentinel meaning "follow every
    /// reachable successor".
    fn find_forward_reach(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>;

    /// Extract a neighborhood subgraph of `root` bounded by `max_depth`
    /// hops in `direction` (Outgoing / Incoming / Both).
    fn extract_subgraph(
        &self,
        root: &SymbolId,
        direction: SubgraphDirection,
        max_depth: usize,
    ) -> SubgraphView;

    /// Explain the lowest-cost path from `from` to `to` by walking the
    /// underlying graph edge-by-edge and collecting
    /// `(dependency_type, confidence, rationale)` per hop.
    fn explain_path(&self, from: &SymbolId, to: &SymbolId) -> Option<ExplanationView>;
}
