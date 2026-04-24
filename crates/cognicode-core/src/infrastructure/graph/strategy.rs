//! Graph construction strategy trait and implementations
//!
//! This module provides a unified interface for different graph construction
//! strategies, allowing callers to choose the appropriate strategy based on
//! their needs (speed vs. completeness).

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::DependencyRepository;
use crate::infrastructure::graph::lightweight_index::LightweightIndex;
use crate::infrastructure::graph::on_demand_graph::{
    CallHierarchyResult, OnDemandGraphBuilder, TraversalDirection,
};
use crate::infrastructure::graph::per_file_graph::PerFileGraphCache;
use crate::infrastructure::graph::symbol_index::SymbolIndex;
use crate::infrastructure::parser::TreeSitterParser;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Trait for graph construction strategies
///
/// This trait defines the interface for building graphs using different
/// strategies. Each implementation can optimize for different use cases:
/// - LightweightStrategy: Fast index-only queries
/// - OnDemandStrategy: Lazy graph construction per query
/// - PerFileStrategy: Modular file-by-file construction
/// - FullGraphStrategy: Complete project graph
pub trait GraphStrategy: Send + Sync {
    /// Builds a lightweight index (symbol name -> locations)
    fn build_index(&mut self, project_dir: &Path) -> std::io::Result<()>;

    /// Queries symbols by name, returning locations
    fn query_symbols(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation>;

    /// Builds a local graph for a single file
    fn build_local_graph(&self, file_path: &Path) -> std::io::Result<CallGraph>;

    /// Builds a subgraph centered on a symbol with given depth
    fn build_subgraph(
        &self,
        symbol_name: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult;

    /// Builds the full project call graph
    fn build_full_graph(&self, project_dir: &Path) -> std::io::Result<CallGraph>;

    /// Returns the strategy name for debugging
    fn name(&self) -> &'static str;
}

/// Lightweight strategy - fast index-only, no graph edges
///
/// This strategy builds only a lightweight index without graph edges.
/// It's the fastest option but provides limited functionality.
pub struct LightweightStrategy {
    index: LightweightIndex,
}

impl LightweightStrategy {
    /// Creates a new LightweightStrategy
    pub fn new() -> Self {
        Self {
            index: LightweightIndex::new(),
        }
    }

    /// Consumes the strategy and returns the underlying LightweightIndex.
    pub fn into_index(self) -> LightweightIndex {
        self.index
    }
}

impl Default for LightweightStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStrategy for LightweightStrategy {
    fn build_index(&mut self, project_dir: &Path) -> std::io::Result<()> {
        self.index.build_index(project_dir)
    }

    fn query_symbols(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation> {
        self.index.find_symbol(symbol_name).to_vec()
    }

    fn build_local_graph(&self, _file_path: &Path) -> std::io::Result<CallGraph> {
        // Lightweight strategy doesn't build graphs
        Ok(CallGraph::new())
    }

    fn build_subgraph(
        &self,
        symbol_name: &str,
        _depth: u32,
        _direction: TraversalDirection,
    ) -> CallHierarchyResult {
        // Return basic result from index
        let locations = self.index.find_symbol(symbol_name);
        let root_symbol = if let Some(loc) = locations.first() {
            crate::domain::aggregates::symbol::Symbol::new(
                symbol_name,
                loc.symbol_kind,
                crate::domain::value_objects::Location::new(&loc.file, loc.line, loc.column),
            )
        } else {
            crate::domain::aggregates::symbol::Symbol::new(
                symbol_name,
                crate::domain::value_objects::SymbolKind::Unknown,
                crate::domain::value_objects::Location::new("unknown", 0, 0),
            )
        };

        CallHierarchyResult {
            root_symbol,
            entries: Vec::new(),
        }
    }

    fn build_full_graph(&self, _project_dir: &Path) -> std::io::Result<CallGraph> {
        // Lightweight strategy doesn't build full graphs
        Ok(CallGraph::new())
    }

    fn name(&self) -> &'static str {
        "LightweightStrategy"
    }
}

/// On-demand strategy - builds graph only when needed
///
/// This strategy uses lazy evaluation to build only the necessary
/// portions of the graph for each query.
pub struct OnDemandStrategy {
    builder: OnDemandGraphBuilder,
    index: Arc<RwLock<LightweightIndex>>,
}

impl OnDemandStrategy {
    /// Creates a new OnDemandStrategy
    pub fn new() -> Self {
        Self {
            builder: OnDemandGraphBuilder::new(),
            index: Arc::new(RwLock::new(LightweightIndex::new())),
        }
    }
}

impl Default for OnDemandStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStrategy for OnDemandStrategy {
    fn build_index(&mut self, project_dir: &Path) -> std::io::Result<()> {
        // Build the strategy's own index — uses RwLock write guard (no silent failure)
        self.index
            .write()
            .unwrap()
            .build_index(project_dir)?;
        // Also build the builder's index
        self.builder.set_index(project_dir)
    }

    fn query_symbols(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation> {
        self.index.read().unwrap().find_symbol(symbol_name).to_vec()
    }

    fn build_local_graph(&self, file_path: &Path) -> std::io::Result<CallGraph> {
        let source = std::fs::read_to_string(file_path)?;
        let file_path_str = file_path.to_string_lossy().to_string();

        let language =
            crate::infrastructure::parser::Language::from_extension(file_path.extension())
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Unsupported file type")
                })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let symbols = parser
            .find_all_symbols_with_path(&source, &file_path_str)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let relationships = parser
            .find_call_relationships(&source, &file_path_str)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut graph = CallGraph::new();
        let mut name_to_id: std::collections::HashMap<
            String,
            crate::domain::aggregates::call_graph::SymbolId,
        > = std::collections::HashMap::new();

        for symbol in symbols {
            let id = graph.add_symbol(symbol.clone());
            name_to_id.insert(symbol.name().to_lowercase(), id);
        }

        for (caller, callee_name) in relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());
            if let Some(callee_id) = name_to_id.get(&callee_name.to_lowercase()).cloned() {
                let _ = graph.add_dependency(
                    &caller_id,
                    &callee_id,
                    crate::domain::value_objects::DependencyType::Calls,
                );
            }
        }

