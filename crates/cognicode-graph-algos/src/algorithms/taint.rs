//! Flow-sensitive forward taint analysis over an intra-procedural DFG.
//!
//! ## What this is
//!
//! A pure function that takes:
//!
//! - A flat statement list (the same shape every algorithm in this crate
//!   consumes).
//! - A list of source statements (indices into the statements slice).
//! - A list of sink statements (indices into the statements slice).
//! - A list of untaint statements (indices into the statements slice).
//!
//! And returns the list of `(source, sink)` pairs that share a taint
//! flow along the DFG (the `reaching_uses` walk from WU3), respecting
//! untaint stops.
//!
//! ## Semantics
//!
//! Taint flows **forward** along the DFG's `from → to` edges (where
//! `from` defines a variable that `to` uses). A statement is tainted if
//! it is a source or if it transitively uses a tainted variable. An
//! untaint statement clears the taint of any variable it **defines**;
//! subsequent uses of that variable are not tainted.
//!
//! ## Determinism
//!
//! Output paths are sorted by `(source, sink)`. Sources and sinks are
//! de-duplicated and sorted in the input order they appear in the
//! statements list.
//!
//! ## Limitations
//!
//! - Intra-procedural only — interprocedural taint requires the call
//!   graph + summaries from WU4. That composition is a later WU.
//! - No flow-sensitive control-flow — we walk the DFG as a flat
//!   edge list, not as a path through the CFG. Conservative for v1.
//! - No conditional untaint (e.g. "tainted only if `x` is non-empty").
//!   Conservative for v1.

use serde::{Deserialize, Serialize};

use crate::algorithms::DefUseEdge;

/// Site descriptor: an index into the statements slice plus its role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaintSite {
    /// Index into the statements slice.
    pub stmt_id: usize,
}

/// One taint path: a source statement reaches a sink statement through
/// the DFG, optionally with intermediate statements listed in order.
///
/// `intermediates` does NOT include `source` or `sink` — they are
/// tracked separately so callers can build chains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintPath {
    /// Statement where taint originates.
    pub source: TaintSite,
    /// Statement where taint is consumed.
    pub sink: TaintSite,
    /// Statements between source and sink (exclusive), in chain order.
    pub intermediates: Vec<TaintSite>,
}

/// Outcome of a taint pass.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintResult {
    /// Every `(source, sink)` path detected, sorted.
    pub paths: Vec<TaintPath>,
    /// Statement ids that became tainted at any point during propagation,
    /// sorted.
    pub tainted_statements: Vec<usize>,
    /// Statement ids that contain a `untaint` pattern, sorted.
    pub untaint_statements: Vec<usize>,
}

