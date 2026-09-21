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
use crate::infrastructure::graph::per_file_graph::{GlobalSymbolIndex, PerFileGraphCache};
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
        self.index.write().unwrap().build_index(project_dir)?;
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

        let parser =
            TreeSitterParser::new(language).map_err(|e| std::io::Error::other(e.to_string()))?;
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
            if path.is_file()
                && let Some(ext) = path.extension()
                && matches!(ext.to_str(), Some("rs" | "py" | "js" | "ts"))
            {
                paths.push(path.to_path_buf());
            }
        }

        let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        Ok(self.cache.merge(&path_refs))
    }

    fn name(&self) -> &'static str {
        "PerFileStrategy"
    }
}

impl PerFileStrategy {
    /// Builds the full project call graph **with an honest coverage report**.
    ///
    /// This is the explicit counterpart of [`GraphStrategy::build_full_graph`].
    /// Unlike the trait method, walk errors are NOT silently dropped
    /// (`filter_map(|e| e.ok())` is replaced by a path that collects
    /// skipped entries), and read/parse failures inside `merge_with_report`
    /// are surfaced in [`BuildStatus::Partial`].
    ///
    /// Use this when the caller needs to know whether the resulting graph
    /// represents complete coverage of the project directory or whether
    /// some files were skipped. The legacy `build_full_graph` is kept
    /// for backward compatibility with the seven CLI consumers in
    /// `interface/cli/commands.rs` and the `merge_file_graphs` MCP tool,
    /// which will be migrated to this method in F2.W2-followup commits.
    pub fn build_full_graph_report(
        &self,
        project_dir: &Path,
    ) -> crate::infrastructure::graph::per_file_graph::BuildReport {
        use crate::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};
        use walkdir::WalkDir;

        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        let mut walk_skipped: Vec<SkippedFile> = Vec::new();