        Ok(graph)
    }

    fn build_subgraph(
        &self,
        symbol_name: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult {
        let mut builder = OnDemandGraphBuilder::with_index(self.index.clone());
        builder.build_for_symbol(symbol_name, depth, direction)
    }

    fn build_full_graph(&self, _project_dir: &Path) -> std::io::Result<CallGraph> {
        // On-demand strategy doesn't pre-build full graphs
        // Return empty graph - use query methods instead
        Ok(CallGraph::new())
    }

    fn name(&self) -> &'static str {
        "OnDemandStrategy"
    }
}

/// Per-file strategy - builds and caches graph per file
///
/// This strategy builds the graph file-by-file and allows merging
/// on demand. Good for incremental analysis.
pub struct PerFileStrategy {
    cache: Arc<PerFileGraphCache>,
}

impl PerFileStrategy {
    /// Creates a new PerFileStrategy
    pub fn new() -> Self {
        Self {
            cache: Arc::new(PerFileGraphCache::new()),
        }
    }

    /// Creates a new PerFileStrategy with a project directory
    pub fn with_project_dir(project_dir: impl Into<String>) -> Self {
        Self {
            cache: Arc::new(PerFileGraphCache::with_project_dir(project_dir)),
        }
    }
}

impl Default for PerFileStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStrategy for PerFileStrategy {
    fn build_index(&mut self, _project_dir: &Path) -> std::io::Result<()> {
        // Per-file strategy builds index lazily
        Ok(())
    }

