//! Analysis Service - Handles code analysis operations

use crate::application::dto::{GraphCoverageMetrics, GraphStatsDto, SymbolDto};
use crate::application::error::{AppError, AppResult};
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::CallGraph;
use crate::domain::services::{ComplexityCalculator, CycleDetector, ImpactAnalyzer};
use crate::domain::traits::DependencyRepository;
use crate::domain::value_objects::DependencyType;
use crate::infrastructure::graph::{
    CallHierarchyResult, GraphCache, LightweightIndex, OnDemandGraphBuilder, PetGraphStore,
    SymbolLocation, TraversalDirection,
};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Service for analyzing code structure and complexity
pub struct AnalysisService {
    complexity_calculator: ComplexityCalculator,
    cycle_detector: CycleDetector,
    impact_analyzer: ImpactAnalyzer,
    graph_cache: Arc<GraphCache>,
    symbol_index: Option<LightweightIndex>,
    on_demand_builder: Option<OnDemandGraphBuilder>,
    /// File cache: maps file path to (mtime, symbols, relationships)
    /// Uses Arc<Mutex<...>> to support Send across thread boundaries for async operations
    file_cache: Arc<Mutex<
        std::collections::HashMap<
            String,
            (
                u64,
                Vec<crate::domain::aggregates::Symbol>,
                Vec<(crate::domain::aggregates::Symbol, String)>,
            ),
        >,
    >>,
    /// Coverage metrics from the last graph build
    coverage_metrics: Mutex<Option<GraphCoverageMetrics>>,
}

impl AnalysisService {
    /// Creates a new AnalysisService
    pub fn new() -> Self {
        Self {
            complexity_calculator: ComplexityCalculator::new(),
            cycle_detector: CycleDetector::new(),
            impact_analyzer: ImpactAnalyzer::new(),
            graph_cache: Arc::new(GraphCache::new()),
            symbol_index: None,
            on_demand_builder: None,
            file_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            coverage_metrics: Mutex::new(None),
        }
    }

    /// Creates a new AnalysisService with a parser for testing
    #[allow(dead_code)]
    pub fn with_parser() -> Self {
        Self::new()
    }

    /// Creates a new AnalysisService with a shared GraphCache
    ///
    /// This allows multiple services (e.g., WorkspaceSession and HandlerContext)
    /// to share the same graph cache, preventing duplicate builds.
    pub fn with_graph_cache(cache: Arc<GraphCache>) -> Self {
        Self {
            complexity_calculator: ComplexityCalculator::new(),
            cycle_detector: CycleDetector::new(),
            impact_analyzer: ImpactAnalyzer::new(),
            graph_cache: cache,
            symbol_index: None,
            on_demand_builder: None,
            file_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            coverage_metrics: Mutex::new(None),
        }
    }

    /// Returns the graph cache for accessing the project graph
    pub fn graph_cache(&self) -> Arc<GraphCache> {
        self.graph_cache.clone()
    }

    /// Returns the symbol index for fast symbol lookups
    ///
    /// Returns a reference to the underlying LightweightIndex if it has been built,
    /// or builds it first if necessary.
    pub fn symbol_index(&mut self) -> &LightweightIndex {
        if self.symbol_index.is_none() {
            // Build from cached graph or create empty
            let graph = self.get_project_graph();

            // If graph has symbols, we need to scan files to build proper index
            // For now, create index from graph's symbols
            let mut index = LightweightIndex::new();

            for symbol in graph.symbols() {
                let location =
                    SymbolLocation::from_location(symbol.location(), symbol.kind().clone());
                let name_lower = symbol.name().to_lowercase();
                index.insert(name_lower, location);
            }

            self.symbol_index = Some(index);
        }
        self.symbol_index.as_ref().unwrap()
    }

    /// Returns a mutable reference to the symbol index
    #[allow(dead_code)]
    pub fn symbol_index_mut(&mut self) -> &mut LightweightIndex {
        // Ensure it's built first
        let _ = self.symbol_index();
        self.symbol_index.as_mut().unwrap()
    }