        for entry in WalkDir::new(project_dir).follow_links(true).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    walk_skipped.push(SkippedFile {
                        path: err
                            .path()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| project_dir.to_string_lossy().to_string()),
                        reason: SkipReason::Read(err.to_string()),
                    });
                    continue;
                }
            };
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && matches!(ext.to_str(), Some("rs" | "py" | "js" | "ts"))
            {
                paths.push(path.to_path_buf());
            }
        }

        let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        let mut report = self.cache.merge_with_report(&path_refs);

        // Merge walk-level skips into the report's status.
        if !walk_skipped.is_empty() {
            report.status = match report.status {
                crate::infrastructure::graph::per_file_graph::BuildStatus::Complete => {
                    crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                        skipped: walk_skipped,
                    }
                }
                crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                    mut skipped,
                } => {
                    skipped.extend(walk_skipped);
                    crate::infrastructure::graph::per_file_graph::BuildStatus::Partial { skipped }
                }
            };
        }

        report
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

        let parser =
            TreeSitterParser::new(language).map_err(|e| std::io::Error::other(e.to_string()))?;
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
            builder = OnDemandGraphBuilder::with_index(Arc::new(RwLock::new(
                idx.underlying_index().clone(),
            )));
        }
        builder.build_for_symbol(symbol_name, depth, direction)
    }

    fn build_full_graph(&self, project_dir: &Path) -> std::io::Result<CallGraph> {
        let mut store = crate::infrastructure::graph::PetGraphStore::new();

        use walkdir::WalkDir;

        // PRF F2.W5 — H-R4-2: pre-walk to build the global symbol
        // index. We parse each source file once just for its symbols;
        // edges are resolved in a second pass using `GlobalSymbolIndex`
        // so cross-file calls reach the right `SymbolId`. The legacy
        // implementation had a per-file `name → SymbolId` map and
        // dropped every cross-file edge.
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut global_index = GlobalSymbolIndex::new();
        let mut per_file_data: Vec<(
            std::path::PathBuf,
            String,
            Vec<crate::domain::aggregates::symbol::Symbol>,
            Vec<(crate::domain::aggregates::symbol::Symbol, String)>,
        )> = Vec::new();

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
            for symbol in &symbols {
                let sid = crate::domain::aggregates::call_graph::SymbolId::new(
                    symbol.fully_qualified_name(),
                );
                global_index.insert(sid, path.to_path_buf(), symbol.name());
            }
            files.push(path.to_path_buf());
            let rels = match parser.find_call_relationships(&source, &file_path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            per_file_data.push((path.to_path_buf(), file_path, symbols, rels));
        }

        // Add every symbol to the petgraph store and remember the
        // mapping `SymbolId → NodeIndex` for edge insertion below.
        let mut id_to_node: std::collections::HashMap<
            crate::domain::aggregates::call_graph::SymbolId,
            petgraph::graph::NodeIndex,
        > = std::collections::HashMap::new();
        for (_, _, symbols, _) in &per_file_data {
            for symbol in symbols {
                let symbol_id = crate::domain::aggregates::call_graph::SymbolId::new(
                    symbol.fully_qualified_name(),
                );
                let n = store.add_symbol_with_location(&symbol_id, symbol.clone());
                id_to_node.insert(symbol_id, n);
            }
        }

        // Resolve edges through `GlobalSymbolIndex`. When the global
        // lookup returns `None` (no candidate, or ambiguous), we DO
        // NOT invent an edge against a random homonym; we drop it.
        for (path_buf, _file_path, _symbols, relationships) in &per_file_data {
            for (caller, callee_name) in relationships {
                let caller_id = crate::domain::aggregates::call_graph::SymbolId::new(
                    caller.fully_qualified_name(),
                );
                let callee_id =
                    global_index.resolve(&callee_name.to_lowercase(), Some(path_buf.as_path()));
                if let Some(callee_id) = callee_id {
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
        let strategy = LightweightStrategy::new();
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

#[cfg(test)]
mod w3_equivalence_tests {
    //! PRF F2.W3 — Equivalencia `full` vs `per_file` (R4).
    //!
    //! These tests **characterize** the divergence between
    //! [`FullGraphStrategy`] and [`PerFileStrategy`] over the corpus
    //! `docs/prf/fixtures/equivalence_full_vs_perfile/`. They do NOT
    //! force equivalence — the two strategies serve different
    //! purposes — but they pin down the contract so that any future
    //! regression is detected immediately.
    //!
    //! What they assert:
    //!  1. Both strategies discover the **same set of symbols** (the
    //!     order may differ, the count must match).
    //!  2. Both strategies skip empty / comments-only files
    //!     (no symbols from those files).
    //!  3. `PerFileStrategy::build_full_graph_report` surfaces the
    //!     broken-syntax file as a `SkippedFile` with `Parse` reason,
    //!     while `FullGraphStrategy::build_full_graph` silently
    //!     ignores it (this is documented as a real R3-equivalent
    //!     bug in `FullGraphStrategy` and tracked as scope for F2.W4).
    //!  4. The same `name` appearing in multiple files produces one
    //!     symbol per file (qualified by the file path) — both
    //!     strategies must agree on this count.
    use super::*;
    use crate::infrastructure::graph::GraphStrategy;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn corpus() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("docs/prf/fixtures/equivalence_full_vs_perfile")
    }

    fn symbol_set(graph: &crate::domain::aggregates::call_graph::CallGraph) -> HashSet<String> {
        graph
            .symbols()
            .map(|s| s.fully_qualified_name().to_string())
            .collect()
    }

    /// S1: both strategies find the same symbol set on a non-trivial
    /// corpus. Order-independent: HashSet comparison.
    #[test]
    fn w3_full_and_per_file_discover_same_symbol_set() {
        let p = corpus();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();

        let full_set = symbol_set(&full);
        let per_set = symbol_set(&per);

        assert_eq!(
            full_set.len(),
            per_set.len(),
            "full and per_file disagree on symbol count: full={}, per_file={}",
            full_set.len(),
            per_set.len()
        );
        let only_full: Vec<_> = full_set.difference(&per_set).collect();
        let only_per: Vec<_> = per_set.difference(&full_set).collect();
        assert!(
            only_full.is_empty() && only_per.is_empty(),
            "full-only: {:?}\nper-only: {:?}",
            only_full,
            only_per
        );
    }

    /// S2: the corpus contains the named symbols at the expected
    /// counts (1 unique simple, 2 cross-file `shared`, 2 cross-file
    /// `compute`, 2 same-file `same_name` inside dup.rs, 1 deeply
    /// nested, 1 cross-file call edge opportunity).
    #[test]
    fn w3_corpus_has_expected_symbol_inventory() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();
        let names: Vec<String> = per.symbols().map(|s| s.name().to_string()).collect();

        let count = |n: &str| names.iter().filter(|x| x.as_str() == n).count();
        assert_eq!(count("hello"), 1, "expected 1 `hello`");
        assert_eq!(count("shared"), 2, "expected 2 `shared` (lib + nested)");
        assert_eq!(
            count("compute"),
            2,
            "expected 2 `compute` (one in lib, one in nested)"
        );
        assert_eq!(
            count("same_name"),
            2,
            "expected 2 `same_name` (dup.rs top + dup.rs::inner)"
        );
        assert_eq!(count("caller"), 1);
        assert_eq!(count("callee"), 1);
        assert_eq!(count("deep_symbol"), 1);

        // Total expected: 1 + 2 + 2 + 2 + 1 + 1 + 1 = 10. Pinning the
        // exact total is what makes this test useful as a regression
        // detector: a future change that adds a phantom symbol (or
        // drops one) flips this assertion.
        assert_eq!(
            names.len(),
            10,
            "expected exactly 10 symbols across the corpus, got {}: {:?}",
            names.len(),
            names
        );
        // empty.rs and comments_only.rs must produce no symbols.
        assert_eq!(count("dummy_marker_to_make_file_non_empty"), 0);
    }

    /// S3: full and per_file agree on the **count** per name (a more
    /// granular check than S1: same set, same multiplicities).
    #[test]
    fn w3_full_and_per_file_agree_on_per_name_counts() {
        let p = corpus();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();

        let count_per_name = |g: &crate::domain::aggregates::call_graph::CallGraph| {
            let mut m = std::collections::BTreeMap::new();
            for s in g.symbols() {
                *m.entry(s.name().to_string()).or_insert(0usize) += 1;
            }
            m
        };
        let full_counts = count_per_name(&full);
        let per_counts = count_per_name(&per);
        assert_eq!(
            full_counts, per_counts,
            "per-name counts diverge: full={:?} per={:?}",
            full_counts, per_counts
        );
    }

    /// S4: per_file REPORT surfaces `broken.rs` as a `SkippedFile`
    /// with reason `Parse`. `full` does not (documented R3-style
    /// behaviour, scope of F2.W4 to fix).
    #[test]
    fn w3_per_file_report_marks_broken_syntax_as_skipped() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph_report(&p);
        let report = per;

        match &report.status {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial { skipped } => {
                let broken = skipped.iter().find(|s| s.path.ends_with("broken.rs"));
                assert!(
                    broken.is_some(),
                    "expected broken.rs in SkippedFile list, got: {:?}",
                    skipped
                );
                let reason = &broken.unwrap().reason;
                assert!(
                    matches!(
                        reason,
                        crate::infrastructure::graph::per_file_graph::SkipReason::Parse(_)
                    ),
                    "expected SkipReason::Parse for broken.rs, got: {:?}",
                    reason
                );
            }
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete => {
                panic!(
                    "BuildStatus::Complete is wrong: broken.rs has obvious syntax                      errors and MUST be reported as skipped"
                );
            }
        }
    }

    /// S5 (characterization, NOT a bug we will fix in F2.W3):
    /// `FullGraphStrategy` silently ignores broken.rs. This is
    /// documented behaviour today; fixing it is scope of F2.W4.
    /// This test pins down the current behaviour so that a future
    /// "silent regression" of `per_file` does not slip in unnoticed.
    #[test]
    fn w3_full_strategy_silently_ignores_broken_syntax_today() {
        let p = corpus();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();

        // `full` does not have a "skipped" report; instead we assert
        // that `oops` (the function name inside broken.rs) does NOT
        // appear among its symbols. If a future change starts
        // surfacing broken.rs symbols from `full`, this test will
        // catch the regression even though `full` itself has no
        // reporting API.
        let has_oops = full
            .symbols()
            .any(|s| s.fully_qualified_name().ends_with(":oops:7"));
        assert!(
            !has_oops,
            "FullGraphStrategy must NOT surface symbols from broken.rs              (it silently ignores parse errors today).              If this assertion fires, decide whether full now has R3              coverage — if yes, update this test and the F2.W4 plan."
        );
    }

    // =========================================================================
    // PRF F2.W5 — H-R4-2 (lookup global `name → SymbolId`).
    //
    // These tests characterise the behaviour that the resolver MUST honour
    // for cross-file call edges (and MUST NOT invent edges when the callee
    // is ambiguous):
    //
    //   1. Cross-file call (S6 of F2.W3 corpus) reaches the right SymbolId
    //      even when the callee name appears in multiple files.
    //   2. Intra-file duplicate name (S3 — `dup.rs::same_name` x2) does
    //      NOT produce cross-file edges when the caller is in a different
    //      file.
    //   3. When the callee name is genuinely ambiguous (homonym across
    //      files with no desambiguation rule that applies), the resolver
    //      drops the edge instead of inventing one.
    //   4. Both `full` and `per_file` agree on the resulting edge set.
    //
    // These tests are RED today (the lookup in `FullGraphStrategy::build_full_graph`
    // and `PerFileStrategy::build_file_graph` is per-file and the per-file
    // map silently drops cross-file edges or, when the same name appears
    // in multiple files, picks the LAST inserted — which can be wrong).
    // =========================================================================

    use crate::domain::aggregates::call_graph::SymbolId;
    use crate::domain::value_objects::DependencyType;

    fn count_edges(
        graph: &crate::domain::aggregates::call_graph::CallGraph,
        caller: &SymbolId,
        callee: &SymbolId,
        kind: DependencyType,
    ) -> usize {
        graph
            .all_dependencies()
            .filter(|(c, t, k)| *c == caller && *t == callee && **k == kind)
            .count()
    }

    /// S6 (PRF F2.W5): the cross-file call `caller → callee` reaches
    /// the nested `callee` (NOT any other homonym). This is the basic
    /// guarantee that the global lookup must satisfy.
    #[test]
    fn w5_cross_file_call_edge_resolves_to_correct_symbol() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();

        let caller_id = per
            .symbols()
            .find(|s| s.name() == "caller")
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .expect("caller must be in the graph");
        let callee_id = per
            .symbols()
            .find(|s| s.name() == "callee")
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .expect("callee must be in the graph");

        let n = count_edges(&per, &caller_id, &callee_id, DependencyType::Calls);
        assert!(
            n >= 1,
            "expected at least one `caller -> callee` edge in the per_file graph; got {} total edges",
            per.edge_count()
        );
    }

    #[test]
    fn w5_cross_file_call_edge_also_present_in_full() {
        let p = corpus();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();

        let caller_id = full
            .symbols()
            .find(|s| s.name() == "caller")
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .expect("caller must be in the graph");
        let callee_id = full
            .symbols()
            .find(|s| s.name() == "callee")
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .expect("callee must be in the graph");

        let n = count_edges(&full, &caller_id, &callee_id, DependencyType::Calls);
        assert!(
            n >= 1,
            "expected at least one `caller -> callee` edge in the full graph; got {} total edges",
            full.edge_count()
        );
    }

    /// S3 (PRF F2.W5): intra-file duplicate (`dup.rs::same_name` x2)
    /// does NOT spawn cross-file edges. The resolver MUST treat the
    /// two `same_name` instances as distinct (different FQN / different
    /// module path) and not invent edges to them from `caller` (which
    /// is in `lib.rs`).
    #[test]
    fn w5_intra_file_duplicate_does_not_invent_cross_file_edges() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();

        let caller_id = per
            .symbols()
            .find(|s| s.name() == "caller" && s.location().file().ends_with("lib.rs"))
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .expect("caller in lib.rs must be in the graph");

        let same_name_ids: Vec<_> = per
            .symbols()
            .filter(|s| s.name() == "same_name")
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .collect();
        assert_eq!(
            same_name_ids.len(),
            2,
            "expected exactly 2 `same_name` instances (dup.rs top + inner); got {}",
            same_name_ids.len()
        );

        for sn in &same_name_ids {
            let fake = count_edges(&per, &caller_id, sn, DependencyType::Calls);
            assert!(
                fake == 0,
                "resolver must NOT invent edge caller -> {:?} (homonym in another file); got {} edges",
                sn,
                fake
            );
        }
    }

    /// S9 (PRF F2.W5): same name across files (overload semantics).
    /// `compute` exists in `lib.rs` and in `nested/mod.rs` with
    /// different signatures. Neither strategy should pretend it
    /// resolved the call; the corpus does NOT include a call to
    /// `compute`, so no edges are expected.
    #[test]
    fn w5_compute_overload_no_call_site_yields_no_invented_edges() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();

        for (label, g) in [("per_file", &per), ("full", &full)] {
            let compute_ids: Vec<_> = g
                .symbols()
                .filter(|s| s.name() == "compute")
                .map(|s| SymbolId::new(s.fully_qualified_name()))
                .collect();
            assert_eq!(
                compute_ids.len(),
                2,
                "[{}] expected exactly 2 `compute` instances (lib + nested); got {}",
                label,
                compute_ids.len()
            );

            // No call site references `compute` in the corpus, so
            // there must be NO edges whose callee is any compute_id.
            for c in &compute_ids {
                let spurious = g.all_dependencies().filter(|(_, t, _)| *t == c).count();
                assert_eq!(
                    spurious, 0,
                    "[{}] spurious edges to compute_id={:?}",
                    label, c
                );
            }
        }
    }
}