/// Run the forward taint propagation.
///
/// - `edges`: the DFG edges (output of [`dfg_edges`]). The walk uses
///   these to trace `from → to` flows.
/// - `sources`: indices of statements that produce tainted data.
/// - `sinks`: indices of statements that consume data — any taint reaching
///   a sink emits a path.
/// - `untaints`: indices of statements whose `defs` are sanitised.
///
/// # Returns
///
/// A [`TaintResult`] with all detected paths. Statements are referenced
/// by their `id` field (== statement index, but the algorithm works on
/// the DFG edges which are keyed by `from`/`to` ids).
pub fn taint_forward(
    edges: &[DefUseEdge],
    sources: &[usize],
    sinks: &[usize],
    untaints: &[usize],
) -> TaintResult {
    // Sort + dedupe site indices for deterministic output.
    let mut sources_sorted: Vec<usize> = sources.to_vec();
    sources_sorted.sort_unstable();
    sources_sorted.dedup();

    let mut sinks_sorted: Vec<usize> = sinks.to_vec();
    sinks_sorted.sort_unstable();
    sinks_sorted.dedup();

    let mut untaints_sorted: Vec<usize> = untaints.to_vec();
    untaints_sorted.sort_unstable();
    untaints_sorted.dedup();

    // Build successor adjacency from the DFG.
    // `from` is a def site, `to` is a use site; taint flows from `from`
    // to `to` along the chain (dfg_edges already encodes def→use).
    use std::collections::{BTreeMap, BTreeSet};
    let mut succ: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for e in edges {
        succ.entry(e.from).or_default().insert(e.to);
    }

    // Track, for each tainted variable, the latest def site that produced
    // it AND whether an untaint has cleared it since. Since the v1 model
    // doesn't carry variables per edge (edges are typed by `variable`),
    // we approximate with statement-level tainted flags: a statement is
    // tainted iff it is a source OR some successor edge carries taint.
    // The variable-precise untaint requires per-(stmt, var) tracking,
    // which is a later refinement; v1 treats each statement as tainted
    // once taint reaches it via any variable and as cleared once a
    // statement on the chain ran an `untaint` pattern.

    // Per-statement origin map: track every source that tainted a node
    // (a statement can be tainted by multiple independent sources that
    // merge downstream). We only keep the FIRST origin for each tainted
    // node in `tainted`, then expand into paths at sink-hit time.
    let mut tainted: BTreeMap<usize, usize> = BTreeMap::new(); // stmt -> first-tainting source
    let mut untainted: BTreeSet<usize> = BTreeSet::new(); // stmts whose taint was cleared
    let mut all_origins: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new(); // stmt -> all tainting sources

    // Seed with sources.
    for &s in &sources_sorted {
        tainted.entry(s).or_insert(s);
        all_origins.entry(s).or_default().insert(s);
    }
    // Mark untaints as cleared.
    for &u in &untaints_sorted {
        untainted.insert(u);
        tainted.remove(&u);
        all_origins.remove(&u);
    }

    // BFS forward propagation.
    let mut work: Vec<usize> = tainted.keys().copied().collect();
    while !work.is_empty() {
        let mut next_work: Vec<usize> = Vec::new();
        for &node in &work {
            let origins_at_node = all_origins.get(&node).cloned().unwrap_or_default();
            if let Some(nbrs) = succ.get(&node) {
                for &to in nbrs {
                    if untainted.contains(&to) {
                        continue;
                    }
                    let entry = all_origins.entry(to).or_default();
                    let before = entry.len();
                    for o in &origins_at_node {
                        entry.insert(*o);
                    }
                    if entry.len() != before {
                        // New origin(s) reached `to`. Re-enqueue even if `to`
                        // was already tainted, otherwise a later-arriving
                        // origin would be recorded but never propagated to
                        // its descendants (unequal-depth merge).
                        tainted
                            .entry(to)
                            .or_insert_with(|| *entry.iter().min().unwrap());
                        next_work.push(to);
                    }
                }
            }
        }
        work = next_work;
    }

    // Detect sink hits — emit one path per (source, sink) that taints the sink.
    let mut paths: Vec<TaintPath> = Vec::new();
    for &sink in &sinks_sorted {
        if let Some(origins) = all_origins.get(&sink) {
            for &origin in origins {
                let intermediates = intermediates_on_chain(&succ, origin, sink, &all_origins);
                paths.push(TaintPath {
                    source: TaintSite { stmt_id: origin },
                    sink: TaintSite { stmt_id: sink },
                    intermediates: intermediates
                        .into_iter()
                        .map(|id| TaintSite { stmt_id: id })
                        .collect(),
                });
            }
        }
    }

    // Sort paths by (source, sink) for determinism.
    paths.sort_by_key(|p| (p.source.stmt_id, p.sink.stmt_id));

    TaintResult {
        paths,
        tainted_statements: tainted.keys().copied().collect(),
        untaint_statements: untaints_sorted,
    }
}

