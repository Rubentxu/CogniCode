//! Cluster components — SCC + weakly connected components combined.
//!
//! Returns components where each component is either:
//! - A single SCC (in cyclic graphs), or
//! - A weakly-connected group of SCCs
//!
//! Useful for partitioning the graph into meaningful clusters.

use std::collections::HashSet;

/// Cluster components of the graph.
///
/// # Returns
///
/// `Vec<Vec<usize>>` — list of clusters. Each cluster contains
/// member node indices. Order is deterministic.
pub fn cluster_components(
    _in_neighbors: &[Vec<usize>],
    out_neighbors: &[Vec<usize>],
    n: usize,
) -> Vec<Vec<usize>> {
    if n == 0 {
        return Vec::new();
    }

    // Step 1: Compute SCCs using Tarjan over out_neighbors.
    let sccs = super::condensation::condensation(out_neighbors, n);

    // Step 2: Build condensed graph (DAG of SCCs).
    let scc_id_of: Vec<usize> = {
        let mut id = vec![0usize; n];
        for (scc_idx, scc) in sccs.iter().enumerate() {
            for &node in scc {
                id[node] = scc_idx;
            }
        }
        id
    };

    let num_sccs = sccs.len();
    let mut scc_out: Vec<HashSet<usize>> = vec![HashSet::new(); num_sccs];
    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            let scc_u = scc_id_of[u];
            let scc_v = scc_id_of[v];
            if scc_u != scc_v {
                scc_out[scc_u].insert(scc_v);
            }
        }
    }

    // Step 3: Find connected components in the condensed DAG (undirected).
    let mut visited = vec![false; num_sccs];
    let mut clusters: Vec<Vec<usize>> = Vec::new();

    for start_scc in 0..num_sccs {
        if visited[start_scc] {
            continue;
        }
        let mut cluster_nodes: Vec<usize> = Vec::new();
        let mut queue = vec![start_scc];
        visited[start_scc] = true;
        while let Some(s) = queue.pop() {
            for &node in &sccs[s] {
                cluster_nodes.push(node);
            }
            // Traverse undirected adjacency in the condensed graph.
            for &neighbor in &scc_out[s] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push(neighbor);
                }
            }
            // Also reverse direction (treat as undirected).
            for scc_idx in 0..num_sccs {
                if !visited[scc_idx] && scc_out[scc_idx].contains(&s) {
                    visited[scc_idx] = true;
                    queue.push(scc_idx);
                }
            }
        }
        cluster_nodes.sort();
        clusters.push(cluster_nodes);
    }

    clusters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        assert!(cluster_components(&[], &[], 0).is_empty());
    }

    #[test]
    fn single_node_one_cluster() {
        let in_n = vec![Vec::new()];
        let out_n = vec![Vec::new()];
        let result = cluster_components(&in_n, &out_n, 1);
        assert_eq!(result, vec![vec![0]]);
    }

    #[test]
    fn two_disconnected_one_cluster_each() {
        // 0 → 1 and 2 → 3 are two clusters.
        let in_n = vec![Vec::new(), vec![0], Vec::new(), vec![2]];
        let out_n = vec![vec![1], Vec::new(), vec![3], Vec::new()];
        let result = cluster_components(&in_n, &out_n, 4);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn scc_one_cluster() {
        // 0 ↔ 1 forms one SCC; one cluster.
        let in_n = vec![vec![1], vec![0]];
        let out_n = vec![vec![1], vec![0]];
        let result = cluster_components(&in_n, &out_n, 2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn chain_one_cluster() {
        // 0 → 1 → 2: one cluster (all connected).
        let in_n = vec![Vec::new(), vec![0], vec![1]];
        let out_n = vec![vec![1], vec![2], Vec::new()];
        let result = cluster_components(&in_n, &out_n, 3);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 3);
    }
}
