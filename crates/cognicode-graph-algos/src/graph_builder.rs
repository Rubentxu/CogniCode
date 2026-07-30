//! The single trait that bridges "I have a graph" to "give me adjacency slices".

/// One-method trait: extract adjacency structure for the algorithms.
///
/// Implementations construct `(in_neighbors, out_degree)` from whatever
/// input type they carry. The algorithm functions consume the slices
/// directly — there is no further trait dispatch in the hot loop.
///
/// # Contract
///
/// - `in_neighbors[v]` contains every `u` such that `u → v` is an edge
///   (callers of `v`). For PageRank, this is the "incoming rank" list.
/// - `out_degree[v]` is the count of edges `v → w` (callees of `v`).
///   Used to normalize rank contributions.
/// - Both vectors have length `n` (= `in_neighbors.len() == out_degree.len()`).
///   Indices are dense `0..n` — caller must reindex if their graph uses
///   sparse indices (e.g. `NodeIndex` from petgraph).
/// - Self-loops count once in `out_degree[v]` and appear once in `in_neighbors[v]`.
///
/// # Example
///
/// ```rust
/// use cognicode_graph_algos::GraphBuilder;
/// use std::collections::{HashMap, HashSet};
///
/// struct MyGraph {
///     nodes: Vec<String>,
///     edges: Vec<(usize, usize)>,
/// }
///
/// impl GraphBuilder for MyGraph {
///     fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>) {
///         let n = self.nodes.len();
///         let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
///         let mut out_degree: Vec<usize> = vec![0; n];
///         for &(u, v) in &self.edges {
///             in_neighbors[v].push(u);
///             out_degree[u] += 1;
///         }
///         (in_neighbors, out_degree)
///     }
/// }
/// ```
pub trait GraphBuilder {
    /// Build `(in_neighbors, out_degree)` for the graph.
    ///
    /// See trait docs for the contract. Implementations may allocate
    /// per call; algorithms call this once at the start.
    fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>);

    /// Build directed adjacency list: `out_neighbors[v]` contains every `w`
    /// such that `v → w` is an edge (callees of `v`).
    ///
    /// Used by conductance and modularity algorithms that need the outgoing
    /// adjacency structure directly rather than the (incoming, out_degree) pair.
    ///
    /// Default implementation derives this from `build_adjacency()`.
    /// Implementations may override with a more efficient version if available.
    fn build_directed_adjacency(&self) -> Vec<Vec<usize>> {
        let (in_neighbors, out_degree) = self.build_adjacency();
        let n = in_neighbors.len();
        let mut out_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
        for v in 0..n {
            out_neighbors[v].reserve(out_degree[v]);
        }
        // Reverse direction: out_neighbors[u] contains v where u → v
        // We need to scan in_neighbors to build out_neighbors
        for v in 0..n {
            for &u in &in_neighbors[v] {
                out_neighbors[u].push(v);
            }
        }
        out_neighbors
    }
}
