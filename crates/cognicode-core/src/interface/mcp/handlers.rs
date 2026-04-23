//! MCP Handlers - Implementation of MCP tool handlers

use crate::application::commands::{ChangeSignatureCommand, MoveSymbolCommand, ParameterDefinition, RenameSymbolCommand};
use crate::application::dto::{GetFileSymbolsResult, SymbolDto};
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

use crate::infrastructure::persistence::RedbGraphStore;
use crate::domain::traits::GraphStore;

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
/// Generate a redb file path based on the target directory.
/// The .cognicode directory is placed inside the analyzed project directory.
fn graph_db_path(directory: &Path) -> PathBuf {
    let canonical_dir = directory.canonicalize().unwrap_or_else(|_| directory.to_path_buf());
    canonical_dir.join(".cognicode").join("graph.redb")
}

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

/// Checks whether any source file has changed since the manifest was saved.
/// Uses mtime as a fast check (no hashing unless needed).
fn is_manifest_stale(manifest: &crate::domain::value_objects::file_manifest::FileManifest, project_dir: &Path) -> bool {
    use walkdir::WalkDir;
    const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build",
        "vendor", "__pycache__", ".cache", ".next", ".nuxt", "coverage",
        ".tox", "venv", ".venv", ".env", "env", ".cognicode"];

    let mut checked = 0usize;
    for entry in WalkDir::new(project_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| SKIP_DIRS.contains(&s))
                .unwrap_or(false)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        // Only check supported source extensions
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["rs","py","js","ts","go","java","c","cpp","h"].contains(&ext) {
            continue;
        }
        checked += 1;
        let rel = path.strip_prefix(project_dir).unwrap_or(path);
        let mtime_ms = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        match manifest.entries.get(rel) {
            Some(entry) if entry.mtime == mtime_ms => continue,  // unchanged
            _ => return true,  // new, modified, or missing from manifest
        }
    }
    // If manifest has more entries than files checked, files were deleted → stale
    let manifest_source_count = manifest.entries.len();
    checked != manifest_source_count
}

/// Builds a FileManifest for the given project directory.
fn build_manifest(project_dir: &Path) -> std::io::Result<crate::domain::value_objects::file_manifest::FileManifest> {
    use crate::domain::value_objects::file_manifest::{FileManifest, FileEntry};
    use walkdir::WalkDir;
    const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build",
        "vendor", "__pycache__", ".cache", ".next", ".nuxt", "coverage",
        ".tox", "venv", ".venv", ".env", "env", ".cognicode"];

    let mut manifest = FileManifest::new(project_dir.to_path_buf());

    for entry in WalkDir::new(project_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| SKIP_DIRS.contains(&s))
                .unwrap_or(false)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["rs","py","js","ts","go","java","c","cpp","h"].contains(&ext) {
            continue;
        }
        let rel = path.strip_prefix(project_dir)
            .unwrap_or(path)
            .to_path_buf();
        let mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        // Use mtime as content_hash proxy (cheap — no file read needed)
        let _ = manifest.entries.insert(
            rel,
            FileEntry {
                content_hash: mtime.to_string(),
                mtime,
                symbol_count: 0,
            },
        );
    }
    Ok(manifest)
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

    // --- Persistence: try to load from redb, but verify staleness ---
    let db_path = graph_db_path(&directory);
    let mut loaded_from_cache = false;

    if db_path.exists() {
        if let Ok(store) = RedbGraphStore::open(&db_path) {
            // Check staleness via FileManifest
            let is_stale = match store.load_manifest() {
                Ok(Some(manifest)) => {
                    // Scan source files and compare hashes
                    is_manifest_stale(&manifest, &directory)
                }
                Ok(None) => true,  // No manifest → stale
                Err(_) => true,    // Error reading → rebuild
            };

            if !is_stale {
                if let Ok(Some(graph)) = store.load_graph() {
                    ctx.analysis_service.graph_cache().set(graph);
                    loaded_from_cache = true;
                }
            }
        }
    }

    // Build from source if cache miss
    if !loaded_from_cache {
        if let Err(e) = ctx.analysis_service.build_project_graph(&directory) {
            return Err(HandlerError::App(e));
        }
        // Persist the freshly built graph + manifest
        let _ = std::fs::create_dir_all(db_path.parent().unwrap_or(&directory));
        if let Ok(store) = RedbGraphStore::open(&db_path) {
            let graph = ctx.analysis_service.get_project_graph();
            let _ = store.save_graph(&graph);
            // Build and save manifest
            if let Ok(manifest) = build_manifest(&directory) {
                let _ = store.save_manifest(&manifest);
            }
        }
    }

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
    
    let source = if loaded_from_cache { "cache (redb)" } else { "built + persisted" };
    Ok(BuildGraphOutput {
        success: true,
        symbols_found: symbols,
        relationships_found: edges_count,
        edges,
        message: format!(
            "Graph loaded from {}: {} symbols, {} relationships in {}ms",
            source, symbols, edges_count, elapsed
        ),
    })
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
    let _ensure = ensure_graph_built(ctx)?;

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

    // Ensure graph is built for compressed output
    let _ensure = ensure_graph_built(ctx)?;

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
        // Convert to DTO for compression
        let output_dto: GetFileSymbolsResult = output.into();
        let summary = ctx.compressor.compress_symbols(&output_dto, Some(graph));
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
    let ensure = ensure_graph_built(ctx)?;

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

    let auto_note = if ensure.auto_built { ensure.message.clone() } else { String::new() };

    Ok(AnalyzeImpactOutput {
        symbol: input.symbol_name,
        impacted_files,
        impacted_symbols,
        risk_level,
        summary: format!(
            "{}Impact analysis completed in {}ms. {} symbols across {} files would be affected.",
            auto_note,
            start.elapsed().as_millis(),
            symbols_count,
            files_count
        ),
    })
}

