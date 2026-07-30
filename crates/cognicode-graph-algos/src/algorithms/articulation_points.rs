//! Articulation Points — Tarjan's algorithm for finding cut vertices.
//!
//! An articulation point (cut vertex) is a node whose removal increases
//! the number of connected components in the graph.
//!
//! Output: `Vec<(usize, usize)>` — (node_index, cut_vertices_count).
//! `cut_vertices_count` = number of components after removal.
//!
//! For root: cut if it has 2+ DFS children.
//! For non-root: cut if it has a child v with low[v] >= disc[v].

/// Run articulation points on an undirected adjacency structure.
///
/// # Arguments
///
/// - `adj`: undirected adjacency list. `adj[u]` lists all neighbors of `u`.
///   Length MUST equal `n`.
/// - `n`: number of nodes.
///
/// # Returns
///
/// `Vec<(usize, usize)>` — for each articulation point:
/// - `node_id`: the node index
/// - `cut_vertices_count`: number of connected components after removing this node
///
/// Results are sorted by `node_id` for determinism.
///
/// # Edge cases
///
/// - `n == 0`: returns empty vec
/// - Single node: no articulation points
/// - Two nodes with edge: neither is an articulation point
/// - Disconnected graph: articulation points per component
pub fn articulation_points(adj: &[Vec<usize>], n: usize) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }

    // Tarjan's algorithm: DFS-based articulation point detection
    let mut disc: Vec<i64> = vec![-1; n]; // discovery time (-1 = unvisited)
    let mut low: Vec<i64> = vec![0; n];
    let mut timer: i64 = 0;
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut articulation: Vec<bool> = vec![false; n];

    // Count DFS children of root (root is cut iff children >= 2)
    let mut root_children: usize = 0;

    fn dfs(
        u: usize,
        disc: &mut Vec<i64>,
        low: &mut Vec<i64>,
        timer: &mut i64,
        parent: &mut Vec<Option<usize>>,
        articulation: &mut Vec<bool>,
        adj: &[Vec<usize>],
        root_children: &mut usize,
    ) {
        disc[u] = *timer;
        low[u] = *timer;
        *timer += 1;
        let mut child_count: usize = 0;
        let is_root = parent[u].is_none();

        for &v in &adj[u] {
            if disc[v] == -1 {
                // v not visited — v is a DFS child of u
                parent[v] = Some(u);
                child_count += 1;
                if is_root {
                    *root_children += 1;
                }
                dfs(
                    v,
                    disc,
                    low,
                    timer,
                    parent,
                    articulation,
                    adj,
                    root_children,
                );

                // After returning from DFS child v:
                // low[u] = min(low[u], low[v])
                low[u] = low[u].min(low[v]);

                // Non-root articulation point: if low[v] >= disc[u]
                if !is_root && low[v] >= disc[u] {
                    articulation[u] = true;
                }
            } else if Some(u) != parent[v] {
                // Back edge: low[u] = min(low[u], disc[v])
                low[u] = low[u].min(disc[v]);
            }
        }

        // Root articulation point: if it has 2+ DFS children
        if is_root && child_count >= 2 {
            articulation[u] = true;
        }
    }

    // Handle disconnected components by running DFS from each unvisited node
    for i in 0..n {
        if disc[i] == -1 {
            dfs(
                i,
                &mut disc,
                &mut low,
                &mut timer,
                &mut parent,
                &mut articulation,
                adj,
                &mut root_children,
            );
        }
    }

    // Now compute cut_vertices_count for each articulation point
    // cut_vertices_count = number of additional components formed by removing the node
    // For a cut vertex: 1 + number of "split" components
    // Algorithm: remove node, count resulting components

    let mut result: Vec<(usize, usize)> = Vec::new();
    for u in 0..n {
        if articulation[u] {
            let cut_count = count_components_without_node(adj, n, u);
            result.push((u, cut_count));
        }
    }

    result.sort_by_key(|r| r.0);
    result
}

