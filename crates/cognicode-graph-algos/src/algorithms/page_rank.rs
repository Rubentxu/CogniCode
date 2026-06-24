//! PageRank — pure function. No petgraph, no domain types.
//!
//! Algorithm: iterative power method with dangling-node mass redistribution.
//! Reference: ADR-031 (Linear PageRank) + the proven body in
//! `crates/cognicode-core/src/application/services/graph_analytics.rs:78-177`.
//!
//! This implementation was extracted and validated by the spike at
//! `/tmp/opencode/spike-petgraph-wasm/` (engram obs-2856).

use std::collections::HashMap;

/// Run PageRank on a pre-built adjacency structure.
///
/// # Arguments
///
/// - `in_neighbors`: `in_neighbors[v]` lists every `u` with edge `u → v`.
///   Length MUST equal `n`.
/// - `out_degree`: `out_degree[u]` is the count of edges `u → w`.
///   Length MUST equal `n`.
/// - `n`: number of nodes.
/// - `alpha`: damping factor (typical: 0.85).
/// - `max_iterations`: hard upper bound on iterations (typical: 100).
///
/// # Returns
///
/// `scores[node_id]` = PageRank score. Mass conservation: sum ≈ 1.0
/// when `n > 0`. Deterministic order: `BTreeMap` would be needed for
/// ordered iteration; `HashMap` is sufficient since PageRank scores are
/// keyed by node id.
///
/// # Edge cases (matches spec REQ-001..014)
///
/// - `n == 0`: returns empty map
/// - Single node, no edges: returns `{0: 1.0}`
/// - Disconnected components: receive non-zero scores via the dangling term
/// - NaN/Inf scores: clamped to 0.0
pub fn page_rank(
    in_neighbors: &[Vec<usize>],
    out_degree: &[usize],
    n: usize,
    alpha: f64,
    max_iterations: usize,
) -> HashMap<usize, f64> {
    if n == 0 {
        return HashMap::new();
    }
    debug_assert_eq!(in_neighbors.len(), n);
    debug_assert_eq!(out_degree.len(), n);

    let inv_n = 1.0 / n as f64;
    let mut ranks: Vec<f64> = vec![inv_n; n];

    const TOLERANCE: f64 = 1e-6;

    for _ in 0..max_iterations.max(1) {
        // Dangling-node mass: nodes with no outgoing edges contribute rank
        // uniformly to all nodes (avoids "black hole" accumulation).
        let mut dangling_sum = 0.0_f64;
        for v in 0..n {
            if out_degree[v] == 0 {
                dangling_sum += ranks[v];
            }
        }
        let dangling_contrib = alpha * dangling_sum * inv_n;
        let base = (1.0 - alpha) * inv_n;

        let mut new_ranks: Vec<f64> = vec![0.0; n];
        let mut max_delta = 0.0_f64;
        for v in 0..n {
            let mut incoming = 0.0_f64;
            for &u in &in_neighbors[v] {
                let od = out_degree[u];
                if od > 0 {
                    incoming += ranks[u] / od as f64;
                }
            }
            let r = base + dangling_contrib + alpha * incoming;
            let new_v = if r.is_finite() && r > 0.0 { r } else { 0.0 };
            let delta = (new_v - ranks[v]).abs();
            if delta > max_delta {
                max_delta = delta;
            }
            new_ranks[v] = new_v;
        }
        ranks = new_ranks;
        if max_delta < TOLERANCE {
            break;
        }
    }

    let mut out: HashMap<usize, f64> = HashMap::with_capacity(n);
    for v in 0..n {
        out.insert(v, ranks[v]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty graph: empty map.
    #[test]
    fn empty_graph_returns_empty_map() {
        let result = page_rank(&[], &[], 0, 0.85, 100);
        assert!(result.is_empty());
    }

    /// Single node, no edges: score = 1.0 (all mass on the only node).
    #[test]
    fn single_node_returns_one() {
        let in_neighbors = vec![Vec::new()];
        let out_degree = vec![0];
        let result = page_rank(&in_neighbors, &out_degree, 1, 0.85, 100);
        assert_eq!(result.len(), 1);
        assert!((result[&0] - 1.0).abs() < 1e-6);
    }

    /// 3-node cycle A→B→C→A: mass conservation, all scores > 0.
    #[test]
    fn cycle_mass_conservation() {
        // A=0, B=1, C=2. A→B→C→A.
        let in_neighbors = vec![
            vec![2], // A is called by C
            vec![0], // B is called by A
            vec![1], // C is called by B
        ];
        let out_degree = vec![1, 1, 1];
        let result = page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);
        let sum: f64 = result.values().sum();
        assert!((sum - 1.0).abs() < 1e-3, "mass conservation: sum={}", sum);
        assert!(result[&0] > 0.0);
        assert!(result[&1] > 0.0);
        assert!(result[&2] > 0.0);
        // Cycle symmetry: all scores equal in a uniform 3-cycle
        let s0 = result[&0];
        assert!((result[&1] - s0).abs() < 1e-6);
        assert!((result[&2] - s0).abs() < 1e-6);
    }

    /// Self-loop: A→A (no edges to other nodes).
    #[test]
    fn self_loop_only() {
        let in_neighbors = vec![vec![0]]; // A is called by A
        let out_degree = vec![1];        // A calls A
        let result = page_rank(&in_neighbors, &out_degree, 1, 0.85, 100);
        // All mass on A
        assert!((result[&0] - 1.0).abs() < 1e-6);
    }

    /// Disconnected: {A,B} cycle + isolated C. A and B share mass; C
    /// gets base amount plus dangling contributions from A and B.
    #[test]
    fn disconnected_components() {
        // A=0 ↔ B=1 (2-cycle), C=2 isolated.
        let in_neighbors = vec![
            vec![1], // A called by B
            vec![0], // B called by A
            Vec::new(), // C has no callers
        ];
        let out_degree = vec![1, 1, 0]; // C has no callees (dangling)
        let result = page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);
        let sum: f64 = result.values().sum();
        assert!((sum - 1.0).abs() < 1e-3, "sum={}", sum);
        // C still gets non-zero score from dangling redistribution
        assert!(result[&2] > 0.0);
    }

    /// Star: center C, leaves L1..L5. Center should have highest rank.
    #[test]
    fn star_center_outranks_leaves() {
        // 6 nodes: 0..5. Center=0, leaves=1..5. Edges: leaf → center.
        // (Wait — let me make center have outgoing edges to leaves, so
        // its out_degree is high but its incoming from itself is the test.)
        // Actually, for "center outranks leaves", we need incoming edges
        // to center. So leaves → center.
        let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); 6];
        let mut out_degree = vec![0usize; 6];
        for leaf in 1..6 {
            in_neighbors[0].push(leaf); // center is called by leaf
            out_degree[leaf] = 1;       // leaf calls center
        }
        let result = page_rank(&in_neighbors, &out_degree, 6, 0.85, 100);
        let center = result[&0];
        for leaf in 1..6 {
            assert!(center > result[&leaf], "center should outrank leaf {}", leaf);
        }
    }

    /// Determinism: same input → same output across runs (within tolerance).
    #[test]
    fn deterministic_across_runs() {
        let in_neighbors = vec![vec![1, 2], vec![0], vec![0]];
        let out_degree = vec![2, 1, 1];
        let r1 = page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);
        let r2 = page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);
        assert_eq!(r1, r2);
    }
}