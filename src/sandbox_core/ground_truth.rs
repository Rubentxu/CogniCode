//! Ground Truth Matching for MCP Tool Quality Evaluation
//!
//! This module provides ground truth structures and matching functions
//! for evaluating MCP tool correctness across Code Intelligence tools.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A symbol extracted from source code with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpectedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Option<SymbolLocation>,
}

/// The kind of a code symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Impl,
    Module,
    Enum,
    Trait,
    Method,
    Field,
    Const,
    Static,
    TypeAlias,
    Macro,
    Variant,
    Property,
    Class,
    Interface,
    Parameter,
    Variable,
    Other(String),
}

impl SymbolKind {
    /// Parse from string representation (used in ground truth YAML).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "function" | "fn" => SymbolKind::Function,
            "struct" => SymbolKind::Struct,
            "impl" => SymbolKind::Impl,
            "module" | "mod" => SymbolKind::Module,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "method" => SymbolKind::Method,
            "field" => SymbolKind::Field,
            "const" => SymbolKind::Const,
            "static" => SymbolKind::Static,
            "type_alias" | "type" => SymbolKind::TypeAlias,
            "macro" => SymbolKind::Macro,
            "variant" => SymbolKind::Variant,
            "property" | "prop" => SymbolKind::Property,
            "class" | "cls" => SymbolKind::Class,
            "interface" | "iface" => SymbolKind::Interface,
            "parameter" | "param" => SymbolKind::Parameter,
            "variable" | "var" => SymbolKind::Variable,
            other => SymbolKind::Other(other.to_string()),
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Struct => "struct",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "module",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Method => "method",
            SymbolKind::Field => "field",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::TypeAlias => "type_alias",
            SymbolKind::Macro => "macro",
            SymbolKind::Variant => "variant",
            SymbolKind::Property => "property",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Variable => "variable",
            SymbolKind::Other(s) => s,
        }
    }
}

/// Location of a symbol in source code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

/// A node in the hierarchical symbol outline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpectedOutlineNode {
    pub name: String,
    pub kind: SymbolKind,
    pub children: Vec<ExpectedOutlineNode>,
    pub location: Option<SymbolLocation>,
}

/// Ground truth for a single tool evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroundTruth {
    /// Expected symbols for get_file_symbols tool.
    #[serde(default)]
    pub symbols: Option<Vec<ExpectedSymbol>>,

    /// Expected outline for get_outline tool.
    #[serde(default)]
    pub outline: Option<Vec<ExpectedOutlineNode>>,

    /// Expected code for get_symbol_code tool.
    #[serde(default)]
    pub code: Option<ExpectedCode>,

    /// Expected complexity metrics for get_complexity tool.
    #[serde(default)]
    pub complexity: Option<ExpectedComplexity>,

    /// Expected usages for find_usages tool.
    #[serde(default)]
    pub usages: Option<Vec<ExpectedUsage>>,

    /// Expected search results for semantic_search tool.
    #[serde(default)]
    pub search_results: Option<Vec<ExpectedSearchResult>>,

    /// Expected call graph edges for build_graph tool.
    /// Format: ["caller_symbol→callee_symbol", ...]
    #[serde(default)]
    pub edges: Option<Vec<ExpectedEdge>>,

    /// Expected entry points for get_entry_points tool.
    #[serde(default)]
    pub entry_points: Option<Vec<ExpectedSymbol>>,

    /// Expected leaf functions for get_leaf_functions tool.
    #[serde(default)]
    pub leaf_functions: Option<Vec<ExpectedSymbol>>,

    /// Expected cycles for check_architecture tool.
    #[serde(default)]
    pub cycles: Option<Vec<ExpectedCycle>>,

    /// Expected paths for trace_path tool.
    #[serde(default)]
    pub paths: Option<Vec<ExpectedPath>>,

    /// Expected hot functions for get_hot_paths tool.
    #[serde(default)]
    pub hot_functions: Option<Vec<ExpectedHotFunction>>,

    /// Expected impacted files for analyze_impact tool.
    #[serde(default)]
    pub impacted_files: Option<Vec<String>>,

    /// Expected indexed symbols for build_lightweight_index tool.
    /// Used to measure index completeness.
    #[serde(default)]
    pub indexed_symbols: Option<Vec<ExpectedSymbol>>,

    /// Expected query results for query_symbol_index tool.
    /// The expected locations where a symbol should be found.
    #[serde(default)]
    pub query_results: Option<Vec<ExpectedQueryResult>>,

    /// Expected per-file edges for get_per_file_graph tool.
    /// Keyed by file path.
    #[serde(default)]
    pub per_file_edges: Option<Vec<PerFileEdges>>,

    /// Expected merged edges for merge_file_graphs tool.
    #[serde(default)]
    pub merged_edges: Option<Vec<ExpectedEdge>>,

    /// Expected pre-refactoring code for safe_refactor behavioral preservation.
    #[serde(default)]
    pub pre_code: Option<String>,

    /// Expected post-refactoring code for safe_refactor behavioral preservation.
    #[serde(default)]
    pub post_code: Option<String>,

    /// Build time target in milliseconds for indexing tools.
    #[serde(default)]
    pub build_time_target_ms: Option<u64>,

    /// Query latency target in milliseconds for indexing tools.
    #[serde(default)]
    pub query_latency_target_ms: Option<u64>,

    /// Tolerance for floating-point comparisons (percentage as decimal).
    /// For example, 0.05 means 5% tolerance.
    #[serde(default)]
    pub tolerance_pct: Option<f64>,

    /// Minimum expected node count for export_mermaid output.
    #[serde(default)]
    pub min_node_count: Option<u32>,

    /// Minimum expected edge count for export_mermaid output.
    #[serde(default)]
    pub min_edge_count: Option<u32>,

    /// Expected patterns that mermaid_code should contain.
    #[serde(default)]
    pub mermaid_contains: Option<Vec<String>>,

    /// Expected matches for search_content tool.
    #[serde(default)]
    pub matches: Option<Vec<ExpectedMatch>>,
}

