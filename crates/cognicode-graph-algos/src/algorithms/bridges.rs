//! Bridges — Tarjan's algorithm for finding bridges (cut edges).
//!
//! A bridge is an edge whose removal increases the number of connected
//! components. In an undirected graph.
//!
//! Output: `Vec<(usize, usize)>` — (node_a, node_b) edge pairs.
//! Results are sorted lexicographically for determinism.

/// Run bridges detection on an undirected adjacency structure.
///
/// # Arguments
///
/// - `adj`: undirected adjacency list. `adj[u]` lists all neighbors of `u`.
///   Length MUST equal `n`.
/// - `n`: number of nodes.
///
/// # Returns
///
/// `Vec<(usize, usize)>` — for each bridge, the (u, v) edge pair where u < v.
/// Results are sorted lexicographically for determinism.
pub fn bridges(adj: &[Vec<usize>], n: usize) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }

    let mut disc: Vec<i64> = vec![-1; n];
    let mut low: Vec<i64> = vec![0; n];
    let mut timer: i64 = 0;
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut bridge_edges: Vec<(usize, usize)> = Vec::new();

    fn dfs(
        u: usize,
        disc: &mut Vec<i64>,
        low: &mut Vec<i64>,
        timer: &mut i64,
        parent: &mut Vec<Option<usize>>,
        bridge_edges: &mut Vec<(usize, usize)>,
        adj: &[Vec<usize>],
    ) {
        disc[u] = *timer;
        low[u] = *timer;
        *timer += 1;

        for &v in &adj[u] {
            if disc[v] == -1 {
                // v is unvisited — this is a tree edge
                parent[v] = Some(u);
                dfs(v, disc, low, timer, parent, bridge_edges, adj);

                // After DFS: low[u] = min(low[u], low[v])
                low[u] = low[u].min(low[v]);

                // Bridge condition: if low[v] > disc[u], edge (u,v) is a bridge
                if low[v] > disc[u] {
                    let (a, b) = if u < v { (u, v) } else { (v, u) };
                    bridge_edges.push((a, b));
                }
            } else if parent[u] != Some(v) {
                // Back edge: low[u] = min(low[u], disc[v])
                low[u] = low[u].min(disc[v]);
            }
        }
    }

    // Handle disconnected components
    for i in 0..n {
        if disc[i] == -1 {
            dfs(
                i,
                &mut disc,
                &mut low,
                &mut timer,
                &mut parent,
                &mut bridge_edges,
                adj,
            );
        }
    }

    bridge_edges.sort_by_key(|e| e.0);
    bridge_edges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty graph: empty result.
    #[test]
    fn empty_graph() {
        let result = bridges(&[], 0);
        assert!(result.is_empty());
    }

    /// Single node: no bridges.
    #[test]
    fn single_node() {
        let adj = vec![Vec::<usize>::new()];
        let result = bridges(&adj, 1);
        assert!(result.is_empty());
    }

    /// Path A-B-C: all edges (A,B) and (B,C) are bridges.
    #[test]
    fn path_all_bridges() {
        // A-B-C
        let adj = vec![vec![1], vec![0, 2], vec![1]];
        let result = bridges(&adj, 3);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&(0, 1)));
        assert!(result.contains(&(1, 2)));
    }

    /// Cycle A-B-C-A: no bridges.
    #[test]
    fn cycle_no_bridges() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let result = bridges(&adj, 3);
        assert!(result.is_empty());
    }

    /// Two disconnected edges: each edge is a bridge (removal disconnects its component).
    #[test]
    fn disconnected_two_bridges() {
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        adj[0].push(1);
        adj[1].push(0);
        adj[2].push(3);
        adj[3].push(2);
        let result = bridges(&adj, 4);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&(0, 1)));
        assert!(result.contains(&(2, 3)));
    }

    /// Diamond (A-B, A-C, B-D, C-D): no bridges.
    #[test]
    fn diamond_no_bridges() {
        let adj = vec![vec![1, 2], vec![0, 3], vec![0, 3], vec![1, 2]];
        let result = bridges(&adj, 4);
        assert!(result.is_empty());
    }

    /// Tree (rooted): every edge is a bridge.
    #[test]
    fn tree_all_bridges() {
        // 0-1, 1-2, 1-3 (a tree)
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        adj[0].push(1);
        adj[1].push(0);
        adj[1].push(2);
        adj[2].push(1);
        adj[1].push(3);
        adj[3].push(1);
        let result = bridges(&adj, 4);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&(0, 1)));
        assert!(result.contains(&(1, 2)));
        assert!(result.contains(&(1, 3)));
    }

    /// Determinism: same input → same output (sorted).
    #[test]
    fn deterministic() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let r1 = bridges(&adj, 3);
        let r2 = bridges(&adj, 3);
        assert_eq!(r1, r2);
    }
}