    fn query_symbols(
        &self,
        _symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation> {
        // Would need to query each file - not efficient for this strategy
        Vec::new()
    }

    fn build_local_graph(&self, file_path: &Path) -> std::io::Result<CallGraph> {
        Ok(self.cache.get_or_build(file_path)?.as_ref().clone())
    }

    fn build_subgraph(
        &self,
        symbol_name: &str,
        _depth: u32,
        _direction: TraversalDirection,
    ) -> CallHierarchyResult {
        // Would need to build from file graphs - simplified version
        CallHierarchyResult {
            root_symbol: crate::domain::aggregates::symbol::Symbol::new(
                symbol_name,
                crate::domain::value_objects::SymbolKind::Unknown,
                crate::domain::value_objects::Location::new("unknown", 0, 0),
            ),
            entries: Vec::new(),
        }
    }

    fn build_full_graph(&self, project_dir: &Path) -> std::io::Result<CallGraph> {
        use walkdir::WalkDir;

        let mut paths: Vec<std::path::PathBuf> = Vec::new();

        for entry in WalkDir::new(project_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if matches!(ext.to_str(), Some("rs" | "py" | "js" | "ts")) {
                        paths.push(path.to_path_buf());
                    }
                }
            }
        }

        let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        Ok(self.cache.merge(&path_refs))
    }

    fn name(&self) -> &'static str {
        "PerFileStrategy"
    }
}

/// Full graph strategy - builds complete project graph
///
/// This strategy builds the complete project graph upfront.
/// It's the most comprehensive but also the slowest.
pub struct FullGraphStrategy {
    symbol_index: Option<SymbolIndex>,
}

impl FullGraphStrategy {
    /// Creates a new FullGraphStrategy
    pub fn new() -> Self {
        Self { symbol_index: None }
    }
}

impl Default for FullGraphStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStrategy for FullGraphStrategy {
    fn build_index(&mut self, project_dir: &Path) -> std::io::Result<()> {
        let mut index = SymbolIndex::new();
        index.build(project_dir)?;
        self.symbol_index = Some(index);
        Ok(())
    }

    fn query_symbols(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation> {
        self.symbol_index
            .as_ref()
            .map(|idx| idx.query(symbol_name))
            .unwrap_or_default()
    }

    fn build_local_graph(&self, file_path: &Path) -> std::io::Result<CallGraph> {
        let source = std::fs::read_to_string(file_path)?;
        let file_path_str = file_path.to_string_lossy().to_string();

        let language =
            crate::infrastructure::parser::Language::from_extension(file_path.extension())
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Unsupported file type")
                })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let symbols = parser
            .find_all_symbols_with_path(&source, &file_path_str)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let relationships = parser
            .find_call_relationships(&source, &file_path_str)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut graph = CallGraph::new();
        let mut name_to_id: std::collections::HashMap<
            String,
            crate::domain::aggregates::call_graph::SymbolId,
        > = std::collections::HashMap::new();

        for symbol in symbols {
            let id = graph.add_symbol(symbol.clone());
            name_to_id.insert(symbol.name().to_lowercase(), id);
        }