/// Expected call graph edge (caller → callee).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedEdge {
    /// The caller symbol name
    pub from: String,
    /// The callee symbol name
    pub to: String,
}

/// Expected cycle in the call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedCycle {
    /// Symbols involved in the cycle (in order)
    pub symbols: Vec<String>,
}

/// Expected path between two symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedPath {
    /// Source symbol name
    pub source: String,
    /// Target symbol name
    pub target: String,
    /// Expected intermediate symbols in the path (excluding source and target)
    pub intermediates: Vec<String>,
}

/// Expected hot function (high fan-in).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedHotFunction {
    /// Symbol name
    pub name: String,
    /// Minimum expected fan-in
    pub min_fan_in: u32,
}

/// Expected code content for get_symbol_code tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedCode {
    pub file: String,
    pub line: u32,
    pub col: u32,
    /// The expected source code content (may include docstrings).
    pub content: String,
}

/// Expected complexity metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedComplexity {
    pub cyclomatic: Option<u32>,
    pub cognitive: Option<u32>,
    pub nesting: Option<u32>,
}

/// Expected symbol usage location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedUsage {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub context: Option<String>,
}

/// Expected search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedSearchResult {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub relevance_score: Option<f64>,
}

/// Expected match for search_content tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedMatch {
    #[serde(alias = "path")]
    pub file: String,
    pub line: Option<u32>,
    pub context: Option<String>,
}

/// Expected query result for query_symbol_index tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedQueryResult {
    pub symbol_name: String,
    pub locations: Vec<SymbolLocation>,
}

/// Per-file edges for get_per_file_graph tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerFileEdges {
    pub file: String,
    pub edges: Vec<ExpectedEdge>,
}

/// Result of comparing returned symbols against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched_symbols: Vec<MatchedSymbol>,
    pub missing_symbols: Vec<ExpectedSymbol>,
    pub extra_symbols: Vec<ReturnedSymbol>,
}

/// A symbol returned by the tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnedSymbol {
    pub name: String,
    pub kind: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

/// A matched symbol pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedSymbol {
    pub expected: ExpectedSymbol,
    pub returned: ReturnedSymbol,
    pub name_match: bool,
    pub kind_match: bool,
}

/// Result of comparing returned outline against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineMatchResult {
    pub total_expected: u32,
    pub total_returned: u32,
    pub matched: u32,
    pub missing: u32,
    pub extra: u32,
    pub structure_score: f64,
    pub details: Vec<OutlineMismatch>,
}

/// A mismatch in the outline structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineMismatch {
    pub path: String,
    pub expected_kind: Option<SymbolKind>,
    pub returned_kind: Option<String>,
    pub mismatch_type: OutlineMismatchType,
}

/// Type of outline mismatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineMismatchType {
    Missing,
    Extra,
    KindMismatch,
    ChildCountMismatch,
}

/// Result of comparing returned code against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMatchResult {
    pub exact_match: bool,
    pub content_similarity: f64,
    pub returned_content: Option<String>,
    pub expected_content: Option<String>,
    pub has_docstring: bool,
}

/// Result of comparing complexity values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMatchResult {
    pub cyclomatic_match: bool,
    pub cognitive_match: bool,
    pub nesting_match: bool,
    pub all_match: bool,
    pub details: ComplexityDetails,
}

/// Details of complexity comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityDetails {
    pub expected_cyclomatic: Option<u32>,
    pub returned_cyclomatic: Option<u32>,
    pub expected_cognitive: Option<u32>,
    pub returned_cognitive: Option<u32>,
    pub expected_nesting: Option<u32>,
    pub returned_nesting: Option<u32>,
}

/// Result of comparing call graph edges against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched_edges: Vec<MatchedEdge>,
    pub missing_edges: Vec<ExpectedEdge>,
    pub extra_edges: Vec<ReturnedEdge>,
}

/// A returned edge from the call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnedEdge {
    pub from: String,
    pub to: String,
}

/// A matched edge pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedEdge {
    pub expected: ExpectedEdge,
    pub returned: ReturnedEdge,
}

/// Result of comparing entry points against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPointMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched: Vec<ExpectedSymbol>,
    pub missing: Vec<ExpectedSymbol>,
    pub extra: Vec<ReturnedSymbol>,
}

/// Result of comparing leaf functions against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafFunctionMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched: Vec<ExpectedSymbol>,
    pub missing: Vec<ExpectedSymbol>,
    pub extra: Vec<ReturnedSymbol>,
}

/// Result of comparing cycle detection against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleMatchResult {
    pub cycles_found: u32,
    pub cycles_expected: u32,
    pub symbols_in_cycles_found: u32,
    pub symbols_in_cycles_expected: u32,
    pub accuracy: f64,
}

/// Result of comparing path finding against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathMatchResult {
    pub path_found: bool,
    pub expected_length: usize,
    pub actual_length: usize,
    pub path_correct: bool,
}

/// Result of comparing hot paths against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPathMatchResult {
    pub functions_found: u32,
    pub functions_expected: u32,
    pub recall: f64,
    pub details: Vec<HotPathDetail>,
}

/// Detail of a hot path comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPathDetail {
    pub name: String,
    pub expected_fan_in: u32,
    pub actual_fan_in: Option<u32>,
    pub is_match: bool,
}

/// Result of index completeness scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCompletenessResult {
    pub expected_count: usize,
    pub actual_count: usize,
    pub completeness_score: f64,
    pub missing_symbols: Vec<String>,
}

/// Result of query accuracy scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAccuracyResult {
    pub expected_locations: usize,
    pub found_locations: usize,
    pub accuracy_score: f64,
    pub missing_locations: Vec<SymbolLocation>,
}

/// Result of per-file edge matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerFileEdgeMatchResult {
    pub file: String,
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub f1_score: f64,
}

/// Result of merged edge matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeAccuracyResult {
    pub total_expected: usize,
    pub total_found: usize,
    pub accuracy_score: f64,
    pub missing_edges: Vec<ExpectedEdge>,
    pub extra_edges: Vec<ReturnedEdge>,
}

