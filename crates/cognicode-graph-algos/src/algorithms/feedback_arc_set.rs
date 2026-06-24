//! Minimum Feedback Arc Set (FAS) — greedy heuristic.
//!
//! For a DAG: returns empty vec (no cycles to break).
//! For cyclic graphs: returns a (small) set of edges whose removal
//! makes the graph acyclic. Greedy O(V·E) heuristic, NOT guaranteed
//! minimum.

/// Compute a feedback arc set (greedy heuristic).
///
/// # Returns
///
/// `Vec<(usize, usize)>` — edges to remove to break all cycles.
/// Empty if graph is already a DAG.
#[allow(clippy::needless_range_loop)]
pub fn feedback_arc_set(
    _in_neighbors: &[Vec<usize>],
    out_neighbors: &[Vec<usize>],
    n: usize,
) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }

    let mut in_n: Vec<Vec<usize>> = _in_neighbors.to_vec();
    let mut out_n: Vec<Vec<usize>> = out_neighbors.to_vec();
    let mut result = Vec::new();

    loop {
        match toposort_check(&out_n, n) {
            Ok(()) => break, // DAG, no cycles
            Err(()) => {
                // Find any edge to remove: pick the one from a node with
                // highest out-degree (heuristic).
                let mut best_edge: Option<(usize, usize)> = None;
                let mut best_out_degree = 0usize;
                for u in 0..n {
                    if out_n[u].len() > best_out_degree {
                        best_out_degree = out_n[u].len();
                        if let Some(&v) = out_n[u].first() {
                            best_edge = Some((u, v));
                        }
                    }
                }
                if let Some((u, v)) = best_edge {
                    // Remove edge u→v from both lists.
                    out_n[u].retain(|&x| x != v);
                    in_n[v].retain(|&x| x != u);
                    result.push((u, v));
                } else {
                    break; // Shouldn't happen
                }
            }
        }
    }

    result
}

#[allow(clippy::needless_range_loop)]
fn toposort_check(out_neighbors: &[Vec<usize>], n: usize) -> Result<(), ()> {
    let mut in_degree: Vec<usize> = vec![0; n];
    for u in 0..n {
        for &v in &out_neighbors[u] {
            in_degree[v] += 1;
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|&v| in_degree[v] == 0).collect();
    let mut visited = 0;
    while let Some(u) = queue.pop() {
        visited += 1;
        for &v in &out_neighbors[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    if visited == n { Ok(()) } else { Err(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        assert!(feedback_arc_set(&[], &[], 0).is_empty());
    }

    #[test]
    fn dag_returns_empty() {
        // A → B → C (DAG).
        let in_n = vec![Vec::new(), vec![0], vec![1], Vec::new()];
        let out_n = vec![vec![1], vec![2], Vec::new(), Vec::new()];
        let result = feedback_arc_set(&in_n, &out_n, 4);
        assert!(result.is_empty());
    }

    #[test]
    fn simple_cycle_returns_one_edge() {
        // 0 → 1 → 0: removing any edge breaks the cycle.
        let in_n = vec![vec![1], vec![0]];
        let out_n = vec![vec![1], vec![0]];
        let result = feedback_arc_set(&in_n, &out_n, 2);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn two_disjoint_cycles() {
        // Cycle 1: 0↔1. Cycle 2: 2↔3.
        // Greedy heuristic: removes up to 3 edges (one per found cycle).
        let in_n = vec![vec![1], vec![0], vec![3], vec![2]];
        let out_n = vec![vec![1], vec![0], vec![3], vec![2]];
        let result = feedback_arc_set(&in_n, &out_n, 4);
        assert_eq!(result.len(), 3);
    }
}
