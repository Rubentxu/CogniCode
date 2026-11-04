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

    /// The threshold index is `len * percentile` into the ASCENDING-sorted
    /// community scores. A mid percentile must land in the MIDDLE of the
    /// community, not at the top: with 10 nodes and p = 0.5 the index is 5,
    /// so the threshold is the 6th-lowest score and the top 5 nodes qualify.
    ///
    /// A `+`/`/` instead of `*` computes a different index (10 for `+`,
    /// 20 for `/`), both clamped to the last element, which would collapse
    /// the result to the single highest-scoring node. The assertion pins the
    /// exact members, so it fails on both mutations.
    #[test]
    fn mid_percentile_selects_the_threshold_at_that_index_of_the_community() {
        // Node i has score i+1, so the ascending order is the node order.
        let community: Vec<usize> = (0..10).collect();
        let scores = score_map(&[
            (0, 1.0),
            (1, 2.0),
            (2, 3.0),
            (3, 4.0),
            (4, 5.0),
            (5, 6.0),
            (6, 7.0),
            (7, 8.0),
            (8, 9.0),
            (9, 10.0),
        ]);

        let result = community_god_nodes(&[community], &scores, 0.5);

        // len * 0.5 = 5 -> threshold = community_scores[5].1 = 6.0.
        // Every node whose score is >= 6.0 qualifies: nodes 5..=9.
        assert_eq!(
            result,
            vec![(9, 10.0), (8, 9.0), (7, 8.0), (6, 7.0), (5, 6.0)]
        );
    }

    /// The threshold is computed PER COMMUNITY, with each community's own
    /// length. Two communities of different sizes at the same percentile must
    /// select different slices: 10 nodes at p = 0.5 keep the top 5, while
    /// 4 nodes at p = 0.5 keep the top 2.
    #[test]
    fn threshold_is_computed_per_community_with_its_own_length() {
        let big: Vec<usize> = (0..10).collect();
        let small: Vec<usize> = (10..14).collect();
        let mut pairs: Vec<(usize, f64)> = (0..10).map(|i| (i, (i + 1) as f64)).collect();
        pairs.extend((10..14).map(|i| (i, (i + 1) as f64)));
        let scores = score_map(&pairs);

        let result = community_god_nodes(&[big, small], &scores, 0.5);

        // Big community: index 5 -> threshold 6.0 -> nodes 5..=9.
        // Small community: index 2 -> threshold 13.0 -> nodes 12..=13.
        // The global result is sorted by score descending.
        assert_eq!(
            result,
            vec![
                (13, 14.0),
                (12, 13.0),
                (9, 10.0),
                (8, 9.0),
                (7, 8.0),
                (6, 7.0),
                (5, 6.0),
            ]
        );
    }

    /// p = 0.0 puts the threshold index at 0, i.e. the LOWEST score of the
    /// community, so every node qualifies. `len + 0` and `len / 0` would both
    /// clamp to the last element and return only the highest-scoring node.
    #[test]
    fn zero_percentile_keeps_the_whole_community() {
        let community: Vec<usize> = (0..4).collect();
        let scores = score_map(&[(0, 4.0), (1, 3.0), (2, 2.0), (3, 1.0)]);

        let result = community_god_nodes(&[community], &scores, 0.0);

        // Index 0 -> threshold = 1.0 (the lowest score) -> all four nodes.
        assert_eq!(result, vec![(0, 4.0), (1, 3.0), (2, 2.0), (3, 1.0)]);
    }
}
