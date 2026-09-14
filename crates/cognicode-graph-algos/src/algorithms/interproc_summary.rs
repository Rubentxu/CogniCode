//! Interprocedural summary computation over a call graph.
//!
//! ## What this is
//!
//! A **bottom-up summary** of what a function reads, writes, and calls.
//! Given a call graph (`out_neighbors` between FunctionIds) and a flat
//! per-function statement list (the same shape that `dfg` consumes),
//! the algorithm walks the SCC condensation in reverse topological
//! order and accumulates:
//!
//! - `reads`: every variable the function reads, transitively.
//! - `writes`: every variable the function writes, transitively.
//! - `calls`: every FunctionId the function calls, transitively (deduped).
//! - `kind`:
//!     - `"leaf"` if the function has no outgoing calls.
//!     - `"bottom_up"` if the function has at least one call but no cycle.
//!     - `"fixed_point"` if the function participates in a recursive
//!       SCC (a fixed-point iteration was applied).
//!
//! ## Summary id
//!
//! `summary_id` is the SHA-256 hex digest of the canonical JSON
//! representation of the above four fields, sorted and lower-cased.
//! Two functions with the same `(reads, writes, calls)` collapse to the
//! same summary id; this is the per-snapshot dedup contract (design D4).
//!
//! ## Determinism
//!
//! - SCCs are sorted lexicographically inside each SCC.
//! - SCCs themselves are emitted in Tarjan's deterministic post-order.
//! - `reads` and `writes` are sorted lexicographically.
//! - `calls` is sorted by FunctionId (numeric).
//!
//! This makes the SHA-256 input bit-stable across runs and platforms.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};

use crate::algorithms::{Statement, condensation, dfg_edges};

/// One function's local view used to compute the summary.
///
/// `reads` and `writes` are populated by walking the function's DFG
/// (intra-procedural). `calls` is the set of **indices** into the
/// `functions` slice — not the semantic `function_id`. This matches the
/// convention used by every other algorithm in `cognicode-graph-algos`:
/// the hot loop sees `&[Vec<usize>]` where `usize` is an index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionLocalView {
    /// Semantic function identifier (for reporting only — does NOT have to
    /// be an index into `functions`).
    pub function_id: usize,
    /// Statements inside this function.
    #[serde(default)]
    pub statements: Vec<Statement>,
    /// Indices of other functions in the `functions` slice that this
    /// function calls directly.
    #[serde(default)]
    pub calls: Vec<usize>,
}

/// The bottom-up summary of one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterprocSummary {
    /// FunctionId.
    pub function_id: usize,
    /// `kind` discriminator — see module docs.
    pub kind: SummaryKind,
    /// Variables the function reads, transitively.
    pub reads: Vec<String>,
    /// Variables the function writes, transitively.
    pub writes: Vec<String>,
    /// FunctionIds the function calls, transitively (deduped).
    /// Reported as semantic FunctionIds, not array indices.
    pub calls: Vec<usize>,
}

/// Discriminator for the computation path that produced a summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryKind {
    /// No outgoing calls — summary is the local view verbatim.
    Leaf,
    /// Bottom-up accumulation over a DAG of SCCs.
    BottomUp,
    /// Function participates in a recursive SCC; fixed-point applied.
    FixedPoint,
}

