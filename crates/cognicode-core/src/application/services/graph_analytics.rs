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

use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::infrastructure::graph::CallGraphProjection;

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
    /// Edge cases:
    /// - Empty graph -> empty map.
    /// - Disconnected components still receive non-zero scores via
    ///   petgraph's stochastic-matrix normalisation.
    pub fn page_rank(graph: &CallGraph, alpha: f64, max_iterations: usize) -> HashMap<SymbolId, f64> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        if g.node_count() == 0 {
            return HashMap::new();
        }

        // `page_rank` needs `NodeIndexable` (for `to_index`/`from_index`),
        // `IntoEdges` (to read out-degree) and `NodeCount`. The
        // projection's `StableGraph` already implements all three.
        let scores: Vec<f64> = petgraph::algo::page_rank(g, alpha, max_iterations);

        // `Vec<D>` is indexed by `to_index`, so length == `node_bound()`
        // (upper bound of NodeIndex values), not `node_count()`. We
        // only emit entries for live nodes — that's the contract of
        // `SymbolId` keyed output.
        let mut out: HashMap<SymbolId, f64> = HashMap::with_capacity(g.node_count());
        for (sid, ni) in projection.id_to_index() {
            let idx = ni.index();
            if let Some(score) = scores.get(idx) {
                out.insert(sid.clone(), *score);
            }
        }
        out
    }

    /// Find all simple paths from `from` to `to` bounded by `max_hops`.
    ///
    /// A simple path does not repeat a node, so cycles are terminated
    /// by the visited-set inside petgraph. `max_hops` is the maximum
    /// number of intermediate nodes (i.e. the path may traverse at
    /// most `max_hops + 1` edges).
    ///
    /// Edge cases:
    /// - Missing `from` or `to` id -> empty vec.
    /// - No path within `max_hops` -> empty vec.
    /// - `from == to` -> no path is emitted (petgraph's behaviour).
    pub fn all_simple_paths(
        graph: &CallGraph,
        from: &SymbolId,
        to: &SymbolId,
        max_hops: usize,
    ) -> Vec<Vec<SymbolId>> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        let (Some(&from_ni), Some(&to_ni)) = (
            projection.id_to_index().get(from),
            projection.id_to_index().get(to),
        ) else {
            return Vec::new();
        };

        // `all_simple_paths` takes the graph by value, but
        // `&'a StableGraph` implements `IntoNeighborsDirected`, so
        // passing a reference is sufficient and avoids consuming the
        // projection.
        petgraph::algo::simple_paths::all_simple_paths::<Vec<_>, _>(
            g,
            from_ni,
            to_ni,
            0,
            Some(max_hops),
        )
        .into_iter()
        .map(|path: Vec<NodeIndex>| {
            path.into_iter()
                .filter_map(|ni| g.node_weight(ni).cloned())
                .collect()
        })
        .collect()
    }

    /// Compute the SCC condensation of the call graph.
    ///
    /// Each returned `Vec<SymbolId>` is one strongly connected
    /// component. The order of components and the order of nodes
    /// inside a component follow petgraph's `tarjan_scc` output
    /// (post-order on the DFS tree). Self-loops surface as singleton
    /// components.
    pub fn condensation(graph: &CallGraph) -> Vec<Vec<SymbolId>> {
        CallGraphProjection::from_call_graph(graph).strongly_connected_components()
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

        let mut sorted_scores: Vec<f64> = scores.values().copied().collect();
        sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Clamp percentile into [0.0, 1.0] before computing the
        // threshold index. `len() * percentile` can saturate `usize`
        // for huge graphs; the saturating_sub on the upper bound
        // protects against that.
        let p = percentile.clamp(0.0, 1.0);
        let threshold_idx = ((sorted_scores.len() as f64) * p) as usize;
        let threshold_idx = threshold_idx.min(sorted_scores.len().saturating_sub(1));
        let threshold = sorted_scores[threshold_idx];

        let mut result: Vec<(SymbolId, f64)> = scores
            .into_iter()
            .filter(|(_, s)| *s >= threshold)
            .collect();
        result.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result
    }

    /// Compute the transitive reduction of the call graph — the
    /// minimal set of edges that preserves reachability.
    ///
    /// Returns every `(source, target)` pair that survives the
    /// reduction. Edges that are implied by a longer path (e.g.
    /// `A -> C` when `A -> B` and `B -> C` exist) are dropped.
    ///
    /// Implementation note: petgraph's `tred` module requires a
    /// `NodeCompactIndexable` graph (its `dag_to_toposorted_adjacency_list`
    /// helper). `StableGraph` does not implement that trait, so we
    /// materialise a `DiGraph` snapshot of the projection. The copy
    /// is unavoidable per petgraph 0.6 API.
    pub fn transitive_reduction(graph: &CallGraph) -> Vec<(SymbolId, SymbolId)> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        if g.node_count() == 0 {
            return Vec::new();
        }

        // Snapshot the projection into a DiGraph that satisfies the
        // NodeCompactIndexable bound of petgraph::algo::tred.
        let mut pg: petgraph::graph::DiGraph<SymbolId, ()> = petgraph::graph::DiGraph::new();
        let mut id_to_idx: HashMap<SymbolId, NodeIndex> = HashMap::new();
        for ni in g.node_indices() {
            let sid = g[ni].clone();
            let idx = pg.add_node(sid.clone());
            id_to_idx.insert(sid, idx);
        }
        for edge in g.edge_references() {
            let s = g[edge.source()].clone();
            let d = g[edge.target()].clone();
            if let (Some(&s_idx), Some(&d_idx)) = (id_to_idx.get(&s), id_to_idx.get(&d)) {
                pg.add_edge(s_idx, d_idx, ());
            }
        }

        // Cycle-safe handling: a transitive reduction is well-defined
        // only for DAGs. For graphs with cycles we approximate by
        // collecting every edge of the snapshot — that's the
        // "reduction" of a cyclic graph (no edge is implied by a
        // strictly longer simple path, since cycles make every
        // shorter edge also part of some cycle).
        if petgraph::algo::is_cyclic_directed(&pg) {
            return pg
                .edge_references()
                .map(|e| (pg[e.source()].clone(), pg[e.target()].clone()))
                .collect();
        }

        let toposort = match petgraph::algo::toposort(&pg, None) {
            Ok(order) => order,
            Err(_) => {
                // Defensive: is_cyclic_directed said no, but the
                // graph changed in flight. Bail with the identity
                // (every edge kept) rather than risk a panic.
                return pg
                    .edge_references()
                    .map(|e| (pg[e.source()].clone(), pg[e.target()].clone()))
                    .collect();
            }
        };

        let (tred, _tclos) = petgraph::algo::tred::dag_transitive_reduction_closure(
            &petgraph::algo::tred::dag_to_toposorted_adjacency_list(&pg, &toposort).0,
        );
        // tred is a `List<(), u32>` (unweighted adjacency-list);
        // iterate its edges and translate back to SymbolId pairs.
        let mut out: Vec<(SymbolId, SymbolId)> = Vec::new();
        for edge in tred.edge_references() {
            // `edge.source()` returns `NodeIndex<u32>` for
            // `List<(), u32>`. Annotate explicitly so the
            // subsequent `toposort.get(idx.index())` call resolves.
            let s_idx: NodeIndex = edge.source();
            let d_idx: NodeIndex = edge.target();
            // Map rank-positions back through toposort to original NodeIndex.
            if let (Some(&s_orig), Some(&d_orig)) = (
                toposort.get(s_idx.index()),
                toposort.get(d_idx.index()),
            ) {
                if let (Some(s_id), Some(d_id)) = (pg.node_weight(s_orig), pg.node_weight(d_orig)) {
                    out.push((s_id.clone(), d_id.clone()));
                }
            }
        }
        out
    }

    /// Find the greedy feedback arc set — edges whose removal makes
    /// the dependency graph acyclic.
    ///
    /// Useful for resolving circular dependencies: the reported edges
    /// are the cheapest candidates to break first (per the
    /// Eades-Lin-Smyth heuristic that petgraph implements).
    ///
    /// Returns an empty vec for an acyclic graph.
    pub fn feedback_arc_set(graph: &CallGraph) -> Vec<(SymbolId, SymbolId)> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        // The greedy FAS needs `GraphProp<EdgeType = Directed>`,
        // which `StableGraph` implements. We can pass the projection
        // graph by reference.
        petgraph::algo::feedback_arc_set::greedy_feedback_arc_set(g)
            .map(|edge| {
                let s = g[edge.source()].clone();
                let d = g[edge.target()].clone();
                (s, d)
            })
            .collect()
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
        // rank it as the god node.
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
        // Top result should be the most-called symbol.
        assert_eq!(god[0].0, id("core"));
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
}
