//! The longest chain of calls.
//!
//! A call graph has cycles: two functions that call each other have no
//! longest path, because the chain can be walked for ever. The depth of
//! the calls is therefore measured over the **condensation** of the
//! graph — the graph whose nodes are the groups of mutually reachable
//! symbols. A group is entered once, and its symbols are counted once
//! each.
//!
//! The result is the number of symbols in the longest chain: how many
//! functions a call goes through before it comes back to one it had
//! already called.
//!
//! The computation is linear in the size of the graph: Tarjan's
//! condensation ([`condensation`]) plus a longest path over the DAG of
//! that condensation, computed in topological order.

use super::condensation::condensation;

/// The number of symbols in the longest chain of calls.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[u]` lists every `v` with edge `u → v`.
///   Length MUST equal `n`, and every index MUST be smaller than `n`.
/// - `n`: number of nodes.
///
/// # Returns
///
/// The length, in symbols, of the longest chain of calls; `0` for an
/// empty graph.
///
/// # Edge cases
///
/// - Empty graph: `0`.
/// - A graph with no edges: `1` — the chain is a single symbol.
/// - A cycle: the size of the group, because a cycle is entered once.
/// - A group of mutually recursive symbols reached from a chain: the
///   chain plus the symbols of the group, each counted once.
pub fn longest_chain(out_neighbors: &[Vec<usize>], n: usize) -> usize {
    if n == 0 {
        return 0;
    }

    let groups = condensation(out_neighbors, n);

    // Which group every node belongs to.
    let mut group_of = vec![0usize; n];
    for (id, group) in groups.iter().enumerate() {
        for &node in group {
            group_of[node] = id;
        }
    }

    // The edges between groups. A group that calls itself is not an edge:
    // the group is one node of the condensation.
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
    for (source, callees) in out_neighbors.iter().enumerate() {
        let from = group_of[source];
        for &callee in callees {
            let to = group_of[callee];
            if from != to {
                adjacency[from].push(to);
            }
        }
    }
    for callees in &mut adjacency {
        callees.sort_unstable();
        callees.dedup();
    }

    // The topological order of the condensation (Kahn's algorithm). The
    // condensation is a DAG by construction, so every group ends up in
    // the order.
    //
    // Every group enters the queue and the order **once**. That is what
    // the two flags say, and it is what bounds the memory of the
    // calculation by the size of the graph: a traversal that did not
    // advance could not accumulate without bound.
    let mut incoming = vec![0usize; groups.len()];
    for callees in &adjacency {
        for &to in callees {
            incoming[to] += 1;
        }
    }
    let mut queued = vec![false; groups.len()];
    let mut queue: Vec<usize> = Vec::with_capacity(groups.len());
    for id in 0..groups.len() {
        if incoming[id] == 0 {
            queued[id] = true;
            queue.push(id);
        }
    }
    let mut processed = vec![false; groups.len()];
    let mut order: Vec<usize> = Vec::with_capacity(groups.len());
    let mut head = 0;
    while head < queue.len() {
        let current = queue[head];
        head += 1;
        if processed[current] {
            continue;
        }
        processed[current] = true;
        order.push(current);
        for &to in &adjacency[current] {
            incoming[to] -= 1;
            if incoming[to] == 0 && !queued[to] {
                queued[to] = true;
                queue.push(to);
            }
        }
    }

    // The longest chain, from the last group backwards: the chain of a
    // group is the group itself plus the longest chain that starts at one
    // of the groups it calls.
    let mut chain: Vec<usize> = groups.iter().map(|group| group.len()).collect();
    for &current in order.iter().rev() {
        let mut continuation = 0;
        for &to in &adjacency[current] {
            continuation = continuation.max(chain[to]);
        }
        chain[current] += continuation;
    }

    chain.into_iter().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `longest_chain` on a worker thread and fail the test if it does
    /// not finish.
    ///
    /// The mutation gate can turn the traversal into one that never
    /// terminates. A test that called the function directly would hang the
    /// whole suite rather than fail it, and cargo-mutants scores a hang as
    /// a timeout instead of as a caught mutant. The deadline turns the
    /// hang into a failure.
    ///
    /// The unmutated code returns in microseconds, so the deadline is
    /// never reached in a green run.
    fn longest_chain_within(out_neighbors: &[Vec<usize>], n: usize) -> usize {
        let out_neighbors = out_neighbors.to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(longest_chain(&out_neighbors, n));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("longest_chain did not terminate")
    }

    #[test]
    fn an_empty_graph_has_no_chain() {
        assert_eq!(longest_chain_within(&[], 0), 0);
    }

    #[test]
    fn a_lone_symbol_is_a_chain_of_one() {
        assert_eq!(longest_chain_within(&[Vec::new()], 1), 1);
    }

    #[test]
    fn a_chain_counts_every_symbol() {
        // 0 → 1 → 2 → 3
        let graph = vec![vec![1], vec![2], vec![3], Vec::new()];
        assert_eq!(longest_chain_within(&graph, 4), 4);
    }

    #[test]
    fn the_longest_of_two_chains_wins() {
        // 0 → 1 → 2 and 0 → 3
        let graph = vec![vec![1, 3], vec![2], Vec::new(), Vec::new()];
        assert_eq!(longest_chain_within(&graph, 4), 3);
    }

    #[test]
    fn a_diamond_counts_the_longest_arm() {
        // 0 → 1 → 3 and 0 → 2 → 3
        let graph = vec![vec![1, 2], vec![3], vec![3], Vec::new()];
        assert_eq!(longest_chain_within(&graph, 4), 3);
    }

    #[test]
    fn a_cycle_counts_its_symbols_once() {
        // 0 → 1 → 2 → 0: the three symbols call each other, so the chain
        // is the group itself.
        let graph = vec![vec![1], vec![2], vec![0]];
        assert_eq!(longest_chain_within(&graph, 3), 3);
    }

    #[test]
    fn a_chain_into_a_cycle_counts_both() {
        // 0 → 1, 1 → 2, 2 → 1: the chain is 0 plus the group {1, 2}.
        let graph = vec![vec![1], vec![2], vec![1]];
        assert_eq!(longest_chain_within(&graph, 3), 3);
    }

    #[test]
    fn a_chain_that_goes_through_a_group_counts_the_group_and_what_it_calls() {
        // 0 → 1, 1 → 2, 2 → 1, 2 → 3: the chain is 0, the group {1, 2},
        // and 3.
        let graph = vec![vec![1], vec![2], vec![1, 3], Vec::new()];
        assert_eq!(longest_chain_within(&graph, 4), 4);
    }

    #[test]
    fn disconnected_chains_are_measured_separately() {
        // 0 → 1 and 2 → 3
        let graph = vec![vec![1], Vec::new(), vec![3], Vec::new()];
        assert_eq!(longest_chain_within(&graph, 4), 2);
    }

    #[test]
    fn the_chain_counts_a_path_whose_middle_node_has_two_callers() {
        // 0 → 3, 1 → 2, 2 → 3, 3 → 4, 3 → 5. The longest path is
        // 1 → 2 → 3 → 4: four symbols.
        //
        // Node 3 is called by two symbols, so it only becomes ready
        // when the second of them is measured. A traversal that takes
        // the group as soon as the FIRST caller reaches it measures 2
        // before 3, and the chain stops at three symbols instead of
        // four.
        let graph = vec![
            vec![3],
            vec![2],
            vec![3],
            vec![4, 5],
            Vec::new(),
            Vec::new(),
        ];
        assert_eq!(longest_chain_within(&graph, 6), 4);
    }

    #[test]
    fn a_self_loop_is_a_chain_of_one() {
        assert_eq!(longest_chain_within(&[vec![0]], 1), 1);
    }

    #[test]
    fn the_numbering_of_the_nodes_does_not_change_the_chain() {
        // The same chain, with the nodes numbered the other way round.
        let graph = vec![Vec::new(), vec![2], vec![0]];
        assert_eq!(longest_chain_within(&graph, 3), 3);
    }
}
