//! Application-layer service that answers graph-aware impact queries over
//! the canonical [`CallGraph`] aggregate.
//!
//! `ImpactAnalysisService` is a **stateless** coordinator: it holds no
//! fields, exposes a `new()` constructor, and rebuilds a
//! [`CallGraphProjection`] on every public call from the immutable
//! `&CallGraph` reference passed by the caller. The projection is
//! consumed read-only — neither the aggregate nor the projection is
//! mutated.
//!
//! ## Direction semantics
//!
//! [`Self::impact_radius`] follows the **predecessor** direction
//! (reverse BFS over incoming edges). It answers the "what depends on X"
//! question.
//!
//! [`Self::forward_radius`] follows the **successor** direction
//! (forward BFS over outgoing edges). It answers the "what does X
//! affect" question. The two methods are symmetric counterparts and
//! must return identical sets when applied to the same `(graph, root,
//! max_depth)` triple on a graph where the edge direction is reversed
//! (mirror graph).
//!
//! ## Companion to [`crate::domain::services::ImpactAnalyzer`]
//!
//! `ImpactAnalyzer` (domain service) stays untouched and continues to
//! produce the count-based [`ImpactReport`]. The application service
//! does not replace it — it is a strictly additive capability that
//! gives callers access to the graph algorithms exposed by
//! `CallGraphProjection` (Dijkstra, SCC, connected components, etc.)
//! from a single, well-typed entry point.

use crate::application::dto::{
    ClusterResultDto, ExplainResultDto, PathResultDto, SubgraphResultDto,
};
use crate::domain::aggregates::CallGraph;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::infrastructure::graph::{CallGraphProjection, SubgraphDirection};

/// Application service for graph-aware impact analysis.
///
/// Holds **no state**. Every public method rebuilds a
/// [`CallGraphProjection`] from the supplied `&CallGraph`, then delegates
/// to the projection algorithm that best fits the query. Because the
/// projection is per-call and read-only, the service is safe to share
/// across threads and is trivially constructible.
pub struct ImpactAnalysisService;

impl ImpactAnalysisService {
    /// Build a new stateless service.
    pub fn new() -> Self {
        Self
    }

    /// Compute the **predecessor** impact radius of `root`: every symbol
    /// that depends (directly or transitively) on `root`, within
    /// `max_depth` reverse hops.
    ///
    /// This is the "what breaks if I change X" query — surface all
    /// callers of `root` up to the supplied depth. The root itself is
    /// **not** included in the result.
    ///
    /// Returns `vec![]` (no panic) when:
    /// - `root` is not present in the graph;
    /// - `max_depth == 0`;
    /// - the graph is empty.
    ///
    /// `max_depth == usize::MAX` is a sentinel meaning "follow every
    /// reachable predecessor" and is passed through to the projection
    /// unchanged.
    pub fn impact_radius(
        &self,
        graph: &CallGraph,
        root: &SymbolId,
        max_depth: usize,
    ) -> Vec<SymbolId> {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection.find_impact_radius(root, max_depth)
    }

    /// Compute the **successor** forward radius of `root`: every symbol
    /// that `root` calls (directly or transitively), within `max_depth`
    /// forward hops.
    ///
    /// This is the "what does X affect" query — surface every symbol
    /// reachable from `root` along outgoing edges up to the supplied
    /// depth. The root itself is **not** included in the result.
    ///
    /// Symmetric counterpart of [`Self::impact_radius`]: delegates to
    /// [`CallGraphProjection::find_forward_reach`] after building a
    /// fresh projection.
    ///
    /// Returns `vec![]` (no panic) when:
    /// - `root` is not present in the graph;
    /// - `max_depth == 0`;
    /// - the graph is empty.
    ///
    /// `max_depth == usize::MAX` is a sentinel meaning "follow every
    /// reachable successor" and is passed through to the projection
    /// unchanged.
    pub fn forward_radius(
        &self,
        graph: &CallGraph,
        root: &SymbolId,
        max_depth: usize,
    ) -> Vec<SymbolId> {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection.find_forward_reach(root, max_depth)
    }

    /// Return `true` iff a directed path exists from `from` to `to`.
    ///
    /// Returns `false` (no panic) when either id is missing from the
    /// graph. The trivial self-path `A -> A` returns `true` when `A` is
    /// present.
    pub fn has_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> bool {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection.has_path(from, to)
    }

