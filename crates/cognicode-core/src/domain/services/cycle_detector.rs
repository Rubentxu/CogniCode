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

    /// Finds the minimal set of symbols that need to be removed to break all cycles
    pub fn find_minimal_feedback_set(&self, graph: &CallGraph) -> Vec<SymbolId> {
        let result = self.detect_cycles(graph);
        if !result.has_cycles {
            return Vec::new();
        }

        // Use a greedy approach to find a small feedback vertex set
        // This is NP-hard in general, so we use a greedy approximation
        let mut symbols_to_consider: Vec<SymbolId> =
            graph.symbol_ids().map(|(id, _)| id.clone()).collect();

        // Sort by degree (lower degree = better candidate for removal)
        symbols_to_consider.sort_by(|a, b| {
            let deg_a = graph.callers(a).len() + graph.callees(a).len();
            let deg_b = graph.callers(b).len() + graph.callees(b).len();
            deg_a.cmp(&deg_b)
        });

        let mut result_set = Vec::new();
        let mut working_graph = graph.clone();

        // Greedily remove symbols that break cycles
        while self.detect_cycles(&working_graph).has_cycles {
            let candidates: Vec<SymbolId> = working_graph
                .symbol_ids()
                .map(|(id, _)| id.clone())
                .filter(|id| !result_set.contains(id))
                .collect();

            if candidates.is_empty() {
                break;
            }

            // Find the symbol whose removal breaks the most cycles
            let mut best_symbol = candidates[0].clone();
            let mut best_breakage = 0;

            for candidate in &candidates {
                let mut test_graph = working_graph.clone();
                test_graph.remove_symbol(candidate);
                let breakage = self.detect_cycles(&test_graph).cycles.len();
                if breakage > best_breakage {
                    best_breakage = breakage;
                    best_symbol = candidate.clone();
                }
            }

            result_set.push(best_symbol.clone());
            working_graph.remove_symbol(&best_symbol);
        }

        result_set
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