/// Result of behavioral preservation scoring for refactoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralPreservationResult {
    pub pre_code_match: bool,
    pub post_code_similarity: f64,
    pub behavioral_preserved: bool,
    pub details: String,
}

// ============================================================================
// Usage and Search Result Matching Types (find_usages, semantic_search)
// ============================================================================

/// A returned usage from FindUsagesOutput.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnedUsage {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub context: String,
    pub is_definition: bool,
}

/// A matched usage pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedUsage {
    pub expected: ExpectedUsage,
    pub returned: ReturnedUsage,
    pub file_match: bool,
    pub line_match: bool,
    pub col_within_tolerance: bool,
}

/// Result of comparing returned usages against ground truth usages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched_usages: Vec<MatchedUsage>,
    pub missing_usages: Vec<ExpectedUsage>,
    pub extra_usages: Vec<ReturnedUsage>,
}

/// A returned search result from SemanticSearchOutput.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnedSearchResult {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
}

/// A matched search result pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedSearchResult {
    pub expected: ExpectedSearchResult,
    pub returned: ReturnedSearchResult,
    pub name_match: bool,
    pub kind_match: bool,
}

/// Result of comparing returned search results against ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultMatchResult {
    pub true_positives: u32,
    pub false_positives: u32,
    pub false_negatives: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub matched_results: Vec<MatchedSearchResult>,
    pub missing_results: Vec<ExpectedSearchResult>,
    pub extra_results: Vec<ReturnedSearchResult>,
}

