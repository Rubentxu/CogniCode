//! Feedback Vertex Set — greedy heuristic.
//!
//! The set of nodes whose removal leaves the graph without cycles. The
//! smallest such set is NP-hard, so this is a greedy approximation and
//! not a minimum: what it returns always breaks every cycle, and it is
//! the same set for the same graph every time.
//!
//! The graph is walked as indices: `out_neighbors[v]` lists the nodes
//! `v` calls.

use crate::algorithms::condensation::condensation;

/// Compute a feedback vertex set (greedy heuristic).
///
/// Every pass takes one node out of a cycle, so the search ends after at
/// most `n` passes and the bound makes termination structural.
///
/// # Arguments
///
/// - `out_neighbors`: `out_neighbors[u]` lists every `v` with edge `u → v`.
///   Length MUST equal `n`.
/// - `n`: number of nodes.
///
/// # Returns
///
/// `Vec<usize>` — the nodes to remove, in the order they were chosen.
/// Empty if the graph has no cycles.
///
/// # Edge cases
///
/// - Empty graph: empty vec
/// - Single node: empty vec
/// - DAG (no cycles): empty vec
pub fn feedback_vertex_set(out_neighbors: &[Vec<usize>], n: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }

    // The live graph. A node that is taken out of it loses its edges:
    // the cycles are found on the graph as it is, not on the graph as it
    // was.
    let mut successors = out_neighbors.to_vec();
    let mut result: Vec<usize> = Vec::new();

    for _ in 0..n {
        let cycles = cycles_of(&successors, n);
        if cycles.is_empty() {
            break;
        }

        // How many cycles does each node take part in? Only the nodes of
        // a cycle can break it, so they are the candidates.
        let mut participation = vec![0usize; n];
        for cycle in &cycles {
            for &node in cycle {
                participation[node] += 1;
            }
        }

        // The callers are read off the live graph on every pass: a node
        // that is taken out of it is no longer calling anybody, and a
        // list kept up to date by hand would have to be right about that
        // too.
        let callers = callers_of(&successors, n);

        // The node that takes part in the most cycles is the one that
        // breaks the most of them. The ties go to the node of the
        // smaller degree —it cuts fewer edges on the way— and then to
        // the smaller index, so the answer does not depend on the order
        // the graph happens to be walked in.
        let mut best: Option<usize> = None;
        for node in 0..n {
            if participation[node] == 0 {
                continue;
            }
            let degree = successors[node].len() + callers[node].len();
            best = Some(match best {
                None => node,
                Some(incumbent) => {
                    let incumbent_degree = successors[incumbent].len() + callers[incumbent].len();
                    if participation[node] > participation[incumbent]
                        || (participation[node] == participation[incumbent]
                            && degree < incumbent_degree)
                    {
                        node
                    } else {
                        incumbent
                    }
                }
            });
        }

        let Some(chosen) = best else {
            break;
        };

        result.push(chosen);
        take_out(&mut successors, chosen);
    }

    result
}

/// The cycles of the graph: the groups of two or more nodes that reach
/// each other, and the nodes that call themselves.
fn cycles_of(successors: &[Vec<usize>], n: usize) -> Vec<Vec<usize>> {
    let mut cycles: Vec<Vec<usize>> = condensation(successors, n)
        .into_iter()
        .filter(|group| group.len() > 1)
        .collect();
    for (node, callees) in successors.iter().enumerate().take(n) {
        for &callee in callees {
            if callee == node {
                cycles.push(vec![node]);
            }
        }
    }
    cycles
}

/// The nodes that call each node.
fn callers_of(successors: &[Vec<usize>], n: usize) -> Vec<Vec<usize>> {
    let mut callers = vec![Vec::new(); n];
    for (source, callees) in successors.iter().enumerate().take(n) {
        for &callee in callees {
            callers[callee].push(source);
        }
    }
    callers
}

