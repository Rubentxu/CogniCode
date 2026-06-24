//! Strongly Connected Components (SCC) condensation.
//!
//! Tarjan's algorithm: O(V + E). Returns SCCs where each SCC is a set
//! of mutually-reachable nodes. The output is deterministic: post-order
//! traversal order, sorted alphabetically within each SCC.

/// Compute SCC condensation using Tarjan's algorithm.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[u]` lists every `v` with edge `u → v`.
///   Length MUST equal `n`.
/// - `n`: number of nodes.
///
/// # Returns
///
/// `Vec<Vec<usize>>` — list of SCCs. Each SCC contains its member
/// node indices. Order is deterministic (post-order DFS, alphabetic
/// within each SCC).
///
/// # Edge cases
///
/// - Empty graph: empty vec
/// - Single node: one singleton SCC
/// - DAG (no cycles): each node is its own singleton SCC
pub fn condensation(out_neighbors: &[Vec<usize>], n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return Vec::new();
    }

    let mut index_counter = 0usize;
    let mut stack: Vec<usize> = Vec::with_capacity(n);
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut lowlinks: Vec<usize> = vec![0; n];
    let mut result: Vec<Vec<usize>> = Vec::new();

    // Iterative DFS using explicit stack with (node, next_child_index, parent) tuples.
    // The parent field lets us update lowlinks when a child finishes.
    for start in 0..n {
        if indices[start].is_some() {
            continue;
        }

        let mut work_stack: Vec<(usize, usize, Option<usize>)> = vec![(start, 0, None)];

        while let Some((v, child_idx, parent)) = work_stack.pop() {
            let neighbors = out_neighbors.get(v).map(|n| n.as_slice()).unwrap_or(&[]);

            if child_idx == 0 {
                // First visit to v.
                indices[v] = Some(index_counter);
                lowlinks[v] = index_counter;
                index_counter += 1;
                stack.push(v);
                on_stack[v] = true;
            }

            if child_idx < neighbors.len() {
                // Process next child.
                let w = neighbors[child_idx];

                if let Some(w_idx) = indices[w] {
                    // w already visited - update lowlink if w is on stack.
                    if on_stack[w] {
                        lowlinks[v] = lowlinks[v].min(w_idx);
                    }
                    // Push v back with incremented child_idx (for next neighbor).
                    work_stack.push((v, child_idx + 1, parent));
                } else {
                    // w not visited - recurse into it.
                    work_stack.push((v, child_idx + 1, parent));
                    work_stack.push((w, 0, Some(v)));
                }
            } else {
                // All children processed. Update parent's lowlink if we have one.
                if let Some(parent_node) = parent {
                    lowlinks[parent_node] = lowlinks[parent_node].min(lowlinks[v]);
                }

                // Check if v is an SCC root.
                if lowlinks[v] == indices[v].unwrap() {
                    let mut scc = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        scc.push(w);
                        if w == v {
                            break;
                        }
                    }
                    scc.sort();
                    result.push(scc);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        assert!(condensation(&[], 0).is_empty());
    }

    #[test]
    fn single_node() {
        let result = condensation(&[Vec::new()], 1);
        assert_eq!(result, vec![vec![0]]);
    }

    #[test]
    fn three_node_cycle() {
        // A → B → C → A: one SCC with all 3 nodes.
        let out_neighbors = vec![
            vec![1], // A calls B
            vec![2], // B calls C
            vec![0], // C calls A
        ];
        let result = condensation(&out_neighbors, 3);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![0, 1, 2]);
    }

    #[test]
    fn dag_singletons() {
        // A → B, A → C: no cycles, three singletons.
        let out_neighbors = vec![
            vec![1, 2], // A calls B and C
            Vec::new(), // B calls nobody
            Vec::new(), // C calls nobody
        ];
        let result = condensation(&out_neighbors, 3);
        assert_eq!(result.len(), 3);
        for scc in &result {
            assert_eq!(scc.len(), 1);
        }
    }

    #[test]
    fn two_disconnected_sccs() {
        // SCC1: 0↔1, SCC2: 2↔3
        let out_neighbors = vec![
            vec![1], // 0 calls 1
            vec![0], // 1 calls 0
            vec![3], // 2 calls 3
            vec![2], // 3 calls 2
        ];
        let result = condensation(&out_neighbors, 4);
        assert_eq!(result.len(), 2);
        for scc in &result {
            assert_eq!(scc.len(), 2);
        }
    }

    #[test]
    fn self_loop_is_singleton_scc() {
        // Node 0 with self-loop: still one node but has a self-loop.
        let out_neighbors = vec![vec![0]];
        let result = condensation(&out_neighbors, 1);
        assert_eq!(result, vec![vec![0]]);
    }
}