/// Normalize a file path to just the basename for comparison.
fn basename_normalize(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Parse returned usages from FindUsagesOutput JSON.
pub fn parse_returned_usages(response: &Value) -> Vec<ReturnedUsage> {
    let unwrapped = unwrap_response(response);
    let usages_array = unwrapped
        .get("usages")
        .and_then(|v| v.as_array())
        .or_else(|| {
            unwrapped
                .get("result")
                .and_then(|r| r.get("usages"))
                .and_then(|v| v.as_array())
        });

    usages_array
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    Some(ReturnedUsage {
                        file: item.get("file")?.as_str()?.to_string(),
                        line: item.get("line")?.as_u64().map(|v| v as u32)?,
                        col: item
                            .get("column")
                            .or_else(|| item.get("col"))?
                            .as_u64()
                            .map(|v| v as u32)?,
                        context: item
                            .get("context")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        is_definition: item
                            .get("is_definition")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Match returned usages against expected usages with position-based F1.
/// Uses basename for file matching and ±1 column tolerance.
pub fn match_usages(returned: &[ReturnedUsage], expected: &[ExpectedUsage]) -> UsageMatchResult {
    let mut true_positives = 0u32;
    let mut matched_usages = Vec::new();
    let mut missing_usages = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    for exp in expected {
        let exp_basename = basename_normalize(&exp.file);
        let exp_line = exp.line;
        let exp_col = exp.col;

        let mut best_match_idx = None;
        let mut best_match_quality = -1i32; // -1 = no match, 0 = basename+line, 1 = +col tolerance

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }

            let ret_basename = basename_normalize(&ret.file);

            if ret_basename == exp_basename && ret.line == exp_line {
                let col_diff = (ret.col as i32 - exp_col as i32).abs();
                let within_tolerance = col_diff <= 1;

                let quality = if within_tolerance { 1 } else { 0 };

                if quality > best_match_quality {
                    best_match_quality = quality;
                    best_match_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = best_match_idx {
            let ret = &returned[idx];
            used_returned[idx] = true;
            true_positives += 1;

            let col_diff = (ret.col as i32 - exp_col as i32).abs();

            matched_usages.push(MatchedUsage {
                expected: exp.clone(),
                returned: ret.clone(),
                file_match: basename_normalize(&ret.file) == exp_basename,
                line_match: ret.line == exp_line,
                col_within_tolerance: col_diff <= 1,
            });
        } else {
            missing_usages.push(exp.clone());
        }
    }

    let false_positives = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .count() as u32;

    let false_negatives = missing_usages.len() as u32;

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let f1_score = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let extra_usages: Vec<ReturnedUsage> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();

    UsageMatchResult {
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1_score,
        matched_usages,
        missing_usages,
        extra_usages,
    }
}

/// Parse returned search results from SemanticSearchOutput JSON.
pub fn parse_returned_search_results(response: &Value) -> Vec<ReturnedSearchResult> {
    let unwrapped = unwrap_response(response);
    let results_array = unwrapped
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| {
            unwrapped
                .get("result")
                .and_then(|r| r.get("results"))
                .and_then(|v| v.as_array())
        });

    results_array
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    Some(ReturnedSearchResult {
                        name: item.get("name")?.as_str()?.to_string(),
                        kind: item
                            .get("kind")
                            .and_then(|k| k.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        file: item.get("file")?.as_str()?.to_string(),
                        line: item.get("line")?.as_u64().map(|v| v as u32)?,
                        col: item
                            .get("column")
                            .or_else(|| item.get("col"))?
                            .as_u64()
                            .map(|v| v as u32)?,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Match returned search results against expected search results.
/// Uses name+kind as primary key (no positional matching).
pub fn match_search_results(
    returned: &[ReturnedSearchResult],
    expected: &[ExpectedSearchResult],
) -> SearchResultMatchResult {
    let mut true_positives = 0u32;
    let mut matched_results = Vec::new();
    let mut missing_results = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    for exp_result in expected {
        let exp_name_lower = exp_result.name.to_lowercase();
        let exp_kind_str = exp_result.kind.as_str();
        let exp_basename = basename_normalize(&exp_result.file);

        let mut best_match_idx = None;

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }

            let name_match = ret.name.to_lowercase() == exp_name_lower;
            let kind_match = ret.kind.to_lowercase() == exp_kind_str.to_lowercase();
            let file_match = basename_normalize(&ret.file) == exp_basename;

            // Primary key: name + kind (file is secondary validation)
            if name_match && kind_match {
                best_match_idx = Some(idx);
                // Prefer matches with same file
                if file_match {
                    break; // Exact match - take immediately
                }
            }
        }

        if let Some(idx) = best_match_idx {
            let ret = &returned[idx];
            used_returned[idx] = true;
            true_positives += 1;

            matched_results.push(MatchedSearchResult {
                expected: exp_result.clone(),
                returned: ret.clone(),
                name_match: ret.name.to_lowercase() == exp_name_lower,
                kind_match: ret.kind.to_lowercase() == exp_kind_str.to_lowercase(),
            });
        } else {
            missing_results.push(exp_result.clone());
        }
    }

    let false_positives = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .count() as u32;

    let false_negatives = missing_results.len() as u32;

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    // When expected is empty, we already computed precision above:
    // - If returned is also empty: precision=1.0 (correct - nothing expected, nothing found)
    // - If returned has results: precision depends on true positives
    // In both cases, if expected is empty, recall should be 1.0 (nothing to "miss")
    // and the f1_score should reflect precision (since recall is not informative)
    let (recall, f1_score) = if expected.is_empty() {
        // When nothing was expected:
        // - If nothing returned: perfect match (precision=1.0, recall=1.0, f1=1.0)
        // - If something returned: the tool found results. This is correct behavior
        //   when ground_truth is empty (meaning "don't verify specific results").
        //   We give full credit: recall=1.0, f1=1.0
        (1.0, 1.0)
    } else {
        let recall = true_positives as f64 / expected.len() as f64;
        let f1_score = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };
        (recall, f1_score)
    };

    let extra_results: Vec<ReturnedSearchResult> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();

    SearchResultMatchResult {
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1_score,
        matched_results,
        missing_results,
        extra_results,
    }
}

/// Match returned symbols against ground truth symbols.
/// Returns precision, recall, and F1 score.
pub fn match_symbols(
    returned: &[ReturnedSymbol],
    expected: &[ExpectedSymbol],
    recall_only: bool,
) -> SymbolMatchResult {
    let mut true_positives = 0u32;
    let mut matched_symbols = Vec::new();
    let mut missing_symbols = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    // Sort expected by name for consistent matching
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_by(|a, b| a.name.cmp(&b.name));

    // Track kind mismatches separately (name matches but kind doesn't)
    let mut kind_mismatches: Vec<ReturnedSymbol> = Vec::new();

    // Find matches
    for exp in &expected_sorted {
        let mut best_match_idx = None;
        let mut best_match = false;

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }

            if ret.name == exp.name {
                // Name matches
                if ret.kind.as_str() == exp.kind.as_str() {
                    // Exact match - this is our best match
                    best_match_idx = Some(idx);
                    best_match = true;
                    break;
                } else {
                    // Name matches but kind doesn't - potential fallback
                    if best_match_idx.is_none() {
                        best_match_idx = Some(idx);
                        best_match = false; // Kind mismatch
                    }
                }
            }
        }

        if let Some(idx) = best_match_idx {
            let ret = &returned[idx];
            used_returned[idx] = true;

            if best_match {
                // Exact match (name + kind)
                true_positives += 1;
                matched_symbols.push(MatchedSymbol {
                    expected: exp.clone(),
                    returned: ret.clone(),
                    name_match: true,
                    kind_match: true,
                });
            } else {
                // Kind mismatch - name matched but kind didn't
                kind_mismatches.push(ret.clone());
            }
        } else {
            // No symbol with this name found at all
            missing_symbols.push(exp.clone());
        }
    }

    // False positives: unused returned symbols + kind mismatches
    // (kind mismatches are technically wrong matches)
    let unused_count = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .count() as u32;
    let false_positives = unused_count + kind_mismatches.len() as u32;

    let false_negatives = missing_symbols.len() as u32;

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let f1_score = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let mut extra_symbols: Vec<ReturnedSymbol> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();
    // Add kind mismatches as extras
    extra_symbols.extend(kind_mismatches);

    SymbolMatchResult {
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1_score,
        matched_symbols,
        missing_symbols,
        extra_symbols,
    }
}

/// Match returned outline against ground truth outline.
/// Uses hierarchical comparison to verify structure.
pub fn match_outline(returned: &Value, expected: &[ExpectedOutlineNode]) -> OutlineMatchResult {
    // Parse returned outline if it's JSON
    let returned_nodes = parse_outline_nodes(returned);

    let total_expected = count_nodes(expected);
    let total_returned = returned_nodes.len() as u32;

    let mut mismatches = Vec::new();
    let mut matched = 0u32;

    // Recursively compare
    compare_outline_nodes(expected, &returned_nodes, "", &mut matched, &mut mismatches);

    let missing = total_expected.saturating_sub(matched);
    let extra = total_returned.saturating_sub(matched);

    let structure_score = if total_expected == 0 && total_returned == 0 {
        1.0
    } else if total_expected == 0 {
        0.0
    } else {
        matched as f64 / total_expected as f64
    };

    OutlineMatchResult {
        total_expected,
        total_returned,
        matched,
        missing,
        extra,
        structure_score,
        details: mismatches,
    }
}

/// Parse outline nodes from returned JSON value.
fn parse_outline_nodes(value: &Value) -> Vec<OutlineNodeParsed> {
    // The MCP response may nest nodes under a "nodes" key
    let nodes_value = value.get("nodes").unwrap_or(value);

    if let Some(arr) = nodes_value.as_array() {
        arr.iter()
            .filter_map(|v| parse_single_outline_node(v))
            .collect()
    } else {
        Vec::new()
    }
}

/// Parse a single outline node from JSON.
fn parse_single_outline_node(value: &Value) -> Option<OutlineNodeParsed> {
    let name = value.get("name")?.as_str()?.to_string();
    let kind = value
        .get("kind")
        .and_then(|v| v.as_str())
        .map(SymbolKind::from_str)
        .unwrap_or(SymbolKind::Other("unknown".to_string()));

    let children = value
        .get("children")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_single_outline_node(v))
                .collect()
        })
        .unwrap_or_default();

    Some(OutlineNodeParsed {
        name,
        kind,
        children,
    })
}

