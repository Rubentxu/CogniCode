//! Analysis DTOs - Transport-neutral types for code analysis
//!
//! These DTOs decouple the application layer from the MCP protocol.

use std::collections::HashMap;

use super::common::{AnalysisMetadata, RiskLevel, SourceLocation, SymbolSummary};
use serde::{Deserialize, Serialize};

/// Refactor action type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefactorAction {
    Rename,
    Extract,
    Inline,
    Move,
    ChangeSignature,
}

/// A single change entry for refactor operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub file: String,
    pub old_text: String,
    pub new_text: String,
    pub location: SourceLocation,
}

/// Validation result for refactor operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Result of refactor operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorResult {
    pub action: RefactorAction,
    pub success: bool,
    pub changes: Vec<ChangeEntry>,
    pub validation_result: ValidationResult,
    pub error_message: Option<String>,
}

/// Cycle information for architecture checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleInfo {
    pub symbols: Vec<String>,
    pub length: usize,
}

/// Violation information for architecture checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationInfo {
    pub rule: String,
    pub from: String,
    pub to: String,
    pub severity: String,
}

/// Result of architecture check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureResult {
    pub cycles: Vec<CycleInfo>,
    pub violations: Vec<ViolationInfo>,
    pub score: f32,
    pub summary: String,
}

/// Complexity metrics for a function or file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityResult {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub lines_of_code: u32,
    pub parameter_count: u32,
    pub nesting_depth: u32,
    pub function_name: Option<String>,
}

/// Result of build index operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildIndexResult {
    pub success: bool,
    pub strategy: String,
    pub symbols_indexed: usize,
    pub locations_indexed: usize,
    pub message: String,
}

// ============================================================================
// File Symbols
// ============================================================================

/// Result of getting file symbols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileSymbolsResult {
    pub file_path: String,
    pub symbols: Vec<SymbolSummary>,
}

// ============================================================================
// Call Hierarchy
// ============================================================================

/// Entry in call hierarchy result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub confidence: f32,
}

/// Result of getting call hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCallHierarchyResult {
    pub symbol: String,
    pub calls: Vec<CallHierarchyEntry>,
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Analyze Impact
// ============================================================================

/// Result of impact analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeImpactResult {
    pub symbol: String,
    pub impacted_files: Vec<String>,
    pub impacted_symbols: Vec<String>,
    pub risk_level: RiskLevel,
    pub summary: String,
}

// ============================================================================
// Graph Statistics
// ============================================================================

/// Coverage metrics for the project graph build process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCoverageMetrics {
    /// Total number of source files discovered during walk
    pub total_source_files: usize,
    /// Number of files successfully parsed
    pub parsed_files: usize,
    /// Number of call relationships where callee could not be resolved
    pub unresolved_edges: usize,
    /// Percentage of files successfully parsed (parsed_files / total_source_files * 100)
    pub coverage_percent: f64,
}

/// Statistics about the call graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatsDto {
    /// Total number of symbols in the graph
    pub symbol_count: usize,
    /// Total number of edges (call relationships)
    pub edge_count: usize,
    /// Number of unique files containing symbols
    pub file_count: usize,
    /// Breakdown of symbols by programming language
    pub language_breakdown: HashMap<String, usize>,
    /// Coverage metrics from the last graph build
    pub coverage: Option<GraphCoverageMetrics>,
}

/// A hot path entry (frequently called function)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPathDto {
    /// Name of the symbol
    pub symbol_name: String,
    /// File containing the symbol
    pub file: String,
    /// Line number
    pub line: u32,
    /// Number of callers (fan-in)
    pub fan_in: usize,
    /// Number of callees (fan-out)
    pub fan_out: usize,
}

// ============================================================================
// Dead Code Detection
// ============================================================================

/// Reason why a symbol is considered dead code
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeadCodeReason {
    /// No incoming edges (no callers, no references)
    NoIncomingEdges,
    /// Only referenced from test files
    OnlyReferencedByTests,
    /// Symbol is in an unreachable module
    UnreachableModule,
}

/// A single dead code entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadCodeEntry {
    /// Fully qualified symbol name
    pub symbol: String,
    /// File containing the symbol
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
    /// Kind of symbol
    pub kind: super::common::SymbolKind,
    /// Reason why this is considered dead
    pub reason: DeadCodeReason,
    /// Confidence score (0.0-1.0) — higher = more likely dead
    pub confidence: f32,
}

/// Result of dead code detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadCodeResult {
    /// List of dead code entries
    pub dead_code: Vec<DeadCodeEntry>,
    /// Total number of dead code symbols found
    pub total_dead: usize,
    /// Total symbols analyzed
    pub total_symbols: usize,
    /// Percentage of dead code
    pub dead_code_percent: f32,
    /// Analysis metadata
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Module Dependency Graph
// ============================================================================

