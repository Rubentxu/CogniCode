//! Personalized PageRank — pure function extending page_rank with personalization.
//!
//! When `personalization` is `None`, this reproduces standard PageRank.
//! When `personalization` is `Some(vec)`, the random walk is biased toward
//! the given probability distribution over nodes (teleportation personalization).
//!
//! Algorithm: iterative power method with dangling-node mass redistribution and
//! optional personalization vector. Reference: ADR-031 (Linear PageRank) + the
//! PageRank variant in `cognicode_graph_algos::page_rank`.

use std::collections::HashMap;

/// Largest per-node change below which an iteration counts as converged.
const TOLERANCE: f64 = 1e-6;

/// A rank is a non-negative finite probability mass; anything else is 0.
fn sanitize_rank(r: f64) -> f64 {
    if r.is_finite() && r > 0.0 { r } else { 0.0 }
}

/// An iteration has converged when its largest change is strictly below
/// [`TOLERANCE`].
fn has_converged(max_delta: f64) -> bool {
    max_delta < TOLERANCE
}

/// Run Personalized PageRank on a pre-built adjacency structure.
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
/// - `personalization`: optional per-node teleportation probabilities.
///   When `None`, standard PageRank (uniform teleportation).
///   When `Some(vec)`, `vec.len()` must equal `n` and sum should be 1.0
///   (will be normalized if not). Used to bias random walk toward specific nodes.
///
/// # Returns
///
/// `scores[node_id]` = Personalized PageRank score. Mass conservation: sum ≈ 1.0
/// when `n > 0` and `personalization` is a proper probability distribution.
///
/// # Edge cases
///
/// - `n == 0`: returns empty map
/// - `personalization.len() != n`: ignored (falls back to uniform)
/// - `personalization` sums to 0 or is otherwise invalid: falls back to uniform
pub fn personalized_pagerank(
    in_neighbors: &[Vec<usize>],
    out_degree: &[usize],
    n: usize,
    alpha: f64,
    max_iterations: usize,
    personalization: Option<&[f64]>,
) -> HashMap<usize, f64> {
    if n == 0 {
        return HashMap::new();
    }
    debug_assert_eq!(in_neighbors.len(), n);
    debug_assert_eq!(out_degree.len(), n);

    // Compute normalized personalization vector (or uniform if None/invalid)
    let teleportation: Vec<f64> = match personalization {
        Some(vec) if vec.len() == n => {
            let sum: f64 = vec.iter().sum();
            if sum > 0.0 && sum.is_finite() {
                vec.iter().map(|&p| p / sum).collect()
            } else {
                vec![1.0 / n as f64; n]
            }
        }
        _ => vec![1.0 / n as f64; n],
    };

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

        let mut new_ranks: Vec<f64> = vec![0.0; n];
        let mut max_delta = 0.0_f64;

        for v in 0..n {
            // Teleportation contribution (personalization or uniform)
            let teleportation_contrib = (1.0 - alpha) * teleportation[v];

            // Dangling contribution redistributed via teleportation distribution
            let dangling_contrib = alpha * dangling_sum * teleportation[v];

            // PageRank contribution from incoming edges
            let mut incoming = 0.0_f64;
            for &u in &in_neighbors[v] {
                let od = out_degree[u];
                if od > 0 {
                    incoming += ranks[u] / od as f64;
                }
            }

            let r = teleportation_contrib + dangling_contrib + alpha * incoming;
            let new_v = sanitize_rank(r);
            let delta = (new_v - ranks[v]).abs();
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

    /// When personalization is None, Personalized PageRank should match standard PageRank.
    #[test]
    fn no_personalization_matches_standard() {
        // 3-node cycle A→B→C→A
        let in_neighbors = vec![vec![2], vec![0], vec![1]];
        let out_degree = vec![1, 1, 1];

        let ppr = personalized_pagerank(&in_neighbors, &out_degree, 3, 0.85, 100, None);
        let pr = crate::page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);

        // Mass conservation
        let ppr_sum: f64 = ppr.values().sum();
        let pr_sum: f64 = pr.values().sum();
        assert!((ppr_sum - pr_sum).abs() < 1e-3);
        // All nodes should have non-zero scores
        assert!(ppr[&0] > 0.0);
        assert!(ppr[&1] > 0.0);
        assert!(ppr[&2] > 0.0);
    }

    /// Personalization biases scores toward the personalized nodes.
    #[test]
    #[allow(clippy::needless_range_loop)] // leaf is used as vec index
    fn personalization_biases_toward_target_nodes() {
        // Star: center=0, leaves=1..5. All leaves point to center.
        let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); 6];
        let mut out_degree = vec![0usize; 6];
        for leaf in 1..6 {
            in_neighbors[0].push(leaf); // center is called by leaf
            out_degree[leaf] = 1; // leaf calls center
        }

        // Personalize toward leaf 1 only
        let mut personalization = vec![0.0f64; 6];
        personalization[1] = 1.0;

        let ppr = personalized_pagerank(
            &in_neighbors,
            &out_degree,
            6,
            0.85,
            100,
            Some(&personalization),
        );

        // Node 1 (personalized) should have higher score than other leaves
        for leaf in 2..6 {
            assert!(
                ppr[&1] > ppr[&leaf],
                "personalized node 1 should outrank non-personalized leaf {}",
                leaf
            );
        }
    }

    /// Empty graph: empty map.
    #[test]
    fn empty_graph_returns_empty_map() {
        let result = personalized_pagerank(&[], &[], 0, 0.85, 100, None);
        assert!(result.is_empty());
    }

    /// Single node: score = 1.0 (all mass on the only node).
    #[test]
    fn single_node_returns_one() {
        let in_neighbors = vec![Vec::new()];
        let out_degree = vec![0];
        let result = personalized_pagerank(&in_neighbors, &out_degree, 1, 0.85, 100, None);
        assert_eq!(result.len(), 1);
        assert!((result[&0] - 1.0).abs() < 1e-6);
    }

    /// Mass conservation with personalization.
    #[test]
    fn mass_conservation_with_personalization() {
        // 3-node cycle
        let in_neighbors = vec![vec![2], vec![0], vec![1]];
        let out_degree = vec![1, 1, 1];
        let personalization = vec![0.5, 0.3, 0.2];

        let result = personalized_pagerank(
            &in_neighbors,
            &out_degree,
            3,
            0.85,
            100,
            Some(&personalization),
        );
        let sum: f64 = result.values().sum();
        assert!((sum - 1.0).abs() < 1e-3, "mass conservation: sum={}", sum);
    }

    /// Runs the function on a worker thread so a loop that never converges
    /// fails the test after 5 s instead of hanging the suite.
    fn run(
        in_neighbors: Vec<Vec<usize>>,
        out_degree: Vec<usize>,
        iterations: usize,
        personalization: Option<Vec<f64>>,
    ) -> Vec<f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let n = out_degree.len();
            let scores = personalized_pagerank(
                &in_neighbors,
                &out_degree,
                n,
                0.85,
                iterations,
                personalization.as_deref(),
            );
            let _ = tx.send((0..n).map(|v| scores[&v]).collect::<Vec<f64>>());
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("personalized_pagerank did not return within 5 s")
    }

    fn assert_close(actual: &[f64], expected: &[f64], eps: f64) {
        assert_eq!(actual.len(), expected.len());
        for (v, (a, e)) in actual.iter().zip(expected).enumerate() {
            assert!((a - e).abs() < eps, "node {v}: got {a}, expected {e}");
        }
    }

    /// `0 → 1` with node 2 isolated: nodes 1 and 2 are dangling (out-degree 0).
    fn dangling_graph() -> (Vec<Vec<usize>>, Vec<usize>) {
        (vec![vec![], vec![0], vec![]], vec![1, 0, 0])
    }

    /// One iteration from the uniform start, by hand (alpha = 0.85, n = 3):
    /// dangling mass = 2/3, teleport = 0.15/3 = 0.05, dangling share =
    /// 0.85 * (2/3) / 3, and node 1 also receives 0.85 * (1/3) from node 0.
    #[test]
    fn one_iteration_matches_hand_computed_values() {
        let (inn, out) = dangling_graph();
        let base = 0.05 + 0.85 * (2.0 / 3.0) / 3.0;
        let got = run(inn, out, 1, None);
        assert_close(&got, &[base, base + 0.85 / 3.0, base], 1e-12);
    }

    /// Second iteration by hand. It pins the convergence bookkeeping: a loop
    /// that stops after the first pass returns the first-iteration values.
    #[test]
    fn two_iterations_match_hand_computed_values() {
        let (inn, out) = dangling_graph();
        let first_base = 0.05 + 0.85 * (2.0 / 3.0) / 3.0;
        let first = [first_base, first_base + 0.85 / 3.0, first_base];
        let base = 0.05 + 0.85 * (first[1] + first[2]) / 3.0;
        let got = run(inn, out, 2, None);
        assert_close(&got, &[base, base + 0.85 * first[0], base], 1e-12);
        assert!(
            (got[0] - first[0]).abs() > 1e-3,
            "must not stop after one pass"
        );
    }

    /// An unnormalized vector `[2, 0, 6]` is scaled to `[0.25, 0, 0.75]`.
    #[test]
    fn personalization_is_normalized_by_its_sum() {
        let (inn, out) = dangling_graph();
        let got = run(inn, out, 1, Some(vec![2.0, 0.0, 6.0]));
        let expected = [
            0.15 * 0.25 + 0.85 * (2.0 / 3.0) * 0.25,
            0.85 / 3.0,
            0.15 * 0.75 + 0.85 * (2.0 / 3.0) * 0.75,
        ];
        assert_close(&got, &expected, 1e-12);
    }

    /// A vector summing to 0 (or to a non-finite value) is invalid and the
    /// teleportation falls back to uniform, never to NaN.
    #[test]
    fn zero_or_non_finite_personalization_sum_falls_back_to_uniform() {
        let (inn, out) = dangling_graph();
        let uniform = run(inn.clone(), out.clone(), 1, None);
        let base = 0.05 + 0.85 * (2.0 / 3.0) / 3.0;
        assert_close(&uniform, &[base, base + 0.85 / 3.0, base], 1e-12);
        let zeros = run(inn.clone(), out.clone(), 1, Some(vec![0.0; 3]));
        assert_close(&zeros, &uniform, 1e-12);
        let infinite = run(inn, out, 1, Some(vec![f64::INFINITY, 1.0, 1.0]));
        assert_close(&infinite, &uniform, 1e-12);
    }

    /// Two-node cycle teleporting only to node 0. Stationary point:
    /// x = 0.15 + 0.85 y and y = 0.85 x, so x = 0.15 / (1 - 0.85^2).
    #[test]
    fn converges_to_the_stationary_distribution() {
        let x = 0.15 / (1.0 - 0.85 * 0.85);
        let got = run(
            vec![vec![1], vec![0]],
            vec![1, 1],
            100,
            Some(vec![1.0, 0.0]),
        );
        assert_close(&got, &[x, 0.85 * x], 1e-5);
    }

    /// Mass is conserved even when most nodes are dangling.
    #[test]
    fn dangling_nodes_do_not_leak_mass() {
        let (inn, out) = dangling_graph();
        let got = run(inn, out, 100, Some(vec![0.2, 0.3, 0.5]));
        assert!((got.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    /// With a huge iteration budget the tolerance must stop the loop: a
    /// uniform cycle is already stationary, so the first pass has delta 0.
    #[test]
    fn tolerance_stops_the_loop_long_before_the_iteration_budget() {
        let got = run(
            vec![vec![2], vec![0], vec![1]],
            vec![1, 1, 1],
            usize::MAX,
            None,
        );
        assert_close(&got, &[1.0 / 3.0; 3], 1e-12);
    }

    /// `0 → 1` and `0 → 2`: node 0 splits its rank over two out-edges.
    #[test]
    fn rank_is_split_over_the_out_degree() {
        let got = run(vec![vec![], vec![0], vec![0]], vec![2, 0, 0], 1, None);
        let base = 0.05 + 0.85 * (2.0 / 3.0) / 3.0;
        let shared = base + 0.85 * (1.0 / 3.0) / 2.0;
        assert_close(&got, &[base, shared, shared], 1e-12);
    }

    /// An in-edge whose source reports out-degree 0 is inconsistent input;
    /// it contributes nothing instead of dividing by zero.
    #[test]
    fn an_in_edge_from_a_zero_out_degree_source_is_ignored() {
        let got = run(vec![vec![1], vec![]], vec![0, 0], 1, None);
        assert_close(&got, &[0.5, 0.5], 1e-12);
    }

    #[test]
    fn sanitize_rank_keeps_only_positive_finite_values() {
        assert_eq!(sanitize_rank(0.25), 0.25);
        assert_eq!(sanitize_rank(f64::INFINITY), 0.0);
        assert_eq!(sanitize_rank(f64::NAN), 0.0);
        assert_eq!(sanitize_rank(-0.5), 0.0);
        assert_eq!(sanitize_rank(0.0), 0.0);
        // Negative zero is normalized to +0.0 rather than passed through.
        assert!(sanitize_rank(-0.0).is_sign_positive());
    }

    #[test]
    fn convergence_threshold_is_strict() {
        assert!(has_converged(TOLERANCE / 2.0));
        assert!(!has_converged(TOLERANCE));
        assert!(!has_converged(TOLERANCE * 2.0));
    }

    /// Invalid personalization (wrong length) falls back to uniform.
    #[test]
    fn invalid_personalization_length_falls_back_to_uniform() {
        let in_neighbors = vec![vec![2], vec![0], vec![1]];
        let out_degree = vec![1, 1, 1];

        let ppr =
            personalized_pagerank(&in_neighbors, &out_degree, 3, 0.85, 100, Some(&[0.5, 0.5]));
        let pr = crate::page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);

        // Should behave like standard PageRank (uniform teleportation)
        let ppr_sum: f64 = ppr.values().sum();
        let pr_sum: f64 = pr.values().sum();
        assert!((ppr_sum - pr_sum).abs() < 1e-3);
    }
}