#[cfg(test)]
mod w10_equivalence_tests {
    //! PRF F2.W10 — Equivalencia de aristas y reproducibilidad.
    //!
    //! F2.W3 pineo equivalencia de símbolos cuando edges eran 0=0.
    //! Desde F2.W5/W7 las aristas cross-file existen; aquí se pinea
    //! que `FullGraphStrategy` y `PerFileStrategy` producen el mismo
    //! conjunto de aristas (caller FQN → callee FQN) sobre el corpus,
    //! y que dos builds consecutivos son reproducibles.

    use super::*;
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::infrastructure::graph::GraphStrategy;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn corpus() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("docs/prf/fixtures/equivalence_full_vs_perfile")
    }

    fn symbol_fqns(graph: &CallGraph) -> HashSet<String> {
        graph
            .symbols()
            .map(|s| s.fully_qualified_name().to_string())
            .collect()
    }

    /// Edge set as (caller_fqn, callee_fqn) pairs. Callee SymbolIds
    /// only appear in the set if resolvable to a symbol in the graph;
    /// otherwise the edge maps to "<unresolved>".
    fn edge_set(graph: &CallGraph) -> HashSet<(String, String)> {
        let id_to_fqn: std::collections::HashMap<&str, &str> = graph
            .symbols()
            .map(|s| (s.fully_qualified_name(), s.fully_qualified_name()))
            .collect();
        let id_lookup = |id: &crate::domain::aggregates::call_graph::SymbolId| -> String {
            let f = id.as_str();
            if id_to_fqn.contains_key(f) {
                f.to_string()
            } else {
                format!("<unresolved:{f}>")
            }
        };
        graph
            .edges_with_metadata()
            .map(|(src, dst, _, _, _)| (id_lookup(&src), id_lookup(&dst)))
            .collect()
    }

    /// W10.1: full y per_file producen el MISMO conjunto de aristas
    /// (caller fqn, callee fqn) sobre el corpus determinista.
    #[test]
    fn w10_full_and_per_file_agree_on_edge_set() {
        let p = corpus();
        let full = FullGraphStrategy::new().build_full_graph(&p).unwrap();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();

        let full_edges = edge_set(&full);
        let per_edges = edge_set(&per);

        let only_full: Vec<_> = full_edges.difference(&per_edges).collect();
        let only_per: Vec<_> = per_edges.difference(&full_edges).collect();
        assert!(
            only_full.is_empty() && only_per.is_empty(),
            "edge sets diverge. full-only: {only_full:?}\nper-only: {only_per:?}"
        );
    }

    /// W10.2: el conjunto de aristas es NO vacío. El corpus tiene
    /// `caller → crate::nested::callee()`; desde F2.W5/W7 debe haber
    /// al menos una arista resuelta. Si esto vuelve a 0, H-R4-1
    /// habría regresado.
    #[test]
    fn w10_edge_set_is_non_empty_on_cross_file_corpus() {
        let p = corpus();
        let per = PerFileStrategy::new().build_full_graph(&p).unwrap();
        let per_edges = edge_set(&per);
        assert!(
            !per_edges.is_empty(),
            "expected at least one resolved call edge on the corpus; got 0 (H-R4-1 regression?)"
        );
    }

    /// W10.3: reproducibilidad — dos builds consecutivos con la
    /// misma entrada producen exactamente el mismo grafo (símbolos
    /// y aristas), sin importar orden del walk ni estado de cache.
    #[test]
    fn w10_repeated_builds_are_reproducible() {
        let p = corpus();

        let a = FullGraphStrategy::new().build_full_graph(&p).unwrap();
        let b = FullGraphStrategy::new().build_full_graph(&p).unwrap();
        assert_eq!(
            symbol_fqns(&a),
            symbol_fqns(&b),
            "full: symbol sets differ across runs"
        );
        assert_eq!(
            edge_set(&a),
            edge_set(&b),
            "full: edge sets differ across runs"
        );

        let c = PerFileStrategy::new().build_full_graph(&p).unwrap();
        let d = PerFileStrategy::new().build_full_graph(&p).unwrap();
        assert_eq!(
            symbol_fqns(&c),
            symbol_fqns(&d),
            "per_file: symbol sets differ across runs"
        );
        assert_eq!(
            edge_set(&c),
            edge_set(&d),
            "per_file: edge sets differ across runs"
        );
    }
}
