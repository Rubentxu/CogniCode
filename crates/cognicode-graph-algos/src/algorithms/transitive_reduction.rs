//! Transitive reduction — minimal edge set preserving reachability.
//!
//! For DAGs: O(V + E). For cyclic graphs: returns all edges (cycles
//! mean no edge is implied by a strictly longer simple path, so
//! "reduction" is the identity).

use std::collections::{HashSet, VecDeque};

/// Compute transitive reduction of a directed graph.
///
/// # Returns
///
/// `Vec<(usize, usize)>` — list of `(source, target)` edges that
/// survive the reduction. Empty if `n == 0`.
///
/// # Edge cases
///
/// - Empty graph: empty vec
/// - Cyclic graph: returns all edges (identity) — TR is undefined for cycles
/// - DAG: returns minimal edge set
pub fn transitive_reduction(
    _in_neighbors: &[Vec<usize>],
    out_neighbors: &[Vec<usize>],
    n: usize,
) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }

    // If the graph has a cycle, the reduction is the identity: return all edges.
    if has_cycle(out_neighbors, n) {
        // Cycle detected — return all edges as identity reduction.
        let mut result = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for u in 0..n {
            let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
            for &v in neighbors {
                if seen.insert((u, v)) {
                    result.push((u, v));
                }
            }
        }
        return result;
    }

    // For each edge (u, v), check if there's an alternative path u → v
    // that doesn't use edge (u, v). If yes, remove (u, v).
    let mut removable: HashSet<(usize, usize)> = HashSet::new();

    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if u != v && reachable_without_edge(u, v, out_neighbors, &(u, v)) {
                removable.insert((u, v));
            }
        }
    }

    // Build result from out_neighbors (forward edges).
    let mut result = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if !removable.contains(&(u, v)) && u != v && seen.insert((u, v)) {
                result.push((u, v));
            }
        }
    }

    // Sort by source then target.
    result.sort_by_key(|&(s, t)| (s, t));
    result
}

/// Kahn's algorithm used only as a cycle detector: the graph is acyclic
/// exactly when every node leaves the queue. Each node is queued at most once
/// (when its in-degree reaches zero), so the loop is bounded by `n`.
fn has_cycle(out_neighbors: &[Vec<usize>], n: usize) -> bool {
    let mut in_degree: Vec<usize> = vec![0; n];
    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            in_degree[v] += 1;
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|&v| in_degree[v] == 0).collect();
    let mut processed = 0;
    while let Some(u) = queue.pop() {
        processed += 1;
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    processed != n
}

fn reachable_without_edge(
    from: usize,
    to: usize,
    out_neighbors: &[Vec<usize>],
    forbidden: &(usize, usize),
) -> bool {
    // BFS from `from`, avoiding the forbidden edge.
    if from == to {
        return true;
    }
    let n = out_neighbors.len();
    let mut visited = vec![false; n];
    let mut queue = VecDeque::new();
    queue.push_back(from);
    visited[from] = true;
    while let Some(u) = queue.pop_front() {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if (u, v) == *forbidden {
                continue;
            }
            if v == to {
                return true;
            }
            if !visited[v] {
                visited[v] = true;
                queue.push_back(v);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the reduction on a thread so a hanging mutant fails the test
    /// instead of hanging the suite.
    fn run(out_neighbors: Vec<Vec<usize>>, n: usize) -> Vec<(usize, usize)> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(transitive_reduction(&[], &out_neighbors, n));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("transitive_reduction did not finish within 5s")
    }

    #[test]
    fn empty_graph() {
        assert_eq!(run(Vec::new(), 0), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn single_edge_preserved() {
        assert_eq!(run(vec![vec![1], Vec::new()], 2), vec![(0, 1)]);
    }

    #[test]
    fn redundant_edge_removed() {
        // 0→1, 0→2, 1→2: (0,2) is implied by 0→1→2.
        let out = vec![vec![1, 2], vec![2], Vec::new()];
        assert_eq!(run(out, 3), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn diamond_keeps_every_edge() {
        // 0→1, 0→2, 1→3, 2→3: no edge is implied by another path.
        let out = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        assert_eq!(run(out, 4), vec![(0, 1), (0, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn chain_with_shortcuts_reduces_to_the_chain() {
        // 0→1→2→3 plus shortcuts 0→2, 0→3, 1→3.
        let out = vec![vec![1, 2, 3], vec![2, 3], vec![3], Vec::new()];
        assert_eq!(run(out, 4), vec![(0, 1), (1, 2), (2, 3)]);
    }

    #[test]
    fn dag_duplicate_edge_is_reported_once() {
        assert_eq!(run(vec![vec![1, 1], Vec::new()], 2), vec![(0, 1)]);
    }

    #[test]
    fn dag_with_a_missing_adjacency_row() {
        // `out_neighbors` is shorter than `n`: node 1 has no row.
        assert_eq!(run(vec![vec![1]], 2), vec![(0, 1)]);
    }

    #[test]
    fn disconnected_dag_components() {
        let out = vec![vec![1], Vec::new(), vec![3], Vec::new()];
        assert_eq!(run(out, 4), vec![(0, 1), (2, 3)]);
    }

    #[test]
    fn cyclic_graph_returns_all_edges_even_the_implied_one() {
        // 0→1, 0→2, 1→2, 2→0. Without cycle detection (0,2) would be dropped
        // because 0→1→2 exists; on a cycle the identity is returned.
        let out = vec![vec![1, 2], vec![2], vec![0]];
        assert_eq!(run(out, 3), vec![(0, 1), (0, 2), (1, 2), (2, 0)]);
    }

    #[test]
    fn cyclic_graph_deduplicates_edges_in_traversal_order() {
        assert_eq!(run(vec![vec![1, 1], vec![0]], 2), vec![(0, 1), (1, 0)]);
    }

    #[test]
    fn self_loop_counts_as_a_cycle() {
        assert_eq!(run(vec![vec![0]], 1), vec![(0, 0)]);
    }
}
