//! Service for detecting cycles in call graphs using Tarjan's SCC algorithm
//!
//! Tarjan's algorithm finds Strongly Connected Components (SCCs) in O(V+E) time.

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
    pub fn detect_cycles(&self, graph: &CallGraph) -> CycleDetectionResult {
        let mut state = TarjanState::new();
        let symbol_ids: Vec<SymbolId> = graph.symbol_ids().map(|(id, _)| id.clone()).collect();

        // Initialize index for all symbols
        for id in &symbol_ids {
            if !state.node_index.contains_key(id) {
                state.index = 0;
                self.tarjanStronglyConnected(id, graph, &mut state);
            }
        }

        // Save total SCCs count before consuming the vector
        let total_sccs = state.strongly_connected_components.len();

        // Collect multi-node SCCs as cycles (with symbol names)
        let mut cycles: Vec<Cycle> = state
            .strongly_connected_components
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let names = scc
                    .iter()
                    .filter_map(|id| graph.get_symbol(id).map(|s| s.name().to_string()))
                    .collect();
                Cycle::new(scc, names)
            })
            .collect();

        // Check for self-loops (single node calling itself)
        for id in &symbol_ids {
            for callee in graph.callees(id).iter().map(|(id, _)| id) {
                if callee == id {
                    // Found a self-loop
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
        let mut ids: Vec<SymbolId> = graph.symbol_ids().map(|(id, _)| id.clone()).collect();
        ids.sort();
        let mut position_of: HashMap<&SymbolId, usize> = HashMap::with_capacity(ids.len());
        for (position, id) in ids.iter().enumerate() {
            position_of.insert(id, position);
        }

        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
        for (position, id) in ids.iter().enumerate() {
            for (callee, _) in graph.callees(id) {
                if let Some(&target) = position_of.get(&callee) {
                    successors[position].push(target);
                }
            }
        }

        cognicode_graph_algos::feedback_vertex_set(&successors, ids.len())
            .into_iter()
            .map(|position| ids[position].clone())
            .collect()
    }

    /// Tarjan's strongly connected components algorithm
    #[allow(non_snake_case)]
    fn tarjanStronglyConnected(&self, node: &SymbolId, graph: &CallGraph, state: &mut TarjanState) {
        // Set the depth index for this node
        state.node_index.insert(node.clone(), state.index);
        state.node_lowlink.insert(node.clone(), state.index);
        state.index += 1;
        state.stack.push(node.clone());
        state.in_stack.insert(node.clone());

        // Consider all successors
        for (successor, _) in graph.callees(node) {
            if !state.node_index.contains_key(&successor) {
                // Successor has not yet been visited
                self.tarjanStronglyConnected(&successor, graph, state);
                state.node_lowlink.insert(
                    node.clone(),
                    *state
                        .node_lowlink
                        .get(node)
                        .unwrap()
                        .min(state.node_lowlink.get(&successor).unwrap()),
                );
            } else if state.in_stack.contains(&successor) {
                // Successor is on the stack, so it's in the current SCC
                state.node_lowlink.insert(
                    node.clone(),
                    *state
                        .node_lowlink
                        .get(node)
                        .unwrap()
                        .min(state.node_index.get(&successor).unwrap()),
                );
            }
        }

        // If node is a root, pop the stack and generate an SCC
        if *state.node_lowlink.get(node).unwrap() == *state.node_index.get(node).unwrap() {
            let mut scc = Vec::new();
            loop {
                let w = state.stack.pop().unwrap();
                state.in_stack.remove(&w);
                scc.push(w.clone());
                if w == *node {
                    break;
                }
            }
            state.strongly_connected_components.push(scc);
        }
    }
}

impl Default for CycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal state for Tarjan's algorithm
struct TarjanState {
    index: usize,
    node_index: HashMap<SymbolId, usize>,
    node_lowlink: HashMap<SymbolId, usize>,
    stack: Vec<SymbolId>,
    in_stack: HashSet<SymbolId>,
    strongly_connected_components: Vec<Vec<SymbolId>>,
}

impl TarjanState {
    fn new() -> Self {
        Self {
            index: 0,
            node_index: HashMap::new(),
            node_lowlink: HashMap::new(),
            stack: Vec::new(),
            in_stack: HashSet::new(),
            strongly_connected_components: Vec::new(),
        }
    }
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
