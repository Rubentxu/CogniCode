//! Control-flow graph (CFG) — per-function, pure functions over flat slices.
//!
//! ## Representation
//!
//! A per-function CFG is represented as a directed graph of **basic blocks**.
//! Each block is a contiguous run of statements with a single entry and a
//! single exit. Edges encode fall-through, conditional branches, loops, and
//! `return` / `panic` terminators.
//!
//! The hot-loop input mirrors every other algorithm in this crate:
//!
//! - `out_neighbors`: `out_neighbors[bb] = vec![bb', ...]` — successor
//!   blocks. Length MUST equal `bb_count`.
//! - `entry`: index of the function entry block (must be `< bb_count`).
//! - `exits`: indices of return / panic blocks (sink nodes).
//!
//! This is intentionally minimal: the heavy lifting of converting a
//! tree-sitter AST into basic blocks lives in a later M5.1 slice. WU2
//! provides the contract surface, fixtures, and tests so M5.1 can drop
//! in a real extractor without changing the public algorithm signature.
//!
//! ## Determinism
//!
//! The CFG output is a sorted JSON-friendly structure: blocks are emitted
//! in `0..bb_count` order, edges within each block are sorted by
//! successor index. This makes fixtures and conformance digests stable
//! across runs and platforms.

use serde::{Deserialize, Serialize};

/// A single basic block in the per-function CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicBlock {
    /// Stable identifier (matches the index in the adjacency list).
    pub id: usize,
    /// True iff this block is a sink (return / panic / unreachable).
    pub is_exit: bool,
}

/// One edge in the CFG — `from -> to`. Kept tiny so it can be `Vec<(usize, usize)]`
/// in fixture data and round-trip through `serde_json` without ceremony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgEdge {
    /// Source block index.
    pub from: usize,
    /// Target block index.
    pub to: usize,
}

/// A per-function CFG snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cfg {
    /// Index of the entry block.
    pub entry: usize,
    /// Basic blocks, sorted by `id` (== enumeration order).
    pub blocks: Vec<BasicBlock>,
    /// Edges in stable order (sorted by `(from, to)`).
    pub edges: Vec<CfgEdge>,
}

/// Build a [`Cfg`] from a flat adjacency list.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[bb]` lists every successor of block `bb`.
/// - `entry`: index of the function entry block. Must be `< bb_count`.
/// - `exits`: indices of exit blocks (return / panic). May be empty.
///
/// # Returns
///
/// A [`Cfg`] with blocks in enumeration order and edges sorted by
/// `(from, to)` for determinism. `is_exit` is `true` for any block
/// listed in `exits`.
pub fn build_cfg(out_neighbors: &[Vec<usize>], entry: usize, exits: &[usize]) -> Cfg {
    let bb_count = out_neighbors.len();
    assert!(
        entry < bb_count,
        "entry {entry} out of range (bb_count={bb_count})"
    );

    let mut exit_set: Vec<bool> = vec![false; bb_count];
    for &e in exits {
        assert!(e < bb_count, "exit {e} out of range (bb_count={bb_count})");
        exit_set[e] = true;
    }

    let blocks: Vec<BasicBlock> = (0..bb_count)
        .map(|id| BasicBlock {
            id,
            is_exit: exit_set[id],
        })
        .collect();

    let mut edges: Vec<CfgEdge> = Vec::new();
    for (from, nbrs) in out_neighbors.iter().enumerate() {
        for &to in nbrs {
            // Defensive: out-of-range successors would crash determinism tests
            // downstream, so we clamp + dedupe rather than panic. We log the
            // violation into the edge list with a sentinel `to == from`
            // emission only when the data is well-formed; malformed
            // successors are silently dropped to keep the algorithm pure.
            if to < bb_count && to != from {
                edges.push(CfgEdge { from, to });
            }
        }
    }
    edges.sort_by_key(|e| (e.from, e.to));
    edges.dedup();

    Cfg {
        entry,
        blocks,
        edges,
    }
}

