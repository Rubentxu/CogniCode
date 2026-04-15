//! Petgraph-based graph store implementation

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::services::CycleDetectionResult;
use crate::domain::traits::dependency_repository::DependencyError;
use crate::domain::traits::DependencyRepository;
use crate::domain::value_objects::{DependencyType, Location};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

struct NodeData {
    symbol: Symbol,
    original_location: Location,
}

impl NodeData {
    fn new(symbol: Symbol, original_location: Location) -> Self {
        Self {
            symbol,
            original_location,
        }
    }
}

/// Graph store using petgraph implementing DependencyRepository
pub struct PetGraphStore {
    graph: DiGraph<NodeData, DependencyType>,
    symbol_to_index: HashMap<String, NodeIndex>,
    index_to_symbol: HashMap<NodeIndex, String>,
}

impl PetGraphStore {
    /// Creates a new empty petgraph store
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            symbol_to_index: HashMap::new(),
            index_to_symbol: HashMap::new(),
        }
    }

    /// Gets the node index for a symbol name
    fn get_index(&self, id: &SymbolId) -> Option<NodeIndex> {
        self.symbol_to_index.get(id.as_str()).copied()
    }

    /// Adds a symbol with its real location to the graph
    pub fn add_symbol_with_location(&mut self, id: &SymbolId, symbol: Symbol) -> NodeIndex {
        if let Some(&index) = self.symbol_to_index.get(id.as_str()) {
            return index;
        }

        let index = self
            .graph
            .add_node(NodeData::new(symbol.clone(), symbol.location().clone()));
        let key = id.as_str().to_string();
        self.symbol_to_index.insert(key.clone(), index);
        self.index_to_symbol.insert(index, key);
        index
    }

    /// Adds a symbol to the graph
    fn ensure_symbol(&mut self, id: &SymbolId) -> NodeIndex {
        if let Some(&index) = self.symbol_to_index.get(id.as_str()) {
            return index;
        }

        let index = self.graph.add_node(NodeData::new(
            Symbol::new(
                id.as_str(),
                crate::domain::value_objects::SymbolKind::Unknown,
                crate::domain::value_objects::Location::new("unknown", 0, 0),
            ),
            crate::domain::value_objects::Location::new("unknown", 0, 0),
        ));
        let key = id.as_str().to_string();
        self.symbol_to_index.insert(key.clone(), index);
        self.index_to_symbol.insert(index, key);
        index
    }

    /// Converts the petgraph representation to a rich domain CallGraph
    ///
    /// This method iterates over all nodes and edges in the petgraph DiGraph
    /// and creates a CallGraph aggregate with full BFS, path finding, roots/leaves support.
    pub fn to_call_graph(&self) -> CallGraph {
        let mut call_graph = CallGraph::new();

        for node_idx in self.graph.node_indices() {
            if let Some(node_data) = self.graph.node_weight(node_idx) {
                if let Some(symbol_id_str) = self.index_to_symbol.get(&node_idx) {
                    let mut sym = node_data.symbol.clone();
                    sym.set_fqn_override(symbol_id_str);
                    call_graph.add_symbol(sym);
                }
            }
        }

        for edge in self.graph.edge_references() {
            let source_idx = edge.source();
            let target_idx = edge.target();
            let dependency_type = *edge.weight();

            if let (Some(source_id_str), Some(target_id_str)) = (
                self.index_to_symbol.get(&source_idx),
                self.index_to_symbol.get(&target_idx),
            ) {
                let source_id = SymbolId::new(source_id_str.as_str());
                let target_id = SymbolId::new(target_id_str.as_str());

                let _ = call_graph.add_dependency(&source_id, &target_id, dependency_type);
            }
        }

        call_graph
    }

    fn parse_location_from_id(id: &str) -> (Location, String) {
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() >= 3 {
            let col = parts[parts.len() - 1].parse().unwrap_or(0);
            let line = parts[parts.len() - 2].parse().unwrap_or(0);
            let file = parts[..parts.len() - 2].join(":");
            (Location::new(file, line, col), id.to_string())
        } else {
            (Location::new(id, 0, 0), format!("{}:0:0", id))
        }
    }
}

