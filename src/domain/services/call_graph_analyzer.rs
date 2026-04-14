//! Service for analyzing call graphs with advanced metrics
//!
//! Provides hot path detection, complexity analysis, and entry/leaf function analysis.

use crate::domain::aggregates::{CallEntry, CallGraph, SymbolId};

/// Service for analyzing call graphs
pub struct CallGraphAnalyzer;

impl CallGraphAnalyzer {
    /// Creates a new CallGraphAnalyzer
    pub fn new() -> Self {
        Self
    }

    /// Finds hot paths in the call graph
    ///
    /// A hot path is a sequence of function calls that is frequently executed.
    /// This implementation identifies functions with high fan-in as potential hot spots.
    ///
    /// Returns a vector of HotPath entries sorted by significance (highest first).
    pub fn find_hot_paths(&self, graph: &CallGraph, limit: usize) -> Vec<HotPath> {
        let mut fan_in_scores: Vec<(SymbolId, usize)> = graph
            .symbols()
            .map(|s| {
                let id = SymbolId::new(s.fully_qualified_name());
                (id.clone(), graph.fan_in(&id))
            })
            .collect();

        // Sort by fan-in descending
        fan_in_scores.sort_by(|a, b| b.1.cmp(&a.1));

        fan_in_scores
            .into_iter()
            .take(limit)
            .filter(|(_, score)| *score > 0)
            .filter_map(|(id, score)| {
                graph.get_symbol(&id).map(|symbol| HotPath {
                    symbol_id: id.clone(),
                    symbol_name: symbol.name().to_string(),
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line(),
                    fan_in: score,
                    fan_out: graph.fan_out(&id),
                })
            })
            .collect()
    }

    /// Calculates complexity metrics for the call graph
    ///
    /// Returns a CallGraphComplexityReport with various complexity indicators.
    pub fn calculate_complexity(&self, graph: &CallGraph) -> CallGraphComplexityReport {
        let total_symbols = graph.symbol_count();
        let total_edges = graph.edge_count();

        // Calculate cyclomatic complexity approximation
        // Based on edges - nodes + 2 * connected components
        let edges_minus_nodes = if total_symbols > 0 {
            total_edges as i64 - total_symbols as i64 + 2
        } else {
            0
        };
        let cyclomatic_complexity = std::cmp::max(0, edges_minus_nodes) as usize;

        // Find entry points (roots) and leaf functions
        let entry_points = graph.roots();
        let leaf_functions = graph.leaves();

        // Calculate depth of the call graph (longest path)
        let max_depth = self.calculate_max_depth(graph);

        // Count functions by fan-out ranges
        let mut high_fan_out = 0;
        let mut medium_fan_out = 0;
        let mut low_fan_out = 0;

        for symbol in graph.symbols() {
            let id = SymbolId::new(symbol.fully_qualified_name());
            let fan_out = graph.fan_out(&id);
            if fan_out >= 10 {
                high_fan_out += 1;
            } else if fan_out >= 5 {
                medium_fan_out += 1;
            } else {
                low_fan_out += 1;
            }
        }

        CallGraphComplexityReport {
            total_symbols,
            total_edges,
            cyclomatic_complexity,
            max_depth,
            entry_point_count: entry_points.len(),
            leaf_function_count: leaf_functions.len(),
            high_fan_out_count: high_fan_out,
            medium_fan_out_count: medium_fan_out,
            low_fan_out_count: low_fan_out,
        }
    }

    /// Calculates the maximum depth of the call graph
    fn calculate_max_depth(&self, graph: &CallGraph) -> usize {
        let mut max_depth = 0;

        for symbol in graph.symbols() {
            let id = SymbolId::new(symbol.fully_qualified_name());
            let depth = self.depth_from_roots(graph, &id);
            max_depth = std::cmp::max(max_depth, depth);
        }

        max_depth
    }

    /// Calculates the depth of a symbol from root nodes
    fn depth_from_roots(&self, graph: &CallGraph, target: &SymbolId) -> usize {
        let roots = graph.roots();
        if roots.is_empty() {
            return 0;
        }

        let mut max_depth = 0;
        for root in roots {
            if let Some(path) = graph.find_path(&root, target) {
                max_depth = std::cmp::max(max_depth, path.len());
            }
        }

        max_depth
    }

    /// Analyzes entry points (functions with no callers)
    ///
    /// Returns detailed information about each entry point.
    pub fn analyze_entry_points(&self, graph: &CallGraph) -> Vec<EntryPointAnalysis> {
        graph
            .roots()
            .into_iter()
            .filter_map(|id| {
                graph.get_symbol(&id).map(|symbol| EntryPointAnalysis {
                    symbol_id: id.clone(),
                    symbol_name: symbol.name().to_string(),
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line(),
                    fan_out: graph.fan_out(&id),
                    callees: graph.traverse_callees(&id, 3), // Limit to 3 levels for analysis
                })
            })
            .collect()
    }

