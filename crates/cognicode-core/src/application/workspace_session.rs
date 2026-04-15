//! WorkspaceSession - Transport-neutral facade for CogniCode operations
//!
//! This module provides a unified API surface for MCP, CLI, and rig-core integrations.
//! It owns all service instances and cached state for a single workspace session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::application::dto::{
    AnalyzeImpactResult, ArchitectureResult, BuildIndexResult, CallHierarchyEntry,
    ComplexityResult, GetCallHierarchyResult, RefactorResult, RiskLevel, SourceLocation,
    SymbolDto, ChangeEntry, ValidationResult,
};
use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::file_operations::FileOperationsService;
use crate::application::services::refactor_service::RefactorService;
use crate::domain::aggregates::CallGraph;
use crate::domain::value_objects::Location;
use crate::infrastructure::graph::TraversalDirection;
use crate::infrastructure::lsp::CompositeProvider;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::infrastructure::parser::Language;
use crate::infrastructure::semantic::{SearchQuery, SemanticSearchService, SymbolCodeService};

/// Error type for workspace operations
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Path outside workspace: {0}")]
    PathOutsideWorkspace(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),
    #[error("Graph not built: {0}")]
    GraphNotBuilt(String),
    #[error("LSP not available: {0}")]
    LspNotAvailable(String),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Result type for workspace operations
pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

/// Transport-neutral facade for CogniCode operations.
/// 
/// Owns all service instances and cached state for a single workspace.
/// This is the primary API surface for MCP, CLI, and rig-core integrations.
pub struct WorkspaceSession {
    /// Root directory of the workspace
    workspace_root: PathBuf,
    /// Analysis service for code structure and complexity
    analysis: Arc<AnalysisService>,
    /// Refactor service for rename, extract, inline operations
    refactor: Arc<RefactorService>,
    /// File operations service
    file_ops: Arc<FileOperationsService>,
    /// Semantic search service (lazy initialized)
    semantic_search: Arc<RwLock<Option<SemanticSearchService>>>,
    /// Symbol code extraction service
    symbol_code: Arc<SymbolCodeService>,
    /// Cached call graph (built on demand)
    graph: Arc<RwLock<Option<Arc<CallGraph>>>>,
    /// LSP navigation provider (lazy initialized)
    lsp: Arc<RwLock<Option<Arc<CompositeProvider>>>>,
}