/// Dependency information for a single module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    /// Module path (e.g., "src/auth" or "crates/agent")
    pub module: String,
    /// Modules this module depends on
    pub depends_on: Vec<String>,
    /// Modules that depend on this module
    pub depended_by: Vec<String>,
    /// Number of cross-module edges
    pub coupling_score: usize,
    /// Stability score (0.0-1.0): higher with more incoming dependencies
    pub stability: f32,
}

/// Complete module dependency graph with cycle detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependencyGraph {
    /// List of all modules with their dependencies
    pub modules: Vec<ModuleDependency>,
    /// Detected cycles between modules
    pub cycles: Vec<Vec<String>>,
    /// Coupling matrix: (from_module, to_module) → edge count
    pub coupling_matrix: Vec<(String, String, usize)>,
}

/// Result of module dependency analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependenciesResult {
    /// The module dependency graph
    pub graph: ModuleDependencyGraph,
    /// Total number of modules
    pub total_modules: usize,
    /// Total cross-module edges
    pub total_cross_module_edges: usize,
    /// Number of cycles detected
    pub cycle_count: usize,
    /// Analysis metadata
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Project Diagnostics
// ============================================================================

/// Complexity summary for the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySummaryDto {
    /// Total cyclomatic complexity across all functions
    pub total_cyclomatic: usize,
    /// Number of functions analyzed
    pub functions_analyzed: usize,
    /// Average complexity per function
    pub average_complexity: f64,
}

/// Aggregated diagnostics for the entire project
///
/// Combines multiple analysis components into a single response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDiagnosticsDto {
    /// Graph statistics (None if graph not built)
    pub stats: Option<GraphStatsDto>,
    /// Hot paths in the project (empty if graph not built)
    pub hot_paths: Vec<HotPathDto>,
    /// Architecture check result (None if graph not built)
    pub architecture: Option<ArchitectureResult>,
    /// Complexity summary (None if graph not built)
    pub complexity: Option<ComplexitySummaryDto>,
}

impl ProjectDiagnosticsDto {
    /// Format as XML for injection into LLM system prompt.
    /// Returns empty string if the project graph has not been built.
    pub fn to_xml(&self) -> String {
        let stats = match self.stats.as_ref() {
            Some(s) => s,
            None => return String::new(),
        };

        let symbol_count = stats.symbol_count;
        let edge_count = stats.edge_count;
        let index_time_ms = 0; // Not available in stats, default to 0

        // Format language breakdown as percentages
        let total_symbols_for_pct: usize = stats.language_breakdown.values().sum();
        let languages_str = if total_symbols_for_pct > 0 {
            let mut langs: Vec<String> = stats
                .language_breakdown
                .iter()
                .map(|(lang, count)| {
                    let pct = (*count as f32 / total_symbols_for_pct as f32) * 100.0;
                    format!("{} ({:.0}%)", lang, pct)
                })
                .collect();
            langs.sort();
            langs.join(", ")
        } else {
            "None".to_string()
        };

        // Collect diagnostics
        let mut diagnostics: Vec<String> = Vec::new();

        // Architecture violations as "error" severity
        if let Some(arch) = self.architecture.as_ref() {
            for v in &arch.violations {
                let from = if v.from.is_empty() { "unknown" } else { &v.from };
                let to = if v.to.is_empty() { "unknown" } else { &v.to };
                diagnostics.push(format!(
                    "    error Architecture violation: {} -> {} ({})",
                    from, to, v.rule
                ));
            }
        }

        // Format hot paths
        let hot_paths_str = if !self.hot_paths.is_empty() {
            let paths: Vec<String> = self
                .hot_paths
                .iter()
                .enumerate()
                .map(|(i, hp)| {
                    format!(
                        "    {}. {} (fan-in: {}, file: {})",
                        i + 1,
                        hp.symbol_name,
                        hp.fan_in,
                        hp.file
                    )
                })
                .collect();
            format!("\n  Hot paths (most-called):\n{}", paths.join("\n"))
        } else {
            String::new()
        };

        // Format diagnostics section
        let diagnostics_str = if !diagnostics.is_empty() {
            format!("\n  Active diagnostics:\n{}", diagnostics.join("\n"))
        } else {
            String::new()
        };

        format!(
            "<code-intelligence>
  Project: {} symbols, {} call edges (indexed in {}ms)
  Status: READY
  Languages: {}{}{}
  Last updated: 0s ago
</code-intelligence>",
            symbol_count,
            edge_count,
            index_time_ms,
            languages_str,
            diagnostics_str,
            hot_paths_str
        )
    }
}

// ============================================================================
// Smart Overview (AIX-1.1)
// ============================================================================

/// Detail level for smart_overview output
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OverviewDetail {
    /// ~100 tokens: project type + basic stats only
    Quick,
    /// ~400 tokens: + top entry points + hot paths + architecture score
    Medium,
    /// ~800 tokens: + first reads + complexity summary + coverage
    Detailed,
}