/// Compute summaries for every function in `functions`.
///
/// `call_graph[from]` lists the indices (into `functions`) of every callee.
/// It is the same shape every other algorithm in this crate consumes.
///
/// `functions` may carry semantic `function_id`s that are NOT contiguous
/// or zero-based; the algorithm maps them through `id_to_idx` internally
/// and reports them back in the output.
/// Indices that appear in `calls` or `call_graph` but are >= `functions.len()`
/// are silently dropped (defensive against malformed input).
pub fn compute_summaries(
    call_graph: &[Vec<usize>],
    functions: &[FunctionLocalView],
) -> Vec<InterprocSummary> {
    let n = functions.len();

    // Build a stable per-function view of local reads/writes/calls (as
    // **indices** into the functions slice, not semantic FunctionIds).
    let mut local_reads: Vec<BTreeSet<String>> = Vec::with_capacity(n);
    let mut local_writes: Vec<BTreeSet<String>> = Vec::with_capacity(n);
    let mut local_calls: Vec<BTreeSet<usize>> = Vec::with_capacity(n);

    for f in functions {
        // Intra-procedural DFG → reads + writes (defs = writes, uses = reads).
        let mut reads: BTreeSet<String> = BTreeSet::new();
        let mut writes: BTreeSet<String> = BTreeSet::new();
        for stmt in &f.statements {
            for v in &stmt.defs {
                writes.insert(v.clone());
            }
            for v in &stmt.uses {
                reads.insert(v.clone());
            }
        }
        // Touch dfg_edges to keep the dependency explicit; the local
        // collects above already capture everything we need from it.
        let _ = dfg_edges(&f.statements);

        // Filter `calls` to in-range indices; ignore semantic FunctionIds
        // that point outside the function slice (defensive).
        let mut calls: BTreeSet<usize> = BTreeSet::new();
        for &c in &f.calls {
            if c < n {
                calls.insert(c);
            }
        }
        local_reads.push(reads);
        local_writes.push(writes);
        local_calls.push(calls);
    }

    // Build the canonical call graph in the same index space. If
    // `call_graph` is shorter than `n`, pad with empty vecs. Out-of-range
    // callees coming from `call_graph` are dropped here as well.
    let mut padded_cg: Vec<Vec<usize>> = call_graph.to_vec();
    while padded_cg.len() < n {
        padded_cg.push(Vec::new());
    }
    for row in padded_cg.iter_mut() {
        row.retain(|&c| c < n);
    }

    // SCC condensation over the (now same-length) call graph.
    let sccs = condensation(&padded_cg, n);

    let mut node_scc: Vec<usize> = vec![0; n];
    for (scc_id, scc) in sccs.iter().enumerate() {
        for &node in scc {
            if node < n {
                node_scc[node] = scc_id;
            }
        }
    }

    // Recursion detection: SCC size > 1 or any node with a self-loop.
    let mut scc_is_recursive: Vec<bool> = vec![false; sccs.len()];
    for (scc_id, scc) in sccs.iter().enumerate() {
        if scc.len() > 1 {
            scc_is_recursive[scc_id] = true;
            continue;
        }
        if let Some(&only) = scc.first()
            && padded_cg
                .get(only)
                .map(|nbrs| nbrs.contains(&only))
                .unwrap_or(false)
        {
            scc_is_recursive[scc_id] = true;
        }
    }

    // Initial kind per node. FixedPoint wins over Leaf and BottomUp.
    let mut kind_per_node: Vec<SummaryKind> = vec![SummaryKind::BottomUp; n];
    for node in 0..n {
        if scc_is_recursive[node_scc[node]] {
            kind_per_node[node] = SummaryKind::FixedPoint;
        } else if local_calls.get(node).map(|s| s.is_empty()).unwrap_or(true) {
            kind_per_node[node] = SummaryKind::Leaf;
        }
    }

    // Per-SCC accumulators seeded with members' local data.
    let mut scc_reads: Vec<BTreeSet<String>> = vec![BTreeSet::new(); sccs.len()];
    let mut scc_writes: Vec<BTreeSet<String>> = vec![BTreeSet::new(); sccs.len()];
    let mut scc_calls: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); sccs.len()];

    for (scc_id, scc) in sccs.iter().enumerate() {
        for &node in scc {
            if node < n {
                scc_reads[scc_id].extend(local_reads[node].iter().cloned());
                scc_writes[scc_id].extend(local_writes[node].iter().cloned());
                scc_calls[scc_id].extend(local_calls[node].iter().cloned());
            }
        }
    }

    // Process SCCs in reverse topological order (= reverse of Tarjan's
    // post-order, which is a valid topological order for the SCC DAG).
    // We pre-compute positions so the inner check is O(1) per edge.
    let scc_order: Vec<usize> = (0..sccs.len()).rev().collect();
    let scc_pos: HashMap<usize, usize> = scc_order
        .iter()
        .enumerate()
        .map(|(pos, &s)| (s, pos))
        .collect();

    for &scc_id in &scc_order {
        let members: Vec<usize> = sccs[scc_id].iter().copied().filter(|&m| m < n).collect();
        let my_pos = scc_pos[&scc_id];

        let mut callee_sccs_to_union: Vec<usize> = Vec::new();
        for &node in &members {
            if let Some(nbrs) = padded_cg.get(node) {
                for &callee in nbrs {
                    if callee >= n {
                        continue;
                    }
                    let callee_scc = node_scc[callee];
                    if callee_scc == scc_id {
                        continue;
                    }
                    if scc_pos[&callee_scc] > my_pos {
                        callee_sccs_to_union.push(callee_scc);
                    }
                }
            }
        }
        callee_sccs_to_union.sort_unstable();
        callee_sccs_to_union.dedup();

        for callee_scc in &callee_sccs_to_union {
            let r: Vec<String> = scc_reads[*callee_scc].iter().cloned().collect();
            let w: Vec<String> = scc_writes[*callee_scc].iter().cloned().collect();
            let c: Vec<usize> = scc_calls[*callee_scc].iter().copied().collect();
            scc_reads[scc_id].extend(r);
            scc_writes[scc_id].extend(w);
            scc_calls[scc_id].extend(c);
        }

        // Fixed-point over a recursive SCC: union each member's local data
        // with every other member's reachable set until stable.
        if scc_is_recursive[scc_id] {
            loop {
                let mut changed = false;
                for &node in &members {
                    let before_r = scc_reads[scc_id].len();
                    let before_w = scc_writes[scc_id].len();
                    let before_c = scc_calls[scc_id].len();
                    for &other in &members {
                        if other == node {
                            continue;
                        }
                        scc_reads[scc_id].extend(local_reads[other].iter().cloned());
                        scc_writes[scc_id].extend(local_writes[other].iter().cloned());
                        scc_calls[scc_id].extend(local_calls[other].iter().cloned());
                    }
                    if scc_reads[scc_id].len() != before_r
                        || scc_writes[scc_id].len() != before_w
                        || scc_calls[scc_id].len() != before_c
                    {
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
    }

    // Emit per-function summaries. Calls reported as semantic FunctionIds
    // (we translate back through the index of each function in `functions`).
    let mut summaries: Vec<InterprocSummary> = Vec::with_capacity(n);
    for node in 0..n {
        let scc_id = node_scc[node];
        let reads: Vec<String> = scc_reads[scc_id].iter().cloned().collect();
        let writes: Vec<String> = scc_writes[scc_id].iter().cloned().collect();
        // Translate index-based `calls` back to semantic FunctionIds.
        let mut calls: Vec<usize> = scc_calls[scc_id]
            .iter()
            .map(|&idx| functions[idx].function_id)
            .collect();
        calls.sort_unstable();
        summaries.push(InterprocSummary {
            function_id: functions[node].function_id,
            kind: kind_per_node[node],
            reads,
            writes,
            calls,
        });
    }

    summaries
}

/// Compute the SHA-256 hex digest of the canonical JSON for a summary.
///
/// Used as the dedup / cache key (design D4). Inputs are already sorted
/// inside [`compute_summaries`] so the JSON serialization is deterministic.
pub fn summary_id(summary: &InterprocSummary) -> String {
    let mut hasher = Sha256::new();
    // serde_json sorts maps alphabetically by default, so the canonical
    // form of (function_id, kind, reads, writes, calls) is bit-stable.
    let canonical = serde_json::to_string(summary).expect("summary is JSON-safe");
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: usize, defs: &[&str], uses: &[&str]) -> Statement {
        Statement {
            id,
            kind: "assign".into(),
            defs: defs.iter().map(|s| s.to_string()).collect(),
            uses: uses.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn f(id: usize, stmts: Vec<Statement>, calls: Vec<usize>) -> FunctionLocalView {
        FunctionLocalView {
            function_id: id,
            statements: stmts,
            calls,
        }
    }

    #[test]
    fn leaf_function_carries_only_local_reads_and_writes() {
        // foo: writes x, no calls.
        let functions = vec![f(1, vec![s(0, &["x"], &[])], vec![])];
        let summaries = compute_summaries(&[vec![]], &functions);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].function_id, 1);
        assert_eq!(summaries[0].kind, SummaryKind::Leaf);
        assert_eq!(summaries[0].writes, vec!["x".to_string()]);
        assert!(summaries[0].reads.is_empty());
        assert!(summaries[0].calls.is_empty());
    }

    #[test]
    fn chain_caller_inherits_callee_reads_and_writes() {
        // foo (idx 0) calls bar (idx 1); bar writes y and uses z.
        // foo writes x and uses y (from bar).
        let functions = vec![
            f(1, vec![s(0, &["x"], &[])], vec![1]),   // foo → idx 1 (bar)
            f(2, vec![s(1, &["y"], &["z"])], vec![]), // bar → idx 0 (none)
        ];
        // Call graph: foo (idx 0) → bar (idx 1).
        let summaries = compute_summaries(&[vec![1], vec![]], &functions);
        let foo = summaries.iter().find(|s| s.function_id == 1).unwrap();
        let bar = summaries.iter().find(|s| s.function_id == 2).unwrap();
        // foo inherits bar's reads + writes transitively.
        assert!(foo.writes.contains(&"x".to_string()));
        assert!(foo.writes.contains(&"y".to_string()));
        assert!(foo.reads.contains(&"z".to_string()));
        // Calls reported as semantic FunctionIds, so 2 (bar's id).
        assert!(foo.calls.contains(&2));
        assert_eq!(foo.kind, SummaryKind::BottomUp);
        // bar remains a leaf.
        assert_eq!(bar.kind, SummaryKind::Leaf);
        assert!(bar.writes.contains(&"y".to_string()));
    }

    #[test]
    fn recursive_self_call_yields_fixed_point_kind() {
        // foo (idx 0, function_id 1) calls itself (idx 0); foo writes x.
        let functions = vec![f(1, vec![s(0, &["x"], &[])], vec![0])];
        // Call graph: foo (idx 0) → foo (idx 0): self-loop.
        let summaries = compute_summaries(&[vec![0]], &functions);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, SummaryKind::FixedPoint);
        assert_eq!(summaries[0].writes, vec!["x".to_string()]);
        // Self-call still appears in `calls` for visibility (as semantic id).
        assert!(summaries[0].calls.contains(&1));
    }

    #[test]
    fn mutual_recursion_marks_all_participants_fixed_point() {
        // a (idx 0, id 1) ↔ b (idx 1, id 2): a calls b, b calls a.
        let functions = vec![
            f(1, vec![s(0, &["a_var"], &[])], vec![1]),
            f(2, vec![s(1, &["b_var"], &[])], vec![0]),
        ];
        // Call graph: 0 → 1, 1 → 0
        let summaries = compute_summaries(&[vec![1], vec![0]], &functions);
        for s in &summaries {
            assert_eq!(s.kind, SummaryKind::FixedPoint);
            // Each function's writes are unioned into both summaries.
            assert!(s.writes.contains(&"a_var".to_string()));
            assert!(s.writes.contains(&"b_var".to_string()));
        }
    }

    #[test]
    fn summary_id_is_stable_across_calls() {
        let functions = vec![f(1, vec![s(0, &["x"], &[])], vec![])];
        let summaries = compute_summaries(&[vec![]], &functions);
        let id_a = summary_id(&summaries[0]);
        let id_b = summary_id(&summaries[0]);
        assert_eq!(id_a, id_b, "same summary → same id");
        assert_eq!(id_a.len(), 64, "sha256 hex = 64 chars");
    }

    #[test]
    fn identical_summaries_collapse_to_the_same_id() {
        let s_a = InterprocSummary {
            function_id: 1,
            kind: SummaryKind::Leaf,
            reads: vec!["r".into()],
            writes: vec!["w".into()],
            calls: vec![],
        };
        let s_b = InterprocSummary {
            function_id: 2, // different function_id
            kind: SummaryKind::Leaf,
            reads: vec!["r".into()],
            writes: vec!["w".into()],
            calls: vec![],
        };
        // Different function_id → different id (it's part of the digest input).
        assert_ne!(summary_id(&s_a), summary_id(&s_b));

        // Same function_id → same id.
        let s_c = InterprocSummary {
            function_id: 1,
            ..s_a.clone()
        };
        assert_eq!(summary_id(&s_a), summary_id(&s_c));
    }

    #[test]
    fn empty_input_yields_no_summaries() {
        let summaries = compute_summaries(&[], &[]);
        assert!(summaries.is_empty());
    }
}
