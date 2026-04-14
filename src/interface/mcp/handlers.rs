//! MCP Handlers - Implementation of MCP tool handlers

use crate::application::commands::{ChangeSignatureCommand, MoveSymbolCommand, ParameterDefinition, RenameSymbolCommand};
use crate::application::dto::SymbolDto;
use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::context_compressor::ContextCompressorService;
use crate::application::services::refactor_service::RefactorService;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::{CallGraph, Symbol};
use crate::domain::services::CycleDetector;
use crate::interface::mcp::schemas::{
    AnalyzeImpactInput, AnalyzeImpactOutput, BuildIndexInput, BuildIndexOutput,
    BuildSubgraphInput, BuildSubgraphOutput, ChangeEntry, CheckArchitectureInput,
    CheckArchitectureOutput, ComplexityMetrics, ContextLines, DependencyInfo, AnalysisMetadata,
    ExportMermaidInput, ExportMermaidOutput, FindReferencesInput, FindReferencesOutput,
    FindUsagesInput, FindUsagesOutput,
    GetCallHierarchyInput, GetCallHierarchyOutput, GetComplexityInput, GetComplexityOutput,
    GetEntryPointsInput, GetEntryPointsOutput, GetFileSymbolsInput, GetFileSymbolsOutput,
    GetHotPathsInput, GetHotPathsOutput, GetLeafFunctionsInput, GetLeafFunctionsOutput,
    GetPerFileGraphInput, GetPerFileGraphOutput, GoToDefinitionInput, GoToDefinitionOutput,
    HierarchyEntryInfo, HierarchySymbolInfo,
    HotPathEntry, HoverInput, HoverOutput, MergeGraphsInput, MergeGraphsOutput,
    OutlineInput, OutlineNodeDto, OutlineOutput,
    PathEntry, QuerySymbolInput, QuerySymbolOutput, RefactorAction, ReferenceEntry, RiskLevel,
    SafeRefactorInput, SafeRefactorOutput, SearchResultDto, SemanticSearchInput, SemanticSearchOutput,
    SourceLocation, StructuralSearchInput, StructuralSearchOutput, SubgraphDirection, SymbolInfo,
    SymbolKind as McpSymbolKind, SymbolCodeInput, SymbolCodeOutput, SymbolLocationEntry,
    TracePathInput, TracePathOutput, UsageEntry, UsageWithContextEntry, ValidateSyntaxInput,
    ValidateSyntaxOutput, ValidationResult, FindUsagesWithContextInput, FindUsagesWithContextOutput,
};
use crate::interface::mcp::security::{InputValidator, SecurityError};
use crate::application::error::AppError;
use crate::infrastructure::graph::{
    FullGraphStrategy, GraphStrategy, LightweightStrategy, OnDemandStrategy,
    PerFileStrategy, TraversalDirection,
};
use crate::infrastructure::semantic::{
    build_outline, SemanticSearchService, SearchSymbolKind, SymbolCodeService,
};
// Re-export file operations handlers
pub use crate::interface::mcp::file_ops_handlers::*;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Context passed to all handlers containing shared services
#[derive(Clone)]
pub struct HandlerContext {
    pub working_dir: PathBuf,
    pub validator: Arc<InputValidator>,
    pub analysis_service: Arc<AnalysisService>,
    pub refactor_service: Arc<RefactorService>,
    pub compressor: Arc<ContextCompressorService>,
    pub semantic_search: Arc<SemanticSearchService>,
    pub symbol_code: Arc<SymbolCodeService>,
    pub client_protocol_version: Option<String>,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub cancellation_token: Arc<AtomicBool>,
    pub log_level: Arc<tokio::sync::RwLock<tracing::Level>>,
}

impl std::fmt::Debug for HandlerContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerContext")
            .field("working_dir", &self.working_dir)
            .field("validator", &self.validator)
            .finish()
    }
}

impl HandlerContext {
    pub fn new(working_dir: PathBuf) -> Self {
        // Canonicalize the working directory to ensure consistent path handling.
        // This is critical because InputValidator::with_workspace canonicalizes paths
        // for security checks. If we store a non-canonical path in working_dir but
        // the validator canonicalizes its allowed_paths, file path validation will
        // fail due to path representation mismatch (e.g., relative vs absolute).
        // By canonicalizing upfront, both working_dir and allowed_paths use the
        // same canonical representation.
        let canonical_working_dir = std::fs::canonicalize(&working_dir).unwrap_or_else(|_| working_dir.clone());

        Self {
            working_dir: canonical_working_dir.clone(),
            validator: Arc::new(InputValidator::new().with_workspace(vec![canonical_working_dir])),
            analysis_service: Arc::new(AnalysisService::new()),
            refactor_service: Arc::new(RefactorService::new()),
            compressor: Arc::new(ContextCompressorService::new()),
            semantic_search: Arc::new(SemanticSearchService::new()),
            symbol_code: Arc::new(SymbolCodeService::new()),
            client_protocol_version: None,
            client_name: None,
            client_version: None,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            log_level: Arc::new(tokio::sync::RwLock::new(tracing::Level::INFO)),
        }
    }

    pub fn with_validator(working_dir: PathBuf, validator: InputValidator) -> Self {
        Self {
            working_dir,
            validator: Arc::new(validator),
            analysis_service: Arc::new(AnalysisService::new()),
            refactor_service: Arc::new(RefactorService::new()),
            compressor: Arc::new(ContextCompressorService::new()),
            semantic_search: Arc::new(SemanticSearchService::new()),
            symbol_code: Arc::new(SymbolCodeService::new()),
            client_protocol_version: None,
            client_name: None,
            client_version: None,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            log_level: Arc::new(tokio::sync::RwLock::new(tracing::Level::INFO)),
        }
    }

    pub fn with_analysis_service(working_dir: PathBuf, analysis_service: AnalysisService) -> Self {
        // Canonicalize working_dir for consistent path handling (same reason as HandlerContext::new)
        let canonical_working_dir = std::fs::canonicalize(&working_dir).unwrap_or_else(|_| working_dir.clone());

        Self {
            working_dir: canonical_working_dir.clone(),
            validator: Arc::new(InputValidator::new().with_workspace(vec![canonical_working_dir])),
            analysis_service: Arc::new(analysis_service),
            refactor_service: Arc::new(RefactorService::new()),
            compressor: Arc::new(ContextCompressorService::new()),
            semantic_search: Arc::new(SemanticSearchService::new()),
            symbol_code: Arc::new(SymbolCodeService::new()),
            client_protocol_version: None,
            client_name: None,
            client_version: None,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            log_level: Arc::new(tokio::sync::RwLock::new(tracing::Level::INFO)),
        }
    }

    pub fn with_refactor_service(working_dir: PathBuf, refactor_service: RefactorService) -> Self {
        // Canonicalize working_dir for consistent path handling (same reason as HandlerContext::new)
        let canonical_working_dir = std::fs::canonicalize(&working_dir).unwrap_or_else(|_| working_dir.clone());

        Self {
            working_dir: canonical_working_dir.clone(),
            validator: Arc::new(InputValidator::new().with_workspace(vec![canonical_working_dir])),
            analysis_service: Arc::new(AnalysisService::new()),
            refactor_service: Arc::new(refactor_service),
            compressor: Arc::new(ContextCompressorService::new()),
            semantic_search: Arc::new(SemanticSearchService::new()),
            symbol_code: Arc::new(SymbolCodeService::new()),
            client_protocol_version: None,
            client_name: None,
            client_version: None,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            log_level: Arc::new(tokio::sync::RwLock::new(tracing::Level::INFO)),
        }
    }

    pub fn cancellation_token(&self) -> &Arc<AtomicBool> {
        &self.cancellation_token
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.load(Ordering::SeqCst)
    }

    pub fn should_log(&self, level: tracing::Level) -> bool {
        // Note: This is a simplified check. For exact tracing level filtering,
        // one would need to compare the numeric representation of levels.
        let stored_level = self.log_level.try_read()
            .map(|g| *g)
            .unwrap_or(tracing::Level::INFO);
        level >= stored_level
    }
}

