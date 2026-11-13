//! Service for detecting cycles in call graphs.
//!
//! The strongly connected components are computed by
//! [`cognicode_graph_algos::condensation`], the one implementation of the
//! algorithm: iterative, so the depth of a call chain is bounded by the heap
//! and not by the stack of the thread that asks, and mutation-tested in its
//! own crate. This service decides what a *cycle* is for the product — a
//! component of more than one symbol, or a symbol that calls itself — and
//! how the answer is ordered.

use std::collections::{HashMap, HashSet};

use crate::domain::aggregates::{CallGraph, SymbolId};

/// Service for detecting cycles in call graphs
pub struct CycleDetector;

impl CycleDetector {
    /// Creates a new CycleDetector
    pub fn new() -> Self {
        Self
    }

    /// Detects all cycles in the call graph and returns the strongly connected components
    ///
    /// The answer does not depend on the order the graph happens to hold
    /// its symbols in: the traversal starts at the smallest symbol id and
    /// visits the successors of each node in the order the graph
    /// publishes them, which is symbol id order, so the same graph always
    /// publishes its cycles in the same order. The members of a cycle are
    /// listed in symbol id order too.
    pub fn detect_cycles(&self, graph: &CallGraph) -> CycleDetectionResult {
        let (symbol_ids, successors) = numbered_successors(graph);

        let components = cognicode_graph_algos::condensation(&successors, symbol_ids.len());
        let total_sccs = components.len();

        // Multi-node components are cycles (with symbol names).
        let mut cycles: Vec<Cycle> = components
            .into_iter()
            .filter(|component| component.len() > 1)
            .map(|component| {
                let ids: Vec<SymbolId> = component
                    .into_iter()
                    .map(|position| symbol_ids[position].clone())
                    .collect();
                let names = ids
                    .iter()
                    .filter_map(|id| graph.get_symbol(id).map(|s| s.name().to_string()))
                    .collect();
                Cycle::new(ids, names)
            })
            .collect();

        // A symbol that calls itself is a cycle of one.
        for (position, callees) in successors.iter().enumerate() {
            for &callee in callees {
                if callee == position {
                    let id = &symbol_ids[position];
                    let name = graph
                        .get_symbol(id)
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| id.as_str().to_string());
                    cycles.push(Cycle::new(vec![id.clone()], vec![name]));
                }
            }
        }

        let has_cycles = !cycles.is_empty();

        CycleDetectionResult {
            has_cycles,
            cycles,
            total_sccs,
        }
    }

    /// Checks if removing a symbol would break any cycles
    pub fn would_break_cycles(&self, graph: &CallGraph, symbol_id: &SymbolId) -> bool {
        let result = self.detect_cycles(graph);
        result.cycles.iter().any(|cycle| cycle.contains(symbol_id))
    }

    /// Finds a set of symbols whose removal breaks every cycle in the graph.
    ///
    /// The minimum feedback vertex set is NP-hard, so this is a greedy
    /// approximation and not a minimum: the returned set always breaks
    /// every cycle the detector reports, but its size is not guaranteed
    /// to be the smallest possible one. The name is the published one.
    ///
    /// The answer is deterministic: the same graph always yields the same
    /// set, whatever order its symbols were added in.
    ///
    /// Cost: `O(F · (V + E))`, where `F` is the size of the set. Every
    /// symbol of the set is taken out of a cycle, so the search ends.
    pub fn find_minimal_feedback_set(&self, graph: &CallGraph) -> Vec<SymbolId> {
        // The symbols are numbered by their identifier, so that the
        // answer does not depend on the order the graph happens to hold
        // them in: the tie between two symbols that break the same
        // number of cycles is the smaller identifier.
        let (ids, successors) = numbered_successors(graph);

        cognicode_graph_algos::feedback_vertex_set(&successors, ids.len())
            .into_iter()
            .map(|position| ids[position].clone())
            .collect()
    }
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// The symbols of `graph` numbered by their identifier, and the successors
/// of each one as positions in that numbering.
///
/// The graph holds its symbols in a hash map, whose order is different for
/// every instance; numbering them by identifier is what makes every answer
/// built on the positions the same for the same graph. The successors of a
/// symbol come in the order the graph publishes them, which is identifier
/// order.
fn numbered_successors(graph: &CallGraph) -> (Vec<SymbolId>, Vec<Vec<usize>>) {
    let mut ids: Vec<SymbolId> = graph.symbol_ids().map(|(id, _)| id.clone()).collect();
    ids.sort();
    let position_of: HashMap<&SymbolId, usize> = ids
        .iter()
        .enumerate()
        .map(|(position, id)| (id, position))
        .collect();

    let successors = ids
        .iter()
        .map(|id| {
            graph
                .callees(id)
                .into_iter()
                .filter_map(|(callee, _)| position_of.get(&callee).copied())
                .collect()
        })
        .collect();
    (ids, successors)
}

