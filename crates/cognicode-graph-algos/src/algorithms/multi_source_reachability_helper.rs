//! Multi-Source Reachability helper — BFS union from multiple sources.
//!
//! NOT admitted to the analytics registry per ADR-015 evidence gate decisions.
//! Rejected because: redundant with `find_forward_reach` (already exposed on
//! `CallGraphProjection`) which achieves the same result via the same BFS
//! approach. Adding a second entry point for identical functionality would
//! confuse callers and add maintenance burden without expanding the capability
//! surface.
//!
//! Use `CallGraphProjection::find_forward_reach` for production work.

use std::collections::{HashSet, VecDeque};

/// Compute the union of forward reachability from multiple source nodes.
///
/// Each source explores its outgoing reachable set independently (via BFS),
/// and the union of all visited nodes is returned. The sources themselves
/// are NOT included in the result.
///
/// # Arguments
///
/// - `out_neighbors`: outgoing adjacency list
/// - `sources`: list of source node indices
/// - `max_depth`: maximum BFS depth (number of hops)
///
/// # Returns
///
/// `Vec<usize>` — union of all nodes reachable from any source, in arbitrary order.
/// Excludes the source nodes themselves.
///
/// # Edge cases
///
/// - Empty sources list: returns empty vec
/// - A source that is not in the graph: skipped
/// - `max_depth == 0`: returns empty vec
pub fn multi_source_reachability(
    out_neighbors: &[Vec<usize>],
    sources: &[usize],
    max_depth: usize,
) -> Vec<usize> {
    if sources.is_empty() || max_depth == 0 {
        return Vec::new();
    }

    let n = out_neighbors.len();
    if n == 0 {
        return Vec::new();
    }

    let mut visited: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();

    // Seed queue with all sources at depth 0 (they won't be added to result)
    for &src in sources {
        if src < n {
            queue.push_back((src, 0));
        }
    }

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for &neighbor in &out_neighbors[node] {
            if neighbor < n && visited.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    visited.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_out_neighbors(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(u, v) in edges {
            if u < n && v < n {
                out[u].push(v);
            }
        }
        out
    }

    /// Two sources reaching the same node: union is deduplicated.
    #[test]
    fn multi_source_reachability_deduplicates() {
        // A→C, B→C. Sources: {A, B}. Both reach C.
        let out_neighbors = build_out_neighbors(3, &[(0, 2), (1, 2)]);
        let result = multi_source_reachability(&out_neighbors, &[0, 1], 10);
        // C (node 2) is reachable from both, but appears once
        assert!(result.contains(&2));
        // No other nodes reachable
        assert_eq!(result.len(), 1);
    }

    /// Sources at max_depth=0: empty result.
    #[test]
    fn multi_source_reachability_zero_depth() {
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2)]);
        let result = multi_source_reachability(&out_neighbors, &[0], 0);
        assert!(result.is_empty());
    }

    /// Empty sources: empty result.
    #[test]
    fn multi_source_reachability_empty_sources() {
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2)]);
        let result = multi_source_reachability(&out_neighbors, &[], 10);
        assert!(result.is_empty());
    }

    /// Disconnected: source A has no outgoing edges → only C reachable from B.
    #[test]
    fn multi_source_reachability_disconnected() {
        // A=0 isolated, B→C
        let out_neighbors = build_out_neighbors(3, &[(1, 2)]);
        let result = multi_source_reachability(&out_neighbors, &[0, 1], 10);
        assert!(result.contains(&2));
        assert_eq!(result.len(), 1);
    }

    /// Sources themselves are NOT included in the result.
    #[test]
    fn multi_source_reachability_excludes_sources() {
        // A→B, B→C. Source A reaches B and C.
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2)]);
        let result = multi_source_reachability(&out_neighbors, &[0], 10);
        assert!(!result.contains(&0)); // source not included
        assert!(result.contains(&1)); // B reachable
        assert!(result.contains(&2)); // C reachable
    }

    /// Determinism: same input → same output (sorted comparison).
    #[test]
    fn multi_source_reachability_deterministic() {
        let out_neighbors = build_out_neighbors(4, &[(0, 2), (1, 2), (2, 3)]);
        let r1 = multi_source_reachability(&out_neighbors, &[0, 1], 10);
        let r2 = multi_source_reachability(&out_neighbors, &[0, 1], 10);
        let mut s1 = r1.clone();
        let mut s2 = r2.clone();
        s1.sort();
        s2.sort();
        assert_eq!(s1, s2);
    }
}