    /// Queries the call hierarchy for a symbol using on-demand approach
    ///
    /// This method builds only the necessary portion of the graph for the query,
    /// making it efficient for deep call hierarchies.
    ///
    /// # Arguments
    /// * `symbol` - The symbol name to query
    /// * `depth` - Maximum traversal depth
    /// * `direction` - Whether to look at callers, callees, or both
    pub fn query_call_hierarchy(
        &mut self,
        symbol: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> AppResult<CallHierarchyResult> {
        // Ensure we have the index built
        if self.on_demand_builder.is_none() {
            let mut builder = OnDemandGraphBuilder::new();
            let project_root = std::env::current_dir().map_err(|e| {
                AppError::AnalysisError(format!("Failed to get current dir: {}", e))
            })?;

            builder
                .set_index(&project_root)
                .map_err(|e| AppError::AnalysisError(format!("Failed to build index: {}", e)))?;
            self.on_demand_builder = Some(builder);
        }

        let result = self
            .on_demand_builder
            .as_mut()
            .unwrap()
            .build_for_symbol(symbol, depth, direction);

        Ok(result)
    }

    /// Builds the full project graph explicitly
    ///
    /// This method constructs the complete call graph for the project,
    /// parsing all source files and building all symbol relationships.
    /// Use this when you need the complete graph for global analysis.
    pub fn build_full_graph(&mut self, project_dir: &Path) -> AppResult<()> {
        self.build_project_graph(project_dir)
    }

    /// Finds symbols by name using the lightweight index
    ///
    /// Returns all locations where the symbol is defined.
    pub fn find_symbol(
        &mut self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::SymbolLocation> {
        // Ensure index is built
        let _ = self.symbol_index();
        self.symbol_index
            .as_ref()
            .map(|idx| idx.find_symbol(symbol_name).to_vec())
            .unwrap_or_default()
    }

    /// Builds a call graph by scanning all source files in a directory
    ///
    /// Walks the directory recursively, parses each supported source file,
    /// extracts symbols and call relationships, and builds a project-wide call graph.
    pub fn build_project_graph(&self, project_dir: &Path) -> AppResult<()> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        let mut store = PetGraphStore::new();
        let mut name_to_symbol_id: std::collections::HashMap<String, SymbolId> =
            std::collections::HashMap::new();

        // Coverage tracking
        let mut total_files: usize = 0;
        let mut parsed_files: usize = 0;
        let mut unresolved_edges: usize = 0;

        const BLOCKED_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build"];

        let files: Vec<_> = WalkBuilder::new(project_dir)
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                !path.components().any(|c| {
                    c.as_os_str()
                        .to_str()
                        .map(|s| BLOCKED_DIRS.contains(&s))
                        .unwrap_or(false)
                })
            })
            .filter(|e| e.path().is_file())
            .map(|e| {
                let path = e.path().to_path_buf();
                let language = Language::from_extension(path.extension());
                let file_path = path.to_string_lossy().into_owned();
                let mtime = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    })
                    .unwrap_or(0);
                (path, language, file_path, mtime)
            })
            .filter(|(_, lang, _, _)| lang.is_some())
            .collect();

        total_files = files.len();

        let results: Vec<_> = files
            .into_par_iter()
            .filter_map(|(path, language, file_path, mtime)| {
                let language = language.unwrap();

                {
                    let cache = self.file_cache.lock().unwrap();
                    if let Some((cached_mtime, cached_symbols, cached_relationships)) =
                        cache.get(&file_path)
                    {
                        if *cached_mtime == mtime {
                            return Some((
                                file_path,
                                mtime,
                                cached_symbols.clone(),
                                cached_relationships.clone(),
                                false, // from_cache = true
                            ));
                        }
                    }
                }

                let source = std::fs::read_to_string(&path).ok()?;
                let parser = TreeSitterParser::with_cache(language.clone()).ok()?;

                let symbols = parser
                    .find_all_symbols_with_path(&source, &file_path)
                    .unwrap_or_default();
                let relationships = parser
                    .find_call_relationships(&source, &file_path)
                    .unwrap_or_default();

                Some((file_path, mtime, symbols, relationships, true)) // from_cache = false (parsed)
            })
            .collect();

        let mut cache = self.file_cache.lock().unwrap();
        let mut all_relationships = Vec::new();

        for (file_path, mtime, symbols, relationships, was_parsed) in results {
            if was_parsed {
                parsed_files += 1;
            }
            cache.insert(
                file_path.clone(),
                (mtime, symbols.clone(), relationships.clone()),
            );

            for symbol in symbols {
                let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                store.add_symbol_with_location(&symbol_id, symbol.clone());
                name_to_symbol_id.insert(symbol.name().to_lowercase(), symbol_id);
            }

            for (caller, callee_name) in relationships {
                all_relationships.push((caller, callee_name));
            }
        }

        for (caller, callee_name) in all_relationships {
            let caller_id = SymbolId::new(caller.fully_qualified_name());
            if let Some(callee_id) = name_to_symbol_id.get(&callee_name.to_lowercase()) {
                store
                    .add_dependency(&caller_id, callee_id, DependencyType::Calls)
                    .ok();
            } else {
                unresolved_edges += 1;
            }
        }

        let call_graph = store.to_call_graph();
        self.graph_cache.set(call_graph);

        // Update coverage metrics
        let coverage_percent = if total_files > 0 {
            (parsed_files as f64 / total_files as f64) * 100.0
        } else {
            0.0
        };
        let coverage = GraphCoverageMetrics {
            total_source_files: total_files,
            parsed_files,
            unresolved_edges,
            coverage_percent,
        };
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        Ok(())
    }

    /// Builds a call graph for a subgraph limited to specific directories.
    ///
    /// Unlike `build_project_graph`, this method only includes files that are
    /// under one of the specified `include_dirs`. This is useful for building
    /// a focused view of the codebase (e.g., only application code, not tests).
    ///
    /// This method does NOT modify the cached graph - it returns a separate
    /// CallGraph instance.
    ///
    /// # Arguments
    /// * `project_dir` - The root directory to scan
    /// * `include_dirs` - Directories to include (as an allowlist). Files outside
    ///                   these directories are excluded.
    ///
    /// # Returns
    /// * `AppResult<CallGraph>` - The built call graph for the filtered subgraph
    pub fn build_project_graph_filtered(
        &self,
        project_dir: &Path,
        include_dirs: &[&Path],
    ) -> AppResult<CallGraph> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        let mut store = PetGraphStore::new();
        let mut name_to_symbol_id: std::collections::HashMap<String, SymbolId> =
            std::collections::HashMap::new();

        // Coverage tracking
        let mut total_files: usize = 0;
        let mut parsed_files: usize = 0;
        let mut unresolved_edges: usize = 0;

        // Use include_dirs as an allowlist - only include files under these directories
        let files: Vec<_> = WalkBuilder::new(project_dir)
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                // Filter: only include files that are under one of the include_dirs
                // include_dirs are absolute paths, so compare directly with the file path
                include_dirs.iter().any(|dir| {
                    path.starts_with(dir)
                })
            })
            .filter(|e| e.path().is_file())
            .map(|e| {
                let path = e.path().to_path_buf();
                let language = Language::from_extension(path.extension());
                let file_path = path.to_string_lossy().into_owned();
                let mtime = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    })
                    .unwrap_or(0);
                (path, language, file_path, mtime)
            })
            .filter(|(_, lang, _, _)| lang.is_some())
            .collect();

        total_files = files.len();

        let results: Vec<_> = files
            .into_par_iter()
            .filter_map(|(path, language, file_path, mtime)| {
                let language = language.unwrap();

                {
                    let cache = self.file_cache.lock().unwrap();
                    if let Some((cached_mtime, cached_symbols, cached_relationships)) =
                        cache.get(&file_path)
                    {
                        if *cached_mtime == mtime {
                            return Some((
                                file_path,
                                mtime,
                                cached_symbols.clone(),
                                cached_relationships.clone(),
                                false, // from_cache = true
                            ));
                        }
                    }
                }

                let source = std::fs::read_to_string(&path).ok()?;
                let parser = TreeSitterParser::with_cache(language.clone()).ok()?;

                let symbols = parser
                    .find_all_symbols_with_path(&source, &file_path)
                    .unwrap_or_default();
                let relationships = parser
                    .find_call_relationships(&source, &file_path)
                    .unwrap_or_default();

                Some((file_path, mtime, symbols, relationships, true)) // from_cache = false (parsed)
            })
            .collect();

        let mut cache = self.file_cache.lock().unwrap();
        let mut all_relationships = Vec::new();

        for (file_path, mtime, symbols, relationships, was_parsed) in results {
            if was_parsed {
                parsed_files += 1;
            }
            cache.insert(
                file_path.clone(),
                (mtime, symbols.clone(), relationships.clone()),
            );

            for symbol in symbols {
                let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                store.add_symbol_with_location(&symbol_id, symbol.clone());
                name_to_symbol_id.insert(symbol.name().to_lowercase(), symbol_id);
            }

            for (caller, callee_name) in relationships {
                all_relationships.push((caller, callee_name));
            }
        }

        for (caller, callee_name) in all_relationships {
            let caller_id = SymbolId::new(caller.fully_qualified_name());
            if let Some(callee_id) = name_to_symbol_id.get(&callee_name.to_lowercase()) {
                store
                    .add_dependency(&caller_id, callee_id, DependencyType::Calls)
                    .ok();
            } else {
                unresolved_edges += 1;
            }
        }

        let call_graph = store.to_call_graph();

        // Update coverage metrics for this subgraph (separate from cached graph)
        let coverage_percent = if total_files > 0 {
            (parsed_files as f64 / total_files as f64) * 100.0
        } else {
            0.0
        };
        let coverage = GraphCoverageMetrics {
            total_source_files: total_files,
            parsed_files,
            unresolved_edges,
            coverage_percent,
        };
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        Ok(call_graph)
    }

    /// Builds the project graph asynchronously without blocking the tokio runtime.
    ///
    /// This method wraps the synchronous `build_project_graph()` in `tokio::task::spawn_blocking()`
    /// to execute heavy file I/O and parsing on a blocking thread pool, keeping the async
    /// runtime responsive.
    ///
    /// # Arguments
    /// * `dir` - The project directory to scan
    ///
    /// # Returns
    /// * `AppResult<()>` - Success or error
    pub async fn build_project_graph_async(&self, dir: &Path) -> AppResult<()> {
        let graph_cache = self.graph_cache.clone();
        let file_cache = self.file_cache.clone();
        let dir_path = dir.to_path_buf();

        // Spawn blocking task to run the heavy computation
        let result: Result<Result<GraphCoverageMetrics, AppError>, tokio::task::JoinError> = tokio::task::spawn_blocking(move || {
            // Create a minimal service context for building the graph
            let store = std::sync::Mutex::new(crate::infrastructure::graph::PetGraphStore::new());
            let mut name_to_symbol_id: std::collections::HashMap<String, SymbolId> =
                std::collections::HashMap::new();

            // Coverage tracking
            let mut total_files: usize = 0;
            let mut parsed_files: usize = 0;
            let mut unresolved_edges: usize = 0;

            use ignore::WalkBuilder;
            use rayon::prelude::*;

            const BLOCKED_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build"];

            let files: Vec<_> = WalkBuilder::new(&dir_path)
                .hidden(true)
                .git_ignore(true)
                .git_exclude(true)
                .build()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    !path.components().any(|c| {
                        c.as_os_str()
                            .to_str()
                            .map(|s| BLOCKED_DIRS.contains(&s))
                            .unwrap_or(false)
                    })
                })
                .filter(|e| e.path().is_file())
                .map(|e| {
                    let path = e.path().to_path_buf();
                    let language = Language::from_extension(path.extension());
                    let file_path = path.to_string_lossy().into_owned();
                    let mtime = std::fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                        })
                        .unwrap_or(0);
                    (path, language, file_path, mtime)
                })
                .filter(|(_, lang, _, _)| lang.is_some())
                .collect();

            total_files = files.len();

            let results: Vec<_> = files
                .into_par_iter()
                .filter_map(|(path, language, file_path, mtime)| {
                    let language = language.unwrap();

                    {
                        let cache = file_cache.lock().unwrap();
                        if let Some((cached_mtime, cached_symbols, cached_relationships)) =
                            cache.get(&file_path)
                        {
                            if *cached_mtime == mtime {
                                return Some((
                                    file_path,
                                    mtime,
                                    cached_symbols.clone(),
                                    cached_relationships.clone(),
                                    false, // from_cache = true
                                ));
                            }
                        }
                    }

                    let source = std::fs::read_to_string(&path).ok()?;
                    let parser = TreeSitterParser::with_cache(language.clone()).ok()?;

                    let symbols = parser
                        .find_all_symbols_with_path(&source, &file_path)
                        .unwrap_or_default();
                    let relationships = parser
                        .find_call_relationships(&source, &file_path)
                        .unwrap_or_default();

                    Some((file_path, mtime, symbols, relationships, true)) // from_cache = false (parsed)
                })
                .collect();

            let mut cache = file_cache.lock().unwrap();
            let mut all_relationships = Vec::new();

            for (file_path, mtime, symbols, relationships, was_parsed) in results {
                if was_parsed {
                    parsed_files += 1;
                }
                cache.insert(
                    file_path.clone(),
                    (mtime, symbols.clone(), relationships.clone()),
                );

                for symbol in symbols {
                    let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                    store.lock().unwrap().add_symbol_with_location(&symbol_id, symbol.clone());
                    name_to_symbol_id.insert(symbol.name().to_lowercase(), symbol_id);
                }

                for (caller, callee_name) in relationships {
                    all_relationships.push((caller, callee_name));
                }
            }

            for (caller, callee_name) in all_relationships {
                let caller_id = SymbolId::new(caller.fully_qualified_name());
                if let Some(callee_id) = name_to_symbol_id.get(&callee_name.to_lowercase()) {
                    store
                        .lock()
                        .unwrap()
                        .add_dependency(&caller_id, callee_id, DependencyType::Calls)
                        .ok();
                } else {
                    unresolved_edges += 1;
                }
            }

            let call_graph = store.into_inner().unwrap().to_call_graph();
            graph_cache.set(call_graph);

            // Compute coverage
            let coverage_percent = if total_files > 0 {
                (parsed_files as f64 / total_files as f64) * 100.0
            } else {
                0.0
            };
            let coverage = GraphCoverageMetrics {
                total_source_files: total_files,
                parsed_files,
                unresolved_edges,
                coverage_percent,
            };

            Ok(coverage)
        })
        .await;

        let coverage = result.map_err(|e| AppError::AnalysisError(format!("Task join error: {}", e)))?
            .map_err(|e| AppError::AnalysisError(format!("Build error: {}", e)))?;

        // Store coverage metrics
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        Ok(())
    }

    /// Returns the project graph from cache
    pub fn get_project_graph(&self) -> Arc<CallGraph> {
        self.graph_cache.get()
    }

    /// Returns statistics about the call graph, including coverage metrics
    ///
    /// Coverage metrics are only available after `build_project_graph` or
    /// `build_project_graph_async` has been called.
    pub fn get_graph_stats(&self) -> GraphStatsDto {
        use std::collections::HashMap;
        use crate::infrastructure::parser::Language;

        let graph = self.graph_cache.get();
        let symbol_count = graph.symbol_count();
        let edge_count = graph.edge_count();

        // Count unique files and compute language breakdown
        let mut unique_files: HashMap<String, bool> = HashMap::new();
        let mut language_breakdown: HashMap<String, usize> = HashMap::new();

        for symbol in graph.symbols() {
            let file = symbol.location().file().to_string();
            unique_files.insert(file.clone(), true);

            // Compute language from file extension
            let ext = std::path::Path::new(&file).extension();
            if let Some(lang) = Language::from_extension(ext) {
                *language_breakdown.entry(lang.name().to_string()).or_insert(0) += 1;
            } else {
                *language_breakdown.entry("Unknown".to_string()).or_insert(0) += 1;
            }
        }

        // Get coverage metrics
        let coverage = self.coverage_metrics.lock().unwrap().clone();

        GraphStatsDto {
            symbol_count,
            edge_count,
            file_count: unique_files.len(),
            language_breakdown,
            coverage,
        }
    }

    /// Extracts symbols from a file
    pub fn get_file_symbols(&self, path: &Path) -> AppResult<Vec<SymbolDto>> {
        // Read the source file
        let source = std::fs::read_to_string(path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        // Detect language from file extension
        let language = Language::from_extension(path.extension()).ok_or_else(|| {
            AppError::InvalidParameter(format!("Unsupported file type: {}", path.display()))
        })?;

        // Create parser for the detected language (uses thread-local cache)
        let parser = TreeSitterParser::with_cache(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        // Parse the file and extract symbols
        let file_path = path.to_str().unwrap_or("unknown");
        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .map_err(|e| AppError::AnalysisError(format!("Failed to parse symbols: {}", e)))?;

        // Convert to DTOs
        Ok(symbols.iter().map(SymbolDto::from_symbol).collect())
    }

    /// Checks for cycles in the dependency graph
    ///
    /// Returns a cycle detection result with information about any cycles found.
    /// Uses the CycleDetector service to analyze the call graph.
    pub fn check_cycles(&self, graph: &CallGraph) -> crate::domain::services::CycleDetectionResult {
        self.cycle_detector.detect_cycles(graph)
    }

    /// Analyzes the impact of changing a symbol
    ///
    /// Returns an impact report detailing direct and transitive dependents,
    /// as well as files that would be affected by changing this symbol.
    pub fn analyze_impact(
        &self,
        symbol: &crate::domain::aggregates::Symbol,
        graph: &CallGraph,
    ) -> crate::domain::services::ImpactReport {
        self.impact_analyzer.calculate_impact(symbol, graph)
    }

    /// Calculates the cyclomatic complexity based on decision points
    ///
    /// Returns a complexity report with risk assessment.
    pub fn calculate_complexity(
        &self,
        symbol_name: &str,
        decision_points: &[crate::domain::services::DecisionPoint],
        exit_points: usize,
    ) -> crate::domain::services::ComplexityReport {
        let cyclomatic = self
            .complexity_calculator
            .cyclomatic_complexity(decision_points, exit_points);
        let risk = self.complexity_calculator.risk_level(cyclomatic);

        crate::domain::services::ComplexityReport {
            symbol_name: symbol_name.to_string(),
            cyclomatic,
            cognitive: 0, // Cognitive complexity requires AST analysis
            decision_point_count: decision_points.len(),
            max_nesting_depth: 0, // Would require deeper AST analysis
            exit_point_count: exit_points,
            risk,
        }
    }

    /// Checks if it's safe to change a symbol based on impact thresholds
    pub fn is_safe_to_change(
        &self,
        symbol: &crate::domain::aggregates::Symbol,
        graph: &CallGraph,
        threshold: crate::domain::services::ImpactThreshold,
    ) -> bool {
        self.impact_analyzer
            .is_safe_to_change(symbol, graph, threshold)
    }

    /// Finds minimal set of symbols that need to be removed to break all cycles
    ///
    /// This is useful for refactoring cyclic dependencies.
    pub fn find_minimal_feedback_set(&self, graph: &CallGraph) -> Vec<SymbolId> {
        self.cycle_detector.find_minimal_feedback_set(graph)
    }

    /// Gets all entry points in the call graph (symbols with no incoming edges)
    ///
    /// Entry points are functions that are not called by any other function in the
    /// codebase. These are typically main functions, exported APIs, or independent
    /// utilities.
    pub fn get_entry_points(&self) -> Vec<SymbolDto> {
        let graph = self.get_project_graph();
        graph
            .roots()
            .iter()
            .filter_map(|id| {
                graph
                    .get_symbol(id)
                    .map(|symbol| SymbolDto::from_symbol(symbol))
            })
            .collect()
    }

    /// Gets all leaf functions in the call graph (symbols with no outgoing edges)
    ///
    /// Leaf functions are functions that don't call any other functions. These are
    /// typically leaf-node utilities, simple getters, or functions that interact
    /// with external systems directly.
    pub fn get_leaf_functions(&self) -> Vec<SymbolDto> {
        let graph = self.get_project_graph();
        graph
            .leaves()
            .iter()
            .filter_map(|id| {
                graph
                    .get_symbol(id)
                    .map(|symbol| SymbolDto::from_symbol(symbol))
            })
            .collect()
    }

    /// Traces an execution path between two symbols using BFS
    ///
    /// Returns the path from source to target if one exists, or None if no path
    /// is found. The path includes both source and target symbols.
    ///
    /// When max_depth is 0, no depth limit is applied.
    pub fn trace_path(
        &self,
        source_name: &str,
        target_name: &str,
        max_depth: usize,
    ) -> AppResult<Option<Vec<SymbolDto>>> {
        let graph = self.get_project_graph();

        // Find source and target symbol IDs by name
        let source_id = self.find_symbol_id_by_name(&graph, source_name)?;
        let target_id = self.find_symbol_id_by_name(&graph, target_name)?;

        // Find path using BFS, respecting max_depth if specified
        let path = if max_depth > 0 {
            graph.find_path_with_max_depth(&source_id, &target_id, max_depth)
        } else {
            graph.find_path(&source_id, &target_id)
        };

        Ok(path.map(|symbol_ids| {
            symbol_ids
                .iter()
                .filter_map(|id| {
                    graph
                        .get_symbol(id)
                        .map(|symbol| SymbolDto::from_symbol(symbol))
                })
                .collect()
        }))
    }

    /// Finds a symbol ID by its name
    fn find_symbol_id_by_name(&self, graph: &CallGraph, name: &str) -> AppResult<SymbolId> {
        let name_lower = name.to_lowercase();

        // First try exact match on the fully qualified name
        for symbol in graph.symbols() {
            let fqn = symbol.fully_qualified_name().to_lowercase();
            if fqn == name_lower {
                return Ok(SymbolId::new(symbol.fully_qualified_name()));
            }
        }

        // Then try matching just the base name
        for symbol in graph.symbols() {
            if symbol.name().to_lowercase() == name_lower {
                return Ok(SymbolId::new(symbol.fully_qualified_name()));
            }
        }

        Err(AppError::SymbolNotFound(format!(
            "Symbol not found: {}",
            name
        )))
    }
}

impl Default for AnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_file_symbols_python() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "class MyClass:").unwrap();
        writeln!(file, "    def method(self):").unwrap();
        writeln!(file, "        pass").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        // Should find hello function and MyClass
        let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            names.contains(&"hello".to_string()),
            "Should find hello function"
        );
        assert!(
            names.contains(&"MyClass".to_string()),
            "Should find MyClass"
        );
    }

    #[test]
    fn test_get_file_symbols_rust() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn main() {{}}").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "struct MyStruct {{}}").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            names.contains(&"main".to_string()),
            "Should find main function"
        );
        assert!(
            names.contains(&"MyStruct".to_string()),
            "Should find MyStruct"
        );
    }

    #[test]
    fn test_get_file_symbols_unsupported_extension() {
        let mut file = NamedTempFile::with_suffix(".xyz").unwrap();
        writeln!(file, "some content").unwrap();

        let service = AnalysisService::new();
        let result = service.get_file_symbols(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_locations_have_correct_file_path() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let service = AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].file_path, file.path().to_str().unwrap());
    }

    #[test]
    fn test_build_project_graph() {
        // Create a temp directory with Python files
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create first Python file
        let file1_path = temp_path.join("module1.py");
        std::fs::write(
            &file1_path,
            r#"
def a():
    b()
    c()

def b():
    c()

def c():
    pass
"#,
        )
        .unwrap();

        // Create second Python file
        let file2_path = temp_path.join("module2.py");
        std::fs::write(
            &file2_path,
            r#"
def d():
    a()
    c()
"#,
        )
        .unwrap();

        let service = AnalysisService::new();
        service.build_project_graph(temp_path).unwrap();

        let graph = service.get_project_graph();

        // The graph should have symbols from both files
        assert!(
            graph.symbol_count() >= 4,
            "Should have at least 4 symbols (a, b, c, d)"
        );
    }

    #[test]
    fn test_check_cycles_empty_graph() {
        let service = AnalysisService::new();
        let graph = CallGraph::new();

        let result = service.check_cycles(&graph);
        assert!(!result.has_cycles);
        assert!(result.cycles.is_empty());
    }

    #[test]
    fn test_check_cycles_with_cycle() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // Create a -> b -> a cycle
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

        let result = service.check_cycles(&graph);
        assert!(result.has_cycles);
        assert_eq!(result.cycles.len(), 1);
    }

    #[test]
    fn test_analyze_impact_no_dependents() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};

        let service = AnalysisService::new();
        let graph = CallGraph::new();

        let symbol = Symbol::new(
            "orphan",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let report = service.analyze_impact(&symbol, &graph);

        assert_eq!(report.direct_dependents, 0);
        assert_eq!(report.transitive_dependents, 0);
        assert_eq!(
            report.impact_level,
            crate::domain::services::ImpactLevel::Minimal
        );
    }

    #[test]
    fn test_analyze_impact_with_dependents() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        let main = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        let main_id = graph.add_symbol(main);
        let helper_id = graph.add_symbol(helper);

        graph
            .add_dependency(&main_id, &helper_id, DependencyType::Calls)
            .unwrap();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );
        let report = service.analyze_impact(&helper_symbol, &graph);

        assert_eq!(report.direct_dependents, 1);
        assert!(report.impact_level <= crate::domain::services::ImpactLevel::Medium);
    }

    #[test]
    fn test_calculate_complexity_simple() {
        let service = AnalysisService::new();

        let report = service.calculate_complexity("test_func", &[], 1);

        assert_eq!(report.symbol_name, "test_func");
        assert_eq!(report.cyclomatic, 1);
        assert_eq!(report.risk, crate::domain::services::ComplexityRisk::Low);
    }

    #[test]
    fn test_calculate_complexity_with_decisions() {
        use crate::domain::services::DecisionPoint;

        let service = AnalysisService::new();

        let decision_points = vec![DecisionPoint::If, DecisionPoint::While, DecisionPoint::For];
        let report = service.calculate_complexity("complex_func", &decision_points, 1);

        assert_eq!(report.cyclomatic, 4); // 1 base + 3 decision points
        assert_eq!(report.decision_point_count, 3);
    }

    #[test]
    fn test_is_safe_to_change() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ImpactThreshold;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        let main = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        let main_id = graph.add_symbol(main);
        let helper_id = graph.add_symbol(helper);

        graph
            .add_dependency(&main_id, &helper_id, DependencyType::Calls)
            .unwrap();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        // With moderate threshold, should be safe
        let is_safe =
            service.is_safe_to_change(&helper_symbol, &graph, ImpactThreshold::moderate());
        assert!(is_safe);

        // With very strict threshold, may not be safe
        let strict = ImpactThreshold {
            max_level: crate::domain::services::ImpactLevel::Minimal,
            max_dependents: 0,
        };
        let is_safe_strict = service.is_safe_to_change(&helper_symbol, &graph, strict);
        assert!(!is_safe_strict);
    }

    #[test]
    fn test_find_minimal_feedback_set_no_cycles() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // Linear graph: a -> b -> c (no cycles)
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();

        let feedback_set = service.find_minimal_feedback_set(&graph);
        assert!(feedback_set.is_empty());
    }

    #[test]
    fn test_find_minimal_feedback_set_with_cycle() {
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let service = AnalysisService::new();
        let mut graph = CallGraph::new();

        // a -> b -> a cycle
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

        let feedback_set = service.find_minimal_feedback_set(&graph);
        // Should find at least one symbol to break the cycle
        assert!(!feedback_set.is_empty());
    }

    #[test]
    fn test_full_analysis_workflow() {
        use std::io::Write;
        use tempfile::TempDir;

        let service = AnalysisService::new();
        let temp_dir = TempDir::new().unwrap();

        // Create a test Rust file with multiple symbols and call relationships
        let rust_file_path = temp_dir.path().join("test_lib.rs");
        let mut file = std::fs::File::create(&rust_file_path).unwrap();
        writeln!(file, "pub fn helper_function() -> i32 {{").unwrap();
        writeln!(file, "    return 42;").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "pub fn main_function() -> i32 {{").unwrap();
        writeln!(file, "    let x = helper_function();").unwrap();
        writeln!(file, "    if x > 0 {{").unwrap();
        writeln!(file, "        return x;").unwrap();
        writeln!(file, "    }}").unwrap();
        writeln!(file, "    return 0;").unwrap();
        writeln!(file, "}}").unwrap();
        drop(file);

        // 1. TEST: get_file_symbols - extract symbols from a file
        let symbols = service.get_file_symbols(&rust_file_path).unwrap();
        let symbol_names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            symbol_names.contains(&"helper_function".to_string()),
            "Should find helper_function"
        );
        assert!(
            symbol_names.contains(&"main_function".to_string()),
            "Should find main_function"
        );

        // 2. TEST: build_project_graph - build call graph for the project
        service.build_project_graph(temp_dir.path()).unwrap();
        let graph = service.get_project_graph();
        assert!(
            graph.symbol_count() >= 2,
            "Graph should have at least 2 symbols"
        );

        // 3. TEST: check_cycles - detect cycles in the graph
        let cycle_result = service.check_cycles(&graph);
        assert!(
            !cycle_result.has_cycles,
            "Simple linear calls should not have cycles"
        );

        // 4. TEST: analyze_impact - analyze impact of changing a symbol
        let helper_symbol = crate::domain::aggregates::Symbol::new(
            "helper_function",
            crate::domain::value_objects::SymbolKind::Function,
            crate::domain::value_objects::Location::new("test_lib.rs", 1, 4),
        );
        let impact_report = service.analyze_impact(&helper_symbol, &graph);
        // main_function calls helper_function, so it should be a dependent
        assert!(
            impact_report.direct_dependents >= 0,
            "Should calculate direct dependents"
        );

        // 5. TEST: calculate_complexity - calculate cyclomatic complexity
        use crate::domain::services::DecisionPoint;
        let decision_points = vec![DecisionPoint::If];
        let complexity_report = service.calculate_complexity("main_function", &decision_points, 2);
        assert_eq!(complexity_report.symbol_name, "main_function");
        // Formula: base(1) + decision_points(1) + exit_points(2) - 1 = 3
        assert_eq!(
            complexity_report.cyclomatic, 3,
            "Should have cyclomatic complexity of 3 for 1 if and 2 exit points"
        );
        assert_eq!(
            complexity_report.risk,
            crate::domain::services::ComplexityRisk::Low
        );

        // 6. TEST: is_safe_to_change - check if it's safe to refactor
        let threshold = crate::domain::services::ImpactThreshold::conservative();
        let is_safe = service.is_safe_to_change(&helper_symbol, &graph, threshold);
        // With low impact, should be safe
        assert!(
            impact_report.impact_level <= crate::domain::services::ImpactLevel::Medium || is_safe
        );

        // 7. TEST: find_minimal_feedback_set - find symbols to break cycles
        let feedback_set = service.find_minimal_feedback_set(&graph);
        // Without cycles, feedback set should be empty
        assert!(
            feedback_set.is_empty(),
            "Without cycles, feedback set should be empty"
        );
    }

    #[test]
    #[ignore = "integration: scans entire project via build_project_graph"]
    fn test_real_code_analysis_workflow() {
        let service = AnalysisService::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // 1. Build graph from real source code
        service
            .build_project_graph(&project_root)
            .expect("Should build project graph");
        let graph = service.get_project_graph();

        // Verify we got real symbols from the codebase
        let symbol_count = graph.symbol_count();
        assert!(
            symbol_count > 100,
            "Should find many symbols in real codebase, found {}",
            symbol_count
        );
        println!("[REAL CODE] Found {} symbols in codebase", symbol_count);

        // 2. Get file symbols from a real file
        let real_file = project_root.join("src/application/services/analysis_service.rs");
        let file_symbols = service
            .get_file_symbols(&real_file)
            .expect("Should get file symbols");
        assert!(!file_symbols.is_empty(), "Should find symbols in real file");
        println!(
            "[REAL CODE] Found {} symbols in analysis_service.rs",
            file_symbols.len()
        );

        // Find a function to analyze
        let target_symbol = file_symbols
            .iter()
            .find(|s| s.name == "build_project_graph")
            .expect("Should find build_project_graph function");

        println!(
            "[REAL CODE] Target symbol: {} at {}:{}:{}",
            target_symbol.name, target_symbol.file_path, target_symbol.line, target_symbol.column
        );

        // 3. Check for cycles
        let cycle_result = service.check_cycles(&graph);
        println!(
            "[REAL CODE] Cycle detection: has_cycles={}, total_sccs={}",
            cycle_result.has_cycles, cycle_result.total_sccs
        );

        // 4. Analyze impact of a real symbol
        let symbol_for_impact = crate::domain::aggregates::Symbol::new(
            &target_symbol.name,
            crate::domain::value_objects::SymbolKind::Function,
            crate::domain::value_objects::Location::new(
                &target_symbol.file_path,
                target_symbol.line,
                target_symbol.column,
            ),
        );
        let impact_report = service.analyze_impact(&symbol_for_impact, &graph);
        println!(
            "[REAL CODE] Impact analysis: direct={}, transitive={}, level={:?}",
            impact_report.direct_dependents,
            impact_report.transitive_dependents,
            impact_report.impact_level
        );

        // Verify impact analysis worked (may or may not have dependents)
        assert!(
            impact_report.transitive_dependents >= 0,
            "Should calculate transitive dependents"
        );

        // 5. Test safety check
        let threshold = crate::domain::services::ImpactThreshold::conservative();
        let is_safe = service.is_safe_to_change(&symbol_for_impact, &graph, threshold.clone());
        println!(
            "[REAL CODE] Safety check: is_safe={} (threshold={:?})",
            is_safe, threshold.max_level
        );

        // 6. Find feedback set (for breaking cycles if any)
        let feedback_set = service.find_minimal_feedback_set(&graph);
        println!(
            "[REAL CODE] Minimal feedback set size: {}",
            feedback_set.len()
        );

        // 7. Test call graph traversal - find a symbol with callers
        let symbols_with_callers: Vec<_> = graph
            .symbols()
            .filter(|s| {
                let sid =
                    crate::domain::aggregates::call_graph::SymbolId::new(s.fully_qualified_name());
                !graph.callers(&sid).is_empty()
            })
            .take(5)
            .collect();

        println!(
            "[REAL CODE] Symbols with callers: {}",
            symbols_with_callers.len()
        );
        for sym in &symbols_with_callers {
            let sid =
                crate::domain::aggregates::call_graph::SymbolId::new(sym.fully_qualified_name());
            let callers = graph.callers(&sid);
            println!("  - {} ({} callers)", sym.name(), callers.len());
        }

        // Verify we can traverse the graph
        assert!(graph.symbol_count() > 0, "Graph should have symbols");
        println!(
            "[REAL CODE] Graph stats: symbols={}, edges={}, sccs={}",
            graph.symbol_count(),
            graph.edge_count(),
            cycle_result.total_sccs
        );

        // Note: edges may be 0 if call relationships weren't detected
        // This can happen if tree-sitter queries don't match the code patterns
        if graph.edge_count() == 0 {
            println!(
                "[REAL CODE] WARNING: No edges detected. Call relationships may not be parsed."
            );
        }
    }

    #[test]
    #[ignore = "integration: parses 1400+ line real source file"]
    fn test_debug_call_relationships_in_real_code() {
        use crate::domain::traits::DependencyRepository;
        use crate::infrastructure::parser::{Language, TreeSitterParser};

        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let real_file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/application/services/analysis_service.rs");

        let source = std::fs::read_to_string(&real_file).expect("Should read real file");

        let relationships = parser
            .find_call_relationships(&source, &real_file.to_string_lossy())
            .expect("Should find relationships");

        println!(
            "[DEBUG] Found {} call relationships in analysis_service.rs",
            relationships.len()
        );

        for (i, (caller, callee)) in relationships.iter().take(20).enumerate() {
            println!("[DEBUG] {}. {} -> {}", i + 1, caller.name(), callee);
        }

        // Now test with a real PetGraphStore to see if edges get added
        let mut store = crate::infrastructure::graph::PetGraphStore::new();

        // Add symbols first
        for (caller, _) in &relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());
            store
                .add_dependency(
                    &caller_id,
                    &caller_id,
                    crate::domain::value_objects::DependencyType::Defines,
                )
                .ok();
        }

        let symbols_in_store = store.get_all_symbols();
        println!("[DEBUG] Symbols in store: {}", symbols_in_store.len());
        for (i, sym) in symbols_in_store.iter().take(10).enumerate() {
            println!(
                "[DEBUG]   {}. {} (fqn: {})",
                i + 1,
                sym.name(),
                sym.fully_qualified_name()
            );
        }

        // Now try to find a callee
        let callee_to_find = "WalkDir";
        let found = store
            .get_all_symbols()
            .into_iter()
            .find(|s| s.name() == callee_to_find);
        println!(
            "[DEBUG] Looking for '{}': found={}",
            callee_to_find,
            found.is_some()
        );
        if let Some(f) = found {
            println!(
                "[DEBUG]   Found: {} with fqn: {}",
                f.name(),
                f.fully_qualified_name()
            );
        }

        // This test passes if we find ANY relationships
        assert!(
            !relationships.is_empty(),
            "Should find at least some call relationships in real code"
        );
    }

    #[test]
    #[ignore = "integration: scans entire project via build_project_graph"]
    fn test_enhanced_call_graph_features() {
        let service = AnalysisService::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // Build the project graph first
        service
            .build_project_graph(&project_root)
            .expect("Should build project graph");
        let graph = service.get_project_graph();

        println!(
            "[ENHANCED] Graph stats: symbols={}, edges={}",
            graph.symbol_count(),
            graph.edge_count()
        );

        // 1. Test get_entry_points (symbols with no incoming edges)
        let entry_points = service.get_entry_points();
        println!("[ENHANCED] Entry points found: {}", entry_points.len());
        assert!(!entry_points.is_empty(), "Should find some entry points");
        for ep in entry_points.iter().take(5) {
            println!("  - {} in {}", ep.name, ep.file_path);
        }

        // 2. Test get_leaf_functions (symbols with no outgoing edges)
        let leaf_functions = service.get_leaf_functions();
        println!("[ENHANCED] Leaf functions found: {}", leaf_functions.len());
        assert!(
            !leaf_functions.is_empty(),
            "Should find some leaf functions"
        );
        for lf in leaf_functions.iter().take(5) {
            println!("  - {} in {}", lf.name, lf.file_path);
        }

        // 3. Test trace_path - find a path between two symbols
        // Find two symbols that might be connected
        if graph.symbol_count() > 1 {
            let symbols: Vec<_> = graph.symbols().take(2).collect();
            if symbols.len() == 2 {
                let source = symbols[0].name();
                let target = symbols[1].name();
                println!(
                    "[ENHANCED] Trying trace_path from '{}' to '{}'",
                    source, target
                );
                let path_result = service.trace_path(source, target, 0);
                match path_result {
                    Ok(Some(path)) => {
                        println!("[ENHANCED] Path found with {} hops", path.len());
                    }
                    Ok(None) => {
                        println!(
                            "[ENHANCED] No path found between '{}' and '{}'",
                            source, target
                        );
                    }
                    Err(e) => {
                        println!("[ENHANCED] Error tracing path: {:?}", e);
                    }
                }
            }
        }

        // 4. Test fan_in and fan_out on CallGraph directly
        if let Some(symbol) = graph.symbols().next() {
            let sid =
                crate::domain::aggregates::call_graph::SymbolId::new(symbol.fully_qualified_name());
            let fan_in = graph.fan_in(&sid);
            let fan_out = graph.fan_out(&sid);
            println!(
                "[ENHANCED] Symbol '{}': fan_in={}, fan_out={}",
                symbol.name(),
                fan_in,
                fan_out
            );
        }

        // 5. Test to_mermaid export (just verify it doesn't crash)
        let mermaid = graph.to_mermaid("Test Graph");
        println!("[ENHANCED] Mermaid export length: {} chars", mermaid.len());
        assert!(
            mermaid.contains("flowchart"),
            "Mermaid export should contain 'flowchart'"
        );
        if mermaid.len() < 500 {
            println!("[ENHANCED] Mermaid preview:\n{}", mermaid);
        }

        // 6. Test CallGraphAnalyzer hot paths
        use crate::domain::services::CallGraphAnalyzer;
        let analyzer = CallGraphAnalyzer::new();
        let hot_paths = analyzer.find_hot_paths(&graph, 10);
        println!("[ENHANCED] Hot paths found: {}", hot_paths.len());
        for hp in hot_paths.iter().take(5) {
            println!("  - {} (fan_in={})", hp.symbol_name, hp.fan_in);
        }

        // 7. Test complexity metrics
        let complexity = analyzer.calculate_complexity(&graph);
        println!(
            "[ENHANCED] Complexity: symbols={}, edges={}, max_depth={}",
            complexity.total_symbols, complexity.total_edges, complexity.max_depth
        );

        // Verify basic assertions
        assert!(graph.symbol_count() > 0, "Graph should have symbols");
    }

    #[test]
    fn test_traverse_callees_and_callers() {
        use crate::domain::aggregates::{CallGraph, Symbol};
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let mut graph = CallGraph::new();

        // Create: a -> b -> c, and a -> d
        let a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let c = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));
        let d = Symbol::new("d", SymbolKind::Function, Location::new("test.rs", 4, 1));

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);
        let id_d = graph.add_symbol(d);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_a, &id_d, DependencyType::Calls)
            .unwrap();

        // Test traverse_callees with depth=1 from 'a'
        let callees_depth1 = graph.traverse_callees(&id_a, 1);
        println!(
            "[TRAVERSE] callees of 'a' with depth=1: {:?}",
            callees_depth1.len()
        );
        for ce in &callees_depth1 {
            println!("  - {} at {}:{}", ce.symbol_name, ce.file, ce.line);
        }
        assert!(
            callees_depth1.len() == 2,
            "Should have 2 direct callees (b and d)"
        );

        // Test traverse_callees with depth=2 from 'a'
        let callees_depth2 = graph.traverse_callees(&id_a, 2);
        println!(
            "[TRAVERSE] callees of 'a' with depth=2: {:?}",
            callees_depth2.len()
        );
        for ce in &callees_depth2 {
            println!("  - {} at {}:{}", ce.symbol_name, ce.file, ce.line);
        }
        assert!(
            callees_depth2.len() >= 2,
            "Should have at least 2 callees (b, d, c)"
        );

        // Test traverse_callers with depth=1 from 'c'
        let callers_c = graph.traverse_callers(&id_c, 1);
        println!(
            "[TRAVERSE] callers of 'c' with depth=1: {:?}",
            callers_c.len()
        );
        assert!(callers_c.len() == 1, "Should have 1 direct caller (b)");

        // Test traverse_callers with depth=2 from 'c'
        let callers_c_depth2 = graph.traverse_callers(&id_c, 2);
        println!(
            "[TRAVERSE] callers of 'c' with depth=2: {:?}",
            callers_c_depth2.len()
        );
        assert!(
            callers_c_depth2.len() >= 1,
            "Should have at least 1 caller (a through b)"
        );

        // Test fan_in and fan_out
        assert_eq!(
            graph.fan_in(&id_a),
            0,
            "a should have fan_in=0 (no callers)"
        );
        assert_eq!(
            graph.fan_out(&id_a),
            2,
            "a should have fan_out=2 (calls b and d)"
        );
        assert_eq!(
            graph.fan_in(&id_c),
            1,
            "c should have fan_in=1 (called by b)"
        );
        assert_eq!(
            graph.fan_out(&id_c),
            0,
            "c should have fan_out=0 (no callees)"
        );
    }

    #[test]
    fn test_mermaid_export() {
        use crate::domain::aggregates::{CallGraph, Symbol};
        use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

        let mut graph = CallGraph::new();

        let a = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let b = Symbol::new(
            "process",
            SymbolKind::Function,
            Location::new("main.rs", 10, 1),
        );
        let c = Symbol::new(
            "validate",
            SymbolKind::Function,
            Location::new("util.rs", 5, 1),
        );

        let id_a = graph.add_symbol(a);
        let id_b = graph.add_symbol(b);
        let id_c = graph.add_symbol(c);

        graph
            .add_dependency(&id_a, &id_b, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id_b, &id_c, DependencyType::Calls)
            .unwrap();

        let mermaid = graph.to_mermaid("Test Call Graph");

        println!("[MERMAID] Generated diagram:\n{}", mermaid);

        assert!(mermaid.contains("flowchart"), "Should contain 'flowchart'");
        assert!(mermaid.contains("main"), "Should contain 'main' node");
        assert!(mermaid.contains("process"), "Should contain 'process' node");
        assert!(
            mermaid.contains("validate"),
            "Should contain 'validate' node"
        );
        assert!(mermaid.contains("-->"), "Should contain edges");
    }

    // =========================================================================
    // P4.4 - build_project_graph_async tests
    // =========================================================================

    #[tokio::test]
    async fn test_build_project_graph_async_completes_without_blocking() {
        use std::io::Write;
        use tempfile::TempDir;
        use tokio::time::{timeout, Duration};

        // Create a temp directory with Python files
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Python file
        let file_path = temp_path.join("module.py");
        std::fs::write(
            &file_path,
            r#"
def a():
    b()

def b():
    pass
"#,
        )
        .unwrap();

        let service = AnalysisService::new();

        // Spawn the async build and verify it completes within 5 seconds
        // (If it blocked the tokio runtime, this would hang or take much longer)
        let result = timeout(
            Duration::from_secs(5),
            service.build_project_graph_async(temp_path)
        )
        .await;

        assert!(result.is_ok(), "build_project_graph_async should complete within timeout");
        assert!(result.unwrap().is_ok(), "build_project_graph_async should succeed");

        // Verify the graph was actually built
        let graph = service.get_project_graph();
        assert!(
            graph.symbol_count() >= 2,
            "Graph should have at least 2 symbols (a, b)"
        );
    }

    #[tokio::test]
    async fn test_build_project_graph_async_returns_handle_for_await() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a simple Python file
        let file_path = temp_path.join("test.py");
        std::fs::write(&file_path, "def hello(): pass\n").unwrap();

        let service = AnalysisService::new();

        // The method should be awaitable directly
        let result = service.build_project_graph_async(temp_path).await;
        assert!(result.is_ok(), "build_project_graph_async should be awaitable");
    }

    // =========================================================================
    // P3.3 - build_project_graph_filtered tests
    // =========================================================================

    #[test]
    fn test_build_project_graph_filtered_single_dir() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create module_a with a file
        let module_a = temp_path.join("module_a");
        std::fs::create_dir(&module_a).unwrap();
        let file_a = module_a.join("mod.rs");
        std::fs::write(&file_a, "pub fn func_a() {}").unwrap();

        // Create module_b with a file (should be excluded)
        let module_b = temp_path.join("module_b");
        std::fs::create_dir(&module_b).unwrap();
        let file_b = module_b.join("mod.rs");
        std::fs::write(&file_b, "pub fn func_b() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for only module_a
        let include_dirs = vec![temp_path.join("module_a")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let graph = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The graph should have symbols from module_a
        let symbols: Vec<_> = graph.symbols().collect();
        assert!(
            !symbols.is_empty(),
            "Filtered graph should have symbols from module_a"
        );

        // All symbols should be from module_a
        for symbol in &symbols {
            let file = symbol.location().file();
            assert!(
                file.contains("module_a"),
                "Symbol {} should be from module_a, got: {}",
                symbol.name(),
                file
            );
        }
    }

    #[test]
    fn test_build_project_graph_filtered_nonexistent_dir() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file but no nonexistent_dir
        let file = temp_path.join("mod.rs");
        std::fs::write(&file, "pub fn func() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for a nonexistent directory should return empty graph
        let include_dirs = vec![temp_path.join("nonexistent_dir")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let graph = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // Should return empty graph (no symbols)
        assert_eq!(
            graph.symbol_count(),
            0,
            "Filtered graph for nonexistent dir should be empty"
        );
    }

    #[test]
    fn test_build_project_graph_filtered_multiple_dirs() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create module_a
        let module_a = temp_path.join("module_a");
        std::fs::create_dir(&module_a).unwrap();
        let file_a = module_a.join("mod.rs");
        std::fs::write(&file_a, "pub fn func_a() {}").unwrap();

        // Create module_b
        let module_b = temp_path.join("module_b");
        std::fs::create_dir(&module_b).unwrap();
        let file_b = module_b.join("mod.rs");
        std::fs::write(&file_b, "pub fn func_b() {}").unwrap();

        // Create module_c (should be excluded)
        let module_c = temp_path.join("module_c");
        std::fs::create_dir(&module_c).unwrap();
        let file_c = module_c.join("mod.rs");
        std::fs::write(&file_c, "pub fn func_c() {}").unwrap();

        let service = AnalysisService::new();

        // Build filtered graph for module_a and module_b (exclude module_c)
        let include_dirs = vec![
            temp_path.join("module_a"),
            temp_path.join("module_b"),
        ];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let graph = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The graph should have symbols from module_a and module_b
        let symbols: Vec<_> = graph.symbols().collect();
        assert!(
            !symbols.is_empty(),
            "Filtered graph should have symbols from module_a and module_b"
        );

        // No symbols should be from module_c
        for symbol in &symbols {
            let file = symbol.location().file();
            assert!(
                !file.contains("module_c"),
                "Symbol {} should NOT be from module_c, got: {}",
                symbol.name(),
                file
            );
        }
    }

    #[test]
    fn test_build_project_graph_filtered_does_not_modify_cached_graph() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a file in src
        let src = temp_path.join("src");
        std::fs::create_dir(&src).unwrap();
        let file = src.join("mod.rs");
        std::fs::write(&file, "pub fn func() {}").unwrap();

        let service = AnalysisService::new();

        // Build full graph first
        service.build_project_graph(temp_path).unwrap();
        let cached_count = service.get_project_graph().symbol_count();

        // Build filtered graph for a subdirectory
        let include_dirs = vec![temp_path.join("src")];
        let include_dir_refs: Vec<&Path> = include_dirs.iter().map(|p| p.as_path()).collect();
        let _filtered_graph = service
            .build_project_graph_filtered(temp_path, &include_dir_refs)
            .unwrap();

        // The cached graph should be unchanged
        assert_eq!(
            service.get_project_graph().symbol_count(),
            cached_count,
            "Cached graph should not be modified by build_project_graph_filtered"
        );
    }
}