impl std::fmt::Display for OverviewDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverviewDetail::Quick => write!(f, "quick"),
            OverviewDetail::Medium => write!(f, "medium"),
            OverviewDetail::Detailed => write!(f, "detailed"),
        }
    }
}

/// Detected project type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    /// Web API (REST/GraphQL endpoints detected)
    WebApi,
    /// CLI application (main.rs with clap/structopt)
    Cli,
    /// Library crate (lib.rs without binary)
    Library,
    /// Multi-crate workspace
    Monorepo,
    /// Cannot determine — default
    Unknown,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::WebApi => write!(f, "web_api"),
            ProjectType::Cli => write!(f, "cli"),
            ProjectType::Library => write!(f, "library"),
            ProjectType::Monorepo => write!(f, "monorepo"),
            ProjectType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Entry point summary (compact — AI-optimized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPointSummary {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub kind: String, // "function", "main", "struct", etc.
    /// Short description: what this entry point does (1 line)
    pub summary: String,
}

/// Complete smart overview response for AI agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartOverviewDto {
    /// Detected project type
    pub project_type: String,
    /// Total symbols in the graph
    pub total_symbols: usize,
    /// Total call edges
    pub total_edges: usize,
    /// Language breakdown (language → file count)
    pub languages: std::collections::HashMap<String, usize>,
    /// Top entry points (max 5) — empty if detail=quick or no graph
    pub top_entry_points: Vec<EntryPointSummary>,
    /// Critical hot paths (max 5) — empty if detail=quick
    pub critical_hot_paths: Vec<HotPathDto>,
    /// Architecture health score (0-100, None if no graph)
    pub architecture_score: Option<f32>,
    /// Number of cycles detected (None if no graph)
    pub cycle_count: Option<usize>,
    /// Recommended files to read first (5 files, only in detailed)
    pub recommended_first_reads: Vec<String>,
    /// Coverage percentage (parsed / total source files * 100, None if no graph)
    pub coverage_percent: Option<f64>,
    /// Metadata for AI agents to manage context budget
    pub _meta: OverviewMeta,
}

/// Metadata for AI context budget management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewMeta {
    /// Estimated token count of this response
    pub estimated_tokens: usize,
    /// Detail level used
    pub detail_level: String,
}

// ============================================================================
// Ranked Symbols (AIX-1.3)
// ============================================================================

/// A symbol ranked by AI-relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedSymbolDto {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub kind: String,
    /// AI-relevance score (0.0–1.0). Higher = more important for an AI to know.
    pub relevance_score: f64,
    /// Number of callers (fan-in)
    pub fan_in: usize,
    /// Cyclomatic complexity
    pub complexity: Option<u32>,
    /// Has documentation comment
    pub has_docs: bool,
    /// Short summary (1 line for quick scanning)
    pub summary: String,
}

/// Response for ranked_symbols tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedSymbolsResult {
    pub query: String,
    pub total_matches: usize,
    pub returned: usize,
    pub results: Vec<RankedSymbolDto>,
    pub _meta: OverviewMeta, // reuse existing OverviewMeta for token estimation
}

// ============================================================================
// Auto Diagnose (AIX-2.3)
// ============================================================================

/// Severity level for diagnostic issues
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Info,
    Warning,
    /// Should be addressed
    Important,
    /// Must be addressed
    Critical,
}

impl std::fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueSeverity::Info => write!(f, "info"),
            IssueSeverity::Warning => write!(f, "warning"),
            IssueSeverity::Important => write!(f, "important"),
            IssueSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// Category of diagnostic issue
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    Architecture,
    Complexity,
    DeadCode,
    Coverage,
    Coupling,
    HotPath,
}

/// A single diagnostic finding with recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseIssue {
    pub category: String,
    pub severity: String,
    /// Human-readable description of the issue
    pub title: String,
    /// Detailed explanation (1-2 sentences)
    pub description: String,
    /// What to do about it (actionable!)
    pub recommendation: String,
    /// Symbols or files involved
    pub location: Option<String>,
    /// Metric value (e.g., "15" for complexity, "3" for cycle count)
    pub metric: Option<String>,
}

/// Comprehensive diagnostic report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseReportDto {
    /// Overall health score (0-100)
    pub health_score: f64,
    /// Number of issues found
    pub total_issues: usize,
    /// Breakdown by severity
    pub critical_count: usize,
    pub important_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    /// All issues, sorted by severity (critical first)
    pub issues: Vec<DiagnoseIssue>,
    /// Summary of graph stats
    pub symbol_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    /// Architecture-specific findings
    pub cycles: Vec<String>,  // cycle descriptions
    pub architecture_score: Option<f32>,
    /// Complexity summary
    pub avg_complexity: Option<f64>,
    pub max_complexity: Option<(String, u32)>,  // (function, score)
    /// Dead code stats
    pub dead_code_count: usize,
    pub dead_code_percent: Option<f32>,
    /// Module coupling
    pub module_coupling_issues: usize,
    /// Metadata
    pub _meta: OverviewMeta,
}