/// Handler error type
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("Security error: {0}")]
    Security(#[from] SecurityError),

    #[error("Application error: {0}")]
    App(#[from] AppError),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<HandlerError> for crate::interface::mcp::schemas::McpError {
    fn from(err: HandlerError) -> Self {
        match err {
            HandlerError::Security(e) => crate::interface::mcp::schemas::McpError::new(-32000, e.to_string()),
            HandlerError::App(e) => crate::interface::mcp::schemas::McpError::new(-32001, e.to_string()),
            HandlerError::InvalidInput(e) => crate::interface::mcp::schemas::McpError::new(-32002, e.to_string()),
            HandlerError::NotFound(e) => crate::interface::mcp::schemas::McpError::new(-32003, e.to_string()),
            HandlerError::Internal(e) => crate::interface::mcp::schemas::McpError::new(-32004, e.to_string()),
        }
    }
}

/// Result type for handler operations
pub type HandlerResult<T> = Result<T, HandlerError>;

/// Resolves a directory input relative to a working directory.
///
/// - If `input` is `None`, returns `working_dir` unchanged.
/// - If `input` is an absolute path, returns it unchanged.
/// - If `input` is a relative path (including "."), joins it to `working_dir`.
fn resolve_directory(input: Option<String>, working_dir: &Path) -> PathBuf {
    match input {
        None => working_dir.to_path_buf(),
        Some(s) => {
            let p = PathBuf::from(&s);
            if p.is_absolute() {
                p
            } else {
                working_dir.join(p)
            }
        }
    }
}

/// Resolves a file path relative to a working directory.
///
/// - If `input_path` is absolute, returns it unchanged.
/// - If `input_path` is relative, joins it to `working_dir`.
fn resolve_file_path(input_path: &str, working_dir: &Path) -> PathBuf {
    let p = Path::new(input_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    }
}

/// Input for build_graph
#[derive(Debug, serde::Deserialize)]
pub struct BuildGraphInput {
    pub directory: Option<String>,
}

/// A single edge in the build_graph response.
#[derive(Debug, serde::Serialize)]
pub struct EdgeInfo {
    pub from: String,
    pub to: String,
}

/// Output for build_graph
#[derive(Debug, serde::Serialize)]
pub struct BuildGraphOutput {
    pub success: bool,
    pub symbols_found: usize,
    pub relationships_found: usize,
    pub edges: Vec<EdgeInfo>,
    pub message: String,
}

/// Handler for build_graph tool
pub async fn handle_build_graph(
    ctx: &HandlerContext,
    input: BuildGraphInput,
) -> HandlerResult<BuildGraphOutput> {
    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    let start = Instant::now();
    
    let directory = resolve_directory(input.directory, &ctx.working_dir);
    
    // Validate directory
    if !directory.exists() {
        return Err(HandlerError::InvalidInput(format!(
            "Directory does not exist: {}",
            directory.display()
        )));
    }

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }
    
    // Build the project graph
    match ctx.analysis_service.build_project_graph(&directory) {
        Ok(()) => {
            if ctx.is_cancelled() {
                return Err(HandlerError::Internal("Cancelled".into()));
            }
            let graph = ctx.analysis_service.get_project_graph();
            let symbols = graph.symbol_count();
            let edges_count = graph.edge_count();
            let elapsed = start.elapsed().as_millis() as u64;

            // Collect actual edges with base names for correctness evaluation
            let edges: Vec<EdgeInfo> = graph
                .all_dependencies()
                .map(|(source_id, target_id, _)| {
                    let from = graph
                        .get_symbol(source_id)
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| source_id.to_string());
                    let to = graph
                        .get_symbol(target_id)
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| target_id.to_string());
                    EdgeInfo { from, to }
                })
                .collect();
            
            Ok(BuildGraphOutput {
                success: true,
                symbols_found: symbols,
                relationships_found: edges_count,
                edges,
                message: format!(
                    "Graph built successfully: {} symbols, {} relationships in {}ms",
                    symbols, edges_count, elapsed
                ),
            })
        }
        Err(e) => Err(HandlerError::App(e)),
    }
}

/// Handler for get_call_hierarchy tool
pub async fn handle_get_call_hierarchy(
    ctx: &HandlerContext,
    input: GetCallHierarchyInput,
) -> HandlerResult<GetCallHierarchyOutput> {
    let start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.symbol_name)?;

    // Ensure the project graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    let index = build_symbol_name_index(&graph);
    let search_name = input.symbol_name.to_lowercase();
    let symbol_ids: Vec<SymbolId> = index
        .get(&search_name)
        .map(|entries| {
            entries.iter()
                .map(|(_, symbol)| SymbolId::new(symbol.fully_qualified_name()))
                .collect()
        })
        .unwrap_or_default();

    let mut all_calls = Vec::new();

    for symbol_id in symbol_ids {
        // Get direct callees
        for (callee_id, _dep_type) in graph.callees(&symbol_id) {
            if let Some(callee) = graph.get_symbol(&callee_id) {
                all_calls.push(crate::interface::mcp::schemas::CallEntry {
                    symbol: callee.name().to_string(),
                    file: callee.location().file().to_string(),
                    line: callee.location().line(),
                    column: callee.location().column(),
                    confidence: 1.0,
                });
            }
        }
    }

    let total_calls = all_calls.len();

    Ok(GetCallHierarchyOutput {
        symbol: input.symbol_name,
        calls: all_calls,
        metadata: AnalysisMetadata {
            total_calls,
            analysis_time_ms: start.elapsed().as_millis() as u64,
        },
    })
}

/// Handler for get_file_symbols tool
pub async fn handle_get_file_symbols(
    ctx: &HandlerContext,
    input: GetFileSymbolsInput,
) -> HandlerResult<serde_json::Value> {
    // Validate file path
    ctx.validator.validate_file_path(&input.file_path)?;

    // Resolve the file path
    let file_path = if Path::new(&input.file_path).is_absolute() {
        PathBuf::from(&input.file_path)
    } else {
        ctx.working_dir.join(&input.file_path)
    };

    // Call the analysis service to get symbols
    let symbol_dtos = ctx.analysis_service.get_file_symbols(&file_path)
        .map_err(HandlerError::App)?;

    // Convert SymbolDto to SymbolInfo
    let symbols: Vec<SymbolInfo> = symbol_dtos
        .into_iter()
        .map(symbol_dto_to_symbol_info)
        .collect();

    let output = GetFileSymbolsOutput {
        file_path: input.file_path.clone(),
        symbols,
    };

    // If compression is requested, return a natural language summary
    if input.compressed {
        let graph_cache = ctx.analysis_service.graph_cache();
        let graph = graph_cache.get_ref();
        let summary = ctx.compressor.compress_symbols(&output, Some(graph));
        Ok(serde_json::json!({
            "compressed": true,
            "summary": summary,
            "file_path": input.file_path,
        }))
    } else {
        Ok(serde_json::to_value(output).map_err(|e| HandlerError::InvalidInput(e.to_string()))?)
    }
}

/// Converts a SymbolDto to SymbolInfo (MCP schema type)
fn symbol_dto_to_symbol_info(dto: SymbolDto) -> SymbolInfo {
    let kind = match dto.kind.to_lowercase().as_str() {
        "function" => McpSymbolKind::Function,
        "class" => McpSymbolKind::Class,
        "struct" => McpSymbolKind::Struct,
        "enum" => McpSymbolKind::Enum,
        "trait" => McpSymbolKind::Trait,
        "method" => McpSymbolKind::Method,
        "field" => McpSymbolKind::Field,
        "variable" => McpSymbolKind::Variable,
        "constant" => McpSymbolKind::Constant,
        "constructor" => McpSymbolKind::Constructor,
        "interface" => McpSymbolKind::Interface,
        "type" => McpSymbolKind::TypeAlias,
        "parameter" => McpSymbolKind::Parameter,
        "module" => McpSymbolKind::Module,
        _ => McpSymbolKind::Variable,
    };

    SymbolInfo {
        name: dto.name,
        kind,
        location: SourceLocation {
            file: dto.file_path,
            line: dto.line,
            column: dto.column,
        },
        signature: dto.signature,
    }
}

struct Usage {
    file: String,
    line: u32,
    column: u32,
    context: String,
    is_definition: bool,
    context_lines: Option<ContextLines>,
}

struct UsageSearchParams {
    project_dir: PathBuf,
    symbol_name: String,
    include_declaration: bool,
    context_lines: Option<usize>,
    first_only_definition: bool,
}

