//! K-Core — iterative degree peeling algorithm.
//!
//! The k-core is the maximal subgraph where every vertex has degree >= k.
//! Core number = highest k for which a node survives the peeling process.

/// Run k-core decomposition on an undirected adjacency structure.
pub fn k_core(adj: &[Vec<usize>], n: usize, k: u32) -> Vec<(usize, u32)> {
    if n == 0 {
        return Vec::new();
    }

    // Compute core numbers via iterative peeling
    let core_numbers = compute_core_numbers(adj, n, k);

    // Filter by k
    let mut result: Vec<(usize, u32)> = core_numbers
        .into_iter()
        .enumerate()
        .filter(|(_, core)| *core >= k)
        .collect();
    result.sort_by_key(|r| r.0);
    result
}

/// Whether `v` is peeled at `threshold`: still in the graph and with fewer than
/// `threshold` neighbours left.
fn is_peelable(removed: bool, degree: usize, threshold: u32) -> bool {
    !removed && degree < threshold as usize
}

/// Compute core number for each node.
/// Core number = highest k at which the node survives the peeling process.
///
/// Known limitation: when the graph has cores of different levels, the nodes
/// left at the stopping threshold all receive that threshold, which can be
/// lower than their true core number. Membership of the `k`-core (`core >= k`)
/// is not affected; the reported level of the surviving nodes can be.
///
/// Both loops are bounded by construction: every productive pass peels at
/// least one node (so `n + 1` passes suffice), and the threshold never needs to
/// exceed the largest degree plus one, where every node is peeled.
fn compute_core_numbers(adj: &[Vec<usize>], n: usize, k: u32) -> Vec<u32> {
    if n == 0 {
        return Vec::new();
    }

    let mut degree: Vec<usize> = adj.iter().map(|nbrs| nbrs.len()).collect();
    let mut core_num: Vec<u32> = vec![0; n];
    let mut removed: Vec<bool> = vec![false; n];
    let max_threshold = degree.iter().copied().max().unwrap_or(0) as u32 + 1;

    // Peel at threshold 1, 2, 3, ...
    for threshold in 1..=max_threshold {
        // Peel all nodes with degree < threshold, until a pass changes nothing.
        for _ in 0..=n {
            let mut changed = false;
            for v in 0..n {
                if is_peelable(removed[v], degree[v], threshold) {
                    core_num[v] = threshold - 1;
                    removed[v] = true;
                    changed = true;

                    // Reduce degree of neighbors still in graph
                    for &neighbor in &adj[v] {
                        if !removed[neighbor] {
                            degree[neighbor] = degree[neighbor].saturating_sub(1);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Every remaining node has degree >= threshold now.
        let remaining: Vec<usize> = (0..n).filter(|&v| !removed[v]).collect();
        if remaining.is_empty() {
            break;
        }

        let min_deg = remaining.iter().map(|&v| degree[v]).min().unwrap();
        if min_deg == threshold as usize && threshold >= k {
            // The remaining graph is a core at this threshold.
            for &v in &remaining {
                core_num[v] = threshold;
            }
            break;
        }
    }

    core_num
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let result = k_core(&[], 0, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn single_node() {
        let adj = vec![Vec::<usize>::new()];
        let result = k_core(&adj, 1, 1);
        assert!(result.is_empty()); // node has degree 0 < 1
    }

    #[test]
    fn k_zero_exhaustive() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let result = k_core(&adj, 3, 0);
        assert_eq!(result.len(), 3);
        for (_, core) in &result {
            assert_eq!(*core, 2); // all have degree 2 = core number
        }
    }

    #[test]
    fn triangle_k2_all_nodes() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let result = k_core(&adj, 3, 2);
        assert_eq!(result.len(), 3, "all 3 nodes should survive k=2");
        for (_, core) in &result {
            assert_eq!(*core, 2);
        }
    }

    #[test]
    fn path_k2_empty() {
        let adj = vec![vec![1], vec![0, 2], vec![1]];
        let result = k_core(&adj, 3, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn path_k1_all_nodes() {
        let adj = vec![vec![1], vec![0, 2], vec![1]];
        let result = k_core(&adj, 3, 1);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn triangle_with_tail() {
        // A-B, B-C, C-A, C-D
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        adj[0].push(1);
        adj[1].push(0); // A-B
        adj[1].push(2);
        adj[2].push(1); // B-C
        adj[2].push(0);
        adj[0].push(2); // C-A
        adj[2].push(3);
        adj[3].push(2); // C-D

        let result = k_core(&adj, 4, 2);
        // Triangle (A,B,C) survives with core=2
        // D (degree 1) is peeled
        assert_eq!(result.len(), 3);
        let nodes: Vec<usize> = result.iter().map(|(n, _)| *n).collect();
        assert!(nodes.contains(&0));
        assert!(nodes.contains(&1));
        assert!(nodes.contains(&2));
        assert!(!nodes.contains(&3));
    }

    #[test]
    fn spec_example_k2() {
        // A-B, B-C, C-D, C-E, D-E
        // For k=2: A (degree 1) and B (degree 1 after A peeled) are peeled
        // Remaining C-D-E form a triangle, all have degree 2
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 5];
        adj[0].push(1);
        adj[1].push(0); // A-B
        adj[1].push(2);
        adj[2].push(1); // B-C
        adj[2].push(3);
        adj[3].push(2); // C-D
        adj[2].push(4);
        adj[4].push(2); // C-E
        adj[3].push(4);
        adj[4].push(3); // D-E

        let result = k_core(&adj, 5, 2);
        let nodes: Vec<usize> = result.iter().map(|(n, _)| *n).collect();
        // C, D, E survive with core=2; A and B are peeled
        assert!(nodes.contains(&2), "C should survive");
        assert!(nodes.contains(&3), "D should survive");
        assert!(nodes.contains(&4), "E should survive");
        assert!(!nodes.contains(&0), "A should be peeled");
        assert!(!nodes.contains(&1), "B should be peeled");
    }

    #[test]
    fn deterministic() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let r1 = k_core(&adj, 3, 2);
        let r2 = k_core(&adj, 3, 2);
        assert_eq!(r1, r2);
    }

    /// Runs `f` on a thread so a hanging mutant fails the test instead of
    /// hanging the suite.
    fn with_deadline<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("k_core did not finish within 5s (or panicked)")
    }

    fn run(adj: Vec<Vec<usize>>, k: u32) -> Vec<(usize, u32)> {
        let n = adj.len();
        with_deadline(move || k_core(&adj, n, k))
    }

    /// Builds an undirected adjacency list from an edge list.
    fn undirected(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut adj = vec![Vec::new(); n];
        for &(a, b) in edges {
            adj[a].push(b);
            adj[b].push(a);
        }
        adj
    }

    #[test]
    fn is_peelable_pins_its_boundary() {
        assert!(is_peelable(false, 0, 1));
        assert!(is_peelable(false, 1, 2));
        assert!(!is_peelable(false, 2, 2), "degree == threshold survives");
        assert!(!is_peelable(false, 3, 2));
        assert!(
            !is_peelable(true, 0, 5),
            "a removed node is never peeled again"
        );
    }

    /// K4: every node has degree 3, so the core number is 3 for all.
    #[test]
    fn clique_k4_k3_keeps_everyone_at_core_3() {
        let g = undirected(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
        assert_eq!(run(g, 3), vec![(0, 3), (1, 3), (2, 3), (3, 3)]);
    }

    #[test]
    fn clique_k4_k4_is_empty() {
        let g = undirected(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
        assert_eq!(run(g, 4), Vec::<(usize, u32)>::new());
    }

    /// Path 0-1-2-3: the 1-core is everything (core 1), the 2-core is empty
    /// (peeling the endpoints cascades through the whole path).
    #[test]
    fn path_has_core_1_and_no_2_core() {
        let g = undirected(4, &[(0, 1), (1, 2), (2, 3)]);
        assert_eq!(run(g.clone(), 1), vec![(0, 1), (1, 1), (2, 1), (3, 1)]);
        assert_eq!(run(g, 2), Vec::<(usize, u32)>::new());
    }

    /// Star with centre 0 and leaves 1..=3: everyone is in the 1-core, none in
    /// the 2-core (the leaves go, and the centre is left with degree 0).
    #[test]
    fn star_has_core_1_and_no_2_core() {
        let g = undirected(4, &[(0, 1), (0, 2), (0, 3)]);
        assert_eq!(run(g.clone(), 1), vec![(0, 1), (1, 1), (2, 1), (3, 1)]);
        assert_eq!(run(g, 2), Vec::<(usize, u32)>::new());
    }

    /// Triangle {0,1,2} plus the separate edge {3,4}: the 2-core is the triangle.
    #[test]
    fn disconnected_triangle_and_edge_k2_keeps_only_the_triangle() {
        let g = undirected(5, &[(0, 1), (1, 2), (2, 0), (3, 4)]);
        assert_eq!(run(g, 2), vec![(0, 2), (1, 2), (2, 2)]);
    }

    /// Isolated nodes have core number 0: kept for k = 0, dropped for k = 1.
    #[test]
    fn isolated_nodes_have_core_0() {
        let g: Vec<Vec<usize>> = vec![Vec::new(), Vec::new()];
        assert_eq!(run(g.clone(), 0), vec![(0, 0), (1, 0)]);
        assert_eq!(run(g, 1), Vec::<(usize, u32)>::new());
    }

    /// With `k` above the largest degree the public filter hides the levels, but
    /// the core numbers themselves must still be the true ones: K4 is a 3-core.
    #[test]
    fn core_numbers_are_reported_even_when_k_exceeds_every_degree() {
        let g = undirected(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
        assert_eq!(
            with_deadline(move || compute_core_numbers(&g, 4, 4)),
            vec![3, 3, 3, 3]
        );
    }
}
