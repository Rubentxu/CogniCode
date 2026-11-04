//! Dominators — pure function. No petgraph, no domain types.
//!
//! Algorithm: Lengauer-Tarjan (simple version, path compression, no
//! balancing). Reference: Lengauer & Tarjan, "A Fast Algorithm for Finding
//! Dominators in a Flowgraph" (1979).
//!
//! Two index spaces are in play and never mixed: *node ids* (`0..n`, the
//! caller's numbering) and *DFS numbers* (`0..reached`, preorder of the
//! depth-first spanning tree). `vertex[dfn]` and `num[node]` translate
//! between them; `semi`, `ancestor`, `label`, the buckets and the
//! intermediate `idom` all live in DFS-number space and are translated back
//! to node ids only when the result is built.

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

    // Predecessor lists, indexed by node id.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, nbrs) in out_neighbors.iter().enumerate() {
        for &v in nbrs {
            if v < n {
                preds[v].push(u);
            }
        }
    }

    // ─── Step 1: forward DFS — numbering and spanning-tree parents ──────────
    const NONE: usize = usize::MAX;
    // vertex[dfn] = node id; num[node] = dfn (NONE when unreachable).
    let mut vertex: Vec<usize> = Vec::with_capacity(n);
    let mut num: Vec<usize> = vec![NONE; n];
    let mut parent_node: Vec<usize> = vec![NONE; n];

    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        if num[v] != NONE {
            continue;
        }
        num[v] = vertex.len();
        vertex.push(v);
        for &next in &out_neighbors[v] {
            if num[next] == NONE {
                parent_node[next] = v;
                stack.push(next);
            }
        }
    }
    let reached = vertex.len();

    // par[dfn] = dfn of the tree parent (the root, dfn 0, has none).
    let par: Vec<usize> = vertex
        .iter()
        .map(|&v| {
            if parent_node[v] == NONE {
                NONE
            } else {
                num[parent_node[v]]
            }
        })
        .collect();

    // ─── Step 2: semidominators and implicit idoms, in DFS-number space ─────
    let mut semi: Vec<usize> = (0..reached).collect();
    let mut ancestor: Vec<usize> = vec![NONE; reached];
    let mut label: Vec<usize> = (0..reached).collect();
    let mut bucket: Vec<Vec<usize>> = vec![Vec::new(); reached];
    let mut idom_dfn: Vec<usize> = vec![0; reached];

    // Union-Find eval with iterative path compression.
    fn eval(v: usize, ancestor: &mut [usize], label: &mut [usize], semi: &[usize]) -> usize {
        if ancestor[v] == NONE {
            return v;
        }
        let mut chain: Vec<usize> = Vec::new();
        let mut u = v;
        while ancestor[u] != NONE && ancestor[ancestor[u]] != NONE {
            chain.push(u);
            u = ancestor[u];
        }
        while let Some(x) = chain.pop() {
            let a = ancestor[x];
            if semi[label[a]] < semi[label[x]] {
                label[x] = label[a];
            }
            ancestor[x] = ancestor[a];
        }
        label[v]
    }

    for w in (1..reached).rev() {
        // semi[w] = min over predecessors reached by the DFS.
        for &pred_node in &preds[vertex[w]] {
            let v = num[pred_node];
            if v == NONE {
                continue;
            }
            let u = eval(v, &mut ancestor, &mut label, &semi);
            if semi[u] < semi[w] {
                semi[w] = semi[u];
            }
        }
        bucket[semi[w]].push(w);
        let p = par[w];
        ancestor[w] = p;

        // Everything in the parent's bucket now has its path compressed.
        for v in std::mem::take(&mut bucket[p]) {
            let u = eval(v, &mut ancestor, &mut label, &semi);
            idom_dfn[v] = if semi[u] < semi[v] { u } else { p };
        }
    }
    // Resolve the deferred cases, in increasing DFS number.
    for w in 1..reached {
        if idom_dfn[w] != semi[w] {
            idom_dfn[w] = idom_dfn[idom_dfn[w]];
        }
    }

    // ─── Step 3: depth in the dominator tree (idom has a smaller dfn) ───────
    let mut depth_dfn: Vec<u32> = vec![0; reached];
    for w in 1..reached {
        depth_dfn[w] = depth_dfn[idom_dfn[w]] + 1;
    }

    // ─── Step 4: translate back to node ids ─────────────────────────────────
    (0..n)
        .map(|v| {
            if num[v] == NONE {
                (v, None, 0)
            } else {
                (v, Some(vertex[idom_dfn[num[v]]]), depth_dfn[num[v]])
            }
        })
        .collect()
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

    // ─── Mutation-gate and correctness tests ───────────────────────────────

    /// Runs `f` on a worker thread and fails the test if it does not return
    /// within five seconds. A mutant that stops the BFS cursor advancing
    /// loops forever; this turns that hang into a red test. The worker is
    /// not killed (std has no way), so the memory ceiling of
    /// `scripts/mem-capped-runner.sh` is what bounds a runaway allocation.
    fn within_deadline<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("dominators did not return within 5s")
    }

    #[test]
    fn edges_from_unreachable_nodes_to_ids_outside_the_graph_are_ignored() {
        // `v < n` guards the predecessor build. A reachable node with such
        // an edge panics later in the DFS, so the guard is only observable
        // from a node the root cannot reach: node 1 points at id 2 == n.
        let result = within_deadline(|| {
            let adj = vec![Vec::<usize>::new(), vec![2usize]];
            dominators(&adj, 2, 0)
        });
        assert_eq!(result, vec![(0, Some(0), 0), (1, None, 0)]);
    }

    // ─── Correctness tests (independent reference) ──────────────────────────
    //
    // Unlike the characterisation tests above, these compare `dominators`
    // with a definition-level reference that shares no code with it: `d`
    // strictly dominates `v` when `v` is reachable from the root and either
    // `d` is the root or removing `d` makes `v` unreachable. The immediate
    // dominator is the strict dominator with the most strict dominators of
    // its own, and the depth is the number of strict dominators of `v`.

    fn reachable_avoiding(adj: &[Vec<usize>], root: usize, banned: Option<usize>) -> Vec<bool> {
        let mut seen = vec![false; adj.len()];
        if banned == Some(root) {
            return seen;
        }
        let mut stack = vec![root];
        seen[root] = true;
        while let Some(u) = stack.pop() {
            for &v in &adj[u] {
                if v < adj.len() && !seen[v] && banned != Some(v) {
                    seen[v] = true;
                    stack.push(v);
                }
            }
        }
        seen
    }

    fn reference_dominators(adj: &[Vec<usize>], root: usize) -> Vec<(usize, Option<usize>, u32)> {
        let n = adj.len();
        let reach = reachable_avoiding(adj, root, None);
        let without: Vec<Vec<bool>> = (0..n)
            .map(|d| reachable_avoiding(adj, root, Some(d)))
            .collect();
        let strict = |d: usize, v: usize| d != v && reach[v] && (d == root || !without[d][v]);
        (0..n)
            .map(|v| {
                if !reach[v] {
                    return (v, None, 0);
                }
                if v == root {
                    return (v, Some(root), 0);
                }
                let sdoms: Vec<usize> = (0..n).filter(|&d| strict(d, v)).collect();
                let idom = sdoms
                    .iter()
                    .copied()
                    .max_by_key(|&d| (0..n).filter(|&x| strict(x, d)).count())
                    .expect("a reachable non-root node has the root as strict dominator");
                (v, Some(idom), sdoms.len() as u32)
            })
            .collect()
    }

    #[test]
    fn a_node_reached_through_a_higher_id_is_dominated_by_it() {
        // 0 -> 2 -> 1: every path to 1 goes through 2.
        let result = within_deadline(|| {
            let adj = vec![vec![2usize], vec![], vec![1usize]];
            dominators(&adj, 3, 0)
        });
        assert_eq!(
            result,
            vec![(0, Some(0), 0), (1, Some(2), 2), (2, Some(0), 1)]
        );
    }

    #[test]
    fn every_small_graph_matches_the_set_based_reference() {
        let mismatches = within_deadline(|| {
            let mut bad: Vec<String> = Vec::new();
            let mut cases = 0usize;
            for n in 1..=4usize {
                let pairs: Vec<(usize, usize)> =
                    (0..n).flat_map(|u| (0..n).map(move |v| (u, v))).collect();
                for mask in 0u32..(1u32 << pairs.len()) {
                    let mut adj = vec![Vec::new(); n];
                    for (i, &(u, v)) in pairs.iter().enumerate() {
                        if (mask >> i) & 1 == 1 {
                            adj[u].push(v);
                        }
                    }
                    for root in 0..n {
                        cases += 1;
                        if dominators(&adj, n, root) != reference_dominators(&adj, root)
                            && bad.len() < 3
                        {
                            bad.push(format!("n={n} root={root} adj={adj:?}"));
                        }
                    }
                }
            }
            (cases, bad)
        });
        assert_eq!(mismatches.0, 263_714);
        assert!(
            mismatches.1.is_empty(),
            "dominators disagrees with the reference, first cases: {:?}",
            mismatches.1
        );
    }
}