impl WorkspaceSession {
    /// Create a new WorkspaceSession for the given directory.
    ///
    /// The directory must exist and contain a codebase.
    pub async fn new(workspace_root: impl AsRef<Path>) -> WorkspaceResult<Self> {
        let root = workspace_root.as_ref();
        if !root.exists() || !root.is_dir() {
            return Err(WorkspaceError::FileNotFound(root.display().to_string()));
        }
        
        let root = root.canonicalize()
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("Failed to canonicalize path: {}", e)))?;
        
        // Initialize services
        let analysis = Arc::new(AnalysisService::new());
        let refactor = Arc::new(RefactorService::new());
        let file_ops = Arc::new(FileOperationsService::new(root.display().to_string()));
        let semantic_search = Arc::new(RwLock::new(None));
        let symbol_code = Arc::new(SymbolCodeService::new());
        let graph = Arc::new(RwLock::new(None));
        let lsp = Arc::new(RwLock::new(None));

        Ok(Self {
            workspace_root: root,
            analysis,
            refactor,
            file_ops,
            semantic_search,
            symbol_code,
            graph,
            lsp,
        })
    }

    /// Returns the workspace root path
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Ensures the graph is built, building it on demand if necessary
    async fn ensure_graph_built(&self) -> WorkspaceResult<()> {
        let mut graph_guard = self.graph.write().await;
        if graph_guard.is_none() {
            self.analysis
                .build_project_graph(&self.workspace_root)
                .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
            *graph_guard = Some(self.analysis.get_project_graph());
        }
        Ok(())
    }

    /// Ensures semantic search is initialized
    async fn ensure_semantic_search(&self) -> WorkspaceResult<()> {
        let mut search_guard = self.semantic_search.write().await;
        if search_guard.is_none() {
            let service = SemanticSearchService::new();
            service
                .populate_from_directory(&self.workspace_root)
                .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("Semantic search init failed: {}", e)))?;
            *search_guard = Some(service);
        }
        Ok(())
    }

    /// Ensures the LSP provider is initialized
    async fn ensure_lsp(&self) -> WorkspaceResult<Arc<CompositeProvider>> {
        let mut lsp_guard = self.lsp.write().await;
        if lsp_guard.is_none() {
            let provider = CompositeProvider::new(&self.workspace_root);
            *lsp_guard = Some(Arc::new(provider)); // Store Arc-wrapped provider
        }
        Ok(Arc::clone(lsp_guard.as_ref().unwrap()))
    }

    // =========================================================================
    // File Operations
    // =========================================================================

    /// Read a file with optional line range and mode
    ///
    /// Mode can be: "raw", "outline", "symbols", "compressed"
    pub async fn read_file(&self, request: crate::application::dto::ReadFileRequest) -> WorkspaceResult<crate::application::dto::ReadFileResult> {
        self.file_ops
            .read_file(request)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("{}", e)))
    }

    /// Write content to a file atomically
    pub async fn write_file(&self, request: crate::application::dto::WriteFileRequest) -> WorkspaceResult<crate::application::dto::WriteFileResult> {
        self.file_ops
            .write_file(request)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("{}", e)))
    }

    /// Apply string-replacement edits to a file
    pub async fn edit_file(&self, request: crate::application::dto::EditFileRequest) -> WorkspaceResult<crate::application::dto::EditFileResult> {
        self.file_ops
            .edit_file(request)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("{}", e)))
    }

    /// Search for content within files
    pub async fn search_content(&self, request: crate::application::dto::SearchContentRequest) -> WorkspaceResult<crate::application::dto::SearchContentResult> {
        self.file_ops
            .search_content(request)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("{}", e)))
    }

    /// List files in a directory with optional filtering
    pub async fn list_files(&self, request: crate::application::dto::ListFilesRequest) -> WorkspaceResult<crate::application::dto::ListFilesResult> {
        self.file_ops
            .list_files(request)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("{}", e)))
    }

    // =========================================================================
    // Code Analysis
    // =========================================================================

    /// Get all symbols from a file
    pub async fn get_file_symbols(&self, file_path: &str) -> WorkspaceResult<Vec<crate::application::dto::SymbolDto>> {
        let path = self.resolve_path(file_path)?;
        self.analysis
            .get_file_symbols(&path)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))
    }

    /// Get an outline (hierarchical structure) of a file
    pub async fn get_outline(&self, file_path: &str, include_private: bool) -> WorkspaceResult<String> {
        let path = self.resolve_path(file_path)?;
        let symbols = self.analysis
            .get_file_symbols(&path)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
        
        let mut outline = String::new();
        for symbol in symbols {
            if !include_private && symbol.name.starts_with('_') {
                continue;
            }
            let kind_str = symbol.kind.to_lowercase();
            outline.push_str(&format!(
                "{}:{}:{}:{}:{}\n",
                symbol.line,
                symbol.column,
                kind_str,
                symbol.name,
                symbol.file_path
            ));
        }
        Ok(outline)
    }

    /// Get complexity metrics for a file or function
    pub async fn get_complexity(&self, file_path: &str, function_name: Option<&str>) -> WorkspaceResult<crate::application::dto::ComplexityResult> {
        use crate::domain::services::ComplexityCalculator;
        
        let path = self.resolve_path(file_path)?;
        let source = std::fs::read_to_string(&path)
            .map_err(|e| WorkspaceError::InvalidInput(format!("Failed to read file: {}", e)))?;
        
        let language = Language::from_extension(path.extension())
            .ok_or_else(|| WorkspaceError::InvalidInput("Unsupported file type".to_string()))?;
        
        let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
        
        let calculator = ComplexityCalculator::new();
        let tree = parser.parse_tree(&source)
            .map_err(|e| WorkspaceError::AnalysisFailed(format!("Parse error: {}", e)))?;
        
        let function_node_type = parser.language().function_node_type();
        let mut max_nesting = 0u32;
        let mut decision_points = Vec::new();
        let mut param_count = 0u32;
        let mut func_start_line = 0u32;
        let mut func_end_line = 0u32;
        
        self.find_function_metrics(
            tree.root_node(),
            &source,
            function_name,
            function_node_type,
            &mut max_nesting,
            &mut decision_points,
            &mut param_count,
            &mut func_start_line,
            &mut func_end_line,
            0,
        );
        
        let cyclomatic = calculator.cyclomatic_complexity(&decision_points, 1);
        let cognitive = calculator.cognitive_complexity(max_nesting, &decision_points, 0);
        let lines_of_code = if func_end_line > func_start_line {
            func_end_line - func_start_line
        } else {
            1
        };
        
        Ok(crate::application::dto::ComplexityResult {
            cyclomatic,
            cognitive,
            lines_of_code,
            parameter_count: param_count,
            nesting_depth: max_nesting,
            function_name: function_name.map(String::from),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn find_function_metrics(
        &self,
        node: tree_sitter::Node,
        source: &str,
        target_name: Option<&str>,
        function_type: &str,
        max_nesting: &mut u32,
        decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
        param_count: &mut u32,
        func_start_line: &mut u32,
        func_end_line: &mut u32,
        current_nesting: u32,
    ) {
        if node.kind() == function_type {
            if let Some(name) = self.find_identifier_in_node(node, source) {
                let should_process = match target_name {
                    Some(target) => name == target,
                    None => *func_start_line == 0,
                };

                if should_process {
                    *func_start_line = node.start_position().row as u32;
                    *func_end_line = node.end_position().row as u32;
                    *param_count = self.count_parameters(node, source);
                    self.process_decision_points(node, source, max_nesting, decision_points, current_nesting);
                }
            }
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.find_function_metrics(
                    child, source, target_name, function_type,
                    max_nesting, decision_points, param_count,
                    func_start_line, func_end_line, current_nesting,
                );
            }
        }
    }

    fn find_identifier_in_node(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "identifier" || child.kind() == "type_identifier" {
                    return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
                }
                if let Some(id) = self.find_identifier_in_node(child, source) {
                    return Some(id);
                }
            }
        }
        None
    }

    fn count_parameters(&self, node: tree_sitter::Node, _source: &str) -> u32 {
        let mut count = 0u32;
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "parameters" {
                    for j in 0..child.child_count() {
                        if let Some(param) = child.child(j) {
                            if param.kind() == "identifier" {
                                count += 1;
                            }
                        }
                    }
                }
                if child.kind() == "identifier" {
                    count += 1;
                }
            }
        }
        count
    }

    fn process_decision_points(
        &self,
        node: tree_sitter::Node,
        source: &str,
        max_nesting: &mut u32,
        decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
        current_nesting: u32,
    ) {
        let kind = node.kind();
        
        match kind {
            "if_statement" | "if_expression" => {
                decision_points.push(crate::domain::services::DecisionPoint::If);
                *max_nesting = (*max_nesting).max(current_nesting + 1);
            }
            "while_statement" | "while_expression" => {
                decision_points.push(crate::domain::services::DecisionPoint::While);
                *max_nesting = (*max_nesting).max(current_nesting + 1);
            }
            "for_statement" | "for_in_statement" => {
                decision_points.push(crate::domain::services::DecisionPoint::For);
                *max_nesting = (*max_nesting).max(current_nesting + 1);
            }
            "case_clause" | "match_expression" => {
                decision_points.push(crate::domain::services::DecisionPoint::Match);
                *max_nesting = (*max_nesting).max(current_nesting + 1);
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.process_decision_points(child, source, max_nesting, decision_points, current_nesting);
            }
        }
    }

    /// Semantic search for symbols
    pub async fn semantic_search(&self, query: &str, max_results: usize) -> WorkspaceResult<Vec<crate::application::dto::SymbolDto>> {
        self.ensure_semantic_search().await?;
        
        let search_guard = self.semantic_search.read().await;
        let service = search_guard.as_ref()
            .ok_or_else(|| WorkspaceError::Internal(anyhow::anyhow!("Semantic search not initialized")))?;
        
        let search_query = SearchQuery {
            query: query.to_string(),
            kinds: Vec::new(),
            max_results,
        };
        let results = service.search(search_query);
        Ok(results
            .into_iter()
            .map(|r| crate::application::dto::SymbolDto::from_symbol(&r.symbol))
            .collect())
    }

    /// Get the source code for a symbol at a specific location
    pub async fn get_symbol_code(&self, file_path: &str, line: u32, column: u32) -> WorkspaceResult<String> {
        let path = self.resolve_path(file_path)?;
        
        let result = self.symbol_code
            .get_symbol_code(&path.to_string_lossy(), line, column)
            .map_err(|e| WorkspaceError::Internal(anyhow::anyhow!("Symbol code error: {}", e)))?;
        
        Ok(result.code)
    }

    // =========================================================================
    // Graph Operations
    // =========================================================================

    /// Build the call graph using the specified strategy
    ///
    /// Strategy can be: "lightweight", "on_demand", "per_file", "full"
    pub async fn build_graph(&self, strategy: &str) -> WorkspaceResult<()> {
        let mut graph_guard = self.graph.write().await;
        
        match strategy {
            "full" | "full_graph" => {
                self.analysis
                    .build_project_graph(&self.workspace_root)
                    .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
                *graph_guard = Some(self.analysis.get_project_graph());
            }
            _ => {
                // For other strategies, use on-demand approach
                self.analysis
                    .build_project_graph(&self.workspace_root)
                    .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
                *graph_guard = Some(self.analysis.get_project_graph());
            }
        }
        Ok(())
    }

    /// Get call hierarchy for a symbol
    pub async fn get_call_hierarchy(
        &self,
        symbol: &str,
        direction: &str,
        depth: usize,
    ) -> WorkspaceResult<GetCallHierarchyResult> {
        self.ensure_graph_built().await?;
        
        let graph_guard = self.graph.read().await;
        let graph = graph_guard.as_ref()
            .ok_or_else(|| WorkspaceError::GraphNotBuilt("Graph not built".to_string()))?;
        
        let direction = match direction {
            "incoming" | "callers" => TraversalDirection::Callers,
            "outgoing" | "callees" => TraversalDirection::Callees,
            _ => TraversalDirection::Both,
        };
        
        let index = self.build_symbol_name_index(graph);
        let search_name = symbol.to_lowercase();
        
        let symbol_ids: Vec<_> = index
            .get(&search_name)
            .map(|entries| {
                entries.iter()
                    .map(|(_, s)| crate::domain::aggregates::call_graph::SymbolId::new(s.fully_qualified_name()))
                    .collect()
            })
            .unwrap_or_default();
        
        let mut all_calls = Vec::new();
        
        for symbol_id in symbol_ids {
            let entries = match direction {
                TraversalDirection::Callers => {
                    graph.traverse_callers(&symbol_id, depth as u8)
                }
                TraversalDirection::Callees => {
                    graph.traverse_callees(&symbol_id, depth as u8)
                }
                TraversalDirection::Both => {
                    let mut both = graph.traverse_callers(&symbol_id, depth as u8);
                    both.extend(graph.traverse_callees(&symbol_id, depth as u8));
                    both
                }
            };
            
            for entry in entries {
                all_calls.push(crate::application::dto::CallHierarchyEntry {
                    symbol: entry.symbol_name,
                    file: entry.file,
                    line: entry.line,
                    column: entry.column,
                    confidence: 1.0,
                });
            }
        }
        
        let total_calls = all_calls.len();
        Ok(crate::application::dto::GetCallHierarchyResult {
            symbol: symbol.to_string(),
            calls: all_calls,
            metadata: crate::application::dto::AnalysisMetadata {
                total_calls,
                analysis_time_ms: 0,
            },
        })
    }

    fn build_symbol_name_index(&self, graph: &CallGraph) -> std::collections::HashMap<String, Vec<(String, crate::domain::aggregates::Symbol)>> {
        let mut index: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
        for symbol in graph.symbols() {
            let name_lower = symbol.name().to_lowercase();
            index.entry(name_lower).or_default().push((symbol.name().to_string(), symbol.clone()));
        }
        index
    }

    /// Analyze the impact of changing a symbol
    pub async fn analyze_impact(&self, symbol: &str) -> WorkspaceResult<AnalyzeImpactResult> {
        use std::collections::HashSet;
        
        self.ensure_graph_built().await?;
        
        let graph_guard = self.graph.read().await;
        let graph = graph_guard.as_ref()
            .ok_or_else(|| WorkspaceError::GraphNotBuilt("Graph not built".to_string()))?;
        
        let index = self.build_symbol_name_index(graph);
        let search_name = symbol.to_lowercase();
        let symbol_ids: Vec<_> = index
            .get(&search_name)
            .map(|entries| {
                entries.iter()
                    .map(|(_, s)| crate::domain::aggregates::call_graph::SymbolId::new(s.fully_qualified_name()))
                    .collect()
            })
            .unwrap_or_default();
        
        let mut impacted_symbols_set: HashSet<String> = HashSet::new();
        let mut impacted_files_set: HashSet<String> = HashSet::new();
        
        for symbol_id in &symbol_ids {
            let dependents = graph.find_all_dependents(symbol_id);
            for dep_id in dependents {
                if let Some(sym) = graph.get_symbol(&dep_id) {
                    impacted_symbols_set.insert(sym.name().to_string());
                    impacted_files_set.insert(sym.location().file().to_string());
                }
            }
        }
        
        let impacted_symbols: Vec<String> = impacted_symbols_set.into_iter().collect();
        let impacted_files: Vec<String> = impacted_files_set.into_iter().collect();
        let symbols_count = impacted_symbols.len();
        let files_count = impacted_files.len();
        
        let risk_level = if symbols_count > 10 {
            RiskLevel::Critical
        } else if symbols_count > 5 {
            RiskLevel::High
        } else if symbols_count > 2 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        
        Ok(AnalyzeImpactResult {
            symbol: symbol.to_string(),
            impacted_files,
            impacted_symbols,
            risk_level,
            summary: format!("{} symbols across {} files would be affected.", symbols_count, files_count),
        })
    }

    /// Check architecture for cycles and violations
    pub async fn check_architecture(&self, _scope: Option<&str>) -> WorkspaceResult<crate::application::dto::ArchitectureResult> {
        use crate::domain::services::CycleDetector;
        
        self.ensure_graph_built().await?;
        
        let graph_guard = self.graph.read().await;
        let graph = graph_guard.as_ref()
            .ok_or_else(|| WorkspaceError::GraphNotBuilt("Graph not built".to_string()))?;
        
        let cycle_detector = CycleDetector::new();
        let cycle_result = cycle_detector.detect_cycles(graph);
        
        let cycles: Vec<crate::application::dto::CycleInfo> = cycle_result
            .cycles
            .iter()
            .map(|c| crate::application::dto::CycleInfo {
                symbols: c.symbols().iter().map(|s| s.as_str().to_string()).collect(),
                length: c.length(),
            })
            .collect();
        
        let cycle_penalty = cycle_result.symbols_in_cycles() * 5;
        let score = (100.0 - cycle_penalty as f32).max(0.0);
        
        let violations: Vec<crate::application::dto::ViolationInfo> = cycle_result
            .cycles
            .iter()
            .map(|c| {
                let symbols = c.symbols();
                let from = symbols.first().map(|s| s.as_str()).unwrap_or("");
                let to = symbols.last().map(|s| s.as_str()).unwrap_or("");
                crate::application::dto::ViolationInfo {
                    rule: "no_cycles".to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                    severity: "high".to_string(),
                }
            })
            .collect();
        
        Ok(crate::application::dto::ArchitectureResult {
            cycles,
            violations,
            score,
            summary: format!("{} cycles detected, {} symbols involved", cycle_result.cycles.len(), cycle_result.symbols_in_cycles()),
        })
    }

    /// Trace execution path between two symbols
    pub async fn trace_path(&self, source: &str, target: &str, _max_depth: usize) -> WorkspaceResult<Vec<String>> {
        self.ensure_graph_built().await?;
        
        let graph_guard = self.graph.read().await;
        let graph = graph_guard.as_ref()
            .ok_or_else(|| WorkspaceError::GraphNotBuilt("Graph not built".to_string()))?;
        
        let path = self.analysis
            .trace_path(source, target)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
        
        match path {
            Some(symbols) => Ok(symbols.into_iter().map(|s| s.name).collect()),
            None => Ok(Vec::new()),
        }
    }

    /// Get entry points (symbols with no incoming edges)
    pub async fn get_entry_points(&self) -> WorkspaceResult<Vec<crate::application::dto::SymbolDto>> {
        self.ensure_graph_built().await?;
        Ok(self.analysis.get_entry_points())
    }

    /// Get leaf functions (symbols with no outgoing edges)
    pub async fn get_leaf_functions(&self) -> WorkspaceResult<Vec<crate::application::dto::SymbolDto>> {
        self.ensure_graph_built().await?;
        Ok(self.analysis.get_leaf_functions())
    }

    /// Export the call graph as Mermaid diagram
    pub async fn export_mermaid(&self, _format: &str, _theme: Option<&str>, _root: Option<&str>) -> WorkspaceResult<String> {
        self.ensure_graph_built().await?;
        
        let graph_guard = self.graph.read().await;
        let graph = graph_guard.as_ref()
            .ok_or_else(|| WorkspaceError::GraphNotBuilt("Graph not built".to_string()))?;
        
        Ok(graph.to_mermaid("Call Graph"))
    }

    /// Build a lightweight symbol index
    pub async fn build_lightweight_index(&self, strategy: &str) -> WorkspaceResult<crate::application::dto::BuildIndexResult> {
        self.analysis
            .build_project_graph(&self.workspace_root)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
        
        let graph = self.analysis.get_project_graph();
        let symbols = graph.symbol_count();
        let edges = graph.edge_count();
        
        Ok(crate::application::dto::BuildIndexResult {
            success: true,
            strategy: strategy.to_string(),
            symbols_indexed: symbols,
            locations_indexed: symbols,
            message: format!("Indexed {} symbols and {} edges", symbols, edges),
        })
    }

    /// Query the symbol index by name
    pub async fn query_symbol_index(&self, symbol: &str) -> WorkspaceResult<Vec<crate::application::dto::SymbolDto>> {
        let mut analysis = AnalysisService::new();
        analysis
            .build_project_graph(&self.workspace_root)
            .map_err(|e| WorkspaceError::AnalysisFailed(e.to_string()))?;
        
        let locations = analysis.find_symbol(symbol);
        
        let mut results = Vec::new();
        for loc in locations {
            // Create a SymbolDto from the location
            let symbol_dto = crate::application::dto::SymbolDto {
                id: format!("{}:{}:{}", loc.file, loc.line, loc.column),
                name: symbol.to_string(),
                kind: format!("{:?}", loc.symbol_kind),
                file_path: loc.file,
                line: loc.line,
                column: loc.column,
                documentation: None,
                signature: None,
            };
            results.push(symbol_dto);
        }
        
        Ok(results)
    }

    // =========================================================================
    // Refactor Operations
    // =========================================================================

    /// Rename a symbol across the codebase
    pub async fn rename_symbol(&self, symbol: &str, new_name: &str, file: &str) -> WorkspaceResult<crate::application::dto::RefactorResult> {
        use crate::application::commands::RenameSymbolCommand;
        
        let path = self.resolve_path(file)?;
        let path_str = path.to_string_lossy().into_owned();
        let command = RenameSymbolCommand::new(symbol, new_name, &path_str);
        
        match self.refactor.rename_symbol(command) {
            Ok(preview) => {
                let edits = self.refactor
                    .generate_rename_edits(&path_str, symbol, new_name)
                    .unwrap_or_default();
                
                let changes: Vec<crate::application::dto::ChangeEntry> = edits.iter().map(|edit| {
                    let start_loc = edit.range.start();
                    crate::application::dto::ChangeEntry {
                        file: start_loc.file().to_string(),
                        old_text: symbol.to_string(),
                        new_text: new_name.to_string(),
                        location: crate::application::dto::SourceLocation {
                            file: start_loc.file().to_string(),
                            line: start_loc.line(),
                            column: start_loc.column(),
                        },
                    }
                }).collect();
                
                Ok(crate::application::dto::RefactorResult {
                    action: crate::application::dto::RefactorAction::Rename,
                    success: true,
                    changes,
                    validation_result: crate::application::dto::ValidationResult {
                        is_valid: true,
                        warnings: vec![format!("Impact: {} symbols affected", preview.symbols_affected.len())],
                        errors: Vec::new(),
                    },
                    error_message: None,
                })
            }
            Err(e) => Ok(crate::application::dto::RefactorResult {
                action: crate::application::dto::RefactorAction::Rename,
                success: false,
                changes: Vec::new(),
                validation_result: crate::application::dto::ValidationResult {
                    is_valid: false,
                    warnings: Vec::new(),
                    errors: vec![e.to_string()],
                },
                error_message: Some(e.to_string()),
            }),
        }
    }

    /// Validate syntax of a file
    pub async fn validate_syntax(&self, file_path: &str) -> WorkspaceResult<crate::application::dto::ValidationResult> {
        let path = self.resolve_path(file_path)?;
        let path_str = path.to_string_lossy().into_owned();
        
        match self.refactor.validate_file_syntax(&path_str) {
            Ok(is_valid) => Ok(crate::application::dto::ValidationResult {
                is_valid,
                warnings: Vec::new(),
                errors: Vec::new(),
            }),
            Err(e) => Ok(crate::application::dto::ValidationResult {
                is_valid: false,
                warnings: Vec::new(),
                errors: vec![e.to_string()],
            }),
        }
    }

    // =========================================================================
    // Navigation (LSP)
    // =========================================================================

    /// Go to definition
    pub async fn go_to_definition(&self, file: &str, line: u32, column: u32) -> WorkspaceResult<Vec<SourceLocation>> {
        let provider = self.ensure_lsp().await?;
        let location = Location::new(
            self.resolve_path(file)?.to_string_lossy().to_string(),
            line.saturating_sub(1),
            column.saturating_sub(1),
        );

        match provider
            .as_ref()
            .get_definition(&location)
            .await
            .map_err(|e| WorkspaceError::LspNotAvailable(e.to_string()))?
        {
            Some(loc) => Ok(vec![SourceLocation::from(&loc)]),
            None => Ok(vec![]),
        }
    }

    /// Get hover information
    pub async fn hover(&self, file: &str, line: u32, column: u32) -> WorkspaceResult<String> {
        let provider = self.ensure_lsp().await?;
        let location = Location::new(
            self.resolve_path(file)?.to_string_lossy().to_string(),
            line.saturating_sub(1),
            column.saturating_sub(1),
        );

        match provider
            .as_ref()
            .hover(&location)
            .await
            .map_err(|e| WorkspaceError::LspNotAvailable(e.to_string()))?
        {
            Some(info) => Ok(info.content),
            None => Ok(String::new()),
        }
    }

    /// Find references to a symbol
    pub async fn find_references(&self, file: &str, line: u32, column: u32, include_decl: bool) -> WorkspaceResult<Vec<SourceLocation>> {
        let provider = self.ensure_lsp().await?;
        let location = Location::new(
            self.resolve_path(file)?.to_string_lossy().to_string(),
            line.saturating_sub(1),
            column.saturating_sub(1),
        );

        let refs = provider
            .as_ref()
            .find_references(&location, include_decl)
            .await
            .map_err(|e| WorkspaceError::LspNotAvailable(e.to_string()))?;

        Ok(refs.into_iter().map(|r| SourceLocation::from(&r.location)).collect())
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Resolve a file path relative to the workspace root
    fn resolve_path(&self, file_path: &str) -> WorkspaceResult<PathBuf> {
        let path = Path::new(file_path);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.workspace_root.join(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workspace_session_creation() {
        let temp_dir = TempDir::new().unwrap();
        let session = WorkspaceSession::new(temp_dir.path()).await;
        assert!(session.is_ok());
        let session = session.unwrap();
        assert_eq!(session.workspace_root(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_workspace_session_invalid_path() {
        let session = WorkspaceSession::new("/nonexistent/path").await;
        assert!(session.is_err());
    }
}
