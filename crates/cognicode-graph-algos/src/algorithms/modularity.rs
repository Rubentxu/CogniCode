//! Modularity — pure function. No petgraph, no domain types.
//!
//! Modularity measures the quality of a community partition. Higher values indicate
//! stronger community structure (more edges within communities than expected by chance).
//!
//! Formula: Q = (1/2m) * Σ_ij [A_ij - (k_i * k_j / 2m)] * δ(c_i, c_j)
//!
//! Where:
//! - m = total number of edges
//! - A_ij = 1 if edge exists between i and j, 0 otherwise
//! - k_i, k_j = degrees of nodes i and j
//! - δ(c_i, c_j) = 1 if i and j are in the same community, 0 otherwise
//!
//! Range: [-1, 1]. Values > 0 indicate non-random community structure.

/// Compute modularity score for a community assignment.
///
/// # Arguments
///
/// - `community_assignment`: slice of `(node_id, community_id)` tuples.
///   Each node appears exactly once per community_id.
/// - `out_neighbors`: outgoing adjacency list where `out_neighbors[u]` contains
///   every `v` such that edge `u → v` exists. Used to compute degree counts.
///
/// # Returns
///
/// `(f64, usize)` — modularity score and number of communities.
/// Range: [-1.0, 1.0]. Values > 0 indicate meaningful community structure.
///
/// # Complexity
///
/// O(V + E) where V = number of nodes, E = number of edges.
///
/// # Edge cases
///
/// - Empty assignment: returns `(0.0, 0)`.
/// - All nodes in one community: Q = 1 - (1/2m) * Σ_i (k_i² / 2m)
///   (can be negative if graph has high cross-community edges).
/// - Single node: returns `(0.0, 1)` (no edges to compare).
pub fn modularity(
    community_assignment: &[(usize, usize)],
    out_neighbors: &[Vec<usize>],
) -> (f64, usize) {
    if community_assignment.is_empty() {
        return (0.0, 0);
    }

    let n = out_neighbors.len();
    if n == 0 {
        return (0.0, 0);
    }

    // Build node_to_community mapping
    let mut node_to_community: Vec<Option<usize>> = vec![None; n];
    let mut community_ids: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for &(node, community) in community_assignment {
        if node < n {
            node_to_community[node] = Some(community);
            community_ids.insert(community);
        }
    }

    let community_count = community_ids.len();

    // Compute degree of each node
    let mut degree: Vec<usize> = vec![0; n];
    for v in 0..n {
        degree[v] = out_neighbors[v].len();
    }

    // Total edges m: sum of out-degrees
    let m: usize = degree.iter().sum();
    let m_f = m as f64;

    if m == 0 {
        // No edges → modularity = 0 (undefined structure)
        return (0.0, community_count);
    }

    // Compute Σ_ij [A_ij - (k_i * k_j / 2m)] * δ(c_i, c_j)
    // = Σ_edges_in_same_community (1 - k_i * k_j / 2m)
    //   - Σ_non_edges_in_same_community (0 - k_i * k_j / 2m)
    //
    // Optimized: iterate over each community separately.
    // For community C with nodes {i1, i2, ..., ik}:
    //   contribution = Σ_{a<b, a,b in C} [A_ab - (k_a * k_b / 2m)]
    //
    // We compute this by:
    // 1. For each edge (u,v) within same community: add (1 - k_u * k_v / 2m)
    // 2. The non-edge terms are captured by the community degree sum formula:
    //    Σ_{a<b in C} (-k_a * k_b / 2m) = -(1/2m) * [ (Σ_{i in C} k_i)² - Σ_{i in C} k_i² ] / 2

    // Sum of degrees per community
    let mut community_degree_sum: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for &(node, community) in community_assignment {
        if node < n {
            *community_degree_sum.entry(community).or_insert(0) += degree[node];
        }
    }

    let two_m = 2.0 * m_f;
    let one_over_two_m = 1.0 / two_m;

    // Part 1: Sum over all edges within the same community
    let mut edge_contribution: f64 = 0.0;
    for u in 0..n {
        let comm_u = match node_to_community[u] {
            Some(c) => c,
            None => continue,
        };
        for &v in &out_neighbors[u] {
            if node_to_community[v] == Some(comm_u) {
                // Edge u→v is within the same community
                // Only count each edge once (u < v to avoid double-counting directed edges)
                // Since our graph is directed, we treat it as directed for the modularity sum
                // BUT: the modularity formula uses A_ij for undirected edge counting.
                // For directed graphs, we count each directed edge once.
                let contrib = 1.0 - (degree[u] as f64 * degree[v] as f64) * one_over_two_m;
                edge_contribution += contrib;
            }
        }
    }

    // Part 2: Non-edge terms within community
    // For community C with degree sum D_C = Σ_{i in C} k_i:
    //   non_edge_contrib_C = -(1/2m) * [D_C² - Σ_{i in C} k_i²] / 2
    // But since A_ij = 0 for non-edges, we only have the -(k_i * k_j / 2m) term:
    //   = -(1/2m) * Σ_{a<b in C} k_a * k_b
    //   = -(1/2m) * (D_C² - Σ_{i in C} k_i²) / 2
    let mut non_edge_contribution: f64 = 0.0;
    for (&comm, &deg_sum) in community_degree_sum.iter() {
        let members: Vec<usize> = community_assignment
            .iter()
            .filter(|&&(node, c)| c == comm && node < n)
            .map(|&(node, _)| node)
            .collect();

        // Σ_i k_i² for nodes in this community
        let sum_sq: usize = members.iter().map(|&i| degree[i] * degree[i]).sum();

        // D_C² - Σ k_i²
        let deg_sum_f = deg_sum as f64;
        let sum_sq_f = sum_sq as f64;

        // (D_C² - Σ k_i²) / 2
        let pair_sum = (deg_sum_f * deg_sum_f - sum_sq_f) / 2.0;

        non_edge_contribution -= pair_sum * one_over_two_m;
    }

    let q = (edge_contribution + non_edge_contribution) / two_m;
    let q_clamped = q.clamp(-1.0, 1.0);

    (q_clamped, community_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build out_neighbors from directed edge list
    fn build_out_neighbors(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(u, v) in edges {
            if u < n && v < n {
                out[u].push(v);
            }
        }
        out
    }

    /// Single node: no edges → modularity = 0.
    #[test]
    fn single_node_no_edges() {
        let out_neighbors: Vec<Vec<usize>> = vec![Vec::new(); 1];
        let assignment = vec![(0, 0)];
        let (q, count) = modularity(&assignment, &out_neighbors);
        assert_eq!(count, 1);
        assert!((q - 0.0).abs() < 1e-9);
    }

    /// Two isolated nodes in separate communities: no edges → Q = 0.
    #[test]
    fn two_nodes_no_edges() {
        let out_neighbors: Vec<Vec<usize>> = vec![Vec::new(), Vec::new()];
        let assignment = vec![(0, 0), (1, 1)];
        let (q, count) = modularity(&assignment, &out_neighbors);
        assert_eq!(count, 2);
        assert!((q - 0.0).abs() < 1e-9);
    }

    /// Single community with two nodes connected: A→B.
    /// m=1, degrees: k_A=1, k_B=0
    /// Q = (1/2m) * [ (1 - 1*0/2) + (0 - 0*1/2) ] = 1/2 * [1 + 0] = 0.5
    #[test]
    fn two_nodes_single_community() {
        // A=0, B=1. Edge: 0→1
        let out_neighbors = build_out_neighbors(2, &[(0, 1)]);
        let assignment = vec![(0, 0), (1, 0)];
        let (q, count) = modularity(&assignment, &out_neighbors);
        assert_eq!(count, 1);
        // m=1, k_0=1, k_1=0
        // Q = (1/2) * [A_01*(1 - 1*0/2)] = (1/2) * [1*1] = 0.5
        assert!((q - 0.5).abs() < 1e-6);
    }

    /// Two communities, no crossing edges: A↔B in community 0, C↔D in community 1.
    /// m=4 (4 edges), degrees: k_A=2, k_B=1, k_C=2, k_D=1 (each node calls one other)
    /// Q should be positive (better than random).
    #[test]
    fn two_communities_no_crossing() {
        // A=0, B=1 (community 0), C=2, D=3 (community 1)
        // 0→1, 1→0 (community 0), 2→3, 3→2 (community 1)
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (1, 0), (2, 3), (3, 2)]);
        let assignment = vec![(0, 0), (1, 0), (2, 1), (3, 1)];
        let (q, count) = modularity(&assignment, &out_neighbors);
        assert_eq!(count, 2);
        // Q should be positive — all edges are within communities
        assert!(q > 0.0, "modularity should be positive for clean community structure, got {}", q);
    }

    /// Determinism: same input → same output.
    #[test]
    fn deterministic_across_runs() {
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (1, 2), (2, 0), (2, 3)]);
        let assignment = vec![(0, 0), (1, 0), (2, 0), (3, 1)];
        let r1 = modularity(&assignment, &out_neighbors);
        let r2 = modularity(&assignment, &out_neighbors);
        assert_eq!(r1.0, r2.0);
        assert_eq!(r1.1, r2.1);
    }

    /// Community count is correct even with non-contiguous community IDs.
    #[test]
    fn community_count_correct() {
        let out_neighbors = build_out_neighbors(4, &[(0, 1), (2, 3)]);
        // Community IDs: 10 and 20 (non-contiguous)
        let assignment = vec![(0, 10), (1, 10), (2, 20), (3, 20)];
        let (_q, count) = modularity(&assignment, &out_neighbors);
        assert_eq!(count, 2);
        // Each community has only one internal edge, so Q should be 0.5 per community
        // (accounting for the fact that each has 2 nodes, 1 edge)
    }

    /// Modularity range is within [-1, 1].
    #[test]
    fn modularity_in_range() {
        let cases = vec![
            build_out_neighbors(3, &[(0, 1), (1, 2)]), // path
            build_out_neighbors(4, &[(0, 1), (1, 0), (2, 3), (3, 2)]), // two pairs
            build_out_neighbors(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]), // cycle
        ];
        for out_neighbors in cases {
            let assignment: Vec<(usize, usize)> = (0..out_neighbors.len())
                .map(|i| (i, i))
                .collect();
            let (q, _) = modularity(&assignment, &out_neighbors);
            assert!(
                q >= -1.0 && q <= 1.0,
                "modularity {} out of range [-1, 1]",
                q
            );
        }
    }
}