fn find_symbol_usages(params: UsageSearchParams) -> Result<Vec<Usage>, String> {
    let mut usages = Vec::new();
    let mut seen_first_definition = false;

    // Directories to skip during traversal (common dependency/build/cache dirs)
    const SKIP_DIRS: &[&str] = &[
        "node_modules",
        ".git",
        "target",
        "vendor",
        "dist",
        "build",
        "__pycache__",
        ".cache",
        ".next",
        ".nuxt",
        "coverage",
        ".tox",
        "venv",
        ".venv",
        "env",
    ];

    for entry in walkdir::WalkDir::new(&params.project_dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            // Skip known dependency/build/cache directories
            if let Some(name) = e.file_name().to_str() {
                !SKIP_DIRS.contains(&name)
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let language = match crate::infrastructure::parser::Language::from_extension(path.extension()) {
            Some(lang) => lang,
            None => continue,
        };

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let parser = match crate::infrastructure::parser::TreeSitterParser::new(language) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Ok(occurrences) = parser.find_all_occurrences_of_identifier(&source, &params.symbol_name) {
            for occ in occurrences {
                let has_def_keyword = occ.context.contains("def ")
                    || occ.context.contains("class ")
                    || occ.context.contains("struct ")
                    || occ.context.contains("fn ")
                    || occ.context.contains("function ")
                    || occ.context.contains("const ")
                    || occ.context.contains("let ");

                let is_definition = if params.first_only_definition {
                    has_def_keyword && !seen_first_definition
                } else {
                    has_def_keyword
                };

                if has_def_keyword && !seen_first_definition {
                    seen_first_definition = true;
                }

                if !params.include_declaration && is_definition {
                    continue;
                }

                let context_lines = params.context_lines.map(|ctx| {
                    get_context_lines(&source, occ.line as usize, ctx)
                });

                usages.push(Usage {
                    file: path.to_string_lossy().into_owned(),
                    line: occ.line + 1,
                    column: occ.column,
                    context: occ.context.clone(),
                    is_definition,
                    context_lines,
                });
            }
        }
    }

    Ok(usages)
}

/// Handler for find_usages tool
pub async fn handle_find_usages(
    ctx: &HandlerContext,
    input: FindUsagesInput,
) -> HandlerResult<FindUsagesOutput> {
    ctx.validator.validate_query(&input.symbol_name)?;

    let usages = find_symbol_usages(UsageSearchParams {
        project_dir: ctx.working_dir.clone(),
        symbol_name: input.symbol_name.clone(),
        include_declaration: input.include_declaration,
        context_lines: None,
        first_only_definition: true,
    })
    .map_err(|e| HandlerError::App(AppError::AnalysisError(e)))?;

    let total = usages.len();

    let usage_entries: Vec<UsageEntry> = usages
        .into_iter()
        .map(|u| UsageEntry {
            file: u.file,
            line: u.line,
            column: u.column,
            context: u.context,
            is_definition: u.is_definition,
        })
        .collect();

    Ok(FindUsagesOutput {
        symbol: input.symbol_name,
        usages: usage_entries,
        total,
    })
}

/// Handler for structural_search tool
pub async fn handle_structural_search(
    ctx: &HandlerContext,
    input: StructuralSearchInput,
) -> HandlerResult<StructuralSearchOutput> {
    let _start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.query)?;
    if let Some(path) = &input.path {
        ctx.validator.validate_file_path(path)?;
    }

    // For now, return empty results since we don't have a real AST search
    let matches = Vec::new();

    Ok(StructuralSearchOutput {
        pattern: input.query,
        matches,
        total: 0,
    })
}

/// Handler for analyze_impact tool
pub async fn handle_analyze_impact(
    ctx: &HandlerContext,
    input: AnalyzeImpactInput,
) -> HandlerResult<AnalyzeImpactOutput> {
    let start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.symbol_name)?;

    // Ensure the project graph is built before querying dependents
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    let index = build_symbol_name_index(&graph);
    let search_name = input.symbol_name.to_lowercase();
    let symbol_ids: Vec<SymbolId> = index
        .get(&search_name)
        .map(|entries| {
            entries.iter()
                .map(|(_, symbol)| SymbolId::new(symbol.fully_qualified_name()))
                .collect()
        })
        .unwrap_or_default();

    let mut impacted_symbols_set: HashSet<String> = HashSet::new();
    let mut impacted_files_set: HashSet<String> = HashSet::new();

    for symbol_id in symbol_ids {
        // Find all dependents (transitive)
        let dependents = graph.find_all_dependents(&symbol_id);

        for dep_id in dependents {
            if let Some(symbol) = graph.get_symbol(&dep_id) {
                impacted_symbols_set.insert(symbol.name().to_string());
                impacted_files_set.insert(symbol.location().file().to_string());
            }
        }
    }

    let impacted_symbols: Vec<String> = impacted_symbols_set.into_iter().collect();
    let impacted_files: Vec<String> = impacted_files_set.into_iter().collect();
    let symbols_count = impacted_symbols.len();
    let files_count = impacted_files.len();

    // Calculate risk level based on impact scope
    let risk_level = if symbols_count > 10 {
        RiskLevel::Critical
    } else if symbols_count > 5 {
        RiskLevel::High
    } else if symbols_count > 2 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    Ok(AnalyzeImpactOutput {
        symbol: input.symbol_name,
        impacted_files,
        impacted_symbols,
        risk_level,
        summary: format!(
            "Impact analysis completed in {}ms. {} symbols across {} files would be affected.",
            start.elapsed().as_millis(),
            symbols_count,
            files_count
        ),
    })
}

/// Ensures the project graph is built, building it on-demand if empty.
/// This prevents empty callgraph results from being returned as "success with no data".
fn ensure_graph_built(ctx: &HandlerContext) -> HandlerResult<()> {
    let graph = ctx.analysis_service.get_project_graph();
    // Check if graph is empty (no symbols built yet)
    if graph.symbols().next().is_none() {
        // Build the graph on demand
        ctx.analysis_service.build_project_graph(&ctx.working_dir)
            .map_err(|e| HandlerError::App(e))?;
    }
    Ok(())
}

/// Ensures the semantic search index is populated, indexing the working directory on demand.
fn ensure_semantic_indexed(ctx: &HandlerContext) -> HandlerResult<()> {
    if ctx.semantic_search.index().is_empty() {
        ctx.semantic_search.populate_from_directory(&ctx.working_dir)
            .map_err(|e| HandlerError::Internal(e))?;
    }
    Ok(())
}

/// Handler for check_architecture tool
pub async fn handle_check_architecture(
    ctx: &HandlerContext,
    input: CheckArchitectureInput,
) -> HandlerResult<CheckArchitectureOutput> {
    let start = Instant::now();

    // Validate scope if provided
    if let Some(scope) = &input.scope {
        ctx.validator.validate_query(scope)?;
    }

    // Ensure graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Use CycleDetector to detect cycles
    let cycle_detector = CycleDetector::new();
    let cycle_result = cycle_detector.detect_cycles(&graph);

    let cycles: Vec<crate::interface::mcp::schemas::CycleInfo> = cycle_result
        .cycles
        .iter()
        .map(|c| crate::interface::mcp::schemas::CycleInfo {
            symbols: c.symbols().iter().map(|s| s.as_str().to_string()).collect(),
            length: c.length(),
        })
        .collect();

    // Calculate architecture score based on cycles
    // Score is 100 if no cycles, reduced by 5 points per cycle symbol
    let cycle_penalty = cycle_result.symbols_in_cycles() * 5;
    let score = (100.0 - cycle_penalty as f32).max(0.0);

    // Violations are cycles that violate architecture rules
    let violations: Vec<crate::interface::mcp::schemas::ViolationInfo> = cycle_result
        .cycles
        .iter()
        .map(|c| {
            let symbols = c.symbols();
            let from = symbols.first().map(|s| s.as_str()).unwrap_or("");
            let to = symbols.last().map(|s| s.as_str()).unwrap_or("");
            crate::interface::mcp::schemas::ViolationInfo {
                rule: "no_cycles".to_string(),
                from: from.to_string(),
                to: to.to_string(),
                severity: "high".to_string(),
            }
        })
        .collect();

    Ok(CheckArchitectureOutput {
        cycles,
        violations,
        score,
        summary: format!(
            "Architecture check completed in {}ms - {} cycles detected, {} symbols involved",
            start.elapsed().as_millis(),
            cycle_result.cycles.len(),
            cycle_result.symbols_in_cycles()
        ),
    })
}