        for (caller, callee_name) in relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());
            if let Some(callee_id) = name_to_id.get(&callee_name.to_lowercase()).cloned() {
                let _ = graph.add_dependency(
                    &caller_id,
                    &callee_id,
                    crate::domain::value_objects::DependencyType::Calls,
                );
            }
        }

        Ok(graph)
    }

    fn build_subgraph(
        &self,
        symbol_name: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult {
        let mut builder = OnDemandGraphBuilder::new();
        if let Some(ref idx) = self.symbol_index {
            builder = OnDemandGraphBuilder::with_index(Arc::new(RwLock::new(idx.underlying_index().clone())));
        }
        builder.build_for_symbol(symbol_name, depth, direction)
    }

    fn build_full_graph(&self, project_dir: &Path) -> std::io::Result<CallGraph> {
        let mut store = crate::infrastructure::graph::PetGraphStore::new();
        let mut name_to_symbol_id: std::collections::HashMap<
            String,
            crate::domain::aggregates::call_graph::SymbolId,
        > = std::collections::HashMap::new();

        use walkdir::WalkDir;

        for entry in WalkDir::new(project_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let language =
                match crate::infrastructure::parser::Language::from_extension(path.extension()) {
                    Some(lang) => lang,
                    None => continue,
                };

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let file_path = path.to_string_lossy().to_string();

            let parser = match TreeSitterParser::new(language) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let symbols = match parser.find_all_symbols_with_path(&source, &file_path) {
                Ok(syms) => syms,
                Err(_) => continue,
            };

            let relationships = match parser.find_call_relationships(&source, &file_path) {
                Ok(rel) => rel,
                Err(_) => continue,
            };

            for symbol in symbols {
                let symbol_id = crate::domain::aggregates::call_graph::SymbolId::new(
                    symbol.fully_qualified_name(),
                );
                store.add_symbol_with_location(&symbol_id, symbol.clone());
                name_to_symbol_id.insert(symbol.name().to_lowercase(), symbol_id);
            }

            for (caller, callee_name) in relationships {
                let caller_id = crate::domain::aggregates::call_graph::SymbolId::new(
                    caller.fully_qualified_name(),
                );

                if let Some(callee_id) = name_to_symbol_id.get(&callee_name.to_lowercase()).cloned()
                {
                    store
                        .add_dependency(
                            &caller_id,
                            &callee_id,
                            crate::domain::value_objects::DependencyType::Calls,
                        )
                        .ok();
                }
            }
        }

        Ok(store.to_call_graph())
    }

    fn name(&self) -> &'static str {
        "FullGraphStrategy"
    }
}

/// Factory for creating graph strategies
pub struct GraphStrategyFactory;

impl GraphStrategyFactory {
    /// Creates a strategy based on the name
    pub fn create(strategy: &str) -> Box<dyn GraphStrategy> {
        match strategy {
            "lightweight" => Box::new(LightweightStrategy::new()),
            "on_demand" | "ondemand" => Box::new(OnDemandStrategy::new()),
            "per_file" | "perfile" => Box::new(PerFileStrategy::new()),
            "full" | "full_graph" => Box::new(FullGraphStrategy::new()),
            _ => Box::new(OnDemandStrategy::new()), // Default
        }
    }

    /// Returns a list of available strategy names
    pub fn available_strategies() -> Vec<&'static str> {
        vec!["lightweight", "on_demand", "per_file", "full"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lightweight_strategy() {
        let mut strategy = LightweightStrategy::new();
        assert_eq!(strategy.name(), "LightweightStrategy");
    }

    #[test]
    fn test_on_demand_strategy() {
        let strategy = OnDemandStrategy::new();
        assert_eq!(strategy.name(), "OnDemandStrategy");
    }

    #[test]
    fn test_per_file_strategy() {
        let strategy = PerFileStrategy::new();
        assert_eq!(strategy.name(), "PerFileStrategy");
    }

    #[test]
    fn test_full_graph_strategy() {
        let strategy = FullGraphStrategy::new();
        assert_eq!(strategy.name(), "FullGraphStrategy");
    }

    #[test]
    fn test_strategy_factory() {
        let strategies = GraphStrategyFactory::available_strategies();
        assert!(strategies.contains(&"lightweight"));
        assert!(strategies.contains(&"on_demand"));
        assert!(strategies.contains(&"per_file"));
        assert!(strategies.contains(&"full"));
    }

    #[test]
    fn test_strategy_factory_create() {
        let s = GraphStrategyFactory::create("lightweight");
        assert_eq!(s.name(), "LightweightStrategy");

        let s = GraphStrategyFactory::create("on_demand");
        assert_eq!(s.name(), "OnDemandStrategy");

        let s = GraphStrategyFactory::create("per_file");
        assert_eq!(s.name(), "PerFileStrategy");

        let s = GraphStrategyFactory::create("full");
        assert_eq!(s.name(), "FullGraphStrategy");
    }

    #[test]
    fn test_strategy_factory_default() {
        let s = GraphStrategyFactory::create("unknown");
        assert_eq!(s.name(), "OnDemandStrategy"); // Default
    }
}
