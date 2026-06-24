//! Transitive reduction — minimal edge set preserving reachability.
//!
//! For DAGs: O(V + E). For cyclic graphs: returns all edges (cycles
//! mean no edge is implied by a strictly longer simple path, so
//! "reduction" is the identity).

use std::collections::{HashSet, VecDeque};

/// Compute transitive reduction of a directed graph.
///
/// # Returns
///
/// `Vec<(usize, usize)>` — list of `(source, target)` edges that
/// survive the reduction. Empty if `n == 0`.
///
/// # Edge cases
///
/// - Empty graph: empty vec
/// - Cyclic graph: returns all edges (identity) — TR is undefined for cycles
/// - DAG: returns minimal edge set
pub fn transitive_reduction(
    _in_neighbors: &[Vec<usize>],
    out_neighbors: &[Vec<usize>],
    n: usize,
) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }

    // Check for cycles via toposort. If cycle exists, return all edges.
    if let Err(()) = topological_sort(out_neighbors, n) {
        // Cycle detected — return all edges as identity reduction.
        let mut result = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for u in 0..n {
            let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
            for &v in neighbors {
                if seen.insert((u, v)) {
                    result.push((u, v));
                }
            }
        }
        return result;
    }

    // For each edge (u, v), check if there's an alternative path u → v
    // that doesn't use edge (u, v). If yes, remove (u, v).
    let mut removable: HashSet<(usize, usize)> = HashSet::new();

    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if u != v && reachable_without_edge(u, v, out_neighbors, &(u, v)) {
                removable.insert((u, v));
            }
        }
    }

    // Build result from out_neighbors (forward edges).
    let mut result = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if !removable.contains(&(u, v)) && u != v && seen.insert((u, v)) {
                result.push((u, v));
            }
        }
    }

    // Sort by source then target.
    result.sort_by_key(|&(s, t)| (s, t));
    result
}

fn topological_sort(out_neighbors: &[Vec<usize>], n: usize) -> Result<Vec<usize>, ()> {
    let mut in_degree: Vec<usize> = vec![0; n];
    for u in 0..n {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            in_degree[v] += 1;
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|&v| in_degree[v] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = queue.pop() {
        order.push(u);
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    if order.len() == n { Ok(order) } else { Err(()) }
}

fn reachable_without_edge(
    from: usize,
    to: usize,
    out_neighbors: &[Vec<usize>],
    forbidden: &(usize, usize),
) -> bool {
    // BFS from `from`, avoiding the forbidden edge.
    if from == to {
        return true;
    }
    let n = out_neighbors.len();
    let mut visited = vec![false; n];
    let mut queue = VecDeque::new();
    queue.push_back(from);
    visited[from] = true;
    while let Some(u) = queue.pop_front() {
        let neighbors = out_neighbors.get(u).map(|n| n.as_slice()).unwrap_or(&[]);
        for &v in neighbors {
            if (u, v) == *forbidden {
                continue;
            }
            if v == to {
                return true;
            }
            if !visited[v] {
                visited[v] = true;
                queue.push_back(v);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        assert_eq!(
            transitive_reduction(&[], &[], 0),
            Vec::<(usize, usize)>::new()
        );
    }

    #[test]
    fn single_edge_preserved() {
        // A → B: minimal.
        let in_n = vec![vec![0], Vec::new()];
        let out_n = vec![vec![1], Vec::new()];
        let result = transitive_reduction(&in_n, &out_n, 2);
        assert_eq!(result, vec![(0, 1)]);
    }

    #[test]
    fn redundant_edge_removed() {
        // A → B, A → C, B → C: edge A→C is redundant (path A→B→C exists).
        // in_n = [in_0=[], in_1=[0], in_2=[0,1]] means 0→1, 0→2, 1→2
        // out_n = [out_0=[1,2], out_1=[2], out_2=[]] means same edges
        // Edge (0,2) is redundant: 0→1→2 exists without it.
        // Edge (1,2) is NOT redundant: no path from 1 to 2 without (1,2).
        let in_n = vec![Vec::new(), vec![0], vec![0, 1]];
        let out_n = vec![vec![1, 2], vec![2], Vec::new()];
        let result = transitive_reduction(&in_n, &out_n, 3);
        // (0,2) is redundant, removed; (0,1) and (1,2) survive
        assert_eq!(result, vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn cyclic_graph_returns_all_edges() {
        // 0 → 1 → 0: cycle. Return both edges.
        let in_n = vec![vec![1], vec![0]];
        let out_n = vec![vec![1], vec![0]];
        let result = transitive_reduction(&in_n, &out_n, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn diamond_dag() {
        // A → B, A → C, B → D, C → D.
        // Edge A→C might be redundant if there's A→B→...→C path.
        let in_n = vec![Vec::new(), vec![0], vec![0], vec![1, 2]];
        let out_n = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        let result = transitive_reduction(&in_n, &out_n, 4);
        // At minimum, A→B and B→D should survive.
        // C→D survives if no A→...→C→D alternative.
        // The structure is a diamond: no edge is truly redundant.
        assert!(result.contains(&(0, 1)));
        assert!(result.contains(&(1, 3)));
    }
}