/// Takes a node out of the live graph: its edges go with it.
///
/// The node stops being a callee —every list loses it— and stops being
/// a caller: its own list is emptied. The callers of the remaining
/// nodes are read off the graph again on the next pass, so nothing
/// here has to remember who called what.
fn take_out(successors: &mut [Vec<usize>], node: usize) {
    for list in successors.iter_mut() {
        list.retain(|&target| target != node);
    }
    if let Some(list) = successors.get_mut(node) {
        list.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The graph of `edges`, with `n` nodes.
    fn graph(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut successors = vec![Vec::new(); n];
        for (source, target) in edges {
            successors[*source].push(*target);
        }
        successors
    }

    #[test]
    fn a_graph_without_cycles_has_no_feedback_set() {
        let successors = graph(3, &[(0, 1), (1, 2)]);

        assert!(feedback_vertex_set(&successors, 3).is_empty());
    }

    #[test]
    fn an_empty_graph_has_no_feedback_set() {
        assert!(feedback_vertex_set(&[], 0).is_empty());
    }

    #[test]
    fn a_cycle_is_broken_by_one_of_its_nodes() {
        let successors = graph(2, &[(0, 1), (1, 0)]);

        // Both nodes break the cycle, and the tie goes to the smaller
        // index: the same graph gets the same answer every time.
        assert_eq!(feedback_vertex_set(&successors, 2), vec![0]);
    }

    #[test]
    fn a_node_that_calls_itself_is_broken_by_itself() {
        let successors = graph(1, &[(0, 0)]);

        assert_eq!(feedback_vertex_set(&successors, 1), vec![0]);
    }

    #[test]
    fn the_node_that_breaks_the_most_cycles_is_chosen_first() {
        // 0 and 1 reach each other, and 1 also calls itself: 1 takes part
        // in two cycles, 0 and 2 in one each.
        let successors = graph(4, &[(0, 1), (1, 0), (1, 1), (2, 2)]);

        let chosen = feedback_vertex_set(&successors, 4);

        assert_eq!(
            chosen.first(),
            Some(&1),
            "the node that breaks two cycles comes first: {chosen:?}"
        );
    }

    #[test]
    fn the_set_holds_no_node_that_breaks_nothing() {
        // 3 calls 0 and is called by nobody: it is in no cycle.
        let successors = graph(4, &[(0, 1), (1, 0), (3, 0)]);

        let chosen = feedback_vertex_set(&successors, 4);

        assert!(
            !chosen.contains(&3),
            "the set holds a node that breaks no cycle: {chosen:?}"
        );
    }

    #[test]
    fn two_cycles_are_broken_by_two_nodes() {
        let successors = graph(4, &[(0, 1), (1, 0), (2, 3), (3, 2)]);

        assert_eq!(feedback_vertex_set(&successors, 4), vec![0, 2]);
    }

    #[test]
    fn every_cycle_is_broken_even_when_there_are_many_of_them() {
        // Three cycles that share nothing: the search takes one pass per
        // cycle, and it does not stop before the last one.
        let successors = graph(3, &[(0, 0), (1, 1), (2, 2)]);

        assert_eq!(feedback_vertex_set(&successors, 3), vec![0, 1, 2]);
    }

    #[test]
    fn the_edges_of_a_removed_node_go_with_it() {
        // 0, 1 and 2 reach each other. Taking 2 out leaves the cycle
        // 0 ↔ 1, which the next pass breaks.
        let successors = graph(3, &[(0, 1), (1, 0), (1, 2), (2, 0)]);

        let chosen = feedback_vertex_set(&successors, 3);

        assert_eq!(chosen, vec![2, 0]);
    }

    #[test]
    fn the_node_of_the_smaller_degree_is_chosen_first() {
        // 0 is called by 1 and 2 and calls 1: three edges. 1 calls and is
        // called by 0: two. The cycle is broken where it costs least.
        let successors = graph(3, &[(0, 1), (1, 0), (2, 0)]);

        assert_eq!(feedback_vertex_set(&successors, 3), vec![1]);
    }

    #[test]
    fn the_answer_is_the_same_every_time() {
        let successors = graph(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)]);

        let first = feedback_vertex_set(&successors, 5);
        let second = feedback_vertex_set(&successors, 5);

        assert_eq!(first, second);
    }
}