/// Handler for safe_refactor tool
pub async fn handle_safe_refactor(
    ctx: &HandlerContext,
    input: SafeRefactorInput,
) -> HandlerResult<SafeRefactorOutput> {
    let _start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.target)?;
    if let Some(params) = &input.params {
        ctx.validator.validate_query(&params.to_string())?;
    }

    // Handle different refactor actions
    match input.action {
        RefactorAction::Rename => {
            // Extract new_name from params
            let new_name = input.params.as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_name' parameter for rename".to_string()))?
                .to_string();

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Create rename command
            let command = RenameSymbolCommand::new(&input.target, &new_name, &file_path_str);

            // Execute rename via RefactorService
            match ctx.refactor_service.rename_symbol(command) {
                Ok(preview) => {
                    // Generate the actual edits for the preview
                    let edits = ctx.refactor_service.generate_rename_edits(
                        &file_path_str,
                        &input.target,
                        &new_name,
                    ).unwrap_or_default();

                    // Convert edits to ChangeEntry
                    let changes: Vec<ChangeEntry> = edits.iter().map(|edit| {
                        let start_loc = edit.range.start();
                        ChangeEntry {
                            file: start_loc.file().to_string(),
                            old_text: input.target.clone(),
                            new_text: new_name.clone(),
                            location: SourceLocation {
                                file: start_loc.file().to_string(),
                                line: start_loc.line(),
                                column: start_loc.column(),
                            },
                        }
                    }).collect();

                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes,
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![format!("Impact: {} symbols affected", preview.symbols_affected.len())],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Extract => {
            // target is the existing symbol to extract from
            // new_name is the name for the extracted function
            let new_name = input.params.as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_name' parameter for extract".to_string()))?
                .to_string();

            // Get the file path from params or use working_dir/target, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute extract via RefactorService
            // Pass input.target (existing symbol) so extract_symbol can find it
            match ctx.refactor_service.extract_symbol_with_target(&file_path_str, &input.target, &new_name) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: format!("// {} block", new_name),
                            new_text: format!("fn {}() {{ ... }}", new_name),
                            location: SourceLocation {
                                file: file_path_str.clone(),
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Inline => {
            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute inline via RefactorService
            match ctx.refactor_service.inline_symbol(&file_path_str, &input.target) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: input.target.clone(),
                            new_text: "// inlined".to_string(),
                            location: SourceLocation {
                                file: file_path_str,
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Move => {
            // Extract source_path and target_path from params, resolving relative paths
            let source_path = input.params.as_ref()
                .and_then(|p| p.get("source_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir).to_string_lossy().to_string())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'source_path' parameter for move".to_string()))?;

            let target_path = input.params.as_ref()
                .and_then(|p| p.get("target_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir).to_string_lossy().to_string())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'target_path' parameter for move".to_string()))?;

            // Create move command
            let command = MoveSymbolCommand::new(&input.target, &source_path, &target_path);

            // Execute move via RefactorService
            match ctx.refactor_service.move_symbol(command) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: source_path.clone(),
                            old_text: input.target.clone(),
                            new_text: format!("// moved to {}", target_path),
                            location: SourceLocation {
                                file: source_path.clone(),
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::ChangeSignature => {
            // Extract new_parameters from params
            let new_parameters_json = input.params.as_ref()
                .and_then(|p| p.get("new_parameters"))
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_parameters' parameter for change_signature".to_string()))?;

            let new_parameters: Vec<ParameterDefinition> = serde_json::from_value(new_parameters_json.clone())
                .map_err(|e| HandlerError::InvalidInput(format!("Invalid new_parameters: {}", e)))?;

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Create change signature command
            let command = ChangeSignatureCommand {
                function_name: input.target.clone(),
                new_parameters,
                file_path: file_path_str.clone(),
            };

            // Execute change_signature via RefactorService
            match ctx.refactor_service.change_signature(command) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: input.target.clone(),
                            new_text: "// signature changed".to_string(),
                            location: SourceLocation {
                                file: file_path_str,
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        // Other refactor actions are not yet implemented
        _ => {
            Ok(SafeRefactorOutput {
                action: input.action.clone(),
                success: false,
                changes: Vec::new(),
                validation_result: ValidationResult {
                    is_valid: false,
                    warnings: Vec::new(),
                    errors: vec![format!("{:?} not yet implemented", input.action)],
                },
                error_message: Some(format!("{:?} not yet implemented", input.action)),
            })
        }
    }
}

/// Handler for validate_syntax tool
pub async fn handle_validate_syntax(
    ctx: &HandlerContext,
    input: ValidateSyntaxInput,
) -> HandlerResult<ValidateSyntaxOutput> {
    // Resolve the file path relative to working directory
    let file_path = resolve_file_path(&input.file_path, &ctx.working_dir);

    // Validate file path
    ctx.validator.validate_file_path(&file_path.to_string_lossy())?;

    // Use VFS-based tree-sitter validation
    match ctx.refactor_service.validate_file_syntax(&file_path.to_string_lossy()) {
        Ok(is_valid) => Ok(ValidateSyntaxOutput {
            file_path: input.file_path,
            is_valid,
            errors: Vec::new(),
            warnings: Vec::new(),
        }),
        Err(e) => Ok(ValidateSyntaxOutput {
            file_path: input.file_path,
            is_valid: false,
            errors: vec![crate::interface::mcp::schemas::SyntaxError {
                line: 1,
                column: 1,
                message: e.to_string(),
                severity: "error".to_string(),
            }],
            warnings: Vec::new(),
        }),
    }
}

/// Handler for get_complexity tool
pub async fn handle_get_complexity(
    ctx: &HandlerContext,
    input: GetComplexityInput,
) -> HandlerResult<GetComplexityOutput> {
    // Resolve the file path relative to working directory
    let file_path = resolve_file_path(&input.file_path, &ctx.working_dir);

    // Validate file path
    ctx.validator.validate_file_path(&file_path.to_string_lossy())?;
    if let Some(function_name) = &input.function_name {
        ctx.validator.validate_query(function_name)?;
    }

    // Calculate real complexity metrics
    let source = std::fs::read_to_string(&file_path).map_err(|e| {
        HandlerError::InvalidInput(format!("Failed to read file: {}", e))
    })?;

    let language = crate::infrastructure::parser::Language::from_extension(
        file_path.extension()
    ).ok_or_else(|| HandlerError::InvalidInput("Unsupported file type".to_string()))?;

    let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
        .map_err(|e| HandlerError::App(crate::application::error::AppError::AnalysisError(e.to_string())))?;

    // Find the function and calculate complexity
    let (cyclomatic, cognitive, nesting_depth, parameter_count, lines_of_code) = 
        calculate_function_complexity(&parser, &source, input.function_name.as_deref());

    Ok(GetComplexityOutput {
        file_path: input.file_path,
        complexity: ComplexityMetrics {
            cyclomatic,
            cognitive,
            lines_of_code,
            parameter_count,
            nesting_depth,
            function_name: input.function_name,
        },
    })
}

/// Calculates complexity metrics for a function
fn calculate_function_complexity(
    parser: &crate::infrastructure::parser::TreeSitterParser,
    source: &str,
    function_name: Option<&str>,
) -> (u32, u32, u32, u32, u32) {
    use crate::domain::services::ComplexityCalculator;

    let calculator = ComplexityCalculator::new();
    let tree = match parser.parse_tree(source) {
        Ok(t) => t,
        Err(_) => return (1, 1, 0, 0, 0),
    };

    let function_node_type = parser.language().function_node_type();
    let mut max_nesting = 0u32;
    let mut decision_points = Vec::new();
    let mut param_count = 0u32;
    let mut func_start_line = 0u32;
    let mut func_end_line = 0u32;

    // Find the function node and count decision points
    find_function_metrics(
        tree.root_node(),
        source,
        function_name,
        function_node_type,
        &mut max_nesting,
        &mut decision_points,
        &mut param_count,
        &mut func_start_line,
        &mut func_end_line,
        0,
    );

    // Calculate cyclomatic complexity
    let cyclomatic = calculator.cyclomatic_complexity(&decision_points, 1);
    
    // Calculate cognitive complexity
    let cognitive = calculator.cognitive_complexity(max_nesting, &decision_points, 0);
    
    // Calculate lines of code
    let lines_of_code = if func_end_line > func_start_line {
        func_end_line - func_start_line
    } else {
        1
    };

    (cyclomatic, cognitive, max_nesting, param_count, lines_of_code)
}

/// Recursively finds a function and its metrics
#[allow(clippy::too_many_arguments)]
fn find_function_metrics(
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
    // Check if this is a function definition
    if node.kind() == function_type {
        // Get function name
        if let Some(name) = find_identifier_in_node(node, source) {
            // If we have a target name, only process that function
            // Otherwise, process the first/only function
            let should_process = match target_name {
                Some(target) => name == target,
                None => *func_start_line == 0, // Process first function if no target
            };

            if should_process {
                *func_start_line = node.start_position().row as u32;
                *func_end_line = node.end_position().row as u32;

                // Count parameters
                *param_count = count_parameters(node, source);

                // Process function body for decision points
                process_decision_points(node, source, max_nesting, decision_points, current_nesting);
            }
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            find_function_metrics(
                child,
                source,
                target_name,
                function_type,
                max_nesting,
                decision_points,
                param_count,
                func_start_line,
                func_end_line,
                current_nesting,
            );
        }
    }
}

/// Finds an identifier in a node
fn find_identifier_in_node(node: tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
            }
            if let Some(id) = find_identifier_in_node(child, source) {
                return Some(id);
            }
        }
    }
    None
}

/// Counts parameters in a function definition
#[allow(dead_code)]
fn count_parameters(node: tree_sitter::Node, _source: &str) -> u32 {
    let mut count = 0u32;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            // Handle Python parameters (within parentheses after function name)
            if child.kind() == "parameters" {
                for j in 0..child.child_count() {
                    if let Some(param) = child.child(j) {
                        if param.kind() == "identifier" {
                            count += 1;
                        }
                    }
                }
            }
            // Handle Rust/JS parameters (identifier list)
            if child.kind() == "identifier" {
                count += 1;
            }
        }
    }
    count
}

/// Processes a node tree to find decision points
fn process_decision_points(
    node: tree_sitter::Node,
    source: &str,
    max_nesting: &mut u32,
    decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
    current_nesting: u32,
) {
    let kind = node.kind();
    
    // Check for decision points
    match kind {
        "if_statement" | "if_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::If);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "elif_clause" | "else_if_clause" => {
            decision_points.push(crate::domain::services::DecisionPoint::ElseIf);
        }
        "while_statement" | "while_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::While);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "for_statement" | "for_expression" | "for_in_statement" => {
            decision_points.push(crate::domain::services::DecisionPoint::For);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "match_expression" | "match_statement" | "switch_statement" => {
            decision_points.push(crate::domain::services::DecisionPoint::Match);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "binary_expression" => {
            // Check for && or ||
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if text.contains("&&") {
                    decision_points.push(crate::domain::services::DecisionPoint::And);
                }
                if text.contains("||") {
                    decision_points.push(crate::domain::services::DecisionPoint::Or);
                }
            }
        }
        "conditional_expression" | "ternary_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::Ternary);
        }
        "catch_clause" | "except_clause" => {
            decision_points.push(crate::domain::services::DecisionPoint::Catch);
        }
        _ => {}
    }

    // Recurse into children with updated nesting
    let child_nesting = if matches!(kind, 
        "if_statement" | "if_expression" | "elif_clause" | "else_if_clause" |
        "while_statement" | "while_expression" | "for_statement" | "for_expression" |
        "for_in_statement" | "match_expression" | "match_statement" | "switch_statement"
    ) {
        current_nesting + 1
    } else {
        current_nesting
    };

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            process_decision_points(child, source, max_nesting, decision_points, child_nesting);
        }
    }
}

/// Handler for get_entry_points tool
#[allow(dead_code)]
pub async fn handle_get_entry_points(
    ctx: &HandlerContext,
    _input: GetEntryPointsInput,
) -> HandlerResult<GetEntryPointsOutput> {
    let start = Instant::now();

    // Ensure graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    let root_ids = graph.roots();
    
    // If still empty, return a clear message indicating graph needs to be built
    if root_ids.len() == 0 && graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "Call graph is empty. Build the project graph first using build_graph tool.".into()
        ));
    }

    let mut entry_points = Vec::new();

    for symbol_id in root_ids {
        if let Some(symbol) = graph.get_symbol(&symbol_id) {
            entry_points.push(SymbolInfo {
                name: symbol.name().to_string(),
                kind: match symbol.kind() {
                    crate::domain::value_objects::SymbolKind::Function => McpSymbolKind::Function,
                    crate::domain::value_objects::SymbolKind::Class => McpSymbolKind::Class,
                    crate::domain::value_objects::SymbolKind::Struct => McpSymbolKind::Struct,
                    crate::domain::value_objects::SymbolKind::Enum => McpSymbolKind::Enum,
                    crate::domain::value_objects::SymbolKind::Trait => McpSymbolKind::Trait,
                    crate::domain::value_objects::SymbolKind::Method => McpSymbolKind::Method,
                    crate::domain::value_objects::SymbolKind::Module => McpSymbolKind::Module,
                    _ => McpSymbolKind::Variable,
                },
                location: SourceLocation {
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line(),
                    column: symbol.location().column(),
                },
                signature: None,
            });
        }
    }

    let total = entry_points.len();

    Ok(GetEntryPointsOutput {
        entry_points,
        total,
        metadata: AnalysisMetadata {
            total_calls: total,
            analysis_time_ms: start.elapsed().as_millis() as u64,
        },
    })
}

