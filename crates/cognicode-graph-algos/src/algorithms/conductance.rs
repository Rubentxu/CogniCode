//! Conductance — pure function. No petgraph, no domain types.
//!
//! Conductance measures how well a community is separated from the rest of the
//! graph. Lower values indicate tighter communities.
//!
//! Formula: φ(S) = cut_edges(S, ¬S) / min(vol(S), vol(¬S))
//!
//! The graph is read as **undirected**: every directed edge `u → v` in the
//! input is one undirected edge between `u` and `v`, whichever way it points.
//!
//! - cut_edges(S, ¬S) = number of edges with exactly one endpoint in S
//!   (edges leaving S and edges entering S both count)
//! - degree(v) = out-degree + in-degree (a self-loop adds 2)
//! - vol(S) = sum of degrees of nodes in S; vol(V) = 2m
//! - min(vol(S), vol(¬S)) = volume of the smaller side

use std::collections::{HashMap, HashSet};

/// Compute conductance for each community in a community assignment.
///
/// # Arguments
///
/// - `community_assignment`: slice of `(node_id, community_id)` tuples.
///   Each node appears exactly once per community_id.
///   community_ids are arbitrary non-negative integers.
/// - `out_neighbors`: outgoing adjacency list where `out_neighbors[u]` contains
///   every `v` such that edge `u → v` exists. Edges whose target is not a node
///   of the graph (`v >= out_neighbors.len()`) are ignored.
///
/// # Returns
///
/// `Vec<(CommunityId, f64)>` — conductance score per community, in arbitrary order.
/// If the graph has no edges, or the smaller side has volume 0 (for example a
/// community with no edges, or one that holds every edge), the score is 1.0.
///
/// # Complexity
///
/// O(V + E) where V = number of nodes, E = number of edges.
///
/// # Edge cases
///
/// - `community_assignment` is empty: returns empty vec.
/// - A community with no internal edges but crossing edges: cut / min(vol(S), vol(¬S))
/// - A community whose smaller side has no edges: returns 1.0 (worst case).
pub fn conductance(
    community_assignment: &[(usize, usize)],
    out_neighbors: &[Vec<usize>],
) -> Vec<(usize, f64)> {
    if community_assignment.is_empty() {
        return Vec::new();
    }

    let n = out_neighbors.len();
    if n == 0 {
        return Vec::new();
    }

    // Undirected view: degree = out + in, plus the incoming adjacency so the
    // cut can count edges that enter a community.
    let mut node_degree: Vec<usize> = vec![0; n];
    let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, targets) in out_neighbors.iter().enumerate() {
        for &v in targets.iter().filter(|&&v| v < n) {
            node_degree[u] += 1;
            node_degree[v] += 1;
            in_neighbors[v].push(u);
        }
    }

    let mut community_members: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(node, community) in community_assignment {
        if node < n {
            community_members.entry(community).or_default().push(node);
        }
    }

    // Total graph volume (2m).
    let total_volume: usize = node_degree.iter().sum();

    if total_volume == 0 {
        // No edges in graph — every community is isolated → worst conductance
        return community_members.keys().map(|&c| (c, 1.0)).collect();
    }

    let mut results: Vec<(usize, f64)> = Vec::with_capacity(community_members.len());

    for (&community, members) in community_members.iter() {
        let members_set: HashSet<usize> = members.iter().cloned().collect();

        // Edges with exactly one endpoint in the community.
        let mut cut_edges: usize = 0;
        let mut vol_s: usize = 0;

        for &v in members {
            vol_s += node_degree[v];
            cut_edges += out_neighbors[v]
                .iter()
                .filter(|&&w| w < n && !members_set.contains(&w))
                .count();
            cut_edges += in_neighbors[v]
                .iter()
                .filter(|&&u| !members_set.contains(&u))
                .count();
        }

        // vol(¬S) = total_volume - vol(S)
        let vol_not_s = total_volume - vol_s;
        let min_vol = vol_s.min(vol_not_s);

        let conductance_score = if min_vol == 0 {
            // The smaller side has no edges — worst case
            1.0
        } else {
            cut_edges as f64 / min_vol as f64
        };

        results.push((community, conductance_score));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build out_neighbors from edge list (directed, u → v)
    fn build_out_neighbors(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(u, v) in edges {
            if u < n && v < n {
                out[u].push(v);
            }
        }
        out
    }

    /// Triangle A-B-C-A: single community → no cut edges, vol(S)=6, vol(¬S)=0
    /// min_vol = 0 → conductance = 1.0 (sentinel for empty complement)
    #[test]
    fn triangle_single_community() {
        // A=0, B=1, C=2. Edges: 0→1, 1→2, 2→0
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2), (2, 0)]);
        let assignment = vec![(0, 0), (1, 0), (2, 0)];

        let result = conductance(&assignment, &out_neighbors);
        assert_eq!(result.len(), 1);
        // All nodes are in the same community → no cut edges
        // vol(S) = 6 (3 nodes × 2 degree each), vol(¬S) = 0
        // min_vol = 0 → conductance = 1.0
        assert_eq!(result[0].0, 0);
        assert!((result[0].1 - 1.0).abs() < 1e-9);
    }

    /// Two communities: {A,B} and {C}. Edges: A→B, B→A, C→C.
    /// Community 0: {A,B}, cut = edges from {A,B} to {C} = 0, vol(S)=4, vol(¬S)=2
    /// conductance = 0 / 2 = 0 (best possible)
    #[test]
    fn two_communities_no_cut() {
        // A=0, B=1 (community 0), C=2 (community 1)
        // Edges: 0→1, 1→0 (within community 0), 2→2 (self-loop, within community 1)
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 0), (2, 2)]);
        let assignment = vec![(0, 0), (1, 0), (2, 1)];

        let result = conductance(&assignment, &out_neighbors);
        let result_map: HashMap<usize, f64> = result.into_iter().collect();

        // Community 0: no edges crossing → 0
        assert!((result_map[&0] - 0.0).abs() < 1e-9);
        // Community 1: C has self-loop, no edges to community 0 → 0
        assert!((result_map[&1] - 0.0).abs() < 1e-9);
    }

    /// Two communities with cut edges: {A} and {B,C}. Edge A→B crosses.
    /// Undirected degrees: A=1, B=2, C=1 (total volume 4).
    /// Community 0: {A}, cut=1, vol(S)=1, vol(¬S)=3 → 1/1 = 1.0
    /// Community 1: {B,C}, cut=1 (the same edge, seen from the other side),
    /// vol(S)=3, vol(¬S)=1 → 1/1 = 1.0
    #[test]
    fn two_communities_with_cut() {
        // A=0 (community 0), B=1, C=2 (community 1)
        // Edges: 0→1 (crossing), 1→2 (within community 1)
        let out_neighbors = build_out_neighbors(3, &[(0, 1), (1, 2)]);
        let assignment = vec![(0, 0), (1, 1), (2, 1)];

        let result = conductance(&assignment, &out_neighbors);
        let result_map: HashMap<usize, f64> = result.into_iter().collect();

        // Both communities have cut=1, min_vol=1 → conductance=1.0
        for &comm in &[0, 1] {
            assert!(
                (result_map[&comm] - 1.0).abs() < 1e-9,
                "community {} conductance should be 1.0",
                comm
            );
        }
    }

    /// Empty assignment: returns empty vec.
    #[test]
    fn empty_assignment() {
        let out_neighbors = build_out_neighbors(3, &[(0, 1)]);
        let result = conductance(&[], &out_neighbors);
        assert!(result.is_empty());
    }

    /// Empty graph: no edges. All communities get conductance 1.0.
    #[test]
    fn empty_graph_all_isolated() {
        let out_neighbors: Vec<Vec<usize>> = vec![Vec::new(); 3];
        let assignment = vec![(0, 0), (1, 0), (2, 1)];

        let result = conductance(&assignment, &out_neighbors);
        let result_map: HashMap<usize, f64> = result.into_iter().collect();

        // No edges → all communities get 1.0 (isolated sentinel)
        for &score in result_map.values() {
            assert!((score - 1.0).abs() < 1e-9);
        }
    }

    /// Determinism: same input → same output.
    #[test]
    fn deterministic_across_runs() {
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
        let assignment = vec![(0, 0), (1, 0), (2, 1), (3, 1)];

        let r1 = conductance(&assignment, &out_neighbors);
        let r2 = conductance(&assignment, &out_neighbors);

        // Sort by community ID for comparison (order is arbitrary)
        let mut s1 = r1.clone();
        let mut s2 = r2.clone();
        s1.sort_by_key(|&(c, _)| c);
        s2.sort_by_key(|&(c, _)| c);
        assert_eq!(s1, s2);
    }

    /// Single node community with no internal edges.
    #[test]
    fn single_node_community_no_internal_edges() {
        // A=0 isolated, B=1, C=2 (community 1)
        // Edge B→C within community 1
        let out_neighbors = build_out_neighbors(3, &[(1, 2)]);
        let assignment = vec![(0, 0), (1, 1), (2, 1)];

        let result = conductance(&assignment, &out_neighbors);
        let result_map: HashMap<usize, f64> = result.into_iter().collect();

        // Community 0: single node, no edges, vol(S)=0, vol(¬S)=2
        // min_vol=0 → 1.0 sentinel
        assert!((result_map[&0] - 1.0).abs() < 1e-9);
        // Community 1: cut=0, vol(S)=2, vol(¬S)=0 → min_vol=0 → 1.0
        assert!((result_map[&1] - 1.0).abs() < 1e-9);
    }

    fn scores(
        assignment: &[(usize, usize)],
        edges: &[(usize, usize)],
        n: usize,
    ) -> HashMap<usize, f64> {
        let out_neighbors = build_out_neighbors(n, edges);
        conductance(assignment, &out_neighbors)
            .into_iter()
            .collect()
    }

    /// The witness for the directed-only defect: 0→1, 1→2 with {0} and {1,2}.
    /// The edge 0→1 enters {1,2}; it must count for that side too.
    #[test]
    fn an_incoming_edge_counts_toward_the_cut() {
        let r = scores(&[(0, 0), (1, 1), (2, 1)], &[(0, 1), (1, 2)], 3);
        assert!((r[&1] - 1.0).abs() < 1e-9, "got {}", r[&1]);
        assert!((r[&0] - 1.0).abs() < 1e-9, "got {}", r[&0]);
    }

    /// A single edge between two singleton communities: each side has
    /// volume 1 and the edge crosses → 1.0 for both (not the "no edges"
    /// sentinel path).
    #[test]
    fn a_single_edge_between_singletons() {
        let r = scores(&[(0, 0), (1, 1)], &[(0, 1)], 2);
        assert!((r[&0] - 1.0).abs() < 1e-9);
        assert!((r[&1] - 1.0).abs() < 1e-9);
    }

    /// Path 0→1→2→3→4→5, S={0,1}, T={2,3,4,5}.
    /// Degrees 1,2,2,2,2,1 (total 10). cut=1. vol(S)=3, vol(T)=7.
    /// φ = 1 / min(3,7) = 1/3 for both sides.
    #[test]
    fn path_split_is_one_third_on_both_sides() {
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)];
        let a = [(0, 0), (1, 0), (2, 1), (3, 1), (4, 1), (5, 1)];
        let r = scores(&a, &edges, 6);
        assert!((r[&0] - 1.0 / 3.0).abs() < 1e-9, "got {}", r[&0]);
        assert!((r[&1] - 1.0 / 3.0).abs() < 1e-9, "got {}", r[&1]);
    }

    /// Reversing every edge describes the same undirected graph.
    #[test]
    fn reversing_every_edge_changes_nothing() {
        let a = [(0, 0), (1, 0), (2, 1), (3, 1), (4, 1), (5, 1)];
        let fwd = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)];
        let rev = [(1, 0), (2, 1), (3, 2), (4, 3), (5, 4)];
        let f = scores(&a, &fwd, 6);
        let b = scores(&a, &rev, 6);
        assert!((f[&0] - b[&0]).abs() < 1e-9);
        assert!((f[&1] - b[&1]).abs() < 1e-9);
    }

    /// Two dense halves joined by one edge, edges in both directions inside
    /// each half. {0,1} and {2,3}: 0↔1, 2↔3, 1→2.
    /// Degrees: 0→2, 1→3, 2→3, 3→2 (total 10). vol=5 each, cut=1 → 0.2.
    #[test]
    fn two_halves_joined_by_one_edge() {
        let edges = [(0, 1), (1, 0), (2, 3), (3, 2), (1, 2)];
        let r = scores(&[(0, 0), (1, 0), (2, 1), (3, 1)], &edges, 4);
        assert!((r[&0] - 0.2).abs() < 1e-9, "got {}", r[&0]);
        assert!((r[&1] - 0.2).abs() < 1e-9, "got {}", r[&1]);
    }

    /// Like `scores` but keeps edges whose target is outside the graph
    /// (`build_out_neighbors` would drop them).
    fn scores_raw(assignment: &[(usize, usize)], out: Vec<Vec<usize>>) -> HashMap<usize, f64> {
        conductance(assignment, &out).into_iter().collect()
    }

    /// An edge whose target is not a node of the graph is not an edge: it
    /// adds neither degree nor cut. Path 0→1→…→5 plus 0→9 gives the same 1/3.
    #[test]
    fn an_edge_to_a_missing_node_is_ignored() {
        let out = vec![vec![1, 9], vec![2], vec![3], vec![4], vec![5], vec![]];
        let a = [(0, 0), (1, 0), (2, 1), (3, 1), (4, 1), (5, 1)];
        let r = scores_raw(&a, out);
        assert!((r[&0] - 1.0 / 3.0).abs() < 1e-9, "got {}", r[&0]);
        assert!((r[&1] - 1.0 / 3.0).abs() < 1e-9, "got {}", r[&1]);
    }

    /// A target equal to `n` (one past the last node) is as missing as 9.
    /// Witness 0→1, 1→2 plus 0→3 with n = 3 still gives 1.0 on both sides.
    #[test]
    fn an_edge_to_node_n_is_ignored() {
        let out = vec![vec![1, 3], vec![2], vec![]];
        let r = scores_raw(&[(0, 0), (1, 1), (2, 1)], out);
        assert!((r[&0] - 1.0).abs() < 1e-9, "got {}", r[&0]);
        assert!((r[&1] - 1.0).abs() < 1e-9, "got {}", r[&1]);
    }

    /// An assigned node equal to `n` does not exist: its community is not
    /// reported and nothing panics.
    #[test]
    fn an_assigned_node_n_is_ignored() {
        let r = scores(&[(0, 0), (1, 1), (2, 1), (3, 7)], &[(0, 1), (1, 2)], 3);
        assert!(!r.contains_key(&7));
        assert!((r[&0] - 1.0).abs() < 1e-9);
    }
}