/// A parsed outline node from returned JSON.
#[derive(Debug, Clone)]
struct OutlineNodeParsed {
    name: String,
    kind: SymbolKind,
    children: Vec<OutlineNodeParsed>,
}

/// Count total nodes in expected outline tree.
fn count_nodes(nodes: &[ExpectedOutlineNode]) -> u32 {
    nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
}

/// Recursively compare outline nodes.
fn compare_outline_nodes(
    expected: &[ExpectedOutlineNode],
    returned: &[OutlineNodeParsed],
    path: &str,
    matched: &mut u32,
    mismatches: &mut Vec<OutlineMismatch>,
) {
    // Match expected nodes to returned nodes by name
    let mut used_returned = vec![false; returned.len()];

    for exp_node in expected {
        let current_path = if path.is_empty() {
            exp_node.name.clone()
        } else {
            format!("{}/{}", path, exp_node.name)
        };

        // Find best matching returned node
        let mut best_idx = None;
        let mut best_score = 0i32;

        for (idx, ret_node) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }

            let score = if ret_node.name == exp_node.name { 2 } else { 0 };

            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            let ret_node = &returned[idx];
            used_returned[idx] = true;

            // Check kind
            if ret_node.kind != exp_node.kind {
                mismatches.push(OutlineMismatch {
                    path: current_path.clone(),
                    expected_kind: Some(exp_node.kind.clone()),
                    returned_kind: Some(ret_node.kind.as_str().to_string()),
                    mismatch_type: OutlineMismatchType::KindMismatch,
                });
            } else {
                *matched += 1;
            }

            // Recursively compare children
            compare_outline_nodes(
                &exp_node.children,
                &ret_node.children,
                &current_path,
                matched,
                mismatches,
            );
        } else {
            mismatches.push(OutlineMismatch {
                path: current_path,
                expected_kind: Some(exp_node.kind.clone()),
                returned_kind: None,
                mismatch_type: OutlineMismatchType::Missing,
            });
        }
    }
}

