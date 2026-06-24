//! Community god nodes — pure function over communities output.
//!
//! For each community, identify nodes whose score is in the top
//! percentile WITHIN that community (vs globally).
//!
//! Extracted from `cognicode-core::CommunityDetector::community_god_nodes`.

use std::collections::HashMap;

/// Find god nodes within each community.
///
/// # Arguments
///
/// - `communities`: `Vec<Vec<usize>>` — output of `communities()`.
/// - `scores`: `scores[node]` — PageRank score for each node.
/// - `percentile`: in `[0.0, 1.0]`, clamped.
///
/// # Returns
///
/// `Vec<(usize, f64)>` — list of `(node_id, score)` for god nodes,
/// sorted by score descending.
///
/// # Edge cases
///
/// - Empty communities: empty vec
/// - Community of 1 node: that node included if percentile ≤ 0.99
/// - `percentile < 0.0` or `> 1.0`: clamped
pub fn community_god_nodes(
    communities: &[Vec<usize>],
    scores: &HashMap<usize, f64>,
    percentile: f64,
) -> Vec<(usize, f64)> {
    let p = percentile.clamp(0.0, 1.0);
    let mut result: Vec<(usize, f64)> = Vec::new();

    for community in communities {
        if community.is_empty() {
            continue;
        }

        // Get scores for nodes in this community.
        let mut community_scores: Vec<(usize, f64)> = community
            .iter()
            .filter_map(|&node| scores.get(&node).map(|&s| (node, s)))
            .collect();

        if community_scores.is_empty() {
            continue;
        }

        // Compute threshold: top percentile.
        community_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let threshold_idx = ((community_scores.len() as f64) * p) as usize;
        let threshold_idx = threshold_idx.min(community_scores.len().saturating_sub(1));
        let threshold = community_scores[threshold_idx].1;

        // Collect nodes above threshold.
        for &(node, score) in &community_scores {
            if score >= threshold {
                result.push((node, score));
            }
        }
    }

    // Sort global result by score desc, ties by id asc.
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score_map(pairs: &[(usize, f64)]) -> HashMap<usize, f64> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn empty_communities_returns_empty() {
        let scores = score_map(&[(0, 1.0)]);
        assert!(community_god_nodes(&[], &scores, 0.95).is_empty());
    }

    #[test]
    fn single_community_top_node() {
        let communities = vec![vec![0, 1, 2]];
        let scores = score_map(&[(0, 1.0), (1, 0.5), (2, 0.1)]);
        let result = community_god_nodes(&communities, &scores, 0.95);
        assert_eq!(result, vec![(0, 1.0)]);
    }

    #[test]
    fn two_communities_each_has_god_node() {
        // Community A: [0, 1] with scores [1.0, 0.5]
        // Community B: [2, 3] with scores [0.8, 0.3]
        let communities = vec![vec![0, 1], vec![2, 3]];
        let scores = score_map(&[(0, 1.0), (1, 0.5), (2, 0.8), (3, 0.3)]);
        let result = community_god_nodes(&communities, &scores, 0.95);
        // Top 5% of [0,1] is node 0; top 5% of [2,3] is node 2.
        assert!(result.contains(&(0, 1.0)));
        assert!(result.contains(&(2, 0.8)));
    }

    #[test]
    fn percentile_clamped() {
        let communities = vec![vec![0]];
        let scores = score_map(&[(0, 1.0)]);
        let r1 = community_god_nodes(&communities, &scores, -0.5);
        let r2 = community_god_nodes(&communities, &scores, 0.0);
        assert_eq!(r1, r2);
    }
}
