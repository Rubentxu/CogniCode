//! God nodes — symbols with PageRank above a percentile threshold.
//!
//! Pure function over PageRank output. No graph traversal, no petgraph.

use std::collections::HashMap;

/// Find god nodes — symbols with PageRank above a percentile threshold.
///
/// `percentile` is in `[0.0, 1.0]`. With `percentile = 0.95`, only the
/// top 5% scoring nodes are returned. Output is sorted by score descending.
///
/// # Returns
///
/// `Vec<(node_id, score)>` — sorted desc by score. Ties broken by
/// `node_id` ascending for determinism.
///
/// # Edge cases (spec REQ-013..014)
///
/// - Empty scores: empty vec
/// - `percentile < 0.0`: clamped to 0.0
/// - `percentile > 1.0`: clamped to 1.0
/// - Single node: returned regardless of percentile
pub fn god_nodes(
    scores: &HashMap<usize, f64>,
    percentile: f64,
) -> Vec<(usize, f64)> {
    if scores.is_empty() {
        return Vec::new();
    }

    let mut sorted_scores: Vec<f64> = scores.values().copied().collect();
    sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p = percentile.clamp(0.0, 1.0);
    let threshold_idx = ((sorted_scores.len() as f64) * p) as usize;
    let threshold_idx = threshold_idx.min(sorted_scores.len().saturating_sub(1));
    let threshold = sorted_scores[threshold_idx];

    let mut result: Vec<(usize, f64)> = scores
        .iter()
        .filter(|&(_, &s)| s >= threshold)
        .map(|(&id, &s)| (id, s))
        .collect();
    // Sort: score descending, ties broken by id ascending.
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> HashMap<usize, f64> {
        // 10 nodes with descending scores (0 is top god).
        let mut m = HashMap::new();
        for i in 0..10 {
            m.insert(i, 1.0 - (i as f64) * 0.05);
        }
        m
    }

    #[test]
    fn empty_scores_returns_empty() {
        let m: HashMap<usize, f64> = HashMap::new();
        assert!(god_nodes(&m, 0.95).is_empty());
    }

    #[test]
    fn percentile_0_returns_all() {
        let m = fixture();
        let result = god_nodes(&m, 0.0);
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn percentile_1_returns_top_only() {
        let m = fixture();
        let result = god_nodes(&m, 1.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0); // top score
    }

    #[test]
    fn percentile_95_returns_top_5() {
        let m = fixture();
        let result = god_nodes(&m, 0.95);
        // Top 5% of 10 nodes = 1 node (index 0)
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0);
    }

    #[test]
    fn sort_descending_by_score() {
        let m = fixture();
        let result = god_nodes(&m, 0.0);
        for w in result.windows(2) {
            assert!(w[0].1 >= w[1].1, "scores not descending: {:?}", w);
        }
    }

    #[test]
    fn tie_breaking_by_id_ascending() {
        // All scores equal — sort by id ascending.
        let mut m = HashMap::new();
        m.insert(5, 1.0);
        m.insert(2, 1.0);
        m.insert(8, 1.0);
        m.insert(1, 1.0);
        let result = god_nodes(&m, 0.0);
        let ids: Vec<usize> = result.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![1, 2, 5, 8]);
    }

    #[test]
    fn percentile_clamped() {
        let m = fixture();
        let r1 = god_nodes(&m, -0.5);
        let r2 = god_nodes(&m, 0.0);
        assert_eq!(r1, r2);
        let r3 = god_nodes(&m, 1.5);
        let r4 = god_nodes(&m, 1.0);
        assert_eq!(r3, r4);
    }
}