/// Count connected components in the graph after removing node `exclude`.
fn count_components_without_node(adj: &[Vec<usize>], n: usize, exclude: usize) -> usize {
    let mut visited: Vec<bool> = vec![false; n];
    visited[exclude] = true;
    let mut count = 0;

    for i in 0..n {
        if !visited[i] {
            count += 1;
            // BFS/DFS from i
            let mut stack = vec![i];
            visited[i] = true;
            while let Some(u) = stack.pop() {
                for &v in &adj[u] {
                    if v != exclude && !visited[v] {
                        visited[v] = true;
                        stack.push(v);
                    }
                }
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty graph: empty result.
    #[test]
    fn empty_graph() {
        let result = articulation_points(&[], 0);
        assert!(result.is_empty());
    }

    /// Single node: no articulation points.
    #[test]
    fn single_node() {
        let adj = vec![Vec::<usize>::new()];
        let result = articulation_points(&adj, 1);
        assert!(result.is_empty());
    }

    /// Two nodes with edge: neither is an articulation point.
    #[test]
    fn two_nodes_connected() {
        // A-B
        let adj = vec![vec![1], vec![0]];
        let result = articulation_points(&adj, 2);
        assert!(result.is_empty());
    }

    /// Path A-B-C: B is the articulation point.
    #[test]
    fn path_articulation() {
        // A-B-C
        let adj = vec![
            vec![1],    // A → B
            vec![0, 2], // B → A, C
            vec![1],    // C → B
        ];
        let result = articulation_points(&adj, 3);
        assert_eq!(result.len(), 1, "B should be the only articulation point");
        assert_eq!(result[0].0, 1, "B's index = 1");
        // Removing B splits into 2 components: {A} and {C}
        assert_eq!(result[0].1, 2, "cut_vertices_count should be 2");
    }

    /// Cycle A-B-C-A: no articulation points.
    #[test]
    fn cycle_no_articulation() {
        // A-B-C-A
        let adj = vec![
            vec![1, 2], // A → B, C
            vec![0, 2], // B → A, C
            vec![1, 0], // C → B, A
        ];
        let result = articulation_points(&adj, 3);
        assert!(result.is_empty(), "cycle has no articulation points");
    }

    /// Disconnected: two isolated edges {A-B, C-D}. No articulation points.
    #[test]
    fn disconnected_no_articulation() {
        // A-B, C-D (two separate edges)
        // adj[0]=[1], adj[1]=[0], adj[2]=[3], adj[3]=[2]
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        adj[0].push(1);
        adj[1].push(0);
        adj[2].push(3);
        adj[3].push(2);
        let result = articulation_points(&adj, 4);
        assert!(result.is_empty());
    }

    /// Star: center A connected to B, C, D. A is articulation point.
    #[test]
    fn star_articulation() {
        // A connected to B, C, D
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        adj[0].extend([1, 2, 3]); // A → B, C, D
        adj[1].push(0);
        adj[2].push(0);
        adj[3].push(0);
        let result = articulation_points(&adj, 4);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0, "A (center) is articulation point");
        // Removing A: B, C, D are all isolated → 3 components
        assert_eq!(result[0].1, 3, "cut_vertices_count = 3");
    }

    /// Graph spec example: {A-B, B-C, C-D, B-D, B-E, E-F}
    /// A=0, B=1, C=2, D=3, E=4, F=5
    /// B is cut vertex, E is cut vertex (removing E leaves F isolated).
    #[test]
    fn spec_example() {
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); 6];
        // A-B (0-1)
        adj[0].push(1);
        adj[1].push(0);
        // B-C (1-2)
        adj[1].push(2);
        adj[2].push(1);
        // C-D (2-3)
        adj[2].push(3);
        adj[3].push(2);
        // B-D (1-3)
        adj[1].push(3);
        adj[3].push(1);
        // B-E (1-4)
        adj[1].push(4);
        adj[4].push(1);
        // E-F (4-5)
        adj[4].push(5);
        adj[5].push(4);

        let result = articulation_points(&adj, 6);
        let nodes: Vec<usize> = result.iter().map(|(n, _)| *n).collect();
        assert!(
            nodes.contains(&1),
            "B (node 1) should be articulation point"
        );
        assert!(
            nodes.contains(&4),
            "E (node 4) should be articulation point"
        );
    }

    /// Determinism: same input → same output.
    #[test]
    fn deterministic() {
        let adj = vec![vec![1, 2], vec![0, 2], vec![1, 0]];
        let r1 = articulation_points(&adj, 3);
        let r2 = articulation_points(&adj, 3);
        assert_eq!(r1, r2);
    }
}