    /// Analyzes leaf functions (functions with no callees)
    ///
    /// Returns detailed information about each leaf function.
    pub fn analyze_leaf_functions(&self, graph: &CallGraph) -> Vec<LeafFunctionAnalysis> {
        graph
            .leaves()
            .into_iter()
            .filter_map(|id| {
                graph.get_symbol(&id).map(|symbol| LeafFunctionAnalysis {
                    symbol_id: id.clone(),
                    symbol_name: symbol.name().to_string(),
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line(),
                    fan_in: graph.fan_in(&id),
                    callers: graph.traverse_callers(&id, 3), // Limit to 3 levels for analysis
                })
            })
            .collect()
    }
}

impl Default for CallGraphAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a hot path (frequently called function sequence)
#[derive(Debug, Clone)]
pub struct HotPath {
    /// Symbol ID of the function
    pub symbol_id: SymbolId,
    /// Name of the function
    pub symbol_name: String,
    /// File containing the function
    pub file: String,
    /// Line number
    pub line: u32,
    /// Number of callers (fan-in)
    pub fan_in: usize,
    /// Number of callees (fan-out)
    pub fan_out: usize,
}

/// Complexity report for a call graph
#[derive(Debug, Clone)]
pub struct CallGraphComplexityReport {
    /// Total number of symbols in the graph
    pub total_symbols: usize,
    /// Total number of edges in the graph
    pub total_edges: usize,
    /// Estimated cyclomatic complexity
    pub cyclomatic_complexity: usize,
    /// Maximum depth of the call graph
    pub max_depth: usize,
    /// Number of entry points (roots)
    pub entry_point_count: usize,
    /// Number of leaf functions
    pub leaf_function_count: usize,
    /// Number of functions with high fan-out (>=10)
    pub high_fan_out_count: usize,
    /// Number of functions with medium fan-out (5-9)
    pub medium_fan_out_count: usize,
    /// Number of functions with low fan-out (<5)
    pub low_fan_out_count: usize,
}

/// Analysis of an entry point (function with no callers)
#[derive(Debug, Clone)]
pub struct EntryPointAnalysis {
    /// Symbol ID
    pub symbol_id: SymbolId,
    /// Function name
    pub symbol_name: String,
    /// File location
    pub file: String,
    /// Line number
    pub line: u32,
    /// Number of direct callees
    pub fan_out: usize,
    /// Traversed callees up to 3 levels
    pub callees: Vec<CallEntry>,
}

/// Analysis of a leaf function (function with no callees)
#[derive(Debug, Clone)]
pub struct LeafFunctionAnalysis {
    /// Symbol ID
    pub symbol_id: SymbolId,
    /// Function name
    pub symbol_name: String,
    /// File location
    pub file: String,
    /// Line number
    pub line: u32,
    /// Number of direct callers
    pub fan_in: usize,
    /// Traversed callers up to 3 levels
    pub callers: Vec<CallEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::{CallGraph, Symbol};
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    #[test]
    fn test_find_hot_paths() {
        let mut graph = CallGraph::new();

        // Create a -> b -> c chain where b has highest fan-in
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        // a -> b, c -> b (b has fan-in of 2)
        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_c, &id_b, DependencyType::Calls)
            .unwrap();

        let analyzer = CallGraphAnalyzer::new();
        let hot_paths = analyzer.find_hot_paths(&graph, 10);

        assert!(!hot_paths.is_empty());
        // b should be the hottest path
        assert_eq!(hot_paths[0].symbol_name, "b");
        assert_eq!(hot_paths[0].fan_in, 2);
    }

    #[test]
    fn test_calculate_complexity() {
        let mut graph = CallGraph::new();

        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();

        let analyzer = CallGraphAnalyzer::new();
        let report = analyzer.calculate_complexity(&graph);

        assert_eq!(report.total_symbols, 2);
        assert_eq!(report.total_edges, 1);
        assert!(report.max_depth > 0);
    }

    #[test]
    fn test_analyze_entry_points() {
        let mut graph = CallGraph::new();

        // Create a simple graph: a -> b where a is entry point
        let a = Symbol::new(
            "entry",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let b = Symbol::new(
            "called",
            SymbolKind::Function,
            Location::new("test.rs", 2, 1),
        );

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();

        let analyzer = CallGraphAnalyzer::new();
        let entry_points = analyzer.analyze_entry_points(&graph);

        assert_eq!(entry_points.len(), 1);
        assert_eq!(entry_points[0].symbol_name, "entry");
    }

    #[test]
    fn test_analyze_leaf_functions() {
        let mut graph = CallGraph::new();

        // Create a simple graph: a -> b where b is leaf
        let a = Symbol::new(
            "caller",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let b = Symbol::new("leaf", SymbolKind::Function, Location::new("test.rs", 2, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();

        let analyzer = CallGraphAnalyzer::new();
        let leaf_functions = analyzer.analyze_leaf_functions(&graph);

        assert_eq!(leaf_functions.len(), 1);
        assert_eq!(leaf_functions[0].symbol_name, "leaf");
    }
}
