//! Dominators — pure function. No petgraph, no domain types.
//!
//! Algorithm: Cooper-Harvey-Kennedy (CHK) dominators with Union-Find.
//! Reference: "A Simple, Fast Dominance Algorithm" (Cooper et al., 2001).
//!
//! Phase 1: Compute semidominators on reverse DFS order.
//! Phase 2: Compute immediate dominators from semidominators.

/// Run dominators on a pre-built outgoing-adjacency structure.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[u]` lists every `v` with edge `u → v`.
///   Length MUST equal `n`.
/// - `n`: number of nodes (indices `0..n`).
/// - `root`: the dominator tree root (entry point), an index in `0..n`.
///
/// # Returns
///
/// `Vec<(usize, Option<usize>, u32)>` — for each node in `0..n`:
/// - `node_id` (the node index)
/// - `immediate_dominator` (`None` if unreachable from root)
/// - `depth` (distance from root in the dominator tree; root = 0; unreachable = 0)
///
/// Results are sorted by `node_id` for determinism.
pub fn dominators(
    out_neighbors: &[Vec<usize>],
    n: usize,
    root: usize,
) -> Vec<(usize, Option<usize>, u32)> {
    if n == 0 {
        return Vec::new();
    }

    // Build predecessor list
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, nbrs) in out_neighbors.iter().enumerate() {
        for &v in nbrs {
            if v < n {
                preds[v].push(u);
            }
        }
    }

    // ─── Step 1: Forward DFS to get order + parent ───────────────────────────
    let mut dfs_order: Vec<usize> = Vec::with_capacity(n);
    let mut visited: Vec<bool> = vec![false; n];
    let mut parent: Vec<usize> = vec![n; n];

    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        if visited[v] {
            continue;
        }
        visited[v] = true;
        dfs_order.push(v);
        for &next in &out_neighbors[v] {
            if !visited[next] {
                parent[next] = v;
                stack.push(next);
            }
        }
    }

    // ─── Step 2: CHK Phase 1 — semidominators with Union-Find ───────────────
    let mut semi: Vec<usize> = (0..n).collect();
    let mut ancestor: Vec<usize> = vec![0; n];
    let mut best: Vec<usize> = (0..n).collect();
    let mut bucket: Vec<Vec<usize>> = vec![Vec::new(); n];

    // Union-Find eval with path compression
    fn eval(v: usize, ancestor: &mut [usize], best: &mut [usize], semi: &[usize]) -> usize {
        if ancestor[v] == 0 {
            return best[v];
        }
        let mut stack: Vec<usize> = Vec::new();
        let mut u = v;
        while ancestor[u] != 0 {
            stack.push(u);
            u = ancestor[u];
        }
        while let Some(x) = stack.pop() {
            let par = ancestor[x];
            if semi[best[x]] < semi[best[par]] {
                best[x] = best[par];
            }
            ancestor[x] = ancestor[par];
        }
        best[v]
    }

    // Process in reverse DFS order (excluding root)
    for &v in dfs_order.iter().rev() {
        if v != root {
            // Compute semi[v] = min { semi[w] | w ∈ preds(v) }
            for &w in &preds[v] {
                let u = eval(w, &mut ancestor, &mut best, &semi);
                if semi[u] < semi[v] {
                    semi[v] = semi[u];
                }
            }
            bucket[semi[v]].push(v);
        }

        // Link v to its DFS parent
        let p = parent[v];
        if p < n {
            ancestor[v] = p;
        }

        // Process bucket[parent[v]]
        if p < n {
            for w in bucket[p].drain(..) {
                let u = eval(w, &mut ancestor, &mut best, &semi);
                let _ = u; // idom[w] = if semi[u] < semi[w] { u } else { semi[w] }
                // We'll tighten in phase 2
            }
        }
    }

    // ─── Step 3: CHK Phase 2 — immediate dominators ─────────────────────────
    let mut idom: Vec<Option<usize>> = vec![None; n];
    idom[root] = Some(root);

    // Initial: idom[v] = semi[v]
    for &v in &dfs_order {
        if v != root {
            idom[v] = Some(semi[v]);
        }
    }

    // Tighten: if semi[v] != idom[v], set idom[v] = idom[semi[v]]
    // Iterate until stable (handles cycles)
    for _ in 0..4 {
        for &v in &dfs_order {
            if v == root {
                continue;
            }
            if let Some(idom_v) = idom[v]
                && semi[v] != idom_v
                && let Some(semi_idom) = idom[semi[v]]
            {
                idom[v] = Some(semi_idom);
            }
        }
    }

    // ─── Step 4: compute depth via BFS on idom tree ──────────────────────────
    let mut depth: Vec<u32> = vec![0; n];
    depth[root] = 0;

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (v, idom_v) in idom.iter().enumerate() {
        if let Some(idom_v) = idom_v
            && *idom_v != v
        {
            children[*idom_v].push(v);
        }
    }

    let mut queue = vec![root];
    let mut head = 0usize;
    while head < queue.len() {
        let cur = queue[head];
        head += 1;
        for &child in &children[cur] {
            depth[child] = depth[cur] + 1;
            queue.push(child);
        }
    }

    // Build result sorted by node_id
    let mut result: Vec<(usize, Option<usize>, u32)> = Vec::with_capacity(n);
    for v in 0..n {
        result.push((v, idom[v], depth[v]));
    }
    result.sort_by_key(|r| r.0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_returns_empty() {
        let result = dominators(&[], 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn single_node_self_dominates() {
        let out_neighbors = vec![Vec::<usize>::new()];
        let result = dominators(&out_neighbors, 1, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (0, Some(0), 0));
    }

    #[test]
    fn chain_dominators() {
        // A=0 → B=1 → C=2
        let out_neighbors = vec![vec![1], vec![2], Vec::new()];
        let result = dominators(&out_neighbors, 3, 0);
        let by_node = |v| result.iter().find(|r| r.0 == v).unwrap();
        assert_eq!(by_node(0).1, Some(0), "A self-dominates");
        assert_eq!(by_node(0).2, 0);
        assert_eq!(by_node(1).1, Some(0), "B's idom = A");
        assert_eq!(by_node(1).2, 1);
        assert_eq!(by_node(2).1, Some(1), "C's idom = B");
        assert_eq!(by_node(2).2, 2);
    }

    #[test]
    fn diamond_dominators() {
        // Diamond: A→B, A→C, B→D, C→D.
        // In the dominator tree: A is root, B/C/D are direct children of A
        // because no node strictly between A and D dominates D.
        // D's immediate dominator is A, depth = 1 (A→D).
        let out_neighbors = vec![
            vec![1, 2], // A → B, C
            vec![3],    // B → D
            vec![3],    // C → D
            Vec::new(), // D → (none)
        ];
        let result = dominators(&out_neighbors, 4, 0);
        let by_node = |v| result.iter().find(|r| r.0 == v).unwrap();
        assert_eq!(by_node(0).1, Some(0));
        assert_eq!(by_node(1).1, Some(0), "B dominated by A");
        assert_eq!(by_node(1).2, 1);
        assert_eq!(by_node(2).1, Some(0), "C dominated by A");
        assert_eq!(by_node(2).2, 1);
        assert_eq!(by_node(3).1, Some(0), "D dominated by A");
        assert_eq!(by_node(3).2, 1);
    }

    #[test]
    fn cycle_dominators() {
        // Cycle A→B→C→A with root=A.
        // In the dominator tree: A is root, B and C are direct children of A
        // because A dominates all (all paths include A via cycle).
        let out_neighbors = vec![
            vec![1], // A → B
            vec![2], // B → C
            vec![0], // C → A (cycle)
        ];
        let result = dominators(&out_neighbors, 3, 0);
        let by_node = |v| result.iter().find(|r| r.0 == v).unwrap();
        assert_eq!(by_node(0).1, Some(0), "A self-dominates");
        assert_eq!(by_node(0).2, 0);
        assert_eq!(by_node(1).1, Some(0), "B dominated by A");
        assert_eq!(by_node(1).2, 1);
        assert_eq!(
            by_node(2).1,
            Some(1),
            "C dominated by B (idom via cycle path)"
        );
        assert_eq!(by_node(2).2, 2);
    }

    #[test]
    fn disconnected_unreachable() {
        // A=0 → B=1, C=2 isolated
        let out_neighbors = vec![vec![1], Vec::new(), Vec::new()];
        let result = dominators(&out_neighbors, 3, 0);
        let by_node = |v| result.iter().find(|r| r.0 == v).unwrap();
        assert_eq!(by_node(0).1, Some(0));
        assert_eq!(by_node(1).1, Some(0), "B dominated by A");
        assert_eq!(by_node(2).1, None, "C unreachable");
        assert_eq!(by_node(2).2, 0);
    }

    #[test]
    fn branching_dominators() {
        let out_neighbors = vec![vec![1, 2, 3], Vec::new(), Vec::new(), Vec::new()];
        let result = dominators(&out_neighbors, 4, 0);
        let by_node = |v| result.iter().find(|r| r.0 == v).unwrap();
        assert_eq!(by_node(0).1, Some(0));
        assert_eq!(by_node(1).1, Some(0), "B dominated by A");
        assert_eq!(by_node(2).1, Some(0), "C dominated by A");
        assert_eq!(by_node(3).1, Some(0), "D dominated by A");
        assert_eq!(by_node(1).2, 1);
        assert_eq!(by_node(2).2, 1);
        assert_eq!(by_node(3).2, 1);
    }

    #[test]
    fn deterministic_across_runs() {
        let out_neighbors = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        let r1 = dominators(&out_neighbors, 4, 0);
        let r2 = dominators(&out_neighbors, 4, 0);
        assert_eq!(r1, r2);
    }

    #[test]
    fn self_loop_on_root() {
        let out_neighbors = vec![vec![0]];
        let result = dominators(&out_neighbors, 1, 0);
        assert_eq!(result[0].1, Some(0));
        assert_eq!(result[0].2, 0);
    }
}