impl Default for PetGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyRepository for PetGraphStore {
    fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), DependencyError> {
        let source_idx = self.ensure_symbol(source_id);
        let target_idx = self.ensure_symbol(target_id);

        self.graph.add_edge(source_idx, target_idx, dependency_type);
        Ok(())
    }

    fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        if let Some(idx) = self.get_index(id) {
            let node_data = self.graph.remove_node(idx)?;
            self.symbol_to_index.remove(id.as_str());
            self.index_to_symbol.remove(&idx);
            Some(node_data.symbol)
        } else {
            None
        }
    }

    fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
        let idx = self.get_index(id)?;
        self.graph.node_weight(idx).map(|n| &n.symbol)
    }

    fn get_all_symbols(&self) -> Vec<Symbol> {
        self.graph
            .node_weights()
            .map(|n| n.symbol.clone())
            .collect()
    }

    fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut result = HashSet::new();
        if let Some(idx) = self.get_index(id) {
            // BFS to find all reachable nodes
            let mut queue = vec![idx];
            while let Some(current) = queue.pop() {
                for edge in self.graph.edges(current) {
                    let target = edge.target();
                    if let Some(target_name) = self.index_to_symbol.get(&target) {
                        let target_id = SymbolId::new(target_name.clone());
                        if result.insert(target_id) {
                            queue.push(target);
                        }
                    }
                }
            }
        }
        result
    }

    fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut result = HashSet::new();
        if let Some(idx) = self.get_index(id) {
            for edge in self
                .graph
                .edges_directed(idx, petgraph::Direction::Incoming)
            {
                let source = edge.source();
                if let Some(name) = self.index_to_symbol.get(&source) {
                    result.insert(SymbolId::new(name.clone()));
                }
            }
        }
        result
    }

    fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut result = HashSet::new();
        if let Some(idx) = self.get_index(id) {
            for edge in self.graph.edges(idx) {
                let target = edge.target();
                if let Some(name) = self.index_to_symbol.get(&target) {
                    result.insert(SymbolId::new(name.clone()));
                }
            }
        }
        result
    }

    fn detect_cycles(&self) -> CycleDetectionResult {
        use petgraph::algo::tarjan_scc;

        let sccs = tarjan_scc(&self.graph);
        let cycles: Vec<_> = sccs
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let names: Vec<String> = scc
                    .into_iter()
                    .filter_map(|idx| self.index_to_symbol.get(&idx).cloned())
                    .collect();
                let ids: Vec<SymbolId> = names.iter().map(|n| SymbolId::new(n)).collect();
                crate::domain::services::Cycle::new(ids, names)
            })
            .collect();

        let has_cycles = !cycles.is_empty();

        CycleDetectionResult {
            has_cycles,
            cycles,
            total_sccs: 0, // Not easily available from petgraph
        }
    }

    fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
        let source_idx = match self.get_index(source) {
            Some(idx) => idx,
            None => return false,
        };
        let target_idx = match self.get_index(target) {
            Some(idx) => idx,
            None => return false,
        };

        // Use DFS to check if there's a path
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![source_idx];

        while let Some(current) = stack.pop() {
            if current == target_idx {
                return true;
            }
            if visited.insert(current) {
                for edge in self.graph.edges(current) {
                    stack.push(edge.target());
                }
            }
        }

        false
    }

    fn get_call_graph(&self) -> CallGraph {
        self.to_call_graph()
    }

    fn symbol_count(&self) -> usize {
        self.graph.node_count()
    }

    fn dependency_count(&self) -> usize {
        self.graph.edge_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pet_graph_store_to_call_graph() {
        let mut store = PetGraphStore::new();

        let a_id = SymbolId::new("a");
        let b_id = SymbolId::new("b");

        store
            .add_dependency(&a_id, &b_id, DependencyType::Calls)
            .unwrap();

        let call_graph = store.to_call_graph();

        assert_eq!(call_graph.symbol_count(), 2, "Expected 2 symbols");
        assert_eq!(call_graph.edge_count(), 1, "Expected 1 edge");

        assert!(
            call_graph.has_path(&a_id, &b_id),
            "Expected path from a to b"
        );
    }

    #[test]
    fn test_pet_graph_store_find_path_after_conversion() {
        let mut store = PetGraphStore::new();

        let a_id = SymbolId::new("a");
        let b_id = SymbolId::new("b");
        let c_id = SymbolId::new("c");

        store
            .add_dependency(&a_id, &b_id, DependencyType::Calls)
            .unwrap();
        store
            .add_dependency(&b_id, &c_id, DependencyType::Calls)
            .unwrap();

        let call_graph = store.to_call_graph();

        let path = call_graph.find_path(&a_id, &c_id);
        assert!(path.is_some(), "Expected path from a to c");
        assert_eq!(path.unwrap().len(), 3);
    }

    #[test]
    fn test_pet_graph_store_get_call_graph_via_trait() {
        let mut store = PetGraphStore::new();

        let a_id = SymbolId::new("a");
        let b_id = SymbolId::new("b");

        store
            .add_dependency(&a_id, &b_id, DependencyType::Calls)
            .unwrap();

        let call_graph = store.get_call_graph();

        assert!(
            call_graph.has_path(&a_id, &b_id),
            "Expected path from a to b"
        );
        assert!(
            !call_graph.has_path(&b_id, &a_id),
            "Did not expect path from b to a"
        );
    }

    #[test]
    fn test_pet_graph_store_roots_and_leaves_after_conversion() {
        let mut store = PetGraphStore::new();

        let root_id = SymbolId::new("root");
        let leaf_id = SymbolId::new("leaf");

        store
            .add_dependency(&root_id, &leaf_id, DependencyType::Calls)
            .unwrap();

        let call_graph = store.to_call_graph();

        let roots = call_graph.roots();
        let leaves = call_graph.leaves();

        assert!(roots.contains(&root_id), "Expected root");
        assert!(leaves.contains(&leaf_id), "Expected leaf");
    }
}