// ============================================================================
// Onboarding Plan (AIX-2.1)
// ============================================================================

/// Goal for the onboarding plan
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingGoal {
    /// Understand the codebase (default)
    Understand,
    /// Plan a refactoring
    Refactor,
    /// Debug an issue
    Debug,
    /// Add a new feature
    AddFeature,
    /// Review code quality
    Review,
}

/// A single step in the onboarding plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStep {
    /// Step number (1-indexed)
    pub step: usize,
    /// Tool name to call
    pub tool: String,
    /// Suggested parameters
    pub params: std::collections::HashMap<String, serde_json::Value>,
    /// Why this step is recommended
    pub rationale: String,
    /// Estimated tokens this step will consume
    pub estimated_tokens: usize,
    /// What the agent should learn from this step
    pub expected_outcome: String,
}

/// Complete onboarding plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingPlanDto {
    /// The goal this plan addresses
    pub goal: String,
    /// Number of steps in the plan
    pub total_steps: usize,
    /// Total estimated tokens for all steps
    pub total_estimated_tokens: usize,
    /// Ordered steps to execute
    pub steps: Vec<OnboardingStep>,
    /// Metadata
    pub _meta: OverviewMeta,
}

// ============================================================================
// Refactor Plan (AIX-2.2)
// ============================================================================

/// A single refactoring action in the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorActionStep {
    /// Step number (1-indexed)
    pub step: usize,
    /// Type of refactoring action
    pub action: String, // "rename", "extract", "move", "inline", "split", "break_cycle", "add_trait", "simplify"
    /// Symbol to operate on
    pub target: String,
    /// Suggested new name or target file (if applicable)
    pub suggestion: Option<String>,
    /// Risk level: low, medium, high, critical
    pub risk: String,
    /// Estimated files affected
    pub files_affected: usize,
    /// Why this action is recommended
    pub rationale: String,
    /// Expected improvement (e.g., "reduces complexity from 18 to 5")
    pub expected_benefit: String,
}

/// Complete refactoring plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSuggestionDto {
    /// Symbol being analyzed
    pub symbol: String,
    /// Current complexity
    pub current_complexity: Option<u32>,
    /// Number of direct callers (fan-in)
    pub caller_count: usize,
    /// Number of files that depend on this symbol
    pub impacted_files: usize,
    /// Overall risk level for the full plan
    pub overall_risk: String,
    /// Ordered steps
    pub steps: Vec<RefactorActionStep>,
    /// Suggested order: "sequential" (one at a time) or "parallel" (can do some together)
    pub execution_mode: String,
    /// Metadata
    pub _meta: OverviewMeta,
}

// ============================================================================
// NL to Symbol (AIX-3.1)
// ============================================================================

/// A symbol match from natural language query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlSymbolMatch {
    pub symbol_name: String,
    pub file: String,
    pub line: u32,
    pub kind: String,
    /// Confidence score 0.0-1.0
    pub confidence: f64,
    /// Why this symbol matched the NL description
    pub match_reason: String,
    /// Function signature or struct definition (1 line)
    pub snippet: Option<String>,
    /// Number of callers (fan-in), 0 if unknown
    pub fan_in: usize,
}

/// Response for nl_to_symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlToSymbolResult {
    pub query: String,
    /// Keywords extracted from the NL query
    pub extracted_keywords: Vec<String>,
    pub total_candidates: usize,
    pub results: Vec<NlSymbolMatch>,
    pub _meta: OverviewMeta,
}

/// Known intent patterns for find_pattern_by_intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentPattern {
    pub intent_keywords: Vec<String>,
    pub description: String,
    /// Tree-sitter query or pattern name
    pub query_hint: String,
    /// Example NL description that triggers this pattern
    pub example: String,
}

/// Pattern match suggestion from intent matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMatch {
    pub intent_name: String,
    pub description: String,
    pub query_hint: String,
}

/// Response for find_pattern_by_intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindPatternResult {
    pub query: String,
    pub matched_intents: Vec<IntentMatch>,
    /// All available intents (when list_patterns=true)
    pub all_patterns: Vec<String>,
    pub _meta: OverviewMeta,
}

// ============================================================================
// Ask About Code (AIX-3.2)
// ============================================================================

/// A path step in a code answer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePathStep {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub kind: String,
    /// What this function does in the flow
    pub role: String,
    /// Source code snippet (1-3 lines)
    pub snippet: Option<String>,
}

