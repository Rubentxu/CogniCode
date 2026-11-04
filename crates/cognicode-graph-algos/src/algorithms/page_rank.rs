//! PageRank — pure function. No petgraph, no domain types.
//!
//! Algorithm: iterative power method with dangling-node mass redistribution.
//! Reference: ADR-031 (Linear PageRank) + the proven body in
//! `crates/cognicode-core/src/application/services/graph_analytics.rs:78-177`.
//!
//! This implementation was extracted and validated by the spike at
//! `/tmp/opencode/spike-petgraph-wasm/` (engram obs-2856).

use std::collections::HashMap;

/// Largest per-node change below which the power method stops.
const TOLERANCE: f64 = 1e-6;

/// `true` when the largest per-node change of an iteration is strictly below
/// [`TOLERANCE`]. A change exactly equal to the tolerance is NOT converged.
fn has_converged(max_delta: f64) -> bool {
    max_delta < TOLERANCE
}

/// Clamp a freshly computed rank: NaN, infinities, negatives and `-0.0` all
/// become a positive `0.0`.
fn sanitize_rank(r: f64) -> f64 {
    if r.is_finite() && r > 0.0 { r } else { 0.0 }
}

/// Absolute change of one node's rank between two iterations.
fn rank_delta(new_rank: f64, old_rank: f64) -> f64 {
    (new_rank - old_rank).abs()
}

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

    for _ in 0..max_iterations.max(1) {
        // Dangling-node mass: nodes with no outgoing edges contribute rank
        // uniformly to all nodes (avoids "black hole" accumulation).
        let mut dangling_sum = 0.0_f64;
        for (v, _) in ranks.iter().enumerate().take(n) {
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
            let new_v = sanitize_rank(r);
            let delta = rank_delta(new_v, ranks[v]);
            if delta > max_delta {
                max_delta = delta;
            }
            new_ranks[v] = new_v;
        }
        ranks = new_ranks;
        if has_converged(max_delta) {
            break;
        }
    }

    let mut out: HashMap<usize, f64> = HashMap::with_capacity(n);
    for (v, &rank) in ranks.iter().enumerate().take(n) {
        out.insert(v, rank);
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
        let out_degree = vec![1]; // A calls A
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
            vec![1],    // A called by B
            vec![0],    // B called by A
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
    #[allow(clippy::needless_range_loop)]
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
            out_degree[leaf] = 1; // leaf calls center
        }
        let result = page_rank(&in_neighbors, &out_degree, 6, 0.85, 100);
        let center = result[&0];
        for leaf in 1..6 {
            assert!(
                center > result[&leaf],
                "center should outrank leaf {}",
                leaf
            );
        }
    }

    /// Runs `page_rank` on a thread so a hanging mutant fails the test
    /// instead of hanging the suite.
    fn run(
        in_neighbors: Vec<Vec<usize>>,
        out_degree: Vec<usize>,
        max_iterations: usize,
    ) -> HashMap<usize, f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let n = out_degree.len();
            let _ = tx.send(page_rank(
                &in_neighbors,
                &out_degree,
                n,
                0.85,
                max_iterations,
            ));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("page_rank did not finish within 5s")
    }

    /// Chain 0→1 with 1 dangling, alpha 0.85. Fixed point by hand:
    /// r0 = 0.075 + 0.425·r1, r1 = 0.075 + 0.425·r1 + 0.85·r0
    /// → r1 = 0.13875 / 0.21375 = 37/57, r0 = 20/57.
    #[test]
    fn chain_with_dangling_tail_reaches_the_fixed_point() {
        let r = run(vec![vec![], vec![0]], vec![1, 0], 1000);
        assert!((r[&0] - 20.0 / 57.0).abs() < 1e-4, "r0 = {}", r[&0]);
        assert!((r[&1] - 37.0 / 57.0).abs() < 1e-4, "r1 = {}", r[&1]);
    }

    /// One iteration from the uniform start (0.5, 0.5): the dangling mass is
    /// 0.5, so r0 = 0.075 + 0.85·0.5·0.5 = 0.2875 and
    /// r1 = 0.075 + 0.2125 + 0.85·0.5 = 0.7125.
    #[test]
    fn a_single_iteration_gives_the_hand_computed_values() {
        let r = run(vec![vec![], vec![0]], vec![1, 0], 1);
        assert!((r[&0] - 0.2875).abs() < 1e-12, "r0 = {}", r[&0]);
        assert!((r[&1] - 0.7125).abs() < 1e-12, "r1 = {}", r[&1]);
    }

    /// The tolerance is strict: a change equal to it has NOT converged.
    #[test]
    fn convergence_threshold_is_strict() {
        assert!(has_converged(0.0));
        assert!(has_converged(TOLERANCE / 2.0));
        assert!(!has_converged(TOLERANCE));
        assert!(!has_converged(TOLERANCE * 2.0));
    }

    #[test]
    fn sanitize_rank_clamps_everything_but_positive_finite() {
        assert_eq!(sanitize_rank(0.25), 0.25);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.5, 0.0, -0.0] {
            let s = sanitize_rank(bad);
            assert_eq!(s, 0.0, "{bad}");
            assert!(s.is_sign_positive(), "{bad} must become +0.0");
        }
    }

    #[test]
    fn rank_delta_is_the_absolute_difference() {
        assert!((rank_delta(0.3, 0.5) - 0.2).abs() < 1e-12);
        assert!((rank_delta(0.5, 0.3) - 0.2).abs() < 1e-12);
        assert_eq!(rank_delta(0.4, 0.4), 0.0);
    }

    /// 0 → {1, 2}; 1 and 2 dangling; one iteration from 1/3 each.
    /// dangling = 2/3, base = 0.05, dangling_contrib = 0.85·(2/3)/3 = 17/90.
    /// r0 = 0.05 + 17/90 = 43/180; r1 = r2 = 43/180 + 0.85·(1/3)/2 = 137/360.
    #[test]
    fn rank_is_split_by_out_degree() {
        let r = run(vec![vec![], vec![0], vec![0]], vec![2, 0, 0], 1);
        assert!((r[&0] - 43.0 / 180.0).abs() < 1e-12, "r0 = {}", r[&0]);
        assert!((r[&1] - 137.0 / 360.0).abs() < 1e-12, "r1 = {}", r[&1]);
        assert!((r[&2] - 137.0 / 360.0).abs() < 1e-12, "r2 = {}", r[&2]);
    }

    /// Inconsistent input: node 0 lists 1 as a caller but 1's out-degree is 0.
    /// Such an edge contributes nothing (no division by zero): both nodes are
    /// dangling and stay at 0.5.
    #[test]
    fn caller_with_zero_out_degree_contributes_nothing() {
        let r = run(vec![vec![1], vec![]], vec![0, 0], 5);
        assert!((r[&0] - 0.5).abs() < 1e-12, "r0 = {}", r[&0]);
        assert!((r[&1] - 0.5).abs() < 1e-12, "r1 = {}", r[&1]);
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