/// Handler for get_leaf_functions tool
#[allow(dead_code)]
pub async fn handle_get_leaf_functions(
    ctx: &HandlerContext,
    _input: GetLeafFunctionsInput,
) -> HandlerResult<GetLeafFunctionsOutput> {
    let start = Instant::now();

    // Ensure graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    let leaf_ids = graph.leaves();
    
    // If still empty, return a clear message indicating graph needs to be built
    if leaf_ids.len() == 0 && graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "Call graph is empty. Build the project graph first using build_graph tool.".into()
        ));
    }

    // Leaf functions are symbols with no outgoing edges (leaves)
    let mut leaf_functions = Vec::new();

    for symbol_id in leaf_ids {
        if let Some(symbol) = graph.get_symbol(&symbol_id) {
            leaf_functions.push(SymbolInfo {
                name: symbol.name().to_string(),
                kind: match symbol.kind() {
                    crate::domain::value_objects::SymbolKind::Function => McpSymbolKind::Function,
                    crate::domain::value_objects::SymbolKind::Class => McpSymbolKind::Class,
                    crate::domain::value_objects::SymbolKind::Struct => McpSymbolKind::Struct,
                    crate::domain::value_objects::SymbolKind::Enum => McpSymbolKind::Enum,
                    crate::domain::value_objects::SymbolKind::Trait => McpSymbolKind::Trait,
                    crate::domain::value_objects::SymbolKind::Method => McpSymbolKind::Method,
                    crate::domain::value_objects::SymbolKind::Module => McpSymbolKind::Module,
                    _ => McpSymbolKind::Variable,
                },
                location: SourceLocation {
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line(),
                    column: symbol.location().column(),
                },
                signature: None,
            });
        }
    }

    let total = leaf_functions.len();

    Ok(GetLeafFunctionsOutput {
        leaf_functions,
        total,
        metadata: AnalysisMetadata {
            total_calls: total,
            analysis_time_ms: start.elapsed().as_millis() as u64,
        },
    })
}

/// Handler for trace_path tool
pub async fn handle_trace_path(
    ctx: &HandlerContext,
    input: TracePathInput,
) -> HandlerResult<TracePathOutput> {
    let start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.source)?;
    ctx.validator.validate_query(&input.target)?;

    // Ensure graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Find source and target symbols
    let source_id = find_symbol_in_graph(&graph, &input.source);
    let target_id = find_symbol_in_graph(&graph, &input.target);

    match (source_id, target_id) {
        (Some(source), Some(target)) => {
            // Use BFS to find path
            let path = find_path_bfs(&graph, &source, &target, input.max_depth);
            
            let path_entries: Vec<PathEntry> = path.iter()
                .filter_map(|sid| {
                    graph.get_symbol(sid).map(|s| PathEntry {
                        symbol: s.name().to_string(),
                        file: s.location().file().to_string(),
                        line: s.location().line(),
                        column: s.location().column(),
                    })
                })
                .collect();

            let path_found = !path.is_empty();

            let path_length = path.len();

            Ok(TracePathOutput {
                source: input.source,
                target: input.target,
                path_found,
                path: path_entries,
                path_length,
                metadata: AnalysisMetadata {
                    total_calls: path_length,
                    analysis_time_ms: start.elapsed().as_millis() as u64,
                },
            })
        }
        (None, _) => Err(HandlerError::NotFound(format!("Source symbol '{}' not found", input.source))),
        (_, None) => Err(HandlerError::NotFound(format!("Target symbol '{}' not found", input.target))),
    }
}

/// Handler for export_mermaid tool
pub async fn handle_export_mermaid(
    ctx: &HandlerContext,
    input: ExportMermaidInput,
) -> HandlerResult<ExportMermaidOutput> {
    let start = Instant::now();

    // Ensure the project graph is built before exporting
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Generate Mermaid flowchart
    let mut mermaid_lines = vec!["flowchart TD".to_string()];
    let mut node_count = 0;
    let mut edge_count = 0;

    // If a root symbol is provided, export subgraph around it
    let symbols_to_export: Box<dyn Iterator<Item = _>> = if let Some(ref root_name) = input.root_symbol {
        if let Some(root_id) = find_symbol_in_graph(&graph, root_name) {
            Box::new(std::iter::once(root_id))
        } else {
            return Err(HandlerError::NotFound(format!("Root symbol '{}' not found", root_name)));
        }
    } else {
        Box::new(graph.symbols().map(|s| SymbolId::new(s.fully_qualified_name())))
    };

    // Collect symbols to process
    let symbol_ids: Vec<SymbolId> = symbols_to_export.collect();

    // Add nodes
    for symbol_id in &symbol_ids {
        if let Some(symbol) = graph.get_symbol(symbol_id) {
            let node_id = sanitize_mermaid_id(symbol.name());
            mermaid_lines.push(format!("    {}[{}]", node_id, symbol.name()));
            node_count += 1;
        }
    }

    // Add edges (respecting max_depth if root_symbol provided)
    if input.root_symbol.is_some() {
        // For subgraph, traverse up to max_depth
        let mut visited = std::collections::HashSet::new();
        for symbol_id in &symbol_ids {
            collect_edges_recursive(&graph, symbol_id, &mut visited, input.max_depth, &mut mermaid_lines, &mut edge_count);
        }
    } else {
        // For full graph, add all edges
        for symbol_id in &symbol_ids {
            if let Some(symbol) = graph.get_symbol(symbol_id) {
                for (callee_id, _) in graph.callees(symbol_id) {
                    if let Some(callee) = graph.get_symbol(&callee_id) {
                        let from_id = sanitize_mermaid_id(symbol.name());
                        let to_id = sanitize_mermaid_id(callee.name());
                        mermaid_lines.push(format!("    {} --> {}", from_id, to_id));
                        edge_count += 1;
                    }
                }
            }
        }
    }

    let mermaid_code = mermaid_lines.join("\n");

    let svg = if input.format.as_deref() == Some("svg") || (input.format.is_none() && input.theme.is_some()) {
        let theme = input.theme.as_deref().unwrap_or("tokyo-night-light");
        render_mermaid_to_svg(&mermaid_code, theme)
    } else {
        None
    };

    Ok(ExportMermaidOutput {
        mermaid_code,
        node_count,
        edge_count,
        metadata: AnalysisMetadata {
            total_calls: node_count,
            analysis_time_ms: start.elapsed().as_millis() as u64,
        },
        svg,
    })
}

