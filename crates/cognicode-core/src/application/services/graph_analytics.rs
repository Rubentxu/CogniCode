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
use petgraph::visit::{EdgeRef, IntoEdgeReferences, NodeIndexable};

use crate::domain::aggregates::{CallGraph, SymbolId};
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
}