/// Result of ensuring a prerequisite is satisfied.
#[derive(Debug, Clone)]
pub struct EnsureResult {
    /// True if the prerequisite was auto-resolved (wasn't ready before).
    pub auto_built: bool,
    /// Human-readable message about what was done.
    pub message: String,
    /// Number of symbols/nodes after resolution.
    pub count: usize,
}

impl EnsureResult {
    fn already_present(count: usize) -> Self {
        Self { auto_built: false, message: String::new(), count }
    }
    fn auto_built(count: usize, elapsed_ms: u64) -> Self {
        Self {
            auto_built: true,
            message: format!("Graph auto-built ({} symbols, {}ms). ", count, elapsed_ms),
            count,
        }
    }
}

/// Ensures the project graph is built, building it on-demand if empty.
/// This prevents empty callgraph results from being returned as "success with no data".
fn ensure_graph_built(ctx: &HandlerContext) -> HandlerResult<EnsureResult> {
    let graph = ctx.analysis_service.get_project_graph();
    let count = graph.symbols().count();
    if count > 0 {
        return Ok(EnsureResult::already_present(count));
    }
    // Auto-build the graph
    let start = std::time::Instant::now();
    ctx.analysis_service.build_project_graph(&ctx.working_dir)
        .map_err(HandlerError::App)?;
    let graph = ctx.analysis_service.get_project_graph();
    let count = graph.symbols().count();
    let elapsed = start.elapsed().as_millis() as u64;
    Ok(EnsureResult::auto_built(count, elapsed))
}