/// Handler for get_hot_paths tool
pub async fn handle_get_hot_paths(
    ctx: &HandlerContext,
    input: GetHotPathsInput,
) -> HandlerResult<GetHotPathsOutput> {
    let start = Instant::now();

    // Ensure graph is built before querying
    ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    if graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "Call graph is empty. Build the project graph first using build_graph tool.".into()
        ));
    }

    // Calculate fan-in and fan-out for each symbol
    let mut hot_paths: Vec<HotPathEntry> = Vec::new();

    for symbol in graph.symbols() {
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
        let fan_in = graph.callers(&symbol_id).len();
        let fan_out = graph.callees(&symbol_id).len();

        // Filter by minimum fan-in threshold
        if fan_in >= input.min_fan_in {
            hot_paths.push(HotPathEntry {
                symbol: symbol.name().to_string(),
                file: symbol.location().file().to_string(),
                line: symbol.location().line(),
                column: symbol.location().column(),
                fan_in,
                fan_out,
            });
        }
    }

    // Sort by fan-in descending and limit
    hot_paths.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));
    hot_paths.truncate(input.limit);

    let total = hot_paths.len();

    Ok(GetHotPathsOutput {
        hot_paths,
        total,
        metadata: AnalysisMetadata {
            total_calls: total,
            analysis_time_ms: start.elapsed().as_millis() as u64,
        },
    })
}

fn build_symbol_name_index(graph: &CallGraph) -> HashMap<String, Vec<(&str, &Symbol)>> {
    let mut index: HashMap<String, Vec<(&str, &Symbol)>> = HashMap::new();
    for (id, symbol) in graph.symbol_ids() {
        let name_lower = symbol.name().to_lowercase();
        index.entry(name_lower).or_default().push((id.as_str(), symbol));
    }
    index
}

fn find_symbol_in_graph(graph: &CallGraph, name: &str) -> Option<SymbolId> {
    let search_name = name.to_lowercase();
    
    // Tier 1: Exact symbol name match
    for symbol in graph.symbols() {
        if symbol.name().to_lowercase() == search_name {
            return Some(SymbolId::new(symbol.fully_qualified_name()));
        }
    }
    
    // Tier 2: Exact FQN match
    for symbol in graph.symbols() {
        if symbol.fully_qualified_name().to_lowercase() == search_name {
            return Some(SymbolId::new(symbol.fully_qualified_name()));
        }
    }
    
    // Tier 3: Segment-exact match (split FQN by :: and . and check any segment equals search_name)
    for symbol in graph.symbols() {
        let fqn = symbol.fully_qualified_name().to_lowercase();
        let segments = fqn.split("::")
            .flat_map(|s| s.split('.'))
            .any(|seg| seg == search_name);
        if segments {
            return Some(SymbolId::new(symbol.fully_qualified_name()));
        }
    }
    
    None
}

// BFS path finding algorithm
#[allow(dead_code)]
fn find_path_bfs(graph: &CallGraph, source: &SymbolId, target: &SymbolId, max_depth: u8) -> Vec<SymbolId> {
    use std::collections::VecDeque;

    let mut queue: VecDeque<(SymbolId, Vec<SymbolId>)> = VecDeque::new();
    let mut visited: std::collections::HashSet<SymbolId> = std::collections::HashSet::new();

    queue.push_back((source.clone(), vec![source.clone()]));
    visited.insert(source.clone());

    while let Some((current, path)) = queue.pop_front() {
        if current == *target {
            return path;
        }

        if path.len() >= max_depth as usize {
            continue;
        }

        for (callee_id, _) in graph.callees(&current) {
            if !visited.contains(&callee_id) {
                visited.insert(callee_id.clone());
                let mut new_path = path.clone();
                new_path.push(callee_id.clone());
                queue.push_back((callee_id.clone(), new_path));
            }
        }
    }

    Vec::new() // No path found
}

// Collect edges recursively up to max_depth
fn collect_edges_recursive(
    graph: &CallGraph,
    symbol_id: &SymbolId,
    visited: &mut std::collections::HashSet<SymbolId>,
    remaining_depth: u8,
    lines: &mut Vec<String>,
    edge_count: &mut usize,
) {
    if remaining_depth == 0 || visited.contains(symbol_id) {
        return;
    }

    visited.insert(symbol_id.clone());

    if let Some(symbol) = graph.get_symbol(symbol_id) {
        for (callee_id, _) in graph.callees(symbol_id) {
            if !visited.contains(&callee_id) {
                if let Some(callee) = graph.get_symbol(&callee_id) {
                    let from_id = sanitize_mermaid_id(symbol.name());
                    let to_id = sanitize_mermaid_id(callee.name());
                    lines.push(format!("    {} --> {}", from_id, to_id));
                    *edge_count += 1;
                    collect_edges_recursive(graph, &callee_id, visited, remaining_depth - 1, lines, edge_count);
                }
            }
        }
    }
}

// ============================================================================
// Graph Strategy Handlers
// ============================================================================

/// Handler for build_lightweight_index tool
pub async fn handle_build_lightweight_index(
    ctx: &HandlerContext,
    input: BuildIndexInput,
) -> HandlerResult<BuildIndexOutput> {
    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    let start = Instant::now();

    let directory = resolve_directory(input.directory, &ctx.working_dir);

    if !directory.exists() {
        return Err(HandlerError::InvalidInput(format!(
            "Directory does not exist: {}",
            directory.display()
        )));
    }

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    // Create the appropriate strategy based on input
    let mut strategy: Box<dyn GraphStrategy> =
        match input.strategy.as_str() {
            "lightweight" => Box::new(LightweightStrategy::new()),
            "on_demand" | "ondemand" => Box::new(OnDemandStrategy::new()),
            "per_file" | "perfile" => Box::new(PerFileStrategy::new()),
            "full" | "full_graph" => Box::new(FullGraphStrategy::new()),
            _ => Box::new(LightweightStrategy::new()),
        };

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    // Build the index
    match strategy.build_index(&directory) {
        Ok(()) => {
            let symbols = strategy.query_symbols("").len();
            let elapsed = start.elapsed().as_millis() as u64;

            // Get actual stats from the strategy
            let symbols_indexed = symbols;
            let locations_indexed = symbols;

            Ok(BuildIndexOutput {
                success: true,
                strategy: strategy.name().to_string(),
                symbols_indexed,
                locations_indexed,
                message: format!(
                    "Index built successfully using {} strategy in {}ms",
                    strategy.name(),
                    elapsed
                ),
            })
        }
        Err(e) => Err(HandlerError::App(AppError::AnalysisError(e.to_string()))),
    }
}

/// Handler for query_symbol_index tool
pub async fn handle_query_symbol_index(
    ctx: &HandlerContext,
    input: QuerySymbolInput,
) -> HandlerResult<QuerySymbolOutput> {
    let _start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.symbol_name)?;

    let directory = resolve_directory(input.directory, &ctx.working_dir);

    // Build a lightweight index to query
    let mut strategy = LightweightStrategy::new();
    if let Err(e) = strategy.build_index(&directory) {
        return Err(HandlerError::App(AppError::AnalysisError(e.to_string())));
    }

    let locations = strategy.query_symbols(&input.symbol_name);

    let location_entries: Vec<SymbolLocationEntry> = locations
        .iter()
        .map(|loc| SymbolLocationEntry {
            file: loc.file.clone(),
            line: loc.line,
            column: loc.column,
            symbol_kind: format!("{:?}", loc.symbol_kind),
        })
        .collect();

    let total = location_entries.len();

    Ok(QuerySymbolOutput {
        symbol_name: input.symbol_name,
        locations: location_entries,
        total,
    })
}

