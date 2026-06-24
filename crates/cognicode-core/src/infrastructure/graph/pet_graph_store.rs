//! Petgraph-based graph store implementation

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::services::CycleDetectionResult;
use crate::domain::traits::DependencyRepository;
use crate::domain::traits::dependency_repository::DependencyError;
use crate::domain::value_objects::DependencyType;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashMap, HashSet};

struct NodeData {
    symbol: Symbol,
}

impl NodeData {
    fn new(symbol: Symbol) -> Self {
        Self { symbol }
    }
}

/// Graph store using petgraph implementing DependencyRepository
pub struct PetGraphStore {
    graph: StableGraph<NodeData, DependencyType>,
    symbol_to_index: HashMap<String, NodeIndex>,
    index_to_symbol: HashMap<NodeIndex, String>,
}

impl PetGraphStore {
    /// Creates a new empty petgraph store
    pub fn new() -> Self {
        Self {
            graph: StableGraph::new(),
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

        let index = self.graph.add_node(NodeData::new(symbol.clone()));
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

        let index = self.graph.add_node(NodeData::new(Symbol::new(
            id.as_str(),
            crate::domain::value_objects::SymbolKind::Unknown,
            crate::domain::value_objects::Location::new("unknown", 0, 0),
        )));
        let key = id.as_str().to_string();
        self.symbol_to_index.insert(key.clone(), index);
        self.index_to_symbol.insert(index, key);
        index
    }

    /// Converts the petgraph representation to a rich domain CallGraph
    ///
    /// This method iterates over all nodes and edges in the petgraph StableGraph
    /// and creates a CallGraph aggregate with full BFS, path finding, roots/leaves support.
    pub fn to_call_graph(&self) -> CallGraph {
        let mut call_graph = CallGraph::new();

        for node_idx in self.graph.node_indices() {
            if let Some(node_data) = self.graph.node_weight(node_idx)
                && let Some(symbol_id_str) = self.index_to_symbol.get(&node_idx)
            {
                let mut sym = node_data.symbol.clone();
                sym.set_fqn_override(symbol_id_str);
                call_graph.add_symbol(sym);
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
        let Some(start) = self.get_index(id) else {
            return result;
        };
        // Use petgraph's BFS visitor; the start node is excluded from
        // the result to preserve the original "impact scope" semantics
        // (i.e. transitive dependents of `id`, not `id` itself).
        let mut bfs = petgraph::visit::Bfs::new(&self.graph, start);
        while let Some(ni) = bfs.next(&self.graph) {
            if ni == start {
                continue;
            }
            if let Some(name) = self.index_to_symbol.get(&ni) {
                result.insert(SymbolId::new(name.clone()));
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
                let ids: Vec<SymbolId> = names.iter().map(SymbolId::new).collect();
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
        let (Some(source_idx), Some(target_idx)) = (self.get_index(source), self.get_index(target))
        else {
            return false;
        };
        // Delegate to petgraph's BFS-based path probe. This handles cycles
        // correctly and avoids the manual visited-set bookkeeping.
        petgraph::algo::has_path_connecting(&self.graph, source_idx, target_idx, None)
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