/// Reconstruct the chain of statement ids from `origin` to `target` along
/// the DFG successor edges. Returns intermediates (excludes origin and
/// target). BFS — when multiple paths exist we take the shortest one,
/// which is deterministic because `succ` is sorted.
///
/// Reconstruction is **constraint-aware**: only nodes that this `origin`
/// actually tainted are traversed. Without this, a witness could route
/// through a node the taint never reached (e.g. an untainted sanitizer on a
/// topologically shorter route) while the taint really arrived another way.
fn intermediates_on_chain(
    succ: &std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>>,
    origin: usize,
    target: usize,
    tainted_by_origin: &std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>>,
) -> Vec<usize> {
    if origin == target {
        return Vec::new();
    }
    let carries_origin = |node: usize| -> bool {
        tainted_by_origin
            .get(&node)
            .map(|origins| origins.contains(&origin))
            .unwrap_or(false)
    };
    use std::collections::{BTreeSet, VecDeque};
    let mut visited: BTreeSet<usize> = BTreeSet::new();
    let mut prev: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    visited.insert(origin);
    queue.push_back(origin);

    while let Some(node) = queue.pop_front() {
        if node == target {
            // Reconstruct chain: origin ... -> target.
            let mut chain: Vec<usize> = Vec::new();
            let mut cur = target;
            while let Some(&p) = prev.get(&cur) {
                chain.push(p);
                if p == origin {
                    break;
                }
                cur = p;
            }
            chain.reverse();
            if !chain.is_empty() && chain[0] == origin {
                chain.remove(0);
            }
            if chain.last().copied() == Some(target) {
                chain.pop();
            }
            return chain;
        }
        if let Some(nbrs) = succ.get(&node) {
            for &nxt in nbrs {
                if visited.contains(&nxt) || !carries_origin(nxt) {
                    continue;
                }
                visited.insert(nxt);
                prev.insert(nxt, node);
                queue.push_back(nxt);
            }
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::{Statement, dfg_edges};

    fn s(id: usize, defs: &[&str], uses: &[&str]) -> Statement {
        Statement {
            id,
            kind: "assign".into(),
            defs: defs.iter().map(|s| s.to_string()).collect(),
            uses: uses.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn source_to_sink_in_linear_chain_emits_one_path() {
        // x = src(); y = x; sink(y);
        // stmt 0: src → x; stmt 1: x → y; stmt 2: y → sink
        let stmts = vec![
            s(0, &["x"], &["src"]),
            s(1, &["y"], &["x"]),
            s(2, &[], &["y"]), // sink statement
        ];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[0], &[2], &[]);
        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].source.stmt_id, 0);
        assert_eq!(result.paths[0].sink.stmt_id, 2);
        assert_eq!(result.paths[0].intermediates.len(), 1);
        assert_eq!(result.paths[0].intermediates[0].stmt_id, 1);
        assert_eq!(result.tainted_statements, vec![0, 1, 2]);
    }

    #[test]
    fn untaint_between_source_and_sink_breaks_path() {
        // src → x; sanitize(x) → y; sink(y);
        // stmt 0: src → x; stmt 1: untaint (defs y from x); stmt 2: sink(y)
        let stmts = vec![
            s(0, &["x"], &["src"]),
            s(1, &["y"], &["x"]), // untaint site
            s(2, &[], &["y"]),    // sink
        ];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[0], &[2], &[1]);
        // The sink is never tainted because the only path goes through
        // the untaint site which clears it.
        assert!(result.paths.is_empty(), "untaint should break the path");
        assert_eq!(result.untaint_statements, vec![1]);
    }

    #[test]
    fn diamond_propagates_taint_to_both_branches() {
        // src → x; x → y; x → z; y → sink_y; z → sink_z
        let stmts = vec![
            s(0, &["x"], &["src"]),
            s(1, &["y"], &["x"]),
            s(2, &["z"], &["x"]),
            s(3, &[], &["y"]), // sink 1
            s(4, &[], &["z"]), // sink 2
        ];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[0], &[3, 4], &[]);
        assert_eq!(result.paths.len(), 2);
        let sinks: Vec<usize> = result.paths.iter().map(|p| p.sink.stmt_id).collect();
        assert!(sinks.contains(&3));
        assert!(sinks.contains(&4));
    }

    #[test]
    fn no_sources_means_no_paths() {
        let stmts = vec![s(0, &["x"], &[]), s(1, &[], &["x"])];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[], &[1], &[]);
        assert!(result.paths.is_empty());
        assert!(result.tainted_statements.is_empty());
    }

    #[test]
    fn no_sinks_means_no_paths_but_taint_still_propagates() {
        // src → x; x → y; x → z; no sinks
        let stmts = vec![
            s(0, &["x"], &["src"]),
            s(1, &["y"], &["x"]),
            s(2, &["z"], &["x"]),
        ];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[0], &[], &[]);
        assert!(result.paths.is_empty());
        assert_eq!(result.tainted_statements, vec![0, 1, 2]);
    }

    #[test]
    fn sink_reached_via_independent_path_still_emits_path() {
        // Two independent sources, both reaching the same sink.
        // src_a → x; src_b → y; sink(x, y)
        let stmts = vec![
            s(0, &["x"], &["src_a"]),
            s(1, &["y"], &["src_b"]),
            s(2, &[], &["x", "y"]), // sink uses both
        ];
        let edges = dfg_edges(&stmts);
        let result = taint_forward(&edges, &[0, 1], &[2], &[]);
        // Both sources independently taint the sink → 2 paths.
        assert_eq!(result.paths.len(), 2);
    }

    #[test]
    fn empty_inputs_yield_empty_result() {
        let result = taint_forward(&[], &[], &[], &[]);
        assert_eq!(result, TaintResult::default());
    }
}
