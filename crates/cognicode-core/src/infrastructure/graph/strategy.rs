//! Indexing strategies and the capabilities that belong to them.
//!
//! A strategy here indexes a project so its symbols can be found by
//! name. The capabilities that answer deeper questions — the graph of a
//! file, the hierarchy around a symbol, the graph of the project — live
//! with the types that can honour them, and the graph of the project
//! lives with the service that publishes its coverage report.

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::TraversalDirection;
use crate::infrastructure::graph::lightweight_index::LightweightIndex;
use crate::infrastructure::graph::on_demand_graph::{CallHierarchyResult, OnDemandGraphBuilder};
use crate::infrastructure::graph::per_file_graph::PerFileGraphCache;
use crate::infrastructure::graph::symbol_index::SymbolIndex;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// A strategy that indexes a project so its symbols can be found by name.
///
/// The trait carries the one capability that every strategy can honour
/// without lying: build the index of a project, and answer questions
/// about the symbols in it.
///
/// Building the graph of the project is not a strategy concern. There is
/// one way to build it, and it lives with the service that owns the
/// coverage report. A strategy that could not build a graph faithfully
/// used to answer `Complete` about a graph it had never measured, and
/// the honest answer to "I did not measure this" is not to offer the
/// capability at all.
pub trait GraphStrategy: Send + Sync {
    /// Builds the index of the project at `project_dir`.
    fn build_index(&mut self, project_dir: &Path) -> std::io::Result<()>;

    /// The locations of the symbols named `symbol_name`.
    ///
    /// An empty name asks for every symbol of the index.
    fn query_symbols(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::lightweight_index::SymbolLocation>;

    /// The name this strategy answers to.
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

    /// The hierarchy around a symbol, built when it is asked for.
    ///
    /// The graph is not held ready: it is built for the symbol that is
    /// asked about, at the depth that is asked for, and only then.
    pub fn build_subgraph(
        &self,
        symbol_name: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult {
        let mut builder = OnDemandGraphBuilder::with_index(self.index.clone());
        builder.build_for_symbol(symbol_name, depth, direction)
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

    fn name(&self) -> &'static str {
        "OnDemandStrategy"
    }
}

/// The strategy that builds and caches the graph of a file.
///
/// It builds the graph file-by-file and allows merging on demand. It
/// is not an indexing strategy: a strategy that cannot index a project
/// does not belong to the contract of the ones that do.
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

    /// Merge an explicit list of files and report what could not be read.
    ///
    /// The honest counterpart of building a project graph, for the case
    /// where the caller named the files instead of a directory to walk.
    /// It exists so [`PerFileGraphCache`] can stay private: the cache is
    /// the strategy's own state, and a caller that needs an explicit
    /// merge asks the strategy, not the cache behind it.
    pub fn merge_files_with_report(
        &self,
        file_paths: &[&Path],
    ) -> crate::infrastructure::graph::per_file_graph::BuildReport {
        self.cache.merge_with_report(file_paths)
    }

    /// The graph of one file, **with the references the file states and
    /// nobody declared**.
    ///
    /// The graph alone does not say them. A file can be read and still
    /// state a relation nobody ties to a declaration — the reference
    /// names something the file does not declare, or names several
    /// things and the build cannot tell which one it means. Those are
    /// facts about what the file says, and a consumer that gets the graph
    /// without them answers "this file states nothing" about a file that
    /// states plenty.
    ///
    /// The cache keeps the references the file states and nobody
    /// declared, and it keeps them on a cache hit too: serving the graph
    /// without them would answer "this file states nothing" about a file
    /// that states plenty.
    pub fn build_local_graph(
        &self,
        file_path: &Path,
    ) -> std::io::Result<(
        CallGraph,
        Vec<crate::domain::value_objects::PendingReference>,
    )> {
        let (graph, pending) = self.cache.get_or_build(file_path)?;
        Ok((graph.as_ref().clone(), pending))
    }
}

impl Default for PerFileStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// The strategy the index tool offers as `full`.
///
/// It indexes the whole project and answers symbol queries about it.
/// The graph of the project is not its business: there is one way to
/// build that graph, and it lives with the service that publishes the
/// coverage report.
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

    fn name(&self) -> &'static str {
        "FullGraphStrategy"
    }
}

/// A name that is not one of the strategies the index tool offers.
///
/// The refusal carries the names that do work, because a client that
/// asked for something that does not exist needs to know what does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStrategy {
    /// The name that was asked for.
    pub requested: String,
}

impl std::fmt::Display for UnknownStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown strategy '{}'; the strategies that index a project are: {}",
            self.requested,
            GraphStrategyFactory::available_strategies().join(", ")
        )
    }
}

impl std::error::Error for UnknownStrategy {}

/// Factory for creating graph strategies
pub struct GraphStrategyFactory;

impl GraphStrategyFactory {
    /// The strategy named `strategy`, if it is one that indexes.
    ///
    /// A name that is not offered is refused rather than answered with
    /// something else: a caller that asked for a strategy deserves to
    /// know that the strategy it asked for does not exist, not to be
    /// handed a different one under the name it used.
    pub fn create(strategy: &str) -> Result<Box<dyn GraphStrategy>, UnknownStrategy> {
        let built: Box<dyn GraphStrategy> = match strategy {
            "lightweight" => Box::new(LightweightStrategy::new()),
            "on_demand" | "ondemand" => Box::new(OnDemandStrategy::new()),
            "full" | "full_graph" => Box::new(FullGraphStrategy::new()),
            _ => {
                return Err(UnknownStrategy {
                    requested: strategy.to_string(),
                });
            }
        };
        Ok(built)
    }

    /// Returns a list of available strategy names
    pub fn available_strategies() -> Vec<&'static str> {
        vec!["lightweight", "on_demand", "full"]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FullGraphStrategy, GraphStrategy, GraphStrategyFactory, LightweightStrategy,
        OnDemandStrategy,
    };

    #[test]
    fn every_strategy_answers_to_its_name() {
        assert_eq!(LightweightStrategy::new().name(), "LightweightStrategy");
        assert_eq!(OnDemandStrategy::new().name(), "OnDemandStrategy");
        assert_eq!(FullGraphStrategy::new().name(), "FullGraphStrategy");
    }

    #[test]
    fn the_factory_answers_with_the_strategy_that_was_asked_for() {
        let lightweight = GraphStrategyFactory::create("lightweight").expect("lightweight indexes");
        assert_eq!(lightweight.name(), "LightweightStrategy");

        let on_demand = GraphStrategyFactory::create("on_demand").expect("on_demand indexes");
        assert_eq!(on_demand.name(), "OnDemandStrategy");

        let full = GraphStrategyFactory::create("full").expect("full indexes");
        assert_eq!(full.name(), "FullGraphStrategy");
    }

    #[test]
    fn the_factory_refuses_a_name_that_does_not_index() {
        let refused = match GraphStrategyFactory::create("per_file") {
            Ok(_) => {
                panic!("the per-file strategy does not index a project, and the factory built it")
            }
            Err(refused) => refused.to_string(),
        };
        assert!(
            refused.contains("lightweight"),
            "the refusal does not name the strategies that do index: {refused}"
        );
    }

    #[test]
    fn the_factory_offers_only_strategies_that_it_can_build() {
        for name in GraphStrategyFactory::available_strategies() {
            assert!(
                GraphStrategyFactory::create(name).is_ok(),
                "the factory offers '{name}' and then refuses to build it"
            );
        }
    }
}