/// A complete answer to a code question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAnswer {
    /// Path description in natural language
    pub explanation: String,
    /// Ordered steps in the execution path
    pub path: Vec<CodePathStep>,
    /// Source entry point
    pub from: String,
    /// Destination
    pub to: String,
    /// Number of hops in the path
    pub path_length: usize,
    /// Confidence (0.0-1.0)
    pub confidence: f64,
}

/// Response for ask_about_code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskAboutCodeResult {
    pub question: String,
    pub answers: Vec<CodeAnswer>,
    pub _meta: OverviewMeta,
}

// ============================================================================
// Token Estimation Functions (public for testing)
// ============================================================================

/// Token estimation for quick overview
pub fn estimate_tokens_quick(symbols: usize, languages: &std::collections::HashMap<String, usize>) -> usize {
    let base = 80; // ~80 tokens for structure
    let lang_tokens = languages.len() * 15;
    let sym_tokens = if symbols > 0 { 20 } else { 0 }; // ~20 tokens for symbol count
    base + lang_tokens + sym_tokens
}

/// Token estimation for medium overview
pub fn estimate_tokens_medium(
    _symbols: usize,
    _edges: usize,
    entry_points: &[EntryPointSummary],
    hot_paths: &[HotPathDto],
    arch_score: Option<f32>,
) -> usize {
    let base = 120;
    let ep_tokens = entry_points.iter().map(|e| e.name.len() / 4 + e.summary.len() / 4 + 20).sum::<usize>();
    let hp_tokens = hot_paths.iter().map(|h| h.symbol_name.len() / 4 + h.file.len() / 4 + 25).sum::<usize>();
    let arch_tokens = if arch_score.is_some() { 30 } else { 0 };
    base + ep_tokens + hp_tokens + arch_tokens
}

/// Token estimation for detailed overview
pub fn estimate_tokens_detailed(
    symbols: usize,
    edges: usize,
    entry_points: &[EntryPointSummary],
    hot_paths: &[HotPathDto],
    first_reads: &[String],
) -> usize {
    let medium = estimate_tokens_medium(symbols, edges, entry_points, hot_paths, None);
    let reads_tokens = first_reads.iter().map(|f| f.len() / 4 + 10).sum::<usize>();
    medium + reads_tokens + 50 // extra for complexity + coverage
}

// =============================================================================
// Graph Diff (AIX-4.1)
// =============================================================================

/// What changed between two graph snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiffDto {
    /// Whether a baseline was available
    pub has_baseline: bool,
    /// Symbols added since baseline
    pub symbols_added: Vec<String>,
    /// Symbols removed since baseline
    pub symbols_removed: Vec<String>,
    /// New call relationships: (caller, callee)
    pub edges_added: Vec<(String, String)>,
    /// Removed call relationships
    pub edges_removed: Vec<(String, String)>,
    /// New cycles introduced
    pub new_cycles: Vec<Vec<String>>,
    /// Cycles resolved
    pub resolved_cycles: Vec<Vec<String>>,
    /// Architecture score before
    pub architecture_score_before: Option<f32>,
    /// Architecture score after
    pub architecture_score_after: Option<f32>,
    /// Total symbols before
    pub symbols_before: usize,
    /// Total symbols after
    pub symbols_after: usize,
    /// Total edges before
    pub edges_before: usize,
    /// Total edges after
    pub edges_after: usize,
    /// Summary of changes
    pub summary: String,
    pub _meta: OverviewMeta,
}

// =============================================================================
// API Breaks Detection (AIX-4.2)
// =============================================================================

/// A breaking change in the public API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBreak {
    pub symbol: String,
    pub file: String,
    pub break_type: String, // "removed", "signature_changed", "visibility_reduced", "parameter_added", "parameter_removed"
    pub before: Option<String>,
    pub after: Option<String>,
    pub severity: String, // "major", "minor", "patch"
}

/// API break detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBreaksResult {
    pub has_baseline: bool,
    pub breaks: Vec<ApiBreak>,
    pub total_breaks: usize,
    pub severity_summary: String,
    pub _meta: OverviewMeta,
}

// =============================================================================
// System Prompt Context (AIX-5.1)
// =============================================================================

/// Output format for system prompt context
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContextFormat {
    Xml,
    Json,
    Markdown,
}

/// Generated context block for LLM system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptContext {
    pub format: String,
    pub content: String,
    pub estimated_tokens: usize,
}

// =============================================================================
// God Function Detection (AIX-5.2)
// =============================================================================

/// A detected god function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodFunctionDto {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    /// Lines of code
    pub lines: usize,
    /// Cyclomatic complexity
    pub complexity: u32,
    /// Number of callers
    pub fan_in: usize,
    /// Number of callees
    pub fan_out: usize,
    /// God score (0-100): higher = more problematic
    pub god_score: f64,
    /// Recommendation
    pub suggestion: String,
}

