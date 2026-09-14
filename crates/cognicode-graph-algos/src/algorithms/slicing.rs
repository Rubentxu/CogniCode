//! Forward + backward program slicing over a per-function CFG.
//!
//! ## Slicing criterion (per design D3)
//!
//! The slice criterion is `(variable, definition_site)`:
//!
//! - `variable`: the name of interest (string).
//! - `definition_site`: the block id where the variable's defining
//!   statement lives. In WU3 we slice at the basic-block granularity;
//!   finer statement-level slicing arrives with the tree-sitter extractor
//!   in M5.1.
//!
//! ## Algorithms
//!
//! - **Forward slice** from a definition: blocks reachable from
//!   `definition_site` in the CFG.
//! - **Backward slice** to a use: blocks that can reach a block tagged as
//!   containing the criterion `variable`. Since WU3 has no per-block
//!   "use site" annotation yet, the conservative semantics are:
//!   "every block transitively reaching a block whose id appears in the
//!   optional `use_sites` list". If `use_sites` is empty, the backward
//!   slice degenerates to "every block that reaches a block whose id is
//!   in `use_sites`" — i.e. only `use_sites` themselves. The full
//!   control-flow-aware backward slice plugs in once the tree-sitter
//!   extractor attaches use-site annotations to each block.
//!
//! ## Determinism
//!
//! Output is a sorted `Vec<usize>` of block ids.

use std::collections::BTreeSet;

/// Forward slice from `definition_site`: every block reachable from
/// `definition_site` in `out_neighbors` (including `definition_site`).
pub fn forward_slice(out_neighbors: &[Vec<usize>], definition_site: usize) -> Vec<usize> {
    if out_neighbors.is_empty() || definition_site >= out_neighbors.len() {
        return Vec::new();
    }
    let n = out_neighbors.len();
    let mut visited: BTreeSet<usize> = BTreeSet::new();
    let mut stack = vec![definition_site];
    while let Some(u) = stack.pop() {
        if !visited.insert(u) {
            continue;
        }
        if let Some(nbrs) = out_neighbors.get(u) {
            for &v in nbrs {
                if v < n && !visited.contains(&v) {
                    stack.push(v);
                }
            }
        }
    }
    visited.into_iter().collect()
}

/// Backward slice to any block in `use_sites`: every block from which
/// at least one block in `use_sites` is reachable.
///
/// If `use_sites` is empty the result is empty — there is no criterion to
/// slice against. Callers that want a "use anywhere" semantics should pass
/// `use_sites = (0..n).collect()` explicitly.
pub fn backward_slice(out_neighbors: &[Vec<usize>], use_sites: &[usize]) -> Vec<usize> {
    if out_neighbors.is_empty() || use_sites.is_empty() {
        return Vec::new();
    }
    let n = out_neighbors.len();
    // Filter out-of-range sites first; we silently drop them so the slice
    // remains total (no panics on malformed input).
    let valid_sites: Vec<usize> = use_sites.iter().copied().filter(|&s| s < n).collect();
    if valid_sites.is_empty() {
        return Vec::new();
    }

    // Inverse adjacency: predecessors[p] = [u | p ∈ out_neighbors[u]].
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, nbrs) in out_neighbors.iter().enumerate() {
        for &v in nbrs {
            if v < n {
                preds[v].push(u);
            }
        }
    }

    let mut visited: BTreeSet<usize> = BTreeSet::new();
    let mut stack: Vec<usize> = valid_sites.clone();
    while let Some(u) = stack.pop() {
        if !visited.insert(u) {
            continue;
        }
        for &p in &preds[u] {
            if !visited.contains(&p) {
                stack.push(p);
            }
        }
    }
    visited.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_slice_linear_chain() {
        // 0 -> 1 -> 2 -> 3
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2], vec![3], vec![]];
        assert_eq!(forward_slice(&adj, 0), vec![0, 1, 2, 3]);
        assert_eq!(forward_slice(&adj, 2), vec![2, 3]);
    }

    #[test]
    fn forward_slice_branch_reaches_both_arms() {
        // 0 -> 1; 1 -> 2; 1 -> 3;
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2, 3], vec![], vec![]];
        assert_eq!(forward_slice(&adj, 1), vec![1, 2, 3]);
    }

    #[test]
    fn forward_slice_out_of_range_returns_empty() {
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![]];
        assert_eq!(forward_slice(&adj, 5), Vec::<usize>::new());
    }

    #[test]
    fn backward_slice_diamond_includes_all_paths_to_use() {
        // 0 -> 1; 1 -> 2; 1 -> 3; 2 -> 4; 3 -> 4;
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2, 3], vec![4], vec![4], vec![]];
        // Use at block 4: predecessors are 2, 3; predecessors of those are 1;
        // predecessors of 1 is 0. So slice = {0, 1, 2, 3, 4}.
        assert_eq!(backward_slice(&adj, &[4]), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn backward_slice_partial_diamond() {
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2, 3], vec![4], vec![4], vec![]];
        // Use only at block 2: predecessors are 1; predecessor of 1 is 0.
        assert_eq!(backward_slice(&adj, &[2]), vec![0, 1, 2]);
    }

    #[test]
    fn backward_slice_empty_criterion_returns_empty() {
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![]];
        assert_eq!(backward_slice(&adj, &[]), Vec::<usize>::new());
    }

    #[test]
    fn backward_slice_silently_drops_out_of_range_sites() {
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![]];
        // Site 99 is invalid → dropped; site 1 is valid → slice = {0, 1}.
        assert_eq!(backward_slice(&adj, &[99, 1]), vec![0, 1]);
    }

    #[test]
    fn forward_and_backward_are_duals_on_a_chain() {
        // For a linear chain 0 → 1 → 2 → 3, the forward slice from 0 is the
        // whole chain and the backward slice to 3 is also the whole chain.
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2], vec![3], vec![]];
        assert_eq!(forward_slice(&adj, 0), vec![0, 1, 2, 3]);
        assert_eq!(backward_slice(&adj, &[3]), vec![0, 1, 2, 3]);
    }
}
