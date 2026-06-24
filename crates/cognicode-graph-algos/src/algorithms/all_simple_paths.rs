//! All simple paths between two nodes, bounded by max_hops.
//!
//! Path: sequence of nodes with no repetition. Cycles are broken by
//! the visited set. max_hops is the maximum number of intermediate
//! nodes (path traverses at most max_hops + 1 edges).
//!
//! WARNING: exponential worst case. Caller must bound max_hops.

use std::collections::HashSet;

/// Find all simple paths from `from` to `to` bounded by `max_hops`.
///
/// # Returns
///
/// `Vec<Vec<usize>>` — list of paths, each path is a sequence of
/// node indices from `from` to `to`. Empty if either node is unknown
/// or no path exists within max_hops.
///
/// # Edge cases
///
/// - `from == to`: no paths emitted (matches petgraph behavior)
/// - `max_hops == 0`: only direct paths (from → to)
/// - Missing endpoints: empty
pub fn all_simple_paths(
    out_neighbors: &[Vec<usize>],
    from: usize,
    to: usize,
    max_hops: usize,
) -> Vec<Vec<usize>> {
    // Empty graph or self-path: no paths (matches petgraph behavior).
    if out_neighbors.is_empty() || from == to {
        return Vec::new();
    }
    if from >= out_neighbors.len() || to >= out_neighbors.len() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut path = vec![from];
    let mut visited: HashSet<usize> = HashSet::new();
    visited.insert(from);

    dfs(
        out_neighbors,
        from,
        to,
        max_hops,
        &mut path,
        &mut visited,
        &mut result,
    );

    result
}

#[allow(clippy::vec_init_then_push)]
fn dfs(
    out_neighbors: &[Vec<usize>],
    current: usize,
    target: usize,
    remaining_hops: usize,
    path: &mut Vec<usize>,
    visited: &mut HashSet<usize>,
    result: &mut Vec<Vec<usize>>,
) {
    if current == target {
        result.push(path.clone());
        return;
    }
    if remaining_hops == 0 {
        return;
    }

    let neighbors = out_neighbors
        .get(current)
        .map(|n| n.as_slice())
        .unwrap_or(&[]);
    for &next in neighbors {
        if visited.insert(next) {
            path.push(next);
            dfs(
                out_neighbors,
                next,
                target,
                remaining_hops - 1,
                path,
                visited,
                result,
            );
            path.pop();
            visited.remove(&next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let result = all_simple_paths(&[], 0, 1, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn direct_path() {
        // A → B: one path.
        let out_n = vec![vec![1], Vec::new()];
        let result = all_simple_paths(&out_n, 0, 1, 5);
        assert_eq!(result, vec![vec![0, 1]]);
    }

    #[test]
    fn two_paths() {
        // A → B, A → C, B → D, C → D: two paths from A to D.
        let out_n = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        let result = all_simple_paths(&out_n, 0, 3, 5);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&vec![0, 1, 3]));
        assert!(result.contains(&vec![0, 2, 3]));
    }

    #[test]
    fn cycle_terminates() {
        // A → B → C → A: no path from A to A (visited prevents).
        let out_n = vec![vec![1], vec![2], vec![0]];
        let result = all_simple_paths(&out_n, 0, 0, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn max_hops_bounds_search() {
        // Diamond DAG: A → B → D, A → C → D. With max_hops=1, only direct
        // paths from A to D are searched; since there's no direct edge,
        // result is empty.
        let out_n = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        let result = all_simple_paths(&out_n, 0, 3, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn missing_endpoint() {
        let out_n = vec![vec![1], Vec::new()];
        let result = all_simple_paths(&out_n, 0, 99, 5);
        assert!(result.is_empty());
    }
}
