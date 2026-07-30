//! Conductance — pure function. No petgraph, no domain types.
//!
//! Conductance measures how well a community is separated from the rest of the
//! graph. Lower values indicate tighter communities.
//!
//! Formula: φ(S) = cut_edges(S, ¬S) / min(vol(S), vol(¬S))
//!
//! Where:
//! - cut_edges(S, ¬S) = number of edges crossing from S to outside S
//! - vol(S) = sum of degrees of nodes in S
//! - min(vol(S), vol(¬S)) = volume of the smaller side (sentinel: 0 if both empty)

use std::collections::HashMap;

/// Compute conductance for each community in a community assignment.
///
/// # Arguments
///
/// - `community_assignment`: slice of `(node_id, community_id)` tuples.
///   Each node appears exactly once per community_id.
///   community_ids are arbitrary non-negative integers.
/// - `out_neighbors`: outgoing adjacency list where `out_neighbors[u]` contains
///   every `v` such that edge `u → v` exists.
///
/// # Returns
///
/// `Vec<(CommunityId, f64)>` — conductance score per community, in arbitrary order.
/// If a community has no edges (neither internal nor crossing), conductance = 1.0
/// (worst case — isolated community).
///
/// # Complexity
///
/// O(V + E) where V = number of nodes, E = number of edges.
///
/// # Edge cases
///
/// - `community_assignment` is empty: returns empty vec.
/// - A community with no internal edges but crossing edges: cut / min(vol(S), vol(¬S))
/// - A community with no edges at all: returns 1.0 (isolated, worst-case conductance).
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

    // Build community assignment array: node_to_community[node] = community_id
    // If a node is not in the assignment, it belongs to a special "unassigned" community.
    let mut node_to_community: Vec<Option<usize>> = vec![None; n];
    let mut community_members: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut node_degree: Vec<usize> = vec![0; n];

    // Compute degrees from out_neighbors
    for v in 0..n {
        node_degree[v] = out_neighbors[v].len();
    }

    // Populate node_to_community and community_members
    for &(node, community) in community_assignment {
        if node < n {
            node_to_community[node] = Some(community);
            community_members
                .entry(community)
                .or_default()
                .push(node);
        }
    }

    // Pre-compute total graph volume (2m)
    let total_volume: usize = node_degree.iter().sum();
    // m = total edges (undirected volume / 2)
    let m = total_volume.saturating_sub(1) / 2;

    if m == 0 {
        // No edges in graph — every community is isolated → worst conductance
        return community_members
            .keys()
            .map(|&c| (c, 1.0))
            .collect();
    }

    let mut results: Vec<(usize, f64)> = Vec::with_capacity(community_members.len());

    for (&community, &ref members) in community_members.iter() {
        if members.is_empty() {
            continue;
        }

        let members_set: std::collections::HashSet<usize> =
            members.iter().cloned().collect();

        // Compute cut edges: edges from community to outside
        let mut cut_edges: usize = 0;
        let mut vol_s: usize = 0;

        for &v in members {
            vol_s += node_degree[v];
            for &w in &out_neighbors[v] {
                if !members_set.contains(&w) {
                    cut_edges += 1;
                }
            }
        }

        // vol(¬S) = total_volume - vol(S)
        let vol_not_s = total_volume.saturating_sub(vol_s);

        // min(vol(S), vol(¬S))
        let min_vol = vol_s.min(vol_not_s);

        let conductance_score = if min_vol == 0 {
            // Community is on the smaller side and has no edges
            // or graph is disconnected — worst case
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
    /// Community 0: {A}, cut=1 (A→B), vol(S)=1, vol(¬S)=4
    /// min_vol = 1 → conductance = 1/1 = 1.0
    /// Community 1: {B,C}, cut=1 (A→B), vol(S)=4, vol(¬S)=1
    /// min_vol = 1 → conductance = 1/1 = 1.0
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
}