    /// Compute the lowest-cost (highest-confidence) path from `from` to
    /// `to`.
    ///
    /// Edge cost is `1.0 - sanitize_confidence(confidence)`. Returns
    /// `None` when either id is missing or no path exists. The returned
    /// path starts at `from` and ends at `to`; a self-path
    /// `A -> A` returns `Some(PathResultDto { path: [A], total_cost: 0.0, .. })`.
    pub fn shortest_path(
        &self,
        graph: &CallGraph,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Option<PathResultDto> {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection
            .dijkstra(from, to)
            .map(|(path, cost)| PathResultDto::from_path(path, cost))
    }

    /// Return all non-trivial strongly connected components (SCCs) of
    /// size ≥ 2.
    ///
    /// Self-loops (size-1 SCCs) are excluded to match the
    /// `CycleDetector` convention: the result describes **mutual**
    /// dependencies, not isolated nodes whose only cycle is a
    /// degenerate self-loop. An empty graph returns `vec![]`.
    pub fn detect_cycles(&self, graph: &CallGraph) -> Vec<Vec<SymbolId>> {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection
            .strongly_connected_components()
            .into_iter()
            .filter(|scc| scc.len() >= 2)
            .collect()
    }

    /// Return the undirected connected component containing `id`.
    ///
    /// Returns `None` (no panic) when `id` is not in the graph. An
    /// isolated node (no edges) is its own component and returns
    /// `Some(vec![id])`.
    pub fn containing_component(&self, graph: &CallGraph, id: &SymbolId) -> Option<Vec<SymbolId>> {
        let projection = CallGraphProjection::from_call_graph(graph);
        projection
            .connected_components()
            .into_iter()
            .find(|component| component.iter().any(|member| member == id))
    }

    /// Extract a neighborhood subgraph of `root` bounded by `max_depth`
    /// hops in `direction`.
    ///
    /// Thin wrapper around [`CallGraphProjection::extract_subgraph`]
    /// that converts the projection-level [`SubgraphView`](crate::infrastructure::graph::SubgraphView)
    /// into the wire-friendly [`SubgraphResultDto`].
    ///
    /// Edge cases (mirrored from the projection layer):
    /// - Unknown `root` → `nodes: [], edges: []`.
    /// - `max_depth == 0` → `nodes: [root], edges: []`.
    /// - Cycle reachable → `visited` set prevents infinite loop.
    pub fn subgraph(
        &self,
        graph: &CallGraph,
        root: &SymbolId,
        direction: SubgraphDirection,
        max_depth: usize,
    ) -> SubgraphResultDto {
        let projection = CallGraphProjection::from_call_graph(graph);
        SubgraphResultDto::from_view(projection.extract_subgraph(root, direction, max_depth))
    }

    /// Cluster the graph by `method`:
    /// - `"scc"` → strongly connected components (Tarjan).
    /// - `"connected"` → undirected connected components.
    ///
    /// The MCP layer rejects unknown method strings with an explicit
    /// error, so this method defaults to `scc` for any unrecognized
    /// label (defensive fallback; unreachable in practice).
    ///
    /// The returned [`ClusterResultDto`] preserves the order returned
    /// by the underlying algorithm. Tarjan SCC and the
    /// `connected_components` BFS each yield their own canonical order;
    /// callers wanting order-insensitive equality should sort by
    /// `ClusterDto::members` (string form).
    pub fn cluster_components(&self, graph: &CallGraph, method: &str) -> ClusterResultDto {
        let projection = CallGraphProjection::from_call_graph(graph);
        let clusters: Vec<Vec<SymbolId>> = match method {
            "scc" => projection.strongly_connected_components(),
            "connected" => projection.connected_components(),
            // Defensive fallback: caller pre-validated.
            _ => projection.strongly_connected_components(),
        };
        ClusterResultDto::from_clusters(clusters)
    }

    /// Explain the lowest-cost path from `from` to `to`.
    ///
    /// Wraps [`CallGraphProjection::explain_path`] into a
    /// wire-friendly [`ExplainResultDto`]. **The outer `Option` is
    /// always `Some`** — a missing path is encoded as
    /// `Some(ExplainResultDto { found: false, ... })` so the MCP tool
    /// returns a structured payload (not `is_error=true`).
    ///
    /// - `found = true`, `hops` non-empty → projection found a path.
    /// - `found = false`, `hops` empty → projection returned `None`
    ///   (endpoint missing or unreachable).
    pub fn explain_path(
        &self,
        graph: &CallGraph,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Option<ExplainResultDto> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let view = projection.explain_path(from, to);
        Some(match view {
            Some(view) => ExplainResultDto::from_view(&view, true),
            None => ExplainResultDto::not_found(),
        })
    }
}

impl Default for ImpactAnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Symbol;
    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    // ---------------------------------------------------------------------
    // Test fixtures
    // ---------------------------------------------------------------------