/// Handler for build_call_subgraph tool
pub async fn handle_build_call_subgraph(
    ctx: &HandlerContext,
    input: BuildSubgraphInput,
) -> HandlerResult<BuildSubgraphOutput> {
    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    let _start = Instant::now();

    // Validate input
    ctx.validator.validate_query(&input.symbol_name)?;

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    let directory = resolve_directory(input.directory, &ctx.working_dir);

    // Create on-demand strategy
    let mut strategy = OnDemandStrategy::new();
    if let Err(e) = strategy.build_index(&directory) {
        return Err(HandlerError::App(AppError::AnalysisError(e.to_string())));
    }

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    // Convert direction
    let direction = match input.direction {
        SubgraphDirection::In => TraversalDirection::Callers,
        SubgraphDirection::Out => TraversalDirection::Callees,
        SubgraphDirection::Both => TraversalDirection::Both,
    };

    // Build subgraph
    let result = strategy.build_subgraph(&input.symbol_name, input.depth, direction);

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    let root_info = HierarchySymbolInfo {
        name: result.root_symbol.name().to_string(),
        file: result.root_symbol.location().file().to_string(),
        line: result.root_symbol.location().line(),
        column: result.root_symbol.location().column(),
        symbol_kind: format!("{:?}", result.root_symbol.kind()),
    };

    let entries: Vec<HierarchyEntryInfo> = result
        .entries
        .iter()
        .map(|entry| HierarchyEntryInfo {
            symbol: HierarchySymbolInfo {
                name: entry.symbol.name().to_string(),
                file: entry.symbol.location().file().to_string(),
                line: entry.symbol.location().line(),
                column: entry.symbol.location().column(),
                symbol_kind: format!("{:?}", entry.symbol.kind()),
            },
            depth: entry.depth,
            direction: format!("{:?}", entry.direction),
        })
        .collect();

    let total_entries = entries.len();

    Ok(BuildSubgraphOutput {
        symbol_name: input.symbol_name,
        root: root_info,
        entries,
        total_entries,
    })
}

/// Handler for get_per_file_graph tool
pub async fn handle_get_per_file_graph(
    ctx: &HandlerContext,
    input: GetPerFileGraphInput,
) -> HandlerResult<GetPerFileGraphOutput> {
    // Validate file path
    ctx.validator.validate_file_path(&input.file_path)?;

    let file_path = if Path::new(&input.file_path).is_absolute() {
        PathBuf::from(&input.file_path)
    } else {
        ctx.working_dir.join(&input.file_path)
    };

    // Use per-file strategy
    let strategy = PerFileStrategy::new();
    match strategy.build_local_graph(&file_path) {
        Ok(graph) => {
            let symbols: Vec<SymbolLocationEntry> = graph
                .symbols()
                .map(|s| SymbolLocationEntry {
                    file: s.location().file().to_string(),
                    line: s.location().line(),
                    column: s.location().column(),
                    symbol_kind: format!("{:?}", s.kind()),
                })
                .collect();

            let dependencies: Vec<DependencyInfo> = graph
                .all_dependencies()
                .filter_map(|(src_id, tgt_id, _)| {
                    let src = graph.get_symbol(src_id)?;
                    let tgt = graph.get_symbol(tgt_id)?;
                    Some(DependencyInfo {
                        caller: src.name().to_string(),
                        caller_file: src.location().file().to_string(),
                        caller_line: src.location().line(),
                        callee: tgt.name().to_string(),
                    })
                })
                .collect();

            Ok(GetPerFileGraphOutput {
                file_path: input.file_path,
                symbols: symbols.clone(),
                symbol_count: symbols.len(),
                dependencies: dependencies.clone(),
                dependency_count: dependencies.len(),
            })
        }
        Err(e) => Err(HandlerError::App(AppError::AnalysisError(e.to_string()))),
    }
}

/// Handler for merge_file_graphs tool
pub async fn handle_merge_graphs(
    ctx: &HandlerContext,
    input: MergeGraphsInput,
) -> HandlerResult<MergeGraphsOutput> {
    // Handle empty file paths case
    if input.file_paths.is_empty() {
        return Ok(MergeGraphsOutput {
            file_count: 0,
            merged_symbol_count: 0,
            merged_dependency_count: 0,
            symbols: Vec::new(),
            dependencies: Vec::new(),
        });
    }

    // Validate and resolve file paths
    let file_paths: Vec<PathBuf> = input
        .file_paths
        .iter()
        .map(|p| {
            if Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                ctx.working_dir.join(p)
            }
        })
        .collect();

    // Use per-file strategy with merge
    let strategy = PerFileStrategy::new();
    let merged = strategy.build_full_graph(file_paths[0].parent().unwrap_or(&ctx.working_dir))
        .unwrap_or_else(|_| CallGraph::new());

    let symbols: Vec<SymbolLocationEntry> = merged
        .symbols()
        .map(|s| SymbolLocationEntry {
            file: s.location().file().to_string(),
            line: s.location().line(),
            column: s.location().column(),
            symbol_kind: format!("{:?}", s.kind()),
        })
        .collect();

    let dependencies: Vec<DependencyInfo> = merged
        .all_dependencies()
        .filter_map(|(src_id, tgt_id, _)| {
            let src = merged.get_symbol(src_id)?;
            let tgt = merged.get_symbol(tgt_id)?;
            Some(DependencyInfo {
                caller: src.name().to_string(),
                caller_file: src.location().file().to_string(),
                caller_line: src.location().line(),
                callee: tgt.name().to_string(),
            })
        })
        .collect();

    Ok(MergeGraphsOutput {
        file_count: input.file_paths.len(),
        merged_symbol_count: symbols.len(),
        merged_dependency_count: dependencies.len(),
        symbols,
        dependencies,
    })
}

// ============================================================================
// Semantic Analysis Handlers
// ============================================================================

/// Handler for get_outline tool
pub async fn handle_get_outline(
    ctx: &HandlerContext,
    input: OutlineInput,
) -> HandlerResult<OutlineOutput> {
    let start = Instant::now();

    // Validate file path
    ctx.validator.validate_file_path(&input.file_path)?;

    // Resolve the file path
    let file_path = if Path::new(&input.file_path).is_absolute() {
        PathBuf::from(&input.file_path)
    } else {
        ctx.working_dir.join(&input.file_path)
    };

    // Read the source file
    let source = std::fs::read_to_string(&file_path)
        .map_err(|e| HandlerError::InvalidInput(format!("Failed to read file: {}", e)))?;

    // Get language from extension
    let language = crate::infrastructure::parser::Language::from_extension(
        file_path.extension()
    ).ok_or_else(|| HandlerError::InvalidInput("Unsupported file type".to_string()))?;

    // Build the outline
    let nodes = build_outline(
        &source,
        &file_path.to_string_lossy(),
        language,
        input.include_private,
        input.include_tests,
    );

    // Convert to DTOs
    let nodes_dto: Vec<OutlineNodeDto> = nodes.iter().map(convert_outline_node).collect();
    let total_nodes: usize = nodes.iter().map(|n| n.total_nodes()).sum();

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(OutlineOutput {
        file_path: input.file_path,
        nodes: nodes_dto,
        total_nodes,
        generation_time_ms: elapsed,
    })
}

/// Converts an OutlineNode to DTO
fn convert_outline_node(node: &crate::infrastructure::semantic::OutlineNode) -> OutlineNodeDto {
    OutlineNodeDto {
        name: node.name.clone(),
        kind: format!("{:?}", node.kind).to_lowercase(),
        line: node.location.line() + 1, // Convert to 1-indexed
        column: node.location.column(),
        signature: node.signature.clone(),
        children: node.children.iter().map(convert_outline_node).collect(),
        is_private: node.is_private,
    }
}

/// Handler for get_symbol_code tool
pub async fn handle_get_symbol_code(
    ctx: &HandlerContext,
    input: SymbolCodeInput,
) -> HandlerResult<SymbolCodeOutput> {
    // Validate file path
    ctx.validator.validate_file_path(&input.file)?;

    // Resolve the file path
    let file_path = if Path::new(&input.file).is_absolute() {
        PathBuf::from(&input.file)
    } else {
        ctx.working_dir.join(&input.file)
    };

    // Check cache first
    let cache_key = crate::infrastructure::semantic::SymbolCodeKey::new(
        &file_path.to_string_lossy(),
        input.line,
        input.col,
    );

    let cached = ctx.symbol_code.cache().get(&cache_key);

    if let Some(cached) = cached {
        return Ok(SymbolCodeOutput {
            file: input.file,
            code: cached.code.clone(),
            docstring: cached.docstring.clone(),
            start_line: cached.start_line,
            end_line: cached.end_line,
            cached: true,
        });
    }

    // Get symbol code
    match ctx.symbol_code.get_symbol_code(
        &file_path.to_string_lossy(),
        input.line,
        input.col,
    ) {
        Ok(result) => Ok(SymbolCodeOutput {
            file: input.file,
            code: result.code,
            docstring: result.docstring,
            start_line: result.start_line,
            end_line: result.end_line,
            cached: false,
        }),
        Err(e) => Err(HandlerError::NotFound(e)),
    }
}