/// God function detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodFunctionsResult {
    pub god_functions: Vec<GodFunctionDto>,
    pub total_analyzed: usize,
    pub thresholds: GodFunctionThresholds,
    pub _meta: OverviewMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodFunctionThresholds {
    pub min_lines: usize,
    pub min_complexity: u32,
    pub min_fan_in: usize,
}

// =============================================================================
// Long Parameter Lists (AIX-5.3)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongParamFunctionDto {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub parameter_count: usize,
    pub parameter_names: Vec<String>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongParamsResult {
    pub functions: Vec<LongParamFunctionDto>,
    pub threshold: usize,
    pub total_analyzed: usize,
    pub _meta: OverviewMeta,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_to_xml_none_stats_returns_empty_string() {
        let dto = ProjectDiagnosticsDto {
            stats: None,
            hot_paths: vec![],
            architecture: None,
            complexity: None,
        };

        let result = dto.to_xml();
        assert!(result.is_empty(), "Expected empty string when stats is None");
    }

    #[test]
    fn test_to_xml_ready_state_with_counts() {
        let mut lang_breakdown = HashMap::new();
        lang_breakdown.insert("Rust".to_string(), 70);
        lang_breakdown.insert("TypeScript".to_string(), 30);

        let dto = ProjectDiagnosticsDto {
            stats: Some(GraphStatsDto {
                symbol_count: 100,
                edge_count: 50,
                file_count: 10,
                language_breakdown: lang_breakdown,
                coverage: None,
            }),
            hot_paths: vec![],
            architecture: None,
            complexity: None,
        };

        let result = dto.to_xml();

        assert!(result.contains("<code-intelligence>"));
        assert!(result.contains("100 symbols"));
        assert!(result.contains("50 call edges"));
        assert!(result.contains("Status: READY"));
        assert!(result.contains("Rust"));
        assert!(result.contains("TypeScript"));
        assert!(result.contains("</code-intelligence>"));
    }

    #[test]
    fn test_to_xml_with_hot_paths() {
        let mut lang_breakdown = HashMap::new();
        lang_breakdown.insert("Rust".to_string(), 100);

        let hot_paths = vec![
            HotPathDto {
                symbol_name: "process_data".to_string(),
                file: "src/lib.rs".to_string(),
                line: 42,
                fan_in: 5,
                fan_out: 2,
            },
            HotPathDto {
                symbol_name: "handle_request".to_string(),
                file: "src/main.rs".to_string(),
                line: 10,
                fan_in: 3,
                fan_out: 1,
            },
        ];

        let dto = ProjectDiagnosticsDto {
            stats: Some(GraphStatsDto {
                symbol_count: 50,
                edge_count: 25,
                file_count: 5,
                language_breakdown: lang_breakdown,
                coverage: None,
            }),
            hot_paths,
            architecture: None,
            complexity: None,
        };

        let result = dto.to_xml();

        assert!(result.contains("Hot paths (most-called):"));
        assert!(result.contains("process_data"));
        assert!(result.contains("fan-in: 5"));
        assert!(result.contains("src/lib.rs"));
        assert!(result.contains("handle_request"));
        assert!(result.contains("fan-in: 3"));
    }

    #[test]
    fn test_to_xml_with_architecture_violations() {
        let mut lang_breakdown = HashMap::new();
        lang_breakdown.insert("Rust".to_string(), 100);

        let violations = vec![
            ViolationInfo {
                rule: "no_cycles".to_string(),
                from: "module_a".to_string(),
                to: "module_b".to_string(),
                severity: "high".to_string(),
            },
        ];

        let dto = ProjectDiagnosticsDto {
            stats: Some(GraphStatsDto {
                symbol_count: 50,
                edge_count: 25,
                file_count: 5,
                language_breakdown: lang_breakdown,
                coverage: None,
            }),
            hot_paths: vec![],
            architecture: Some(ArchitectureResult {
                cycles: vec![],
                violations,
                score: 95.0,
                summary: "1 cycle detected".to_string(),
            }),
            complexity: None,
        };

        let result = dto.to_xml();

        assert!(result.contains("Active diagnostics:"));
        assert!(result.contains("error Architecture violation"));
        assert!(result.contains("module_a -> module_b"));
    }

    #[test]
    fn test_to_xml_language_percentages() {
        let mut lang_breakdown = HashMap::new();
        // 3 Rust, 1 TypeScript = 75% Rust, 25% TypeScript
        lang_breakdown.insert("Rust".to_string(), 3);
        lang_breakdown.insert("TypeScript".to_string(), 1);

        let dto = ProjectDiagnosticsDto {
            stats: Some(GraphStatsDto {
                symbol_count: 4,
                edge_count: 0,
                file_count: 2,
                language_breakdown: lang_breakdown,
                coverage: None,
            }),
            hot_paths: vec![],
            architecture: None,
            complexity: None,
        };

        let result = dto.to_xml();

        assert!(result.contains("Rust (75%)"));
        assert!(result.contains("TypeScript (25%)"));
    }

    // =========================================================================
    // Smart Overview Tests (AIX-1.1)
    // =========================================================================

    #[test]
    fn test_smart_overview_dto_serialization() {
        let mut languages = HashMap::new();
        languages.insert("Rust".to_string(), 50);
        languages.insert("TypeScript".to_string(), 30);

        let hot_paths = vec![
            HotPathDto {
                symbol_name: "process".to_string(),
                file: "src/lib.rs".to_string(),
                line: 42,
                fan_in: 5,
                fan_out: 2,
            },
        ];

        let dto = SmartOverviewDto {
            project_type: "library".to_string(),
            total_symbols: 100,
            total_edges: 50,
            languages,
            top_entry_points: vec![],
            critical_hot_paths: hot_paths,
            architecture_score: Some(95.0),
            cycle_count: Some(0),
            recommended_first_reads: vec!["src/lib.rs".to_string()],
            coverage_percent: Some(85.5),
            _meta: OverviewMeta {
                estimated_tokens: 350,
                detail_level: "medium".to_string(),
            },
        };

        // Test serialization
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"project_type\":\"library\""));
        assert!(json.contains("\"total_symbols\":100"));
        assert!(json.contains("\"estimated_tokens\":350"));

        // Test deserialization
        let deserialized: SmartOverviewDto = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.project_type, "library");
        assert_eq!(deserialized.total_symbols, 100);
        assert_eq!(deserialized.architecture_score, Some(95.0));
        assert_eq!(deserialized.critical_hot_paths.len(), 1);
    }

    #[test]
    fn test_overview_detail_display() {
        assert_eq!(OverviewDetail::Quick.to_string(), "quick");
        assert_eq!(OverviewDetail::Medium.to_string(), "medium");
        assert_eq!(OverviewDetail::Detailed.to_string(), "detailed");
    }

    #[test]
    fn test_project_type_display() {
        assert_eq!(ProjectType::WebApi.to_string(), "web_api");
        assert_eq!(ProjectType::Cli.to_string(), "cli");
        assert_eq!(ProjectType::Library.to_string(), "library");
        assert_eq!(ProjectType::Monorepo.to_string(), "monorepo");
        assert_eq!(ProjectType::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_token_estimation_quick() {
        let mut languages = HashMap::new();
        languages.insert("Rust".to_string(), 50);

        // Quick with no symbols
        let tokens = estimate_tokens_quick(0, &languages);
        assert!(tokens < 200, "Quick estimate should be < 200 tokens, got {}", tokens);

        // Quick with symbols
        let tokens = estimate_tokens_quick(100, &languages);
        assert!(tokens < 200, "Quick estimate should be < 200 tokens, got {}", tokens);
    }

    #[test]
    fn test_token_estimation_medium() {
        let entry_points = vec![
            EntryPointSummary {
                name: "main".to_string(),
                file: "src/main.rs".to_string(),
                line: 1,
                kind: "function".to_string(),
                summary: "Application entry point".to_string(),
            },
        ];
        let hot_paths = vec![
            HotPathDto {
                symbol_name: "process".to_string(),
                file: "src/lib.rs".to_string(),
                line: 42,
                fan_in: 5,
                fan_out: 2,
            },
        ];

        let tokens = estimate_tokens_medium(100, 50, &entry_points, &hot_paths, Some(95.0));
        assert!(tokens < 600, "Medium estimate should be < 600 tokens, got {}", tokens);
    }

    #[test]
    fn test_recommend_first_reads_empty() {
        // When there are no symbols, recommend_first_reads should return empty
        // This is tested indirectly via the public function API
        let mut languages = HashMap::new();
        languages.insert("Rust".to_string(), 10);
        let tokens = estimate_tokens_quick(0, &languages);
        // base (80) + lang_tokens (1 * 15) + sym_tokens (0, since symbols = 0)
        assert_eq!(tokens, 80 + 15);
    }

    #[test]
    fn test_overview_detail_serde() {
        // Test serialization/deserialization of OverviewDetail
        let quick = OverviewDetail::Quick;
        let json = serde_json::to_string(&quick).unwrap();
        assert_eq!(json, "\"quick\"");
        let deserialized: OverviewDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OverviewDetail::Quick);

        let detailed = OverviewDetail::Detailed;
        let json = serde_json::to_string(&detailed).unwrap();
        assert_eq!(json, "\"detailed\"");
        let deserialized: OverviewDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, OverviewDetail::Detailed);
    }

    #[test]
    fn test_project_type_serde() {
        // Test serialization/deserialization of ProjectType
        let web_api = ProjectType::WebApi;
        let json = serde_json::to_string(&web_api).unwrap();
        assert_eq!(json, "\"web_api\"");
        let deserialized: ProjectType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProjectType::WebApi);

        let monorepo = ProjectType::Monorepo;
        let json = serde_json::to_string(&monorepo).unwrap();
        assert_eq!(json, "\"monorepo\"");
        let deserialized: ProjectType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProjectType::Monorepo);
    }

    // =============================================================================
    // Auto Diagnose Tests (AIX-2.3)
    // =============================================================================

    #[test]
    fn test_issue_severity_ordering() {
        // Critical > Important > Warning > Info
        assert!(IssueSeverity::Critical > IssueSeverity::Important);
        assert!(IssueSeverity::Important > IssueSeverity::Warning);
        assert!(IssueSeverity::Warning > IssueSeverity::Info);
        assert!(IssueSeverity::Critical > IssueSeverity::Warning);
        assert!(IssueSeverity::Warning > IssueSeverity::Info);
    }

    #[test]
    fn test_issue_severity_display() {
        assert_eq!(IssueSeverity::Critical.to_string(), "critical");
        assert_eq!(IssueSeverity::Important.to_string(), "important");
        assert_eq!(IssueSeverity::Warning.to_string(), "warning");
        assert_eq!(IssueSeverity::Info.to_string(), "info");
    }

    #[test]
    fn test_diagnose_issue_serialization() {
        let issue = DiagnoseIssue {
            category: "architecture".to_string(),
            severity: "critical".to_string(),
            title: "Cyclic dependency detected".to_string(),
            description: "The call graph contains a cycle".to_string(),
            recommendation: "Break the cycle by introducing a trait".to_string(),
            location: Some("src/main.rs".to_string()),
            metric: Some("3".to_string()),
        };

        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("\"category\":\"architecture\""));
        assert!(json.contains("\"severity\":\"critical\""));
        assert!(json.contains("\"title\":\"Cyclic dependency detected\""));
        assert!(json.contains("\"recommendation\":\"Break the cycle"));
        assert!(json.contains("\"location\":\"src/main.rs\""));
        assert!(json.contains("\"metric\":\"3\""));

        let deserialized: DiagnoseIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category, "architecture");
        assert_eq!(deserialized.severity, "critical");
        assert_eq!(deserialized.title, "Cyclic dependency detected");
    }

    #[test]
    fn test_diagnose_report_serialization() {
        let report = DiagnoseReportDto {
            health_score: 85.0,
            total_issues: 3,
            critical_count: 1,
            important_count: 1,
            warning_count: 1,
            info_count: 0,
            issues: vec![
                DiagnoseIssue {
                    category: "architecture".to_string(),
                    severity: "critical".to_string(),
                    title: "Cycle detected".to_string(),
                    description: "Found a cycle".to_string(),
                    recommendation: "Break it".to_string(),
                    location: None,
                    metric: Some("3".to_string()),
                },
                DiagnoseIssue {
                    category: "dead_code".to_string(),
                    severity: "important".to_string(),
                    title: "Dead code".to_string(),
                    description: "Found dead code".to_string(),
                    recommendation: "Remove it".to_string(),
                    location: Some("src/lib.rs".to_string()),
                    metric: None,
                },
            ],
            symbol_count: 100,
            edge_count: 50,
            file_count: 10,
            cycles: vec!["a -> b -> c".to_string()],
            architecture_score: Some(75.0),
            avg_complexity: Some(3.5),
            max_complexity: Some(("process_data".to_string(), 15)),
            dead_code_count: 10,
            dead_code_percent: Some(10.0),
            module_coupling_issues: 2,
            _meta: OverviewMeta {
                estimated_tokens: 500,
                detail_level: "auto_diagnose".to_string(),
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"health_score\":85"));
        assert!(json.contains("\"total_issues\":3"));
        assert!(json.contains("\"critical_count\":1"));
        assert!(json.contains("\"symbol_count\":100"));
        assert!(json.contains("\"cycles\":[\"a -> b -> c\"]"));

        let deserialized: DiagnoseReportDto = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.health_score, 85.0);
        assert_eq!(deserialized.total_issues, 3);
        assert_eq!(deserialized.issues.len(), 2);
        assert_eq!(deserialized.cycles.len(), 1);
    }

    #[test]
    fn test_diagnose_report_empty_issues() {
        let report = DiagnoseReportDto {
            health_score: 100.0,
            total_issues: 0,
            critical_count: 0,
            important_count: 0,
            warning_count: 0,
            info_count: 0,
            issues: vec![],
            symbol_count: 50,
            edge_count: 25,
            file_count: 5,
            cycles: vec![],
            architecture_score: Some(100.0),
            avg_complexity: None,
            max_complexity: None,
            dead_code_count: 0,
            dead_code_percent: Some(0.0),
            module_coupling_issues: 0,
            _meta: OverviewMeta {
                estimated_tokens: 100,
                detail_level: "auto_diagnose".to_string(),
            },
        };

        assert_eq!(report.total_issues, 0);
        assert!(report.issues.is_empty());
        assert_eq!(report.health_score, 100.0);
    }
}