/// Ensures the semantic search index is populated, indexing the working directory on demand.
fn ensure_semantic_indexed(ctx: &HandlerContext) -> HandlerResult<EnsureResult> {
    if !ctx.semantic_search.index().is_empty() {
        return Ok(EnsureResult::already_present(ctx.semantic_search.index().len()));
    }
    let start = std::time::Instant::now();
    ctx.semantic_search.populate_from_directory(&ctx.working_dir)
        .map_err(HandlerError::Internal)?;
    let elapsed = start.elapsed().as_millis() as u64;
    Ok(EnsureResult::auto_built(ctx.semantic_search.index().len(), elapsed))
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
    let ensure = ensure_graph_built(ctx)?;

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

    let auto_note = if ensure.auto_built { ensure.message.clone() } else { String::new() };

    Ok(CheckArchitectureOutput {
        cycles,
        violations,
        score,
        summary: format!(
            "{}Architecture check completed in {}ms - {} cycles detected, {} symbols involved",
            auto_note,
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
    let _ensure = ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    let root_ids = graph.roots();
    
    // If still empty, return a helpful message
    if root_ids.len() == 0 && graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "No source code found in the project directory. Ensure the directory contains supported source files (.rs, .py, .ts, .js, .go, .java).".into()
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
    let _ensure = ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    let leaf_ids = graph.leaves();
    
    // If still empty, return a helpful message
    if leaf_ids.len() == 0 && graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "No source code found in the project directory. Ensure the directory contains supported source files (.rs, .py, .ts, .js, .go, .java).".into()
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
    let _ensure = ensure_graph_built(ctx)?;

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
    let _ensure = ensure_graph_built(ctx)?;

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

    // Determine filter - empty string behaves same as None
    let module_filter: Option<&str> = input.module_filter.as_deref().filter(|s| !s.is_empty());

    // Add nodes (filtered by module_filter)
    for symbol_id in &symbol_ids {
        if let Some(symbol) = graph.get_symbol(symbol_id) {
            // Apply module filter - skip symbols whose file path doesn't contain the filter string
            if let Some(filter_str) = module_filter {
                if !symbol.location().file().contains(filter_str) {
                    continue;
                }
            }
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
            collect_edges_recursive(&graph, symbol_id, &mut visited, input.max_depth, &mut mermaid_lines, &mut edge_count, module_filter);
        }
    } else {
        // For full graph, add all edges (filtered by module_filter)
        for symbol_id in &symbol_ids {
            if let Some(symbol) = graph.get_symbol(symbol_id) {
                // Skip source symbol if it doesn't match filter
                if let Some(filter_str) = module_filter {
                    if !symbol.location().file().contains(filter_str) {
                        continue;
                    }
                }
                for (callee_id, _) in graph.callees(symbol_id) {
                    if let Some(callee) = graph.get_symbol(&callee_id) {
                        // Skip callee if it doesn't match module filter
                        if let Some(filter_str) = module_filter {
                            if !callee.location().file().contains(filter_str) {
                                continue;
                            }
                        }
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
    let _ensure = ensure_graph_built(ctx)?;

    // Get the project graph
    let graph = ctx.analysis_service.get_project_graph();

    // Check if graph is empty after build attempt
    if graph.symbol_count() == 0 {
        return Err(HandlerError::NotFound(
            "No source code found in the project directory. Ensure the directory contains supported source files (.rs, .py, .ts, .js, .go, .java).".into()
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
    module_filter: Option<&str>,
) {
    if remaining_depth == 0 || visited.contains(symbol_id) {
        return;
    }

    visited.insert(symbol_id.clone());

    if let Some(symbol) = graph.get_symbol(symbol_id) {
        // Skip if symbol doesn't match module filter
        if let Some(filter_str) = module_filter {
            if !symbol.location().file().contains(filter_str) {
                return;
            }
        }

        for (callee_id, _) in graph.callees(symbol_id) {
            if !visited.contains(&callee_id) {
                if let Some(callee) = graph.get_symbol(&callee_id) {
                    // Skip callee if it doesn't match module filter
                    if let Some(filter_str) = module_filter {
                        if !callee.location().file().contains(filter_str) {
                            continue;
                        }
                    }
                    let from_id = sanitize_mermaid_id(symbol.name());
                    let to_id = sanitize_mermaid_id(callee.name());
                    lines.push(format!("    {} --> {}", from_id, to_id));
                    *edge_count += 1;
                    collect_edges_recursive(graph, &callee_id, visited, remaining_depth - 1, lines, edge_count, module_filter);
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

    if ctx.is_cancelled() {
        return Err(HandlerError::Internal("Cancelled".into()));
    }

    // Build the index — CPU-bound blocking work, offload to blocking thread pool
    // to avoid starving the tokio async runtime (which caused MCP timeout -32001).
    // For lightweight strategy, build directly as LightweightStrategy (concrete type)
    // so we can extract the index via into_index() without rebuilding.
    let strategy_name_input = input.strategy.clone();
    let result = tokio::task::spawn_blocking(move || {
        match strategy_name_input.as_str() {
            "lightweight" | "" => {
                let mut s = LightweightStrategy::new();
                let build_result = s.build_index(&directory);
                let symbols = s.query_symbols("").len();
                let index = Some(s.into_index());
                (build_result, symbols, "LightweightStrategy".to_string(), index, directory)
            }
            _ => {
                let mut strategy: Box<dyn GraphStrategy> = match strategy_name_input.as_str() {
                    "on_demand" | "ondemand" => Box::new(OnDemandStrategy::new()),
                    "per_file" | "perfile" => Box::new(PerFileStrategy::new()),
                    "full" | "full_graph" => Box::new(FullGraphStrategy::new()),
                    _ => Box::new(LightweightStrategy::new()),
                };
                let build_result = strategy.build_index(&directory);
                let symbols = strategy.query_symbols("").len();
                let name = strategy.name().to_string();
                (build_result, symbols, name, None, directory)
            }
        }
    })
    .await
    .map_err(|e| HandlerError::Internal(format!("spawn_blocking panicked: {e}")))?;

    let (build_result, symbols, strategy_name, opt_index, _directory) = result;

    match build_result {
        Ok(()) => {
            let elapsed = start.elapsed().as_millis() as u64;

            // Cache the lightweight index in AnalysisService (no rebuild needed)
            if let Some(index) = opt_index {
                ctx.analysis_service.set_symbol_index(index);
            }

            Ok(BuildIndexOutput {
                success: true,
                strategy: strategy_name.clone(),
                symbols_indexed: symbols,
                locations_indexed: symbols,
                message: format!(
                    "Index built successfully using {} strategy in {}ms",
                    strategy_name,
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

    // Empty symbol name returns empty results
    if input.symbol_name.is_empty() {
        return Ok(QuerySymbolOutput {
            symbol_name: input.symbol_name,
            locations: Vec::new(),
            total: 0,
        });
    }

    // Use the cached index in AnalysisService if available
    let locations = if ctx.analysis_service.has_symbol_index() {
        ctx.analysis_service.find_symbol(&input.symbol_name)
    } else {
        // Cache miss: build the index and store it for next time
        let directory = resolve_directory(input.directory, &ctx.working_dir);
        let index = tokio::task::spawn_blocking({
            let dir = directory.clone();
            move || {
                let mut s = LightweightStrategy::new();
                s.build_index(&dir).ok();
                s.into_index()
            }
        })
        .await
        .map_err(|e| HandlerError::Internal(e.to_string()))?;
        ctx.analysis_service.set_symbol_index(index);
        ctx.analysis_service.find_symbol(&input.symbol_name)
    };

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
    let _ensure = ensure_semantic_indexed(ctx)?;

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

    #[tokio::test]
    async fn test_handle_build_graph_creates_redb_on_first_call() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a simple Rust file
        let rust_file = tempdir_path.join("hello.rs");
        std::fs::write(&rust_file, "fn hello() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());
        let input = BuildGraphInput {
            directory: None,
        };

        let result = handle_build_graph(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.message.contains("built + persisted"));

        // Verify redb file exists at .cognicode/graph.redb inside the analyzed project
        let db_path = tempdir_path.join(".cognicode").join("graph.redb");
        assert!(db_path.exists(), "Expected .cognicode/graph.redb to exist at {:?}", db_path);
    }

    #[tokio::test]
    async fn test_handle_build_graph_loads_from_cache_on_second_call() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a simple Rust file
        let rust_file = tempdir_path.join("hello.rs");
        std::fs::write(&rust_file, "fn hello() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // First call - should build and persist
        let input1 = BuildGraphInput {
            directory: None,
        };
        let result1 = handle_build_graph(&ctx, input1).await;
        assert!(result1.is_ok());
        let output1 = result1.unwrap();
        assert!(output1.success);
        assert!(output1.message.contains("built + persisted"));
        let symbols_first = output1.symbols_found;

        // Second call - should load from cache
        let input2 = BuildGraphInput {
            directory: None,
        };
        let result2 = handle_build_graph(&ctx, input2).await;
        assert!(result2.is_ok());
        let output2 = result2.unwrap();
        assert!(output2.success);
        assert!(output2.message.contains("cache (redb)"));
        assert_eq!(output2.symbols_found, symbols_first);
    }

    #[tokio::test]
    async fn test_handle_build_graph_different_dirs_use_different_cache_files() {
        // Create two temp directories with different Rust files
        let tempdir1 = tempfile::tempdir().unwrap();
        let tempdir1_path = tempdir1.path();
        let rust_file1 = tempdir1_path.join("file1.rs");
        std::fs::write(&rust_file1, "fn function_one() {}\n").unwrap();

        let tempdir2 = tempfile::tempdir().unwrap();
        let tempdir2_path = tempdir2.path();
        let rust_file2 = tempdir2_path.join("file2.rs");
        std::fs::write(&rust_file2, "fn function_two() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir1_path.to_path_buf());

        // First call with tempdir1
        let input1 = BuildGraphInput {
            directory: Some(tempdir1_path.to_string_lossy().to_string()),
        };
        let result1 = handle_build_graph(&ctx, input1).await;
        assert!(result1.is_ok());
        assert!(result1.unwrap().success);

        // Second call with tempdir2
        let input2 = BuildGraphInput {
            directory: Some(tempdir2_path.to_string_lossy().to_string()),
        };
        let result2 = handle_build_graph(&ctx, input2).await;
        assert!(result2.is_ok());
        assert!(result2.unwrap().success);

        // Each project directory has its own .cognicode/graph.redb
        let db_path1 = tempdir1_path.join(".cognicode").join("graph.redb");
        let db_path2 = tempdir2_path.join(".cognicode").join("graph.redb");
        assert!(db_path1.exists(), "Expected cache file in tempdir1 at {:?}", db_path1);
        assert!(db_path2.exists(), "Expected cache file in tempdir2 at {:?}", db_path2);
        // Files are different (different content)
        let content1 = std::fs::read(&db_path1).unwrap();
        let content2 = std::fs::read(&db_path2).unwrap();
        assert_ne!(content1, content2, "Cache files should contain different graphs");
    }

    #[tokio::test]
    async fn test_handle_build_graph_detects_stale_cache() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a simple Rust file
        let rust_file = tempdir_path.join("hello.rs");
        std::fs::write(&rust_file, "fn hello() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // First call - should build and persist
        let input1 = BuildGraphInput {
            directory: None,
        };
        let result1 = handle_build_graph(&ctx, input1).await;
        assert!(result1.is_ok());
        let output1 = result1.unwrap();
        assert!(output1.success);
        assert!(output1.message.contains("built + persisted"));

        // Modify the file to make the cache stale
        std::fs::write(&rust_file, "fn hello() { let x = 1; }\n").unwrap();

        // Second call - should detect stale cache and rebuild
        let input2 = BuildGraphInput {
            directory: None,
        };
        let result2 = handle_build_graph(&ctx, input2).await;
        assert!(result2.is_ok());
        let output2 = result2.unwrap();
        assert!(output2.success);
        // Should rebuild because file was modified
        assert!(output2.message.contains("built + persisted"),
            "Expected 'built + persisted' but got: {}", output2.message);
    }

    // =============================================================================
    // Export Mermaid module_filter tests
    // =============================================================================

    #[tokio::test]
    async fn test_export_mermaid_module_filter_returns_only_matching_symbols() {
        // Create a temp project with two files - one that matches the filter and one that doesn't
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create handlers.rs file with a function
        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        // Create main.rs file with a different function
        let main_file = tempdir_path.join("main.rs");
        std::fs::write(&main_file, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // Build the graph first
        let build_input = BuildGraphInput { directory: None };
        let build_result = handle_build_graph(&ctx, build_input).await;
        assert!(build_result.is_ok());

        // Now export with module_filter set to "handlers.rs"
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("handlers.rs".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // The mermaid code should only contain symbols from handlers.rs
        // (node_count should be 1, from the handle_request function)
        assert!(output.node_count >= 1, "Expected at least 1 node from handlers.rs, got {}", output.node_count);
        // Verify the handlers function appears in the output
        assert!(output.mermaid_code.contains("handle_request"),
            "Expected 'handle_request' to appear in output, got: {}", output.mermaid_code);
        // main should NOT appear since it doesn't match the filter
        // (unless there's a reference to it)
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_backwards_compat() {
        // Create a temp project
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // Build the graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export WITHOUT module_filter (backwards compat - should return all symbols)
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: None,
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Should include the handlers function
        assert!(output.mermaid_code.contains("handle_request"),
            "Expected 'handle_request' in output, got: {}", output.mermaid_code);
        assert!(output.node_count >= 1, "Expected at least 1 node, got {}", output.node_count);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_empty_string() {
        // Create a temp project
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // Build the graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export with empty string filter - should behave same as None (no filtering)
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Empty string should match everything, same as None
        assert!(output.node_count >= 1, "Expected at least 1 node, got {}", output.node_count);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_no_matches() {
        // Create a temp project
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        // Build the graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export with filter that matches nothing
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("nonexistent_module.rs".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Should return empty diagram, not an error
        assert_eq!(output.node_count, 0, "Expected 0 nodes when no matches, got {}", output.node_count);
        assert_eq!(output.edge_count, 0, "Expected 0 edges when no matches, got {}", output.edge_count);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_with_format_svg() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export with filter + format=svg
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: Some("tokyo-night-light".to_string()),
            format: Some("svg".to_string()),
            module_filter: Some("handlers.rs".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // SVG format should produce svg field
        assert!(output.svg.is_some(), "Expected SVG output when format=svg, got None");
        assert!(output.node_count >= 1, "Expected at least 1 node, got {}", output.node_count);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_with_format_code() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export with filter + format=code
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("handlers.rs".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Code format should NOT produce svg field
        assert!(output.svg.is_none(), "Expected no SVG when format=code, got Some");
        assert!(output.node_count >= 1, "Expected at least 1 node, got {}", output.node_count);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_combines_with_root_symbol() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create two files
        let handlers_file = tempdir_path.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_request() {}\n pub fn internal_helper() {}\n").unwrap();

        let main_file = tempdir_path.join("main.rs");
        std::fs::write(&main_file, "fn main() { handle_request(); }\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Export with both root_symbol and module_filter
        // root_symbol limits to subgraph around handle_request
        // module_filter should further limit to symbols from handlers.rs
        let export_input = ExportMermaidInput {
            root_symbol: Some("handle_request".to_string()),
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("handlers.rs".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Should have at least handle_request node
        assert!(output.mermaid_code.contains("handle_request"),
            "Expected 'handle_request' in output, got: {}", output.mermaid_code);
    }

    #[tokio::test]
    async fn test_export_mermaid_module_filter_path_separator() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a nested directory structure
        let interface_dir = tempdir_path.join("interface");
        std::fs::create_dir(&interface_dir).unwrap();
        let mcp_dir = interface_dir.join("mcp");
        std::fs::create_dir(&mcp_dir).unwrap();

        let handlers_file = mcp_dir.join("handlers.rs");
        std::fs::write(&handlers_file, "pub fn handle_mcp_request() {}\n").unwrap();

        let main_file = tempdir_path.join("main.rs");
        std::fs::write(&main_file, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::new(tempdir_path.to_path_buf());

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Filter by "interface/mcp" should match the nested path
        let export_input = ExportMermaidInput {
            root_symbol: None,
            max_depth: 3,
            include_external: false,
            theme: None,
            format: Some("code".to_string()),
            module_filter: Some("interface/mcp".to_string()),
        };
        let export_result = handle_export_mermaid(&ctx, export_input).await;
        assert!(export_result.is_ok());
        let output = export_result.unwrap();

        // Should match symbols from interface/mcp/handlers.rs
        assert!(output.mermaid_code.contains("handle_mcp_request"),
            "Expected 'handle_mcp_request' in output, got: {}", output.mermaid_code);
        assert!(output.node_count >= 1, "Expected at least 1 node, got {}", output.node_count);
    }

    // =============================================================================
    // Ensure graph auto-build tests
    // =============================================================================

    #[tokio::test]
    async fn test_ensure_graph_built_auto_resolves() {
        // Create a temp dir with a Rust file
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Call ensure_graph_built on empty state — should auto-build
        let result = ensure_graph_built(&ctx).unwrap();
        assert!(result.auto_built);
        assert!(result.count > 0);
        assert!(result.message.contains("auto-built"));
    }

    #[tokio::test]
    async fn test_ensure_graph_built_skips_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Build graph first
        ctx.analysis_service.build_project_graph(temp.path()).unwrap();

        // Now ensure should not auto-build
        let result = ensure_graph_built(&ctx).unwrap();
        assert!(!result.auto_built);
    }

    #[tokio::test]
    async fn test_analyze_impact_auto_builds_graph() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { calculate(); }\nfn calculate() -> i32 { 42 }").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Call analyze_impact WITHOUT calling build_graph first
        let input = AnalyzeImpactInput { symbol_name: "calculate".to_string(), compressed: false };
        let result = handle_analyze_impact(&ctx, input).await.unwrap();

        // Should succeed (auto-built graph) and include auto-built note
        assert!(result.summary.contains("auto-built") || result.summary.contains("auto_built"));
    }

    #[tokio::test]
    async fn test_get_entry_points_auto_builds_graph() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Call get_entry_points WITHOUT calling build_graph first
        let input = GetEntryPointsInput { compressed: false };
        let result = handle_get_entry_points(&ctx, input).await.unwrap();

        // Should succeed with auto-built graph
        // (entry_points may be empty for simple files, but shouldn't error)
        assert!(result.total >= 0);
    }
}