/// Result of cycle detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleDetectionResult {
    /// Whether any cycles were found
    pub has_cycles: bool,
    /// The cycles (SCCs with more than one node)
    pub cycles: Vec<Cycle>,
    /// Total number of strongly connected components found
    pub total_sccs: usize,
}

impl CycleDetectionResult {
    /// Returns the total number of symbols involved in cycles
    pub fn symbols_in_cycles(&self) -> usize {
        let mut unique: HashSet<SymbolId> = HashSet::new();
        for cycle in &self.cycles {
            for symbol in cycle.symbols() {
                unique.insert(symbol.clone());
            }
        }
        unique.len()
    }

    /// Returns all symbols involved in cycles
    pub fn all_cycle_symbols(&self) -> HashSet<SymbolId> {
        let mut symbols: HashSet<SymbolId> = HashSet::new();
        for cycle in &self.cycles {
            for symbol in cycle.symbols() {
                symbols.insert(symbol.clone());
            }
        }
        symbols
    }
}

/// Represents a cycle (strongly connected component with more than one node)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle {
    /// Symbol IDs in this cycle (for contains checks)
    symbol_ids: Vec<SymbolId>,
    /// Human-readable names for path display
    symbol_names: Vec<String>,
}

impl Cycle {
    /// Creates a new cycle with the given symbols and their names
    pub fn new(symbol_ids: Vec<SymbolId>, symbol_names: Vec<String>) -> Self {
        debug_assert_eq!(symbol_ids.len(), symbol_names.len());
        Self {
            symbol_ids,
            symbol_names,
        }
    }

    /// Returns the symbols in this cycle
    pub fn symbols(&self) -> &[SymbolId] {
        &self.symbol_ids
    }

    /// Returns the length of this cycle (number of symbols)
    pub fn length(&self) -> usize {
        self.symbol_ids.len()
    }

    /// Returns true if this cycle contains the given symbol
    pub fn contains(&self, symbol_id: &SymbolId) -> bool {
        self.symbol_ids.contains(symbol_id)
    }

    /// Returns a description of the cycle as a path
    pub fn as_path(&self) -> String {
        self.symbol_names.join(" -> ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::{CallGraph, Symbol};
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    /// The cycles are published in a deterministic order.
    ///
    /// The traversal starts at the smallest symbol id and visits each
    /// node's callees in symbol id order, so the published list does not
    /// depend on the order the graph happens to hold its symbols and
    /// edges in — which is the iteration order of a hash map, different
    /// for every instance of the graph.
    ///
    /// The witness is TEN disjoint cycles: the published order is the
    /// order the traversal finds them in, which starts at the smallest
    /// symbol id. Ten, and not three, because a hash map could hand back
    /// the sorted order by chance — with ten cycles that chance is one in
    /// three million, so a red here means the defect and not the luck.
    #[test]
    fn the_cycles_are_published_in_a_deterministic_order() {
        const CYCLES: usize = 10;

        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        // The symbols are added in an order that is NOT the sorted one,
        // so the published order cannot come from the order of the
        // additions either.
        let mut names: Vec<String> = (0..CYCLES)
            .flat_map(|cycle| [format!("c{cycle:02}a"), format!("c{cycle:02}b")])
            .collect();
        names.sort();
        names.reverse();
        for name in &names {
            graph.add_symbol(Symbol::new(
                name,
                SymbolKind::Function,
                Location::new("test.rs", 1, 1),
            ));
        }

        for cycle in 0..CYCLES {
            let from = SymbolId::new(format!("test.rs:c{cycle:02}a:1"));
            let to = SymbolId::new(format!("test.rs:c{cycle:02}b:1"));
            graph
                .add_dependency(&from, &to, DependencyType::Calls)
                .expect("the dependency is added");
            graph
                .add_dependency(&to, &from, DependencyType::Calls)
                .expect("the dependency is added");
        }

        let result = detector.detect_cycles(&graph);

        let published: Vec<String> = result
            .cycles
            .iter()
            .map(|cycle| {
                let mut members: Vec<String> = cycle
                    .symbols()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect();
                members.sort();
                members.join(",")
            })
            .collect();

        let expected: Vec<String> = (0..CYCLES)
            .map(|cycle| format!("test.rs:c{cycle:02}a:1,test.rs:c{cycle:02}b:1"))
            .collect();

        assert_eq!(
            published, expected,
            "the cycles are published in the order the traversal finds them, starting at the smallest symbol id"
        );
    }

    #[test]
    fn test_no_cycles() {
        let detector = CycleDetector::new();
        let graph = CallGraph::new();

        let result = detector.detect_cycles(&graph);
        assert!(!result.has_cycles);
        assert!(result.cycles.is_empty());
    }

    #[test]
    fn test_simple_linear_no_cycle() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        // a -> b -> c (no cycle)
        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();

        let result = detector.detect_cycles(&graph);
        assert!(!result.has_cycles);
    }

    #[test]
    fn test_simple_cycle() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        // a -> b -> a (cycle)
        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_a, DependencyType::Calls)
            .unwrap();