/// Match returned code against expected code.
pub fn match_code(returned: &Value, expected: &ExpectedCode) -> CodeMatchResult {
    let returned_content = returned
        .get("code")
        .or_else(|| returned.get("content"))
        .or_else(|| returned.get("text"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let has_docstring = returned_content
        .as_ref()
        .map(|c| c.starts_with("///") || c.starts_with("/*") || c.starts_with("/**"))
        .unwrap_or(false);

    let exact_match = returned_content
        .as_ref()
        .map(|c| c.trim() == expected.content.trim())
        .unwrap_or(false);

    let content_similarity =
        calculate_similarity(returned_content.as_deref().unwrap_or(""), &expected.content);

    CodeMatchResult {
        exact_match,
        content_similarity,
        returned_content,
        expected_content: Some(expected.content.clone()),
        has_docstring,
    }
}

/// Calculate similarity between two strings (simple Jaccard-like similarity).
fn calculate_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_words: std::collections::HashSet<_> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<_> = b.split_whitespace().collect();

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Match complexity values against expected with tolerance.
pub fn match_complexity(
    returned: &Value,
    expected: &ExpectedComplexity,
    tolerance_pct: Option<f64>,
) -> ComplexityMatchResult {
    let tolerance = tolerance_pct.unwrap_or(0.05); // Default 5% tolerance

    // The MCP response may nest complexity under a "complexity" key
    let complexity_data = returned.get("complexity").unwrap_or(returned);

    let returned_cyclomatic = complexity_data
        .get("cyclomatic")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let returned_cognitive = complexity_data
        .get("cognitive")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let returned_nesting = complexity_data
        .get("nesting_depth")
        .or_else(|| complexity_data.get("nesting"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let cyclomatic_match =
        check_within_tolerance(expected.cyclomatic, returned_cyclomatic, tolerance);
    let cognitive_match = check_within_tolerance(expected.cognitive, returned_cognitive, tolerance);
    let nesting_match = check_within_tolerance(expected.nesting, returned_nesting, tolerance);

    // all_match: all expected values matched OR both absent
    // If both are Some, values must match; if both are None, that's a pass
    let all_match =
        ((expected.cyclomatic.is_some() && returned_cyclomatic.is_some() && cyclomatic_match)
            || (expected.cyclomatic.is_none() && returned_cyclomatic.is_none()))
            && ((expected.cognitive.is_some() && returned_cognitive.is_some() && cognitive_match)
                || (expected.cognitive.is_none() && returned_cognitive.is_none()))
            && ((expected.nesting.is_some() && returned_nesting.is_some() && nesting_match)
                || (expected.nesting.is_none() && returned_nesting.is_none()));

    ComplexityMatchResult {
        cyclomatic_match,
        cognitive_match,
        nesting_match,
        all_match,
        details: ComplexityDetails {
            expected_cyclomatic: expected.cyclomatic,
            returned_cyclomatic,
            expected_cognitive: expected.cognitive,
            returned_cognitive,
            expected_nesting: expected.nesting,
            returned_nesting,
        },
    }
}

/// Check if two values are within tolerance percentage.
fn check_within_tolerance(expected: Option<u32>, returned: Option<u32>, tolerance: f64) -> bool {
    match (expected, returned) {
        (Some(e), Some(r)) => {
            if e == 0 {
                r == 0
            } else {
                let diff = (e as f64 - r as f64).abs();
                let tolerance_value = e as f64 * tolerance;
                diff <= tolerance_value
            }
        }
        (None, None) => true,
        _ => false,
    }
}

// ============================================================================
// Call Graph Matching Functions
// ============================================================================

/// Parse edges from a build_graph response.
pub fn parse_returned_edges(response: &Value) -> Vec<ReturnedEdge> {
    // Try to extract edges from various response formats
    // build_graph returns: { success, symbols_found, relationships_found, message }
    // MCP wraps this in: { content: [{ text: "<JSON string>" }] }
    // We need to parse relationships from the message or a dedicated field

    // First, unwrap MCP content format if present
    let unwrapped = unwrap_response(response);

    let mut edges = Vec::new();

    // Try top-level "edges" field
    if let Some(edges_arr) = unwrapped.get("edges").and_then(|v| v.as_array()) {
        for edge_val in edges_arr {
            if let (Some(from), Some(to)) = (
                edge_val.get("from").and_then(|v| v.as_str()),
                edge_val.get("to").and_then(|v| v.as_str()),
            ) {
                edges.push(ReturnedEdge {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        }
        return edges;
    }

    // Try nested "relationships" field
    if let Some(rels) = unwrapped.get("relationships").and_then(|v| v.as_array()) {
        for rel in rels {
            if let (Some(from), Some(to)) = (
                rel.get("from").and_then(|v| v.as_str()),
                rel.get("to").and_then(|v| v.as_str()),
            ) {
                edges.push(ReturnedEdge {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        }
        return edges;
    }

    // Try "result" wrapper
    if let Some(result) = unwrapped.get("result") {
        if let Some(edges_arr) = result.get("edges").and_then(|v| v.as_array()) {
            for edge_val in edges_arr {
                if let (Some(from), Some(to)) = (
                    edge_val.get("from").and_then(|v| v.as_str()),
                    edge_val.get("to").and_then(|v| v.as_str()),
                ) {
                    edges.push(ReturnedEdge {
                        from: from.to_string(),
                        to: to.to_string(),
                    });
                }
            }
            return edges;
        }
        // Also try relationships inside result
        if let Some(rels) = result.get("relationships").and_then(|v| v.as_array()) {
            for rel in rels {
                if let (Some(from), Some(to)) = (
                    rel.get("from").and_then(|v| v.as_str()),
                    rel.get("to").and_then(|v| v.as_str()),
                ) {
                    edges.push(ReturnedEdge {
                        from: from.to_string(),
                        to: to.to_string(),
                    });
                }
            }
            return edges;
        }
    }

    edges
}

/// Unwrap MCP content format: {"content": [{"text": "<JSON string>"}]}
/// Returns the inner JSON value, or the original if not MCP format.
pub fn unwrap_response(response: &Value) -> Value {
    // Try content array first
    if let Some(content_arr) = response.get("content").and_then(|v| v.as_array()) {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    return parsed;
                }
            }
        }
    }
    // Try result.content
    if let Some(result) = response.get("result") {
        if let Some(content_arr) = result.get("content").and_then(|v| v.as_array()) {
            for item in content_arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        return parsed;
                    }
                }
            }
        }
        // result itself might be the direct response
        return result.clone();
    }
    response.clone()
}

/// Parse entry points from get_entry_points response.
pub fn parse_returned_entry_points(response: &Value) -> Vec<ReturnedSymbol> {
    let mut symbols = Vec::new();
    let unwrapped = unwrap_response(response);

    // Try "entry_points" field
    if let Some(arr) = unwrapped.get("entry_points").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string();
                symbols.push(ReturnedSymbol {
                    name: name.to_string(),
                    kind,
                    file: item
                        .get("location")
                        .and_then(|l| l.get("file"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    line: item
                        .get("location")
                        .and_then(|l| l.get("line"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    col: item
                        .get("location")
                        .and_then(|l| l.get("column"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                });
            }
        }
        return symbols;
    }

    // Try nested result
    if let Some(arr) = unwrapped
        .get("result")
        .and_then(|r| r.get("entry_points").and_then(|v| v.as_array()))
    {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string();
                symbols.push(ReturnedSymbol {
                    name: name.to_string(),
                    kind,
                    file: item
                        .get("location")
                        .and_then(|l| l.get("file"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    line: item
                        .get("location")
                        .and_then(|l| l.get("line"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    col: item
                        .get("location")
                        .and_then(|l| l.get("column"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                });
            }
        }
    }

    symbols
}

/// Parse leaf functions from get_leaf_functions response.
pub fn parse_returned_leaf_functions(response: &Value) -> Vec<ReturnedSymbol> {
    let mut symbols = Vec::new();
    let unwrapped = unwrap_response(response);

    // Try "leaf_functions" field
    if let Some(arr) = unwrapped.get("leaf_functions").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string();
                symbols.push(ReturnedSymbol {
                    name: name.to_string(),
                    kind,
                    file: item
                        .get("location")
                        .and_then(|l| l.get("file"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    line: item
                        .get("location")
                        .and_then(|l| l.get("line"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    col: item
                        .get("location")
                        .and_then(|l| l.get("column"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                });
            }
        }
        return symbols;
    }

    // Try nested result
    if let Some(arr) = unwrapped
        .get("result")
        .and_then(|r| r.get("leaf_functions").and_then(|v| v.as_array()))
    {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string();
                symbols.push(ReturnedSymbol {
                    name: name.to_string(),
                    kind,
                    file: item
                        .get("location")
                        .and_then(|l| l.get("file"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    line: item
                        .get("location")
                        .and_then(|l| l.get("line"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    col: item
                        .get("location")
                        .and_then(|l| l.get("column"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                });
            }
        }
    }

    symbols
}

/// Parse hot paths from get_hot_paths response.
pub fn parse_returned_hot_paths(response: &Value) -> Vec<(String, u32)> {
    let mut hot_paths = Vec::new();
    let unwrapped = unwrap_response(response);

    // Try "hot_paths" field
    if let Some(arr) = unwrapped.get("hot_paths").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(name) = item.get("symbol").and_then(|v| v.as_str()) {
                let fan_in = item
                    .get("fan_in")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(0);
                hot_paths.push((name.to_string(), fan_in));
            }
        }
        return hot_paths;
    }

    // Try nested result
    if let Some(arr) = unwrapped
        .get("result")
        .and_then(|r| r.get("hot_paths").and_then(|v| v.as_array()))
    {
        for item in arr {
            if let Some(name) = item.get("symbol").and_then(|v| v.as_str()) {
                let fan_in = item
                    .get("fan_in")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(0);
                hot_paths.push((name.to_string(), fan_in));
            }
        }
    }

    hot_paths
}

/// Match call graph edges against ground truth.
pub fn match_edges(returned: &[ReturnedEdge], expected: &[ExpectedEdge]) -> EdgeMatchResult {
    let mut true_positives = 0u32;
    let mut matched_edges = Vec::new();
    let mut missing_edges = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    // Normalize edge strings for comparison
    fn normalize_edge(from: &str, to: &str) -> (String, String) {
        (from.to_lowercase(), to.to_lowercase())
    }

    // Find matches
    for exp in expected {
        let exp_normalized = normalize_edge(&exp.from, &exp.to);
        let mut best_match_idx = None;

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }
            let ret_normalized = normalize_edge(&ret.from, &ret.to);
            if ret_normalized == exp_normalized {
                best_match_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = best_match_idx {
            let ret = &returned[idx];
            used_returned[idx] = true;
            true_positives += 1;
            matched_edges.push(MatchedEdge {
                expected: exp.clone(),
                returned: ret.clone(),
            });
        } else {
            missing_edges.push(exp.clone());
        }
    }

    let false_positives = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .count() as u32;

    let false_negatives = missing_edges.len() as u32;

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let f1_score = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let extra_edges: Vec<ReturnedEdge> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();

    EdgeMatchResult {
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1_score,
        matched_edges,
        missing_edges,
        extra_edges,
    }
}

/// Match entry points against ground truth using symbol name matching.
pub fn match_entry_points(
    returned: &[ReturnedSymbol],
    expected: &[ExpectedSymbol],
) -> EntryPointMatchResult {
    let mut true_positives = 0u32;
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    for exp in expected {
        let exp_name_lower = exp.name.to_lowercase();
        let mut best_match_idx = None;

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }
            if ret.name.to_lowercase() == exp_name_lower {
                best_match_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = best_match_idx {
            used_returned[idx] = true;
            true_positives += 1;
            matched.push(exp.clone());
        } else {
            missing.push(exp.clone());
        }
    }

    let extra: Vec<ReturnedSymbol> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let f1_score = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    EntryPointMatchResult {
        true_positives,
        false_positives: extra.len() as u32,
        false_negatives: missing.len() as u32,
        precision,
        recall,
        f1_score,
        matched,
        missing,
        extra,
    }
}

/// Match leaf functions against ground truth using symbol name matching.
pub fn match_leaf_functions(
    returned: &[ReturnedSymbol],
    expected: &[ExpectedSymbol],
) -> LeafFunctionMatchResult {
    let mut true_positives = 0u32;
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut used_returned = vec![false; returned.len()];

    for exp in expected {
        let exp_name_lower = exp.name.to_lowercase();
        let mut best_match_idx = None;

        for (idx, ret) in returned.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }
            if ret.name.to_lowercase() == exp_name_lower {
                best_match_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = best_match_idx {
            used_returned[idx] = true;
            true_positives += 1;
            matched.push(exp.clone());
        } else {
            missing.push(exp.clone());
        }
    }

    let extra: Vec<ReturnedSymbol> = returned
        .iter()
        .enumerate()
        .filter(|(idx, _)| !used_returned[*idx])
        .map(|(_, ret)| ret.clone())
        .collect();

    let precision = if returned.is_empty() {
        if expected.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / returned.len() as f64
    };

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let f1_score = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    LeafFunctionMatchResult {
        true_positives,
        false_positives: extra.len() as u32,
        false_negatives: missing.len() as u32,
        precision,
        recall,
        f1_score,
        matched,
        missing,
        extra,
    }
}

/// Match hot paths against ground truth.
pub fn match_hot_paths(
    returned: &[(String, u32)],
    expected: &[ExpectedHotFunction],
) -> HotPathMatchResult {
    let mut details = Vec::new();
    let mut functions_found = 0u32;

    for exp in expected {
        let exp_name_lower = exp.name.to_lowercase();
        let actual_fan_in = returned
            .iter()
            .find(|(name, _)| name.to_lowercase() == exp_name_lower)
            .map(|(_, fan_in)| *fan_in);

        let is_match = actual_fan_in
            .map(|actual| actual >= exp.min_fan_in)
            .unwrap_or(false);

        if is_match {
            functions_found += 1;
        }

        details.push(HotPathDetail {
            name: exp.name.clone(),
            expected_fan_in: exp.min_fan_in,
            actual_fan_in,
            is_match,
        });
    }

    let recall = if expected.is_empty() {
        if returned.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        functions_found as f64 / expected.len() as f64
    };

    HotPathMatchResult {
        functions_found,
        functions_expected: expected.len() as u32,
        recall,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_expected_symbols() -> Vec<ExpectedSymbol> {
        vec![
            ExpectedSymbol {
                name: "greet".to_string(),
                kind: SymbolKind::Function,
                location: Some(SymbolLocation {
                    file: "src/lib.rs".to_string(),
                    line: 2,
                    col: 0,
                }),
            },
            ExpectedSymbol {
                name: "add".to_string(),
                kind: SymbolKind::Function,
                location: Some(SymbolLocation {
                    file: "src/lib.rs".to_string(),
                    line: 7,
                    col: 0,
                }),
            },
            ExpectedSymbol {
                name: "Calculator".to_string(),
                kind: SymbolKind::Struct,
                location: Some(SymbolLocation {
                    file: "src/lib.rs".to_string(),
                    line: 15,
                    col: 0,
                }),
            },
        ]
    }

    fn make_returned_symbols() -> Vec<ReturnedSymbol> {
        vec![
            ReturnedSymbol {
                name: "greet".to_string(),
                kind: "function".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(2),
                col: Some(0),
            },
            ReturnedSymbol {
                name: "add".to_string(),
                kind: "function".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(7),
                col: Some(0),
            },
            ReturnedSymbol {
                name: "Calculator".to_string(),
                kind: "struct".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(15),
                col: Some(0),
            },
            ReturnedSymbol {
                name: "extra_func".to_string(),
                kind: "function".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(25),
                col: Some(0),
            },
        ]
    }

    #[test]
    fn test_match_symbols_perfect_match() {
        let expected = make_expected_symbols();
        let returned = make_returned_symbols();

        let result = match_symbols(&returned, &expected, false);

        // All 3 expected symbols should be found
        assert_eq!(result.true_positives, 3);
        assert_eq!(result.missing_symbols.len(), 0);
        assert_eq!(result.false_positives, 1); // extra_func
        assert!(result.precision > 0.7);
        assert_eq!(result.recall, 1.0);
        assert!(result.f1_score > 0.8);
    }

    #[test]
    fn test_match_symbols_kind_mismatch() {
        let expected = vec![ExpectedSymbol {
            name: "test_func".to_string(),
            kind: SymbolKind::Function,
            location: None,
        }];
        let returned = vec![ReturnedSymbol {
            name: "test_func".to_string(),
            kind: "struct".to_string(), // Wrong kind
            file: None,
            line: None,
            col: None,
        }];

        let result = match_symbols(&returned, &expected, false);

        // Name matches but kind doesn't - should still count as match for recall
        assert_eq!(result.missing_symbols.len(), 0);
        assert_eq!(result.false_positives, 1); // Wrong kind counts as extra
    }

    #[test]
    fn test_match_symbols_empty_inputs() {
        let result = match_symbols(&[], &[], false);
        assert_eq!(result.precision, 1.0);
        assert_eq!(result.recall, 1.0);
        assert_eq!(result.f1_score, 1.0);
    }

    #[test]
    fn test_match_symbols_all_missing() {
        let expected = make_expected_symbols();
        let returned = vec![];

        let result = match_symbols(&returned, &expected, false);

        assert_eq!(result.true_positives, 0);
        assert_eq!(result.false_positives, 0);
        assert_eq!(result.missing_symbols.len(), 3);
        assert_eq!(result.recall, 0.0);
        assert_eq!(result.f1_score, 0.0);
    }

    #[test]
    fn test_calculate_similarity() {
        let a = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let b = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let c = "fn multiply(a: i32, b: i32) -> i32 { a * b }";

        assert!((calculate_similarity(a, b) - 1.0).abs() < 0.01);
        assert!(calculate_similarity(a, c) > 0.3); // Some common words
        assert_eq!(calculate_similarity("", ""), 1.0);
        assert_eq!(calculate_similarity("foo", ""), 0.0);
    }

    #[test]
    fn test_symbol_kind_from_str() {
        assert_eq!(SymbolKind::from_str("function"), SymbolKind::Function);
        assert_eq!(SymbolKind::from_str("fn"), SymbolKind::Function);
        assert_eq!(SymbolKind::from_str("struct"), SymbolKind::Struct);
        assert_eq!(SymbolKind::from_str("method"), SymbolKind::Method);
        assert_eq!(
            SymbolKind::from_str("unknown_type"),
            SymbolKind::Other("unknown_type".to_string())
        );
    }

    #[test]
    fn test_match_complexity_within_tolerance() {
        let expected = ExpectedComplexity {
            cyclomatic: Some(5),
            cognitive: Some(10),
            nesting: Some(3),
        };
        let returned = serde_json::json!({
            "cyclomatic": 5,
            "cognitive": 10, // Exact match
            "nesting": 3,
        });

        let result = match_complexity(&returned, &expected, Some(0.05));

        assert!(result.cyclomatic_match);
        assert!(result.cognitive_match);
        assert!(result.nesting_match);
        assert!(result.all_match);
    }

    #[test]
    fn test_match_complexity_outside_tolerance() {
        let expected = ExpectedComplexity {
            cyclomatic: Some(5),
            cognitive: None,
            nesting: Some(3),
        };
        let returned = serde_json::json!({
            "cyclomatic": 10, // 100% off - outside 5% tolerance
            "cognitive": 15,
            "nesting": 3,
        });

        let result = match_complexity(&returned, &expected, Some(0.05));

        assert!(!result.cyclomatic_match);
        assert!(!result.all_match); // cyclomatic doesn't match
    }

    #[test]
    fn test_match_code_exact() {
        let expected = ExpectedCode {
            file: "src/lib.rs".to_string(),
            line: 1,
            col: 0,
            content: "/// A simple greeting function.\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}".to_string(),
        };
        let returned = serde_json::json!({
            "content": "/// A simple greeting function.\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}"
        });

        let result = match_code(&returned, &expected);

        assert!(result.exact_match);
        assert!((result.content_similarity - 1.0).abs() < 0.01);
        assert!(result.has_docstring);
    }

    #[test]
    fn test_match_code_partial() {
        let expected = ExpectedCode {
            file: "src/lib.rs".to_string(),
            line: 1,
            col: 0,
            content: "/// A simple greeting function.\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}".to_string(),
        };
        let returned = serde_json::json!({
            "content": "/// A greeting function.\npub fn greet(name: &str) -> String {\n    format!(\"Hello!\", name)\n}"
        });

        let result = match_code(&returned, &expected);

        assert!(!result.exact_match);
        assert!(result.content_similarity > 0.5);
        assert!(result.has_docstring);
    }

    #[test]
    fn test_match_outline_hierarchy() {
        let expected = vec![
            ExpectedOutlineNode {
                name: "Calculator".to_string(),
                kind: SymbolKind::Struct,
                location: None,
                children: vec![
                    ExpectedOutlineNode {
                        name: "add".to_string(),
                        kind: SymbolKind::Method,
                        location: None,
                        children: vec![],
                    },
                    ExpectedOutlineNode {
                        name: "subtract".to_string(),
                        kind: SymbolKind::Method,
                        location: None,
                        children: vec![],
                    },
                ],
            },
            ExpectedOutlineNode {
                name: "helper".to_string(),
                kind: SymbolKind::Function,
                location: None,
                children: vec![],
            },
        ];

        let returned = serde_json::json!([
            {
                "name": "Calculator",
                "kind": "struct",
                "children": [
                    {"name": "add", "kind": "method", "children": []},
                    {"name": "subtract", "kind": "method", "children": []}
                ]
            },
            {
                "name": "helper",
                "kind": "function",
                "children": []
            }
        ]);

        let result = match_outline(&returned, &expected);

        assert_eq!(result.total_expected, 4); // 2 top-level + 2 children
        assert!(result.structure_score > 0.9);
        assert!(result.details.is_empty());
    }
}