/// Handler for semantic_search tool
pub async fn handle_semantic_search(
    ctx: &HandlerContext,
    input: SemanticSearchInput,
) -> HandlerResult<SemanticSearchOutput> {
    let start = Instant::now();

    // Validate query
    if input.query.is_empty() {
        return Err(HandlerError::InvalidInput("Query cannot be empty".to_string()));
    }

    // Ensure the search index is populated before querying
    ensure_semantic_indexed(ctx)?;

    // Convert kind filters
    let kinds: Vec<SearchSymbolKind> = input.kinds.as_ref()
        .map(|kinds| {
            kinds.iter().filter_map(|k| match k.to_lowercase().as_str() {
                "function" => Some(SearchSymbolKind::Function),
                "class" => Some(SearchSymbolKind::Class),
                "method" => Some(SearchSymbolKind::Method),
                "variable" => Some(SearchSymbolKind::Variable),
                "trait" => Some(SearchSymbolKind::Trait),
                "struct" => Some(SearchSymbolKind::Struct),
                "enum" => Some(SearchSymbolKind::Enum),
                "module" => Some(SearchSymbolKind::Module),
                "constant" => Some(SearchSymbolKind::Constant),
                _ => None,
            }).collect()
        })
        .unwrap_or_default();

    // Build search query
    let query_text = input.query.clone();
    let query = crate::infrastructure::semantic::SearchQuery {
        query: input.query,
        kinds,
        max_results: input.max_results,
    };

    // Search
    let results = ctx.semantic_search.search(query);

    // Convert to DTOs
    let results_dto: Vec<SearchResultDto> = results.iter().map(|r| {
        SearchResultDto {
            name: r.symbol.name().to_string(),
            kind: format!("{:?}", r.symbol.kind()).to_lowercase(),
            file: r.symbol.location().file().to_string(),
            line: r.symbol.location().line() + 1, // 1-indexed
            column: r.symbol.location().column(),
            score: r.score,
            match_type: format!("{:?}", r.match_type).to_lowercase(),
        }
    }).collect();

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(SemanticSearchOutput {
        query: query_text,
        results: results_dto,
        total: results.len(),
        search_time_ms: elapsed,
    })
}

/// Handler for find_usages_with_context tool
pub async fn handle_find_usages_with_context(
    ctx: &HandlerContext,
    input: FindUsagesWithContextInput,
) -> HandlerResult<FindUsagesWithContextOutput> {
    ctx.validator.validate_query(&input.symbol)?;

    let usages = find_symbol_usages(UsageSearchParams {
        project_dir: ctx.working_dir.clone(),
        symbol_name: input.symbol.clone(),
        include_declaration: input.include_declaration,
        context_lines: Some(input.context_lines as usize),
        first_only_definition: false,
    })
    .map_err(|e| HandlerError::App(AppError::AnalysisError(e)))?;

    let total = usages.len();

    let usage_entries: Vec<UsageWithContextEntry> = usages
        .into_iter()
        .map(|u| UsageWithContextEntry {
            file: u.file,
            line: u.line,
            column: u.column,
            context: u.context,
            context_lines: u.context_lines.unwrap_or(ContextLines {
                before: vec![],
                current: String::new(),
                after: vec![],
            }),
            is_definition: u.is_definition,
        })
        .collect();

    Ok(FindUsagesWithContextOutput {
        symbol: input.symbol,
        usages: usage_entries,
        total,
    })
}

/// Gets surrounding context lines for a given line
fn get_context_lines(source: &str, line: usize, context_size: usize) -> ContextLines {
    let lines: Vec<&str> = source.lines().collect();

    let before_start = line.saturating_sub(context_size);
    let after_end = (line + 1 + context_size).min(lines.len());

    let before: Vec<String> = (before_start..line)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    let current = lines.get(line).unwrap_or(&"").to_string();

    let after: Vec<String> = ((line + 1)..after_end)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    ContextLines {
        before,
        current,
        after,
    }
}

// Sanitize a name for use as a Mermaid node ID
fn sanitize_mermaid_id(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn render_mermaid_to_svg(mermaid_code: &str, theme: &str) -> Option<String> {
    crate::infrastructure::mermaid::render_mermaid(mermaid_code, theme).ok()
}

// ============================================================================
// LSP Navigation Handlers
// ============================================================================

/// Handler for go_to_definition tool
pub async fn handle_go_to_definition(
    ctx: &HandlerContext,
    input: GoToDefinitionInput,
) -> HandlerResult<GoToDefinitionOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.get_definition(&location).await {
        Ok(Some(def_loc)) => {
            let source = std::fs::read_to_string(def_loc.file()).ok();
            let context = source.as_ref().map(|s| {
                let lines: Vec<&str> = s.lines().collect();
                let line_idx = (def_loc.line() as usize).saturating_sub(1);
                if line_idx < lines.len() {
                    lines[line_idx].to_string()
                } else {
                    String::new()
                }
            });
            Ok(GoToDefinitionOutput {
                found: true,
                file: Some(def_loc.file().to_string()),
                line: Some(def_loc.line()),
                column: Some(def_loc.column()),
                context,
                message: None,
            })
        }
        Ok(None) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some("No definition found at this position".to_string()),
        }),
        Err(e) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some(e.to_string()),
        }),
    }
}

/// Handler for hover tool
pub async fn handle_hover(
    ctx: &HandlerContext,
    input: HoverInput,
) -> HandlerResult<HoverOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.hover(&location).await {
        Ok(Some(info)) => Ok(HoverOutput {
            found: true,
            content: Some(info.content),
            documentation: info.documentation,
            kind: Some(format!("{:?}", info.kind)),
        }),
        Ok(None) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
        Err(_) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
    }
}

/// Handler for find_references tool
pub async fn handle_find_references(
    ctx: &HandlerContext,
    input: FindReferencesInput,
) -> HandlerResult<FindReferencesOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.find_references(&location, input.include_declaration).await {
        Ok(refs) => {
            let entries: Vec<ReferenceEntry> = refs.iter().map(|r| ReferenceEntry {
                file: r.location.file().to_string(),
                line: r.location.line(),
                column: r.location.column(),
                kind: format!("{:?}", r.reference_kind),
                context: r.container.clone().unwrap_or_default(),
            }).collect();
            let total = entries.len();
            Ok(FindReferencesOutput {
                symbol: input.file_path,
                references: entries,
                total,
            })
        }
        Err(_) => Ok(FindReferencesOutput {
            symbol: input.file_path,
            references: vec![],
            total: 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_build_lightweight_index_invalid_directory() {
        let ctx = HandlerContext::new(PathBuf::from("/nonexistent/path"));
        let input = BuildIndexInput {
            directory: Some("/nonexistent/path".to_string()),
            strategy: "lightweight".to_string(),
        };
        
        let result = handle_build_lightweight_index(&ctx, input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_query_symbol_index_empty_symbol() {
        let ctx = HandlerContext::new(PathBuf::from("."));
        let input = QuerySymbolInput {
            symbol_name: "".to_string(),
            directory: None,
        };
        
        let result = handle_query_symbol_index(&ctx, input).await;
        // Empty symbol name should still work but return empty results
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.total, 0);
    }

    #[tokio::test]
    async fn test_handle_build_call_subgraph_empty_symbol() {
        let ctx = HandlerContext::new(PathBuf::from("."));
        let input = BuildSubgraphInput {
            symbol_name: "".to_string(),
            depth: 3,
            direction: SubgraphDirection::Both,
            directory: None,
        };
        
        let result = handle_build_call_subgraph(&ctx, input).await;
        // Should return a result even with empty symbol
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_get_per_file_graph_nonexistent_file() {
        let ctx = HandlerContext::new(PathBuf::from("."));
        let input = GetPerFileGraphInput {
            file_path: "/nonexistent/file.py".to_string(),
        };
        
        let result = handle_get_per_file_graph(&ctx, input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_merge_graphs_empty_list() {
        let ctx = HandlerContext::new(PathBuf::from("."));
        let input = MergeGraphsInput {
            file_paths: vec![],
        };
        
        let result = handle_merge_graphs(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.file_count, 0);
        assert_eq!(output.merged_symbol_count, 0);
    }
}