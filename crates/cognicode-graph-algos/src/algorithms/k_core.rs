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

/// Compute core number for each node.
/// Core number = highest k at which the node survives the peeling process.
fn compute_core_numbers(adj: &[Vec<usize>], n: usize, k: u32) -> Vec<u32> {
    if n == 0 {
        return Vec::new();
    }

    let mut degree: Vec<usize> = adj.iter().map(|nbrs| nbrs.len()).collect();
    let mut core_num: Vec<u32> = vec![0; n];
    let mut removed: Vec<bool> = vec![false; n];

    // Peel at threshold 1, 2, 3, ...
    let mut threshold: u32 = 1;

    loop {
        // Peel all nodes with degree < threshold (in the current graph)
        let mut changed = true;

        // Keep peeling until no change in this threshold
        while changed {
            changed = false;
            for v in 0..n {
                if !removed[v] && degree[v] < threshold as usize {
                    // Peel v
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
        }

        // After no more peeling at this threshold, check remaining graph
        let remaining: Vec<usize> = (0..n).filter(|&v| !removed[v]).collect();

        if remaining.is_empty() {
            break;
        }

        let min_deg = remaining.iter().map(|&v| degree[v]).min().unwrap();

        if min_deg >= threshold as usize && min_deg == threshold as usize && threshold >= k {
            // Remaining graph is a k-core (where k = min_deg = threshold)
            for &v in &remaining {
                if core_num[v] == 0 {
                    core_num[v] = threshold;
                }
            }
            break;
        }

        // min_deg > threshold or threshold < k: increase threshold and continue
        threshold += 1;
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
}