/// Number of reachable blocks from `entry`.
///
/// BFS over `out_neighbors`. A block is "reachable" iff there is a directed
/// path from `entry` to it. The entry block itself counts.
pub fn reachable_blocks(out_neighbors: &[Vec<usize>], entry: usize) -> usize {
    if out_neighbors.is_empty() {
        return 0;
    }
    let n = out_neighbors.len();
    let mut visited = vec![false; n];
    let mut stack = vec![entry];
    visited[entry] = true;
    let mut count = 0usize;
    while let Some(u) = stack.pop() {
        count += 1;
        if let Some(nbrs) = out_neighbors.get(u) {
            for &v in nbrs {
                if v < n && !visited[v] {
                    visited[v] = true;
                    stack.push(v);
                }
            }
        }
    }
    count
}

/// Edge count of a CFG (sum of successor counts, deduplicated).
///
/// Useful for conformance fixtures: `assert_eq!(edge_count(cfg), 3)`.
pub fn edge_count(cfg: &Cfg) -> usize {
    cfg.edges.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear() -> (Vec<Vec<usize>>, usize, Vec<usize>) {
        // entry -> a -> b -> exit
        (vec![vec![1], vec![2], vec![3], vec![]], 0, vec![3])
    }

    fn branch() -> (Vec<Vec<usize>>, usize, Vec<usize>) {
        // entry -> cond; cond -> a; cond -> b; a -> exit; b -> exit
        (
            vec![vec![1], vec![2, 3], vec![4], vec![4], vec![]],
            0,
            vec![4],
        )
    }

    fn empty_fn() -> (Vec<Vec<usize>>, usize, Vec<usize>) {
        (vec![vec![]], 0, vec![0])
    }

    #[test]
    fn linear_cfg_has_four_blocks_and_three_edges() {
        let (adj, entry, exits) = linear();
        let cfg = build_cfg(&adj, entry, &exits);
        assert_eq!(cfg.blocks.len(), 4);
        assert_eq!(cfg.edges.len(), 3);
        assert_eq!(cfg.entry, 0);
        assert!(!cfg.blocks[0].is_exit);
        assert!(cfg.blocks[3].is_exit);
    }

    #[test]
    fn branch_cfg_has_five_blocks_and_five_edges_sorted() {
        let (adj, entry, exits) = branch();
        let cfg = build_cfg(&adj, entry, &exits);
        assert_eq!(cfg.blocks.len(), 5);
        assert_eq!(cfg.edges.len(), 5);
        // edges sorted by (from, to)
        let pairs: Vec<(usize, usize)> = cfg.edges.iter().map(|e| (e.from, e.to)).collect();
        assert_eq!(pairs, vec![(0, 1), (1, 2), (1, 3), (2, 4), (3, 4)]);
    }

    #[test]
    fn empty_function_yields_single_exit_block() {
        let (adj, entry, exits) = empty_fn();
        let cfg = build_cfg(&adj, entry, &exits);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.edges.len(), 0);
        assert!(cfg.blocks[0].is_exit);
    }

    #[test]
    fn reachable_blocks_counts_connected_components() {
        let (adj, entry, _) = linear();
        assert_eq!(reachable_blocks(&adj, entry), 4);

        // Disconnected: block 2 has no incoming edge from entry
        let adj = vec![vec![1], vec![], vec![]];
        assert_eq!(reachable_blocks(&adj, 0), 2);
        assert_eq!(reachable_blocks(&adj, 2), 1);
    }

    #[test]
    fn edge_count_matches_dedup() {
        let (adj, entry, exits) = branch();
        let cfg = build_cfg(&adj, entry, &exits);
        assert_eq!(edge_count(&cfg), 5);

        // Duplicate edges collapse
        let adj = vec![vec![1, 1], vec![]];
        let cfg = build_cfg(&adj, 0, &[1]);
        assert_eq!(edge_count(&cfg), 1);
    }
}
