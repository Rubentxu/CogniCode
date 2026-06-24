//! Surprising connections — pure function over communities output.
//!
//! Edges that cross community boundaries AND are between nodes that
//! shouldn't normally interact (e.g., far apart in PageRank or
//! representing unusual dependencies).
//!
//! Extracted from `cognicode-core::CommunityDetector::surprising_connections`.

use std::collections::HashMap;

/// Find surprising cross-community connections.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[u]` lists every `v` with edge `u → v`.
/// - `community_of`: `community_of[node]` is the community containing node.
/// - `scores`: PageRank scores for scoring edges.
/// - `limit`: max number of surprising connections to return.
///
/// # Returns
///
/// `Vec<(usize, usize, f64)>` — list of `(source, target, score)` for
/// surprising edges. Score = relative importance (PageRank product).
/// Sorted by score descending.
///
/// # Edge cases
///
/// - Empty graph: empty vec
/// - No cross-community edges: empty vec
/// - limit == 0: empty vec
pub fn surprising_connections(
    out_neighbors: &[Vec<usize>],
    community_of: &[usize],
    scores: &HashMap<usize, f64>,
    limit: usize,
) -> Vec<(usize, usize, f64)> {
    if limit == 0 {
        return Vec::new();
    }

    let mut cross_community: Vec<(usize, usize, f64)> = Vec::new();

    for u in 0..out_neighbors.len() {
        let community_u = community_of[u];
        for &v in &out_neighbors[u] {
            if v >= community_of.len() {
                continue;
            }
            let community_v = community_of[v];
            if community_u != community_v {
                let score_u = scores.get(&u).copied().unwrap_or(0.0);
                let score_v = scores.get(&v).copied().unwrap_or(0.0);
                let edge_score = score_u * score_v;
                cross_community.push((u, v, edge_score));
            }
        }
    }

    // Sort by score desc, ties by source then target.
    cross_community.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    cross_community.truncate(limit);
    cross_community
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn score_map(pairs: &[(usize, f64)]) -> HashMap<usize, f64> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn empty_graph() {
        let result = surprising_connections(&[], &[], &HashMap::new(), 10);
        assert!(result.is_empty());
    }

    #[test]
    fn no_cross_community_edges() {
        // All in same community.
        let out_n = vec![vec![1], vec![0]];
        let comm = vec![0, 0];
        let result = surprising_connections(&out_n, &comm, &HashMap::new(), 10);
        assert!(result.is_empty());
    }

    #[test]
    fn one_cross_community_edge() {
        // Nodes 0,1 in community 0; nodes 2,3 in community 1.
        // Edge 0 → 2 crosses communities.
        let out_n = vec![vec![2], Vec::new(), Vec::new(), Vec::new()];
        let comm = vec![0, 0, 1, 1];
        let scores = score_map(&[(0, 1.0), (2, 0.8)]);
        let result = surprising_connections(&out_n, &comm, &scores, 10);
        assert_eq!(result, vec![(0, 2, 0.8)]);
    }

    #[test]
    fn limit_truncates() {
        let out_n = vec![vec![2], vec![3], Vec::new(), Vec::new()];
        let comm = vec![0, 0, 1, 1];
        let scores = score_map(&[(0, 1.0), (1, 0.5), (2, 0.8), (3, 0.3)]);
        let result = surprising_connections(&out_n, &comm, &scores, 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn sorted_by_score_desc() {
        // Two cross-community edges, different scores.
        let out_n = vec![vec![2], vec![3], Vec::new(), Vec::new()];
        let comm = vec![0, 0, 1, 1];
        let scores = score_map(&[(0, 1.0), (1, 0.5), (2, 0.8), (3, 0.3)]);
        let result = surprising_connections(&out_n, &comm, &scores, 10);
        // Edge 0→2 has score 1.0 * 0.8 = 0.8; edge 1→3 has 0.5 * 0.3 = 0.15.
        assert_eq!(result[0].0, 0);
        assert_eq!(result[0].1, 2);
        assert!((result[0].2 - 0.8).abs() < 1e-9);
    }
}
