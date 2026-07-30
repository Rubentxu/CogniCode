//! K-Shortest Paths helper — wraps `all_simple_paths` and sorts by path length.
//!
//! NOT admitted to the analytics registry per ADR-015 evidence gate decisions.
//! Rejected because: unbounded result set requires a `k` limit, and the
//! result ordering semantics are ambiguous (shortest by hops? by edge count?
//! by weight?). A bounded single-source-single-target version would be
//! cleaner, but this module provides the composition helper for cases where
//! callers want top-k paths by length.
//!
//! Use `bounded_shortest_paths` from the registry for production work.

use crate::all_simple_paths;

/// Find the k shortest paths by hop count between two nodes.
///
/// # Arguments
///
/// - `out_neighbors`: outgoing adjacency list
/// - `from`: source node index
/// - `to`: target node index
/// - `max_hops`: maximum hop depth for any individual path
/// - `k`: number of paths to return
///
/// # Returns
///
/// `Vec<Vec<usize>>` — up to k paths, sorted by path length (shortest first).
///
/// # Edge cases
///
/// - Fewer than k paths exist: returns all found paths (sorted)
/// - `from == to`: returns empty (no self-paths)
/// - No path exists within max_hops: returns empty vec
pub fn k_shortest_paths(
    out_neighbors: &[Vec<usize>],
    from: usize,
    to: usize,
    max_hops: usize,
    k: usize,
) -> Vec<Vec<usize>> {
    if from == to {
        return Vec::new();
    }

    let all_paths = all_simple_paths(out_neighbors, from, to, max_hops);

    // Sort by path length (number of hops)
    let mut sorted: Vec<Vec<usize>> = all_paths;
    sorted.sort_by_key(|path| path.len());

    sorted.into_iter().take(k).collect()
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

    /// Diamond: A→B, A→C, B→D, C→D. Two paths from A to D.
    #[test]
    fn k_shortest_paths_diamond_two_paths() {
        // A=0, B=1, C=2, D=3
        // Paths: [0,1,3] and [0,2,3] — each has 3 nodes = 2 hops
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        let paths = k_shortest_paths(&out_neighbors, 0, 3, 5, 10);
        assert_eq!(paths.len(), 2);
        // Both should be length 3 (3 nodes = 2 hops: A→B→D, A→C→D)
        assert!(paths.iter().all(|p| p.len() == 3));
        // First should be shorter or equal to second
        if paths.len() >= 2 {
            assert!(paths[0].len() <= paths[1].len());
        }
    }

    /// Self-path: returns empty.
    #[test]
    fn k_shortest_paths_self_path_empty() {
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2)]);
        let paths = k_shortest_paths(&out_neighbors, 0, 0, 5, 3);
        assert!(paths.is_empty());
    }

    /// No path: returns empty.
    #[test]
    fn k_shortest_paths_no_path() {
        let out_neighbors = build_out_neighbors(3, &[(0, 1)]);
        let paths = k_shortest_paths(&out_neighbors, 0, 2, 5, 3);
        assert!(paths.is_empty());
    }

    /// k limit is respected.
    #[test]
    fn k_shortest_paths_respects_k() {
        // A→B, A→C, B→D, C→D (diamond) → 2 paths
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        let paths = k_shortest_paths(&out_neighbors, 0, 3, 5, 1);
        assert_eq!(paths.len(), 1);
    }

    /// Determinism: same input → same output.
    #[test]
    fn k_shortest_paths_deterministic() {
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        let r1 = k_shortest_paths(&out_neighbors, 0, 3, 5, 10);
        let r2 = k_shortest_paths(&out_neighbors, 0, 3, 5, 10);
        assert_eq!(r1, r2);
    }
}