        let result = detector.detect_cycles(&graph);
        assert!(result.has_cycles);
        assert_eq!(result.cycles.len(), 1);
        assert_eq!(result.cycles[0].length(), 2);
    }

    #[test]
    fn test_self_loop() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let id_a = graph.add_symbol(a);

        // a -> a (self loop)
        graph
            .add_dependency(&id_a, &id_a, DependencyType::Calls)
            .unwrap();

        let result = detector.detect_cycles(&graph);
        assert!(result.has_cycles);
    }

    #[test]
    fn test_would_break_cycles() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_a, DependencyType::Calls)
            .unwrap();

        assert!(detector.would_break_cycles(&graph, &id_a));
        assert!(detector.would_break_cycles(&graph, &id_b));
    }

    /// A graph with two cycles and one symbol that belongs to none.
    ///
    /// `a` and `b` call each other, `d` calls itself, and `c` calls
    /// nobody: removing `c` breaks nothing, so a set that holds it is
    /// holding a symbol for no reason.
    fn graph_with_two_cycles() -> (CallGraph, SymbolId, SymbolId, SymbolId, SymbolId) {
        let mut graph = CallGraph::new();

        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        ));
        let c = graph.add_symbol(Symbol::new(
            "c",
            SymbolKind::Function,
            Location::new("test.rs", 3, 1),
        ));
        let d = graph.add_symbol(Symbol::new(
            "d",
            SymbolKind::Function,
            Location::new("test.rs", 4, 1),
        ));

        graph.add_dependency(&a, &b, DependencyType::Calls).unwrap();
        graph.add_dependency(&b, &a, DependencyType::Calls).unwrap();
        graph.add_dependency(&d, &d, DependencyType::Calls).unwrap();

        (graph, a, b, c, d)
    }

    #[test]
    fn the_feedback_set_breaks_every_cycle() {
        let detector = CycleDetector::new();
        let (graph, _, _, _, _) = graph_with_two_cycles();

        let chosen = detector.find_minimal_feedback_set(&graph);

        let mut without = graph.clone();
        for symbol in &chosen {
            without.remove_symbol(symbol);
        }
        assert!(
            !detector.detect_cycles(&without).has_cycles,
            "the set leaves a cycle alive: {chosen:?}"
        );
    }

    #[test]
    fn the_feedback_set_holds_only_the_symbols_that_break_a_cycle() {
        let detector = CycleDetector::new();
        let (graph, _, _, harmless, _) = graph_with_two_cycles();

        let chosen = detector.find_minimal_feedback_set(&graph);

        assert!(
            !chosen.contains(&harmless),
            "the set holds a symbol that breaks no cycle: {chosen:?}"
        );
        assert_eq!(
            chosen.len(),
            2,
            "one symbol breaks the pair and one breaks the self-loop: {chosen:?}"
        );
    }

    #[test]
    fn the_feedback_set_does_not_depend_on_the_order_of_the_symbols() {
        let detector = CycleDetector::new();
        let (graph, _, _, _, _) = graph_with_two_cycles();

        // The same graph, built the other way round. The symbols of a
        // graph live in a hash map whose order depends on the order they
        // were added, and the answer must not: the same graph is the same
        // question, and it has the same answer every time it is asked.
        let mut reversed = CallGraph::new();
        let d = reversed.add_symbol(Symbol::new(
            "d",
            SymbolKind::Function,
            Location::new("test.rs", 4, 1),
        ));
        let c = reversed.add_symbol(Symbol::new(
            "c",
            SymbolKind::Function,
            Location::new("test.rs", 3, 1),
        ));
        let b = reversed.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        ));
        let a = reversed.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        ));
        reversed
            .add_dependency(&a, &b, DependencyType::Calls)
            .unwrap();
        reversed
            .add_dependency(&b, &a, DependencyType::Calls)
            .unwrap();
        reversed
            .add_dependency(&d, &d, DependencyType::Calls)
            .unwrap();

        let mut first = detector.find_minimal_feedback_set(&graph);
        let mut second = detector.find_minimal_feedback_set(&reversed);
        first.sort();
        second.sort();

        assert_eq!(
            first, second,
            "the same graph got two different answers: {first:?} and {second:?}"
        );
        assert!(
            !first.contains(&c),
            "the set holds a symbol that breaks no cycle: {first:?}"
        );
    }

    #[test]
    fn a_graph_without_cycles_has_no_feedback_set() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        ));
        graph.add_dependency(&a, &b, DependencyType::Calls).unwrap();

        assert!(detector.find_minimal_feedback_set(&graph).is_empty());
    }

    /// A call chain as deep as a large project can have.
    ///
    /// The answer must not depend on the stack of the thread that asks:
    /// a traversal that recurses once per call-chain link dies with a stack
    /// overflow — an abort of the whole process, not an error — on the
    /// project that needs the answer most. The witness runs on a thread with
    /// a deliberately small stack, so it measures the traversal and not the
    /// default the test harness happens to give.
    #[test]
    fn a_call_chain_deeper_than_the_stack_still_answers() {
        const LINKS: usize = 20_000;
        const SMALL_STACK: usize = 256 * 1024;

        let answer = std::thread::Builder::new()
            .stack_size(SMALL_STACK)
            .spawn(|| {
                let mut graph = CallGraph::new();
                let ids: Vec<SymbolId> = (0..LINKS)
                    .map(|link| {
                        graph.add_symbol(Symbol::new(
                            format!("f{link:05}"),
                            SymbolKind::Function,
                            Location::new("chain.rs", 1, 1),
                        ))
                    })
                    .collect();
                for pair in ids.windows(2) {
                    graph
                        .add_dependency(&pair[0], &pair[1], DependencyType::Calls)
                        .expect("the dependency is added");
                }
                // The last link calls the first: one cycle of every link.
                graph
                    .add_dependency(&ids[LINKS - 1], &ids[0], DependencyType::Calls)
                    .expect("the dependency is added");

                let result = CycleDetector::new().detect_cycles(&graph);
                (
                    result.has_cycles,
                    result.cycles.len(),
                    result.cycles.first().map(Cycle::length),
                    result.total_sccs,
                )
            })
            .expect("the thread starts")
            .join()
            .expect("the detector answers instead of overflowing the stack");

        assert_eq!(answer, (true, 1, Some(LINKS), 1));
    }

    #[test]
    fn total_sccs_counts_every_component_the_singletons_too() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();
        let mut add = |name: &str| {
            graph.add_symbol(Symbol::new(
                name,
                SymbolKind::Function,
                Location::new("test.rs", 1, 1),
            ))
        };
        let a = add("a");
        let b = add("b");
        let c = add("c");
        graph.add_dependency(&a, &b, DependencyType::Calls).unwrap();
        graph.add_dependency(&b, &a, DependencyType::Calls).unwrap();
        graph.add_dependency(&b, &c, DependencyType::Calls).unwrap();

        let result = detector.detect_cycles(&graph);

        assert_eq!(result.cycles.len(), 1, "{{a, b}} is the only cycle");
        assert_eq!(result.total_sccs, 2, "{{a, b}} and {{c}}");
    }

    #[test]
    fn the_members_of_a_cycle_come_in_symbol_id_order() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();
        // Added out of order, and the edges close the cycle backwards.
        let c = graph.add_symbol(Symbol::new(
            "c",
            SymbolKind::Function,
            Location::new("test.rs", 3, 1),
        ));
        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        ));
        graph.add_dependency(&c, &b, DependencyType::Calls).unwrap();
        graph.add_dependency(&b, &a, DependencyType::Calls).unwrap();
        graph.add_dependency(&a, &c, DependencyType::Calls).unwrap();

        let result = detector.detect_cycles(&graph);

        let members: Vec<&str> = result.cycles[0]
            .symbols()
            .iter()
            .map(SymbolId::as_str)
            .collect();
        assert_eq!(members, ["test.rs:a:1", "test.rs:b:2", "test.rs:c:3"]);
        assert_eq!(result.cycles[0].as_path(), "a -> b -> c");
    }

    #[test]
    fn test_cycle_as_path() {
        let detector = CycleDetector::new();
        let mut graph = CallGraph::new();

        let a = Symbol::new(
            "func_a",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let b = Symbol::new(
            "func_b",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        );

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_a, DependencyType::Calls)
            .unwrap();

        let result = detector.detect_cycles(&graph);
        let path = result.cycles[0].as_path();
        assert!(path.contains("func_a"));
        assert!(path.contains("func_b"));
    }
}
