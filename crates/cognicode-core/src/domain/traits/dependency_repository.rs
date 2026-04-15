//! Trait for dependency repository operations
//!
//! Provides methods for managing symbol dependencies and analyzing impact.

use std::collections::HashSet;

use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
use crate::domain::services::CycleDetectionResult;
use crate::domain::value_objects::DependencyType;

/// Repository for managing symbol dependencies
pub trait DependencyRepository: Send + Sync {
    /// Adds a dependency between two symbols
    fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), DependencyError>;

    /// Removes a symbol and all its dependencies
    fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol>;

    /// Gets a symbol by ID
    fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol>;

    /// Gets all symbols
    fn get_all_symbols(&self) -> Vec<Symbol>;

    /// Finds the impact scope of changing a symbol
    fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId>;

    /// Finds all symbols that depend on the given symbol (directly or transitively)
    fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId>;

    /// Finds all symbols that this symbol depends on (directly or transitively)
    fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId>;

    /// Detects cycles in the dependency graph
    fn detect_cycles(&self) -> CycleDetectionResult;

    /// Checks if there's a path between two symbols
    fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool;

    /// Gets the call graph as an owned value (converted from internal storage)
    fn get_call_graph(&self) -> CallGraph;

    /// Returns the total number of symbols
    fn symbol_count(&self) -> usize;

    /// Returns the total number of dependencies
    fn dependency_count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
    use crate::domain::services::CycleDetectionResult;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
    use std::collections::{HashMap, HashSet};

    struct MockDependencyRepository {
        symbols: HashMap<SymbolId, Symbol>,
        dependencies: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>,
    }

    impl MockDependencyRepository {
        fn new() -> Self {
            Self {
                symbols: HashMap::new(),
                dependencies: HashMap::new(),
            }
        }
    }

    impl DependencyRepository for MockDependencyRepository {
        fn add_dependency(
            &mut self,
            source_id: &SymbolId,
            target_id: &SymbolId,
            dependency_type: DependencyType,
        ) -> Result<(), DependencyError> {
            self.dependencies
                .entry(source_id.clone())
                .or_insert_with(HashSet::new)
                .insert((target_id.clone(), dependency_type));
            Ok(())
        }

        fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
            self.symbols.remove(id)
        }

        fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
            self.symbols.get(id)
        }

        fn get_all_symbols(&self) -> Vec<Symbol> {
            self.symbols.values().cloned().collect()
        }

        fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId> {
            let mut scope = HashSet::new();
            if let Some(deps) = self.dependencies.get(id) {
                for (target_id, _) in deps {
                    scope.insert(target_id.clone());
                }
            }
            scope
        }

        fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
            let mut dependents = HashSet::new();
            for (source_id, deps) in &self.dependencies {
                for (target_id, _) in deps {
                    if target_id == id {
                        dependents.insert(source_id.clone());
                    }
                }
            }
            dependents
        }

        fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
            self.dependencies
                .get(id)
                .map(|deps| deps.iter().map(|(id, _)| id.clone()).collect())
                .unwrap_or_default()
        }

        fn detect_cycles(&self) -> CycleDetectionResult {
            CycleDetectionResult {
                has_cycles: false,
                cycles: vec![],
                total_sccs: 0,
            }
        }

        fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
            self.find_dependencies(source).contains(target)
        }

        fn get_call_graph(&self) -> CallGraph {
            CallGraph::new()
        }

        fn symbol_count(&self) -> usize {
            self.symbols.len()
        }

        fn dependency_count(&self) -> usize {
            self.dependencies.values().map(|s| s.len()).sum()
        }
    }

    #[test]
    fn test_mock_get_symbols() {
        let mut repo = MockDependencyRepository::new();
        let location1 = Location::new("test.rs", 0, 0);
        let location2 = Location::new("test.rs", 10, 0);
        let symbol1 = Symbol::new("func1", SymbolKind::Function, location1);
        let symbol2 = Symbol::new("func2", SymbolKind::Function, location2);

        let id1 = SymbolId::new("test.rs:func1:0");
        let id2 = SymbolId::new("test.rs:func2:10");

        repo.symbols.insert(id1.clone(), symbol1.clone());
        repo.symbols.insert(id2.clone(), symbol2.clone());

        let symbols = repo.get_all_symbols();
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_mock_get_dependencies() {
        let mut repo = MockDependencyRepository::new();
        let location1 = Location::new("test.rs", 0, 0);
        let location2 = Location::new("test.rs", 10, 0);
        let symbol1 = Symbol::new("caller", SymbolKind::Function, location1);
        let symbol2 = Symbol::new("callee", SymbolKind::Function, location2);

        let id1 = SymbolId::new("test.rs:caller:0");
        let id2 = SymbolId::new("test.rs:callee:10");

        repo.symbols.insert(id1.clone(), symbol1);
        repo.symbols.insert(id2.clone(), symbol2);
        repo.add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();

        let deps = repo.find_dependencies(&id1);
        assert!(deps.contains(&id2));
        assert_eq!(repo.dependency_count(), 1);
    }
}

/// Error type for dependency operations
#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(SymbolId),

    #[error("Dependency already exists: {0} -> {1}")]
    DependencyAlreadyExists(SymbolId, SymbolId),

    #[error("Invalid dependency: {0}")]
    InvalidDependency(String),

    #[error("Cyclic dependency detected")]
    CyclicDependency,

    #[error("Internal error: {0}")]
    Internal(String),
}