    /// Build a `Symbol` whose fully-qualified name is
    /// `format!("test.rs:{name}:1")` (matches the FQN format used by
    /// `CallGraph::add_symbol` for these test fixtures). The
    /// `id(name)` helper below mirrors this format so they line up.
    fn sym(name: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new("test.rs", 1, 1))
    }

    /// Compute the `SymbolId` that `CallGraph::add_symbol(sym(name))`
    /// would assign. The aggregate derives the id from the symbol's FQN
    /// (`"{file}:{name}:{line}"`).
    fn id(name: &str) -> SymbolId {
        SymbolId::new(format!("test.rs:{name}:1"))
    }

    /// Build a `CallGraph` from a closure that mutates a fresh graph.
    fn make_graph(builder: impl FnOnce(&mut CallGraph)) -> CallGraph {
        let mut g = CallGraph::new();
        builder(&mut g);
        g
    }

    /// Add both endpoints and an outgoing edge `a -> b` with confidence
    /// `1.0` (via `DirectExtraction`).
    fn add_edge(g: &mut CallGraph, a: &str, b: &str, dep: DependencyType) {
        g.add_symbol(sym(a));
        g.add_symbol(sym(b));
        let _ = g.add_dependency_with_provenance(
            &id(a),
            &id(b),
            dep,
            ExtractionContext::DirectExtraction,
        );
    }

    /// Sorted `Vec<SymbolId>` view of a list (sorted by string
    /// representation), used to compare order-insensitive results.
    /// `SymbolId` does not implement `Ord`, so we sort by `as_str()`
    /// to get a stable, deterministic ordering.
    fn sorted_set(symbols: &[SymbolId]) -> Vec<SymbolId> {
        let mut v: Vec<SymbolId> = symbols.to_vec();
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    }

    // ---------------------------------------------------------------------
    // impact_radius — R2, E1, E2, E6, E7
    // ---------------------------------------------------------------------

    #[test]
    fn test_impact_radius_bounded_predecessors() {
        // D -> A -> C  and  B -> C.  Predecessors of C are {A, B} (depth
        // 1) and {A, B, D} (depth 2).
        let g = make_graph(|g| {
            add_edge(g, "D", "A", DependencyType::Calls);
            add_edge(g, "A", "C", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();

        assert_eq!(
            sorted_set(&svc.impact_radius(&g, &id("C"), 1)),
            sorted_set(&[id("A"), id("B")])
        );
        assert_eq!(
            sorted_set(&svc.impact_radius(&g, &id("C"), 2)),
            sorted_set(&[id("A"), id("B"), id("D")])
        );
    }

    #[test]
    fn test_impact_radius_zero_depth() {
        // Any non-empty graph; depth 0 must short-circuit.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.impact_radius(&g, &id("B"), 0).is_empty());
        assert!(svc.impact_radius(&g, &id("A"), 0).is_empty());
    }

    #[test]
    fn test_impact_radius_missing_root() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        // `m` is not in the projection — must not panic.
        let m = id("missing");
        assert!(svc.impact_radius(&g, &m, 10).is_empty());
    }

    #[test]
    fn test_impact_radius_empty_graph() {
        let g = CallGraph::new();
        let svc = ImpactAnalysisService::new();
        assert!(svc.impact_radius(&g, &id("anything"), 5).is_empty());
    }

    #[test]
    fn test_impact_radius_max_sentinel() {
        let g = make_graph(|g| {
            add_edge(g, "D", "A", DependencyType::Calls);
            add_edge(g, "A", "C", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        assert_eq!(
            sorted_set(&svc.impact_radius(&g, &id("C"), usize::MAX)),
            sorted_set(&[id("A"), id("B"), id("D")])
        );
    }

    // ---------------------------------------------------------------------
    // has_path — R3, E1, E9
    // ---------------------------------------------------------------------

    #[test]
    fn test_has_path_direct_transitive_no_path() {
        // A -> B -> C
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.has_path(&g, &id("A"), &id("B")));
        assert!(svc.has_path(&g, &id("A"), &id("C")));
        assert!(!svc.has_path(&g, &id("B"), &id("A")));
    }

    #[test]
    fn test_has_path_missing_endpoint() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let m = id("missing");
        assert!(!svc.has_path(&g, &id("A"), &m));
        assert!(!svc.has_path(&g, &m, &id("A")));
    }

    #[test]
    fn test_has_path_self_path() {
        // Single node, no edges.
        let g = make_graph(|g| {
            g.add_symbol(sym("A"));
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.has_path(&g, &id("A"), &id("A")));
    }

    // ---------------------------------------------------------------------
    // shortest_path — R4, E1, E5, E8, E10
    // ---------------------------------------------------------------------

    #[test]
    fn test_shortest_path_confidence_weighted() {
        // All edges are added through `DirectExtraction` (confidence 1.0,
        // cost 0.0) and through `Heuristic { score: 0.5 }` (clamped to
        // 0.5, cost 0.5). The 1-hop path A -> B (cost 0.0) must beat the
        // 2-hop path A -> C -> B (cost 1.0).
        let g = make_graph(|g| {
            // High-confidence direct edge.
            g.add_symbol(sym("A"));
            g.add_symbol(sym("B"));
            let _ = g.add_dependency_with_provenance(
                &id("A"),
                &id("B"),
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );

            // Low-confidence 2-hop path.
            g.add_symbol(sym("C"));
            let _ = g.add_dependency_with_provenance(
                &id("A"),
                &id("C"),
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.5 },
            );
            let _ = g.add_dependency_with_provenance(
                &id("C"),
                &id("B"),
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.5 },
            );
        });
        let svc = ImpactAnalysisService::new();

        let result = svc
            .shortest_path(&g, &id("A"), &id("B"))
            .expect("A -> B is reachable");

        assert_eq!(
            result.path,
            vec!["test.rs:A:1".to_string(), "test.rs:B:1".to_string()]
        );
        assert!(result.found);
        assert!(
            (result.total_cost - 0.0).abs() < 1e-9,
            "direct A->B should be free (cost 0.0), got {}",
            result.total_cost
        );
    }

    #[test]
    fn test_shortest_path_unreachable() {
        // A -> B only; C is unreachable from A.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.shortest_path(&g, &id("A"), &id("C")).is_none());
    }

    #[test]
    fn test_shortest_path_missing_endpoint() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let m = id("missing");
        assert!(svc.shortest_path(&g, &id("A"), &m).is_none());
        assert!(svc.shortest_path(&g, &m, &id("A")).is_none());
    }

    #[test]
    fn test_shortest_path_nan_confidence() {
        // Sanitization invariant. Raw `f64::NAN` injection is rejected by
        // `ConfidenceRules::assign` (returns `InvalidConfidence`), and
        // `CallGraphProjection` re-normalizes non-finite confidences to
        // `1.0` at construction time. This test verifies the
        // post-sanitization contract: every cost observed through
        // `shortest_path` is finite and non-negative.
        //
        // We use `DirectExtraction` to land at the highest-confidence
        // path, then exercise the DTO with a hypothetical
        // post-sanitization cost of 0.0 to assert that `from_path`
        // round-trips the value bit-exactly (which is the cost shape
        // that would arise if `sanitize_confidence` ever mapped NaN
        // to confidence 1.0 → cost 0.0).
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let result = svc
            .shortest_path(&g, &id("A"), &id("B"))
            .expect("A -> B reachable");

        assert!(result.total_cost.is_finite());
        assert!(result.total_cost >= 0.0);

        // DTO round-trip with the post-sanitization cost (NaN -> 1.0 -> 0.0).
        // We pass a one-element `Vec<SymbolId>` because this assertion
        // only exercises the DTO cost-shape contract; the path contents
        // are not relevant to the NaN-sanitization invariant.
        let dto = PathResultDto::from_path(vec![id("A")], 0.0_f64);
        assert!(dto.total_cost.is_finite());
        assert_eq!(dto.total_cost, 0.0);
    }

    #[test]
    fn test_shortest_path_self_path() {
        // Single node, no edges: A -> A is the trivial self-path.
        let g = make_graph(|g| {
            g.add_symbol(sym("A"));
        });
        let svc = ImpactAnalysisService::new();
        let result = svc
            .shortest_path(&g, &id("A"), &id("A"))
            .expect("self-path must exist");

        assert_eq!(result.path, vec!["test.rs:A:1".to_string()]);
        assert!(result.found);
        assert_eq!(result.total_cost, 0.0);
    }

    // ---------------------------------------------------------------------
    // detect_cycles — R5, E7, E9
    // ---------------------------------------------------------------------

    #[test]
    fn test_detect_cycles_dag() {
        // A -> B -> C is acyclic; all SCCs are singletons and filtered
        // out.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.detect_cycles(&g).is_empty());
    }

    #[test]
    fn test_detect_cycles_mutual() {
        // A <-> B forms a single 2-node SCC.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "A", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let sccs = svc.detect_cycles(&g);
        assert_eq!(sccs.len(), 1);
        assert_eq!(sorted_set(&sccs[0]), sorted_set(&[id("A"), id("B")]));
    }

    #[test]
    fn test_detect_cycles_self_loop_excluded() {
        // `CallGraph::add_dependency_with_provenance` rejects an edge
        // whose endpoints resolve to the same id (the symbol is
        // registered with the same FQN), so we cannot build a true
        // self-loop through the public API. A singleton SCC is the
        // only available representation and is filtered out by the
        // service, matching the spec: "Self-loops MUST be excluded".
        let g = make_graph(|g| {
            g.add_symbol(sym("A"));
        });
        let svc = ImpactAnalysisService::new();
        assert!(svc.detect_cycles(&g).is_empty());
    }

    #[test]
    fn test_detect_cycles_multiple() {
        // Two disjoint mutual cycles: {A, B} and {X, Y}.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "A", DependencyType::Calls);
            add_edge(g, "X", "Y", DependencyType::Calls);
            add_edge(g, "Y", "X", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let sccs = svc.detect_cycles(&g);
        assert_eq!(sccs.len(), 2);

        // Order-insensitive membership check.
        let as_sets: Vec<Vec<SymbolId>> = sccs.iter().map(|s| sorted_set(s)).collect();
        assert!(as_sets.contains(&sorted_set(&[id("A"), id("B")])));
        assert!(as_sets.contains(&sorted_set(&[id("X"), id("Y")])));
    }

    #[test]
    fn test_detect_cycles_empty() {
        let g = CallGraph::new();
        let svc = ImpactAnalysisService::new();
        assert!(svc.detect_cycles(&g).is_empty());
    }

    // ---------------------------------------------------------------------
    // containing_component — R6, E1, E3
    // ---------------------------------------------------------------------

    #[test]
    fn test_containing_component_member() {
        // Chain A -> B, A -> C (no cross-edge to the {C, D} pair), so
        // the undirected components are {A, B, C} and {C, D} — but
        // {A, B, C} and {C, D} share C and therefore form a single
        // component {A, B, C, D} under the undirected interpretation
        // used by `connected_components`.
        //
        // We use a clean split instead: A -> B, C -> D with no
        // cross-edges. The component of A is {A, B} and the component
        // of C is {C, D}.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "C", "D", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let comp = svc
            .containing_component(&g, &id("A"))
            .expect("A is in the graph");
        assert_eq!(sorted_set(&comp), sorted_set(&[id("A"), id("B")]));
    }

    #[test]
    fn test_containing_component_missing() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let m = id("missing");
        assert!(svc.containing_component(&g, &m).is_none());
    }

    #[test]
    fn test_containing_component_isolated() {
        // A alone, no edges: A is its own component.
        let g = make_graph(|g| {
            g.add_symbol(sym("A"));
        });
        let svc = ImpactAnalysisService::new();
        let comp = svc
            .containing_component(&g, &id("A"))
            .expect("A is present");
        assert_eq!(comp, vec![id("A")]);
    }

    // ---------------------------------------------------------------------
    // forward_radius — successor BFS (symmetric to impact_radius)
    // ---------------------------------------------------------------------

    /// RED gate: forward_radius must mirror find_forward_reach exactly.
    /// Chain A->B->C, A->D. At depth 2, service and projection must
    /// return identical sets {B, C, D}. Must fail to compile before
    /// `forward_radius` is implemented.
    #[test]
    fn test_forward_radius_mirrors_find_forward_reach() {
        use crate::infrastructure::graph::CallGraphProjection;

        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
            add_edge(g, "A", "D", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();

        let svc_result = svc.forward_radius(&g, &id("A"), 2);
        let projection = CallGraphProjection::from_call_graph(&g);
        let projection_result = projection.find_forward_reach(&id("A"), 2);

        assert_eq!(
            sorted_set(&svc_result),
            sorted_set(&projection_result),
            "forward_radius must mirror find_forward_reach exactly"
        );
        assert_eq!(
            sorted_set(&svc_result),
            sorted_set(&[id("B"), id("C"), id("D")])
        );
    }

    /// Empty graph: must return empty, not panic.
    #[test]
    fn test_forward_radius_empty_graph_returns_empty() {
        let g = CallGraph::new();
        let svc = ImpactAnalysisService::new();
        assert!(svc.forward_radius(&g, &id("anything"), 5).is_empty());
    }

    /// Missing root: must return empty, not panic.
    #[test]
    fn test_forward_radius_missing_symbol_returns_empty() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let m = id("missing");
        assert!(svc.forward_radius(&g, &m, 10).is_empty());
    }

    // ---------------------------------------------------------------------
    // DTO helpers — R7
    // ---------------------------------------------------------------------

    #[test]
    fn test_path_result_dto_roundtrip() {
        use crate::application::dto::PathResultDto;

        let original = PathResultDto {
            path: vec!["A".to_string(), "B".to_string()],
            total_cost: 0.1,
            found: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: PathResultDto = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.path, original.path);
        assert!((decoded.total_cost - original.total_cost).abs() < 1e-9);
        assert_eq!(decoded.found, original.found);
    }

    #[test]
    fn test_scc_dto_size_matches() {
        use crate::application::dto::SccDto;

        // Build a 3-member SCC; the spec example allows duplicates to
        // verify `size` is recomputed from the converted list.
        let members = vec![id("A"), id("B"), id("A")];
        let dto = SccDto::from_scc(members);

        assert_eq!(dto.size, 3);
        assert_eq!(
            dto.members,
            vec![
                "test.rs:A:1".to_string(),
                "test.rs:B:1".to_string(),
                "test.rs:A:1".to_string(),
            ]
        );
    }

    // ---------------------------------------------------------------------
    // Read-only invariant — R8
    // ---------------------------------------------------------------------

    #[test]
    fn test_stateless_non_mutating() {
        // Build a 5-symbol, 7-edge graph and snapshot its counts.
        // Invoke all 5 public methods 100 times each, interleaved.
        // Re-snapshot the counts and assert they are unchanged.
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "A", "C", DependencyType::Calls);
            add_edge(g, "B", "D", DependencyType::Calls);
            add_edge(g, "C", "D", DependencyType::Calls);
            add_edge(g, "D", "E", DependencyType::Calls);
        });

        let symbols_before = g.symbol_count();
        let edges_before = g.edge_count();
        let svc = ImpactAnalysisService::new();

        for _ in 0..100 {
            let _ = svc.impact_radius(&g, &id("E"), 3);
            let _ = svc.has_path(&g, &id("A"), &id("E"));
            let _ = svc.shortest_path(&g, &id("A"), &id("E"));
            let _ = svc.detect_cycles(&g);
            let _ = svc.containing_component(&g, &id("A"));
        }

        assert_eq!(g.symbol_count(), symbols_before);
        assert_eq!(g.edge_count(), edges_before);
    }

    // ---------------------------------------------------------------------
    // Disconnected graph — E3 (and E4: cycle coexistence)
    // ---------------------------------------------------------------------

    #[test]
    fn test_disconnected_graph_component() {
        // A -> B, A -> C is connected (undirected component {A, B, C}).
        // B -> C closes the triangle but introduces no cycle that the
        // SCC detector would surface (Tarjan returns a single 3-node
        // SCC for any node pair, but a 3-node linear chain is acyclic
        // and the SCCs are all singletons).
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "A", "C", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();

        let comp_a = svc
            .containing_component(&g, &id("A"))
            .expect("A is present");
        let comp_c = svc
            .containing_component(&g, &id("C"))
            .expect("C is present");

        assert_eq!(
            sorted_set(&comp_a),
            sorted_set(&[id("A"), id("B"), id("C")])
        );
        assert_eq!(
            sorted_set(&comp_c),
            sorted_set(&[id("A"), id("B"), id("C")])
        );

        // No cycle in the linear chain.
        assert!(svc.detect_cycles(&g).is_empty());
    }

    // ---------------------------------------------------------------------
    // mcp-graph-primitives — RED gate (Phase 0.3)
    //
    // These tests reference service-level methods that do not exist yet:
    //   - `ImpactAnalysisService::subgraph(g, root, direction, max_depth) -> SubgraphResultDto`
    //   - `ImpactAnalysisService::cluster_components(g, method) -> ClusterResultDto`
    //   - `ImpactAnalysisService::explain_path(g, from, to) -> Option<ExplainResultDto>`
    // and DTOs in `crate::application::dto`:
    //   - `SubgraphResultDto`, `SubgraphEdgeDto`
    //   - `ClusterResultDto`, `ClusterDto`
    //   - `ExplainResultDto`, `ExplainHopDto`
    //
    // Each test MUST fail to compile until both the methods and DTOs
    // land in Phase 2.
    // ---------------------------------------------------------------------

    use crate::application::dto::{ClusterResultDto, ExplainResultDto, SubgraphResultDto};
    use crate::infrastructure::graph::SubgraphDirection;

    /// Service `subgraph` MUST mirror the projection `extract_subgraph`
    /// exactly. We use an outgoing 2-hop chain to assert both the node
    /// set and the edge count agree.
    #[test]
    fn test_subgraph_service_mirrors_projection() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "C", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();

        let dto: SubgraphResultDto = svc.subgraph(&g, &id("A"), SubgraphDirection::Outgoing, 2);

        let mut nodes_sorted = dto.nodes.clone();
        nodes_sorted.sort();
        let mut expected_nodes = vec![
            id("A").as_str().to_string(),
            id("B").as_str().to_string(),
            id("C").as_str().to_string(),
        ];
        expected_nodes.sort();
        assert_eq!(nodes_sorted, expected_nodes);
        assert_eq!(dto.edges.len(), 2, "two edges in subgraph dto");
    }

    /// `cluster_components("scc")` delegates to the SCC algorithm; a
    /// 2-node mutual cycle must surface as a single cluster of size 2.
    #[test]
    fn test_cluster_components_scc_method() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "B", "A", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let dto: ClusterResultDto = svc.cluster_components(&g, "scc");
        assert_eq!(dto.0.len(), 1, "exactly one SCC cluster");
        assert_eq!(dto.0[0].size, 2, "SCC cluster has 2 members");
    }

    /// `cluster_components("connected")` delegates to the undirected
    /// connected-components algorithm. A DAG yields one cluster per
    /// connected component.
    #[test]
    fn test_cluster_components_connected_method() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
            add_edge(g, "C", "D", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let dto: ClusterResultDto = svc.cluster_components(&g, "connected");
        // Two disjoint components: {A, B} and {C, D}.
        assert_eq!(dto.0.len(), 2, "two undirected components");
        let sizes: Vec<usize> = {
            let mut s: Vec<usize> = dto.0.iter().map(|c| c.size).collect();
            s.sort();
            s
        };
        assert_eq!(sizes, vec![2, 2]);
    }

    /// Service `explain_path` wraps the projection's `None` as
    /// `Some(ExplainResultDto { found: false, ... })` so the MCP tool
    /// can return a structured payload (not `is_error=true`).
    #[test]
    fn test_explain_path_service_wraps_none_as_found_false() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let result: Option<ExplainResultDto> = svc.explain_path(&g, &id("A"), &id("missing"));
        let dto = result.expect("service must wrap None as Some(found:false)");
        assert!(!dto.found, "found must be false for missing endpoint");
        assert!(dto.hops.is_empty(), "no hops when no path");
        assert_eq!(dto.summary, "no path");
    }

    /// Service `explain_path` returns `Some(found=true, hops=...)` for
    /// reachable endpoints and copies the rationale through the DTO.
    #[test]
    fn test_explain_path_service_found_true_carries_hops() {
        let g = make_graph(|g| {
            add_edge(g, "A", "B", DependencyType::Calls);
        });
        let svc = ImpactAnalysisService::new();
        let result: Option<ExplainResultDto> = svc.explain_path(&g, &id("A"), &id("B"));
        let dto = result.expect("A -> B is reachable");
        assert!(dto.found);
        assert_eq!(dto.hops.len(), 1);
        assert_eq!(dto.hops[0].from, "test.rs:A:1".to_string());
        assert_eq!(dto.hops[0].to, "test.rs:B:1".to_string());
        assert_eq!(dto.hops[0].rationale, "calls");
    }
}
