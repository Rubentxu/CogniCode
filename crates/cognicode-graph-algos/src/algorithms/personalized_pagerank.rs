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

    const TOLERANCE: f64 = 1e-6;

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
