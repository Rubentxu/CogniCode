//! Per-function dominators on a CFG.
//!
//! This module is a thin façade over the existing
//! [`crate::algorithms::dominators`] CHK implementation. It accepts a CFG
//! adjacency and a root block, and returns dominator info keyed by block
//! id (the same indices used in the adjacency list).
//!
//! ## Determinism
//!
//! Output blocks are sorted by `id` (== enumeration order). When two blocks
//! share the same depth or root, the one with the smaller id appears
//! first. This matches the determinism contract used by every other
//! algorithm in this crate.

use crate::algorithms::dominators as chk;
use serde::{Deserialize, Serialize};

/// Per-block dominator info inside a per-function CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DominatorInfo {
    /// Block id (== index in the original adjacency list).
    pub block_id: usize,
    /// Immediate dominator of this block (`None` if unreachable from root,
    /// or if this block IS the root).
    pub immediate_dominator: Option<usize>,
    /// Depth in the dominator tree (root has depth 0; unreachable blocks
    /// also report depth 0 by convention).
    pub depth: u32,
}

/// Compute per-function dominators from a CFG adjacency list.
///
/// # Arguments
///
/// - `out_neighbors`: same shape as in [`crate::algorithms::cfg`].
/// - `entry`: id of the function entry block.
///
/// # Returns
///
/// `Vec<DominatorInfo>` sorted by `block_id` (== enumeration order). Each
/// entry carries the immediate dominator and the depth in the dominator
/// tree. Unreachable blocks carry `immediate_dominator = None` and
/// `depth = 0`.
pub fn dominators_cfg(out_neighbors: &[Vec<usize>], entry: usize) -> Vec<DominatorInfo> {
    let n = out_neighbors.len();
    if n == 0 {
        return Vec::new();
    }
    assert!(
        entry < n,
        "entry {entry} out of range (n={n}) for dominators_cfg"
    );

    let raw = chk::dominators(out_neighbors, n, entry);
    let mut out: Vec<DominatorInfo> = raw
        .into_iter()
        .map(|(id, idom, depth)| DominatorInfo {
            block_id: id,
            immediate_dominator: idom,
            depth,
        })
        .collect();
    out.sort_by_key(|d| d.block_id);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_self_dominates_with_depth_zero() {
        // entry -> a -> exit
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2], vec![]];
        let dom = dominators_cfg(&adj, 0);
        assert_eq!(dom.len(), 3);
        // The CHK backend reports the root as self-dominating (idom == Some(root)).
        assert_eq!(dom[0].block_id, 0);
        assert_eq!(dom[0].immediate_dominator, Some(0));
        assert_eq!(dom[0].depth, 0);
        assert_eq!(dom[1].block_id, 1);
        assert_eq!(dom[1].immediate_dominator, Some(0));
        assert_eq!(dom[1].depth, 1);
        assert_eq!(dom[2].block_id, 2);
        assert_eq!(dom[2].immediate_dominator, Some(1));
        assert_eq!(dom[2].depth, 2);
    }

    #[test]
    fn branch_yields_diamond_dominators() {
        // entry -> cond; cond -> a; cond -> b; a -> join; b -> join; join -> exit
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![2, 3], vec![4], vec![4], vec![5], vec![]];
        let dom = dominators_cfg(&adj, 0);
        assert_eq!(dom[0].immediate_dominator, Some(0));
        assert_eq!(dom[1].immediate_dominator, Some(0));
        assert_eq!(dom[2].immediate_dominator, Some(1));
        assert_eq!(dom[3].immediate_dominator, Some(1));
        assert_eq!(dom[4].immediate_dominator, Some(1));
        assert_eq!(dom[5].immediate_dominator, Some(4));
    }

    #[test]
    fn unreachable_block_has_no_idom() {
        // entry -> a; b is unreachable
        let adj: Vec<Vec<usize>> = vec![vec![1], vec![], vec![]];
        let dom = dominators_cfg(&adj, 0);
        assert_eq!(dom[2].block_id, 2);
        assert_eq!(dom[2].immediate_dominator, None);
        assert_eq!(dom[2].depth, 0);
    }
}
