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

    let mut out_n: Vec<Vec<usize>> = out_neighbors.to_vec();
    let mut result = Vec::new();

    // Every productive pass removes one edge, so the graph is acyclic after at
    // most `edge_count` removals. The bound makes termination structural.
    let edge_count: usize = out_neighbors.iter().map(Vec::len).sum();
    for _ in 0..edge_count {
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
                    // Only `out_n` drives the search, so only it is updated.
                    out_n[u].retain(|&x| x != v);
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

    /// Runs on a thread so a hang fails the test instead of the suite.
    fn run(in_n: Vec<Vec<usize>>, out_n: Vec<Vec<usize>>, n: usize) -> Vec<(usize, usize)> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(feedback_arc_set(&in_n, &out_n, n));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("feedback_arc_set did not finish within 5s")
    }

    #[test]
    fn tie_on_out_degree_picks_the_lowest_node() {
        // 0 → 1 → 0: both nodes have out-degree 1; the first one wins.
        let g = vec![vec![1], vec![0]];
        assert_eq!(run(g.clone(), g, 2), vec![(0, 1)]);
    }

    #[test]
    fn the_highest_out_degree_node_loses_its_first_edge() {
        // 0→1, 1→2, 2→0 and 1→0. Node 1 has out-degree 2 (edges 2, 0),
        // so its first edge (1, 2) goes first; 0→1, 2→0, 1→0 remain
        // and 0↔1 is still a cycle: node 0 (degree 1) → (0, 1).
        let out_n = vec![vec![1], vec![2, 0], vec![0]];
        assert_eq!(run(vec![Vec::new(); 3], out_n, 3), vec![(1, 2), (0, 1)]);
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
        // Known heuristic defect: after (0, 1) the pair 0↔1 is already
        // acyclic, but (1, 0) is still removed because only the global
        // out-degree is looked at. The result is not minimum (2 would do).
        assert_eq!(result, vec![(0, 1), (1, 0), (2, 3)]);
    }
}
