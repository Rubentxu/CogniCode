//! Scoring Engine for MCP Tool Quality Evaluation
//!
//! Provides 5-dimension scoring (Correctitud, Latencia, Escalabilidad,
//! Consistencia, Robustez) with weighted health score calculation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ground_truth::{
    match_code, match_complexity, match_edges, match_entry_points, match_hot_paths,
    match_leaf_functions, match_outline, match_search_results, match_symbols, match_usages,
    parse_returned_edges, parse_returned_entry_points, parse_returned_hot_paths,
    parse_returned_leaf_functions, parse_returned_search_results, parse_returned_usages,
    BehavioralPreservationResult, CodeMatchResult, ComplexityMatchResult, EdgeMatchResult,
    EntryPointMatchResult,
    GroundTruth, HotPathMatchResult, IndexCompletenessResult,
    LeafFunctionMatchResult, MergeAccuracyResult, OutlineMatchResult, PerFileEdgeMatchResult,
    QueryAccuracyResult, ReturnedSymbol,
    SearchResultMatchResult, SymbolMatchResult, UsageMatchResult,
};

/// Dimension scores for a single tool evaluation (0-100 each).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DimensionScores {
    /// Correctitud: Is the response correct? (ground truth comparison)
    pub correctitud: Option<f64>,
    /// Latencia: How fast was the response?
    pub latencia: Option<f64>,
    /// Escalabilidad: How does it scale?
    pub escalabilidad: Option<f64>,
    /// Consistencia: Is it consistent across runs?
    pub consistencia: Option<f64>,
    /// Robustez: How does it handle edge cases?
    pub robustez: Option<f64>,
}

/// Metrics definition for scoring.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsDefinition {
    pub correctness: Option<CorrectnessMetrics>,
    pub latency: Option<LatencyMetrics>,
    pub scalability: Option<ScalabilityMetrics>,
    pub consistency: Option<ConsistencyMetrics>,
    pub robustness: Option<RobustnessMetrics>,
}

/// Correctness metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectnessMetrics {
    /// Type of correctness check: precision_recall, exact_match, within_tolerance, structure_match
    #[serde(rename = "type", default)]
    pub metric_type: Option<String>,
    /// Minimum acceptable score (0-100)
    pub min_score: Option<f64>,
    /// Tolerance for floating-point comparisons
    pub tolerance_pct: Option<f64>,
    /// When true, use recall instead of F1 for symbol scoring
    pub recall_only: Option<bool>,
}

/// Latency metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Target latency in milliseconds
    pub target_ms: Option<u64>,
    /// Maximum acceptable latency in milliseconds
    pub max_ms: Option<u64>,
}

/// Scalability metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalabilityMetrics {
    /// Expected scalability classification: linear, sub_linear, quadratic
    pub classification: Option<String>,
    /// Breakpoint size in KB
    pub breakpoint_kb: Option<u64>,
}

/// Consistency metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyMetrics {
    /// Expected variance threshold
    pub variance_threshold: Option<f64>,
}

/// Robustness metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobustnessMetrics {
    /// Types of edge cases to test
    pub edge_cases: Option<Vec<String>>,
}

/// Tool score result with all dimensions and health score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolScore {
    /// Tool name
    pub tool: String,
    /// Language
    pub language: String,
    /// Scenario ID
    pub scenario_id: String,
    /// Correctitud score (0-100)
    pub correctitud: f64,
    /// Latencia score (0-100)
    pub latencia: f64,
    /// Escalabilidad score (0-100)
    pub escalabilidad: f64,
    /// Consistencia score (0-100)
    pub consistencia: f64,
    /// Robustez score (0-100)
    pub robustez: f64,
    /// Weighted health score (0-100)
    pub health_score: f64,
    /// Detailed match results (if ground truth was provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_match: Option<SymbolMatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_match: Option<OutlineMatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_match: Option<CodeMatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_match: Option<ComplexityMatchResult>,
    /// Call graph edge match result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_match: Option<EdgeMatchResult>,
    /// Entry point match result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_point_match: Option<EntryPointMatchResult>,
    /// Leaf function match result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_function_match: Option<LeafFunctionMatchResult>,
    /// Hot path match result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hot_path_match: Option<HotPathMatchResult>,
    /// Index completeness result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_completeness: Option<IndexCompletenessResult>,
    /// Query accuracy result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_accuracy: Option<QueryAccuracyResult>,
    /// Per-file edge match result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_file_edge_match: Option<PerFileEdgeMatchResult>,
    /// Merge accuracy result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_accuracy: Option<MergeAccuracyResult>,
    /// Behavioral preservation result for refactoring
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavioral_preservation: Option<BehavioralPreservationResult>,
    /// Usage match result for find_usages tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_match: Option<UsageMatchResult>,
    /// Search result match for semantic_search tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_result_match: Option<SearchResultMatchResult>,
}

/// Compute correctness score using precision/recall against ground truth.
pub fn compute_correctness_score(
    tool_response: &Value,
    ground_truth: &GroundTruth,
    metrics: &Option<MetricsDefinition>,
) -> (
    f64,
    Option<SymbolMatchResult>,
    Option<OutlineMatchResult>,
    Option<CodeMatchResult>,
    Option<ComplexityMatchResult>,
) {
    let tolerance_pct = ground_truth
        .tolerance_pct
        .or_else(|| {
            metrics
                .as_ref()
                .and_then(|m| m.correctness.as_ref().and_then(|c| c.tolerance_pct))
        })
        .unwrap_or(0.05);

    let recall_only = metrics
        .as_ref()
        .and_then(|m| m.correctness.as_ref())
        .and_then(|c| c.recall_only)
        .unwrap_or(false);

    // Try symbols match
    if let Some(expected_symbols) = &ground_truth.symbols {
        let returned_symbols = parse_returned_symbols(tool_response);
        let match_result = match_symbols(&returned_symbols, expected_symbols, recall_only);
        let score = if recall_only {
            match_result.recall * 100.0
        } else {
            match_result.f1_score * 100.0
        };
        return (score, Some(match_result), None, None, None);
    }

    // Try outline match
    if let Some(expected_outline) = &ground_truth.outline {
        let unwrapped = unwrap_mcp_content(tool_response);
        let match_result = match_outline(&unwrapped, expected_outline);
        let score = match_result.structure_score * 100.0;
        return (score, None, Some(match_result), None, None);
    }

    // Try code match
    if let Some(expected_code) = &ground_truth.code {
        let unwrapped = unwrap_mcp_content(tool_response);
        let match_result = match_code(&unwrapped, expected_code);
        let score = if match_result.exact_match {
            100.0
        } else {
            match_result.content_similarity * 100.0
        };
        return (score, None, None, Some(match_result), None);
    }

    // Try complexity match
    if let Some(expected_complexity) = &ground_truth.complexity {
        let unwrapped = unwrap_mcp_content(tool_response);
        let match_result = match_complexity(&unwrapped, expected_complexity, Some(tolerance_pct));
        let score = if match_result.all_match {
            100.0
        } else {
            // Partial credit: 50 if cyclomatic or nesting matches
            if match_result.cyclomatic_match || match_result.nesting_match {
                50.0
            } else {
                0.0
            }
        };
        return (score, None, None, None, Some(match_result));
    }

    // No ground truth available - return N/A score
    (f64::NAN, None, None, None, None)
}

/// Parse returned symbols from tool response.
fn parse_returned_symbols(response: &Value) -> Vec<ReturnedSymbol> {
    // Try to find symbols array in common response formats
    // 1. Top-level "symbols" field: {"symbols": [...]}
    // 2. Nested in "result": {"result": {"symbols": [...]}}
    // 3. Nested in "content": {"content": {"symbols": [...]}}
    // 4. Top-level array (direct): [...]

    // Check for top-level array first
    if let Some(arr) = response.as_array() {
        return parse_symbols_from_array(arr);
    }

    // Try top-level "symbols" field
    if let Some(arr) = response.get("symbols").and_then(|v| v.as_array()) {
        return parse_symbols_from_array(arr);
    }

    // Try nested in "result"
    if let Some(result_obj) = response.get("result") {
        if let Some(arr) = result_obj.get("symbols").and_then(|v| v.as_array()) {
            return parse_symbols_from_array(arr);
        }
    }

    // Try nested in "content"
    if let Some(content_obj) = response.get("content") {
        if let Some(arr) = content_obj.get("symbols").and_then(|v| v.as_array()) {
            return parse_symbols_from_array(arr);
        }
    }

    // 5. Try MCP content format: {"content": [{"text": "<JSON string>"}]}
    if let Some(content_arr) = response.get("content").and_then(|v| v.as_array()) {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                // Try to parse the text as JSON
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    // Recursively parse the parsed JSON
                    let symbols = parse_returned_symbols(&parsed);
                    if !symbols.is_empty() {
                        return symbols;
                    }
                }
            }
        }
    }

    Vec::new()
}

/// Unwrap MCP content format: {"content": [{"text": "<JSON string>"}]}
/// Returns the inner JSON value, or the original if not MCP format.
fn unwrap_mcp_content(response: &Value) -> Value {
    if let Some(content_arr) = response.get("content").and_then(|v| v.as_array()) {
        for item in content_arr {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    return parsed;
                }
            }
        }
    }
    // Check for direct result nesting: {"result": {"content": [...]}}
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
    }
    response.clone()
}

/// Parse symbols from a JSON array.
fn parse_symbols_from_array(arr: &[Value]) -> Vec<ReturnedSymbol> {
    arr.iter()
        .filter_map(|item| {
            Some(ReturnedSymbol {
                name: item.get("name")?.as_str()?.to_string(),
                kind: item
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                file: item.get("file").and_then(|f| f.as_str()).map(String::from),
                line: item.get("line").and_then(|l| l.as_u64()).map(|l| l as u32),
                col: item.get("col").and_then(|c| c.as_u64()).map(|c| c as u32),
            })
        })
        .collect()
}

/// Compute call graph specific scores based on ground truth.
/// Returns (edge_match, entry_point_match, leaf_function_match, hot_path_match, score)
pub fn compute_call_graph_score(
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> (
    Option<EdgeMatchResult>,
    Option<EntryPointMatchResult>,
    Option<LeafFunctionMatchResult>,
    Option<HotPathMatchResult>,
    f64,
) {
    // Try edges match (build_graph)
    if let Some(expected_edges) = &ground_truth.edges {
        let returned_edges = parse_returned_edges(tool_response);
        let match_result = match_edges(&returned_edges, expected_edges);
        let score = match_result.f1_score * 100.0;
        return (Some(match_result), None, None, None, score);
    }

    // Try entry points match (get_entry_points)
    if let Some(expected_entry_points) = &ground_truth.entry_points {
        let returned_entry_points = parse_returned_entry_points(tool_response);
        let match_result = match_entry_points(&returned_entry_points, expected_entry_points);
        let score = match_result.f1_score * 100.0;
        return (None, Some(match_result), None, None, score);
    }

    // Try leaf functions match (get_leaf_functions)
    if let Some(expected_leaf_functions) = &ground_truth.leaf_functions {
        let returned_leaf_functions = parse_returned_leaf_functions(tool_response);
        let match_result = match_leaf_functions(&returned_leaf_functions, expected_leaf_functions);
        let score = match_result.f1_score * 100.0;
        return (None, None, Some(match_result), None, score);
    }

    // Try hot paths match (get_hot_paths)
    if let Some(expected_hot_functions) = &ground_truth.hot_functions {
        let returned_hot_paths = parse_returned_hot_paths(tool_response);
        let match_result = match_hot_paths(&returned_hot_paths, expected_hot_functions);
        let score = match_result.recall * 100.0;
        return (None, None, None, Some(match_result), score);
    }

    // No call graph ground truth available
    (None, None, None, None, f64::NAN)
}

/// Score cycles detection (check_architecture).
/// Compares detected cycles against expected cycles using set comparison.
/// Score = recall of expected cycles (how many expected cycles were detected).
pub fn score_cycles(tool_response: &Value, ground_truth: &GroundTruth) -> f64 {
    if let Some(expected_cycles) = &ground_truth.cycles {
        if expected_cycles.is_empty() {
            // No cycles expected — check if response indicates no cycles
            let text = unwrap_response_text(tool_response);
            if text.contains("no cycles")
                || text.contains("0 cycles")
                || text.contains("\"cycles\":[]")
            {
                return 100.0;
            }
            // If no cycles expected but some found, partial credit
            return 50.0;
        }

        // Extract cycle information from response
        let text = unwrap_response_text(tool_response);
        let mut matched = 0u32;
        for cycle in expected_cycles {
            // Check if all symbols in the expected cycle appear in the response
            let all_found = cycle.symbols.iter().all(|sym| text.contains(sym));
            if all_found {
                matched += 1;
            }
        }

        (matched as f64 / expected_cycles.len() as f64) * 100.0
    } else {
        f64::NAN
    }
}

/// Score path finding (trace_path).
/// Checks if the response indicates a path was found between source and target.
pub fn score_paths(tool_response: &Value, ground_truth: &GroundTruth) -> f64 {
    if let Some(expected_paths) = &ground_truth.paths {
        if expected_paths.is_empty() {
            return 100.0;
        }

        let text = unwrap_response_text(tool_response);
        let mut total_score = 0.0;

        for path in expected_paths {
            let response_has_path =
                text.contains("\"path_found\":true") || text.contains("\"path_found\": true");
            let response_no_path =
                text.contains("\"path_found\":false") || text.contains("\"path_found\": false");

            if response_has_path {
                // Path was found — check if expected intermediates match
                if path.intermediates.is_empty() {
                    // No intermediates expected, path found — this means direct call,
                    // which is the correct answer
                    total_score += 100.0;
                } else {
                    // Check if expected intermediates appear in response
                    let all_found = path.intermediates.iter().all(|sym| text.contains(sym));
                    total_score += if all_found { 100.0 } else { 50.0 };
                }
            } else if response_no_path {
                // Path not found in response
                if path.intermediates.is_empty() {
                    // No path expected, no path found — correct!
                    total_score += 100.0;
                } else {
                    // Path was expected but not found
                    if text.contains(&path.source) && text.contains(&path.target) {
                        total_score += 30.0; // Partial: symbols resolved but no path
                    }
                    // else: 0 — complete failure
                }
            } else {
                // Unknown format — give partial credit if source and target appear
                if text.contains(&path.source) && text.contains(&path.target) {
                    total_score += 50.0;
                }
            }
        }

        total_score / expected_paths.len() as f64
    } else {
        f64::NAN
    }
}

/// Score impacted files (analyze_impact).
/// Compares returned impacted files against expected impacted files using set comparison.
pub fn score_impacted_files(tool_response: &Value, ground_truth: &GroundTruth) -> f64 {
    if let Some(expected_files) = &ground_truth.impacted_files {
        if expected_files.is_empty() {
            return 100.0;
        }

        let text = unwrap_response_text(tool_response);
        let mut matched = 0u32;

        for file in expected_files {
            if text.contains(file) {
                matched += 1;
            }
        }

        (matched as f64 / expected_files.len() as f64) * 100.0
    } else {
        f64::NAN
    }
}

/// Score export_mermaid output against ground truth.
/// Checks node_count, edge_count, and optionally that mermaid_code contains expected patterns.
pub fn score_mermaid(tool_response: &Value, ground_truth: &GroundTruth) -> f64 {
    // Unwrap the MCP content to get the inner JSON
    let inner: Value =
        if let Some(content_arr) = tool_response.get("content").and_then(|v| v.as_array()) {
            content_arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .filter_map(|text| serde_json::from_str::<Value>(text).ok())
                .next()
                .unwrap_or_else(|| tool_response.clone())
        } else {
            tool_response.clone()
        };

    let unwrapped_text = inner.to_string();
    let mut score = 100.0_f64;
    let mut checks = 0u32;

    // Check min_node_count if specified
    if let Some(min_nodes) = ground_truth.min_node_count {
        checks += 1;
        let actual_nodes = inner
            .get("node_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if actual_nodes < min_nodes as u64 {
            score *= actual_nodes as f64 / min_nodes as f64;
        }
    }

    // Check min_edge_count if specified
    if let Some(min_edges) = ground_truth.min_edge_count {
        checks += 1;
        let actual_edges = inner
            .get("edge_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if actual_edges < min_edges as u64 {
            score *= actual_edges as f64 / min_edges as f64;
        }
    }

    // Check that mermaid_code contains expected patterns
    if let Some(expected_patterns) = &ground_truth.mermaid_contains {
        checks += 1;
        let mut matched = 0u32;
        for pattern in expected_patterns {
            if unwrapped_text.contains(pattern) {
                matched += 1;
            }
        }
        if !expected_patterns.is_empty() {
            score *= matched as f64 / expected_patterns.len() as f64;
        }
    }

    if checks == 0 {
        f64::NAN
    } else {
        score
    }
}

/// Score search_content output against ground truth matches.
/// Compares returned file matches against expected matches.
pub fn score_search_content(
    tool_response: &Value,
    ground_truth: &GroundTruth,
    _recall_only: bool,
) -> f64 {
    let Some(expected_matches) = &ground_truth.matches else {
        return f64::NAN;
    };

    // Unwrap MCP content to get the inner JSON
    let inner: Value =
        if let Some(content_arr) = tool_response.get("content").and_then(|v| v.as_array()) {
            content_arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .filter_map(|text| serde_json::from_str::<Value>(text).ok())
                .next()
                .unwrap_or_else(|| tool_response.clone())
        } else {
            tool_response.clone()
        };

    // Parse returned matches - look for matches array or result.matches
    let returned_matches: Vec<(String, Option<u32>)> =
        if let Some(arr) = inner.get("matches").and_then(|v| v.as_array()) {
            arr.iter()
                .filter_map(|m| {
                    let file = m
                        .get("file")
                        .or_else(|| m.get("path"))?
                        .as_str()?
                        .to_string();
                    let line = m.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);
                    Some((file, line))
                })
                .collect()
        } else if let Some(result) = inner.get("result") {
            if let Some(arr) = result.get("matches").and_then(|v| v.as_array()) {
                arr.iter()
                    .filter_map(|m| {
                        let file = m
                            .get("file")
                            .or_else(|| m.get("path"))?
                            .as_str()?
                            .to_string();
                        let line = m.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);
                        Some((file, line))
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

    if expected_matches.is_empty() {
        return if returned_matches.is_empty() {
            100.0
        } else {
            0.0
        };
    }

    let mut matched = 0u32;
    let mut used_returned = vec![false; returned_matches.len()];

    for exp in expected_matches {
        let exp_basename = std::path::Path::new(&exp.file)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&exp.file)
            .to_string();

        for (idx, (ret_file, ret_line)) in returned_matches.iter().enumerate() {
            if used_returned[idx] {
                continue;
            }
            let ret_basename = std::path::Path::new(ret_file)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(ret_file)
                .to_string();

            // When expected line is None or 0, match on file basename only
            let line_matches = match (&exp.line, ret_line) {
                (None | Some(0), _) => true,
                (Some(expected_line), Some(ret_line)) => *ret_line == *expected_line,
                (Some(_), None) => false,
            };
            if ret_basename == exp_basename && line_matches {
                matched += 1;
                used_returned[idx] = true;
                break;
            }
        }
    }

    let total_expected = expected_matches.len() as f64;
    (matched as f64 / total_expected) * 100.0
}

/// Unwrap MCP response to extract text content for string-based matching.
fn unwrap_response_text(response: &Value) -> String {
    // Try content array
    if let Some(content_arr) = response.get("content").and_then(|v| v.as_array()) {
        let texts: Vec<String> = content_arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
            .collect();
        if !texts.is_empty() {
            return texts.join(" ");
        }
    }
    // Try result.content
    if let Some(result) = response.get("result") {
        if let Some(content_arr) = result.get("content").and_then(|v| v.as_array()) {
            let texts: Vec<String> = content_arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
                .collect();
            if !texts.is_empty() {
                return texts.join(" ");
            }
        }
    }
    // Fallback: serialize the whole response
    response.to_string()
}

/// Compute score for search tools (find_usages, semantic_search).
/// Returns (usage_match, search_result_match, score)
pub fn compute_search_tools_score(
    tool_name: &str,
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> (
    Option<UsageMatchResult>,
    Option<SearchResultMatchResult>,
    f64,
) {
    match tool_name {
        "find_usages" => {
            if let Some(expected_usages) = &ground_truth.usages {
                let returned_usages = parse_returned_usages(tool_response);
                let match_result = match_usages(&returned_usages, expected_usages);
                let score = match_result.f1_score * 100.0;
                return (Some(match_result), None, score);
            }
        }
        "semantic_search" => {
            if let Some(expected_results) = &ground_truth.search_results {
                let returned_results = parse_returned_search_results(tool_response);
                let match_result = match_search_results(&returned_results, expected_results);
                let score = match_result.f1_score * 100.0;
                return (None, Some(match_result), score);
            }
            // Fallback: when search_results is None, check symbols
            if let Some(expected_symbols) = &ground_truth.symbols {
                let returned_symbols = parse_returned_symbols(tool_response);
                let match_result = match_symbols(&returned_symbols, expected_symbols, false);
                let score = match_result.f1_score * 100.0;
                return (None, None, score);
            }
        }
        _ => {}
    }
    // No ground truth available
    (None, None, f64::NAN)
}

/// Compute latency score based on target and actual latency.
pub fn compute_latency_score(actual_latency_ms: u64, metrics: &Option<MetricsDefinition>) -> f64 {
    let target_ms = metrics
        .as_ref()
        .and_then(|m| m.latency.as_ref())
        .and_then(|l| l.target_ms)
        .unwrap_or(200); // Default target: 200ms

    let max_ms = metrics
        .as_ref()
        .and_then(|m| m.latency.as_ref())
        .and_then(|l| l.max_ms)
        .unwrap_or(1000); // Default max: 1000ms

    if actual_latency_ms == 0 {
        // Sub-millisecond response — tool is faster than timer resolution.
        // Count as within target (tool is clearly responsive).
        return 100.0;
    }

    if actual_latency_ms <= target_ms {
        100.0
    } else if actual_latency_ms >= max_ms {
        0.0
    } else {
        let ratio = (max_ms - actual_latency_ms) as f64 / (max_ms - target_ms) as f64;
        ratio * 100.0
    }
}

// =========================================================================
// Execution Metadata for KPI Scoring
// =========================================================================

/// Metadata about a tool execution used for computing KPI dimension scores.
/// This allows computing real escalabilidad, robustez, and consistencia scores
/// instead of placeholder values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionMetadata {
    /// Workspace size in kilobytes (for escalabilidad scoring)
    pub workspace_size_kb: u64,
    /// Number of tool calls that returned errors (for robustez scoring)
    pub error_count: u32,
    /// Total number of tool operations (for robustez scoring)
    pub total_operations: u32,
    /// Latency measurements for consistency computation (for consistencia scoring)
    /// When empty, single-sample heuristic is used instead.
    pub latency_samples_ms: Vec<u64>,
}

impl ExecutionMetadata {
    /// Create metadata with workspace size only (no error tracking, no consistency samples).
    pub fn with_workspace_size(workspace_size_kb: u64) -> Self {
        Self {
            workspace_size_kb,
            ..Default::default()
        }
    }

    /// Create metadata with error tracking.
    pub fn with_errors(workspace_size_kb: u64, error_count: u32, total_operations: u32) -> Self {
        Self {
            workspace_size_kb,
            error_count,
            total_operations,
            ..Default::default()
        }
    }

    /// Create metadata with consistency samples.
    pub fn with_consistency_samples(workspace_size_kb: u64, latency_samples_ms: Vec<u64>) -> Self {
        Self {
            workspace_size_kb,
            latency_samples_ms,
            ..Default::default()
        }
    }
}

// =========================================================================
// Scalability Classification Types and Functions
// =========================================================================

/// Scalability classification based on how latency grows with workspace size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalabilityClass {
    /// Latency is constant regardless of size - O(1)
    Constant,
    /// Latency grows sub-linearly (logarithmic or sqrt) - O(log n) or O(sqrt n)
    SubLinear,
    /// Latency grows linearly with size - O(n)
    Linear,
    /// Latency grows quadratically - O(n^2)
    Quadratic,
    /// Latency grows exponentially - O(2^n) or worse
    Exponential,
}

impl ScalabilityClass {
    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "constant" | "o1" | "o(1)" => Some(ScalabilityClass::Constant),
            "sub_linear" | "sublinear" | "log" | "logarithmic" => Some(ScalabilityClass::SubLinear),
            "linear" | "o_n" | "o(n)" => Some(ScalabilityClass::Linear),
            "quadratic" | "o_n2" | "o(n^2)" => Some(ScalabilityClass::Quadratic),
            "exponential" | "o_2n" | "o(2^n)" => Some(ScalabilityClass::Exponential),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ScalabilityClass::Constant => "constant",
            ScalabilityClass::SubLinear => "sub_linear",
            ScalabilityClass::Linear => "linear",
            ScalabilityClass::Quadratic => "quadratic",
            ScalabilityClass::Exponential => "exponential",
        }
    }

    /// Get the base score for this classification (before R² adjustment).
    fn base_score(&self) -> f64 {
        match self {
            ScalabilityClass::Constant => 100.0,
            ScalabilityClass::SubLinear => 85.0,
            ScalabilityClass::Linear => 70.0,
            ScalabilityClass::Quadratic => 40.0,
            ScalabilityClass::Exponential => 15.0,
        }
    }
}

/// Result of scalability classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalabilityResult {
    /// The classified scalability behavior.
    pub classification: ScalabilityClass,
    /// Latency measurements at each size: (size_kb, latency_ms)
    pub latency_at_sizes: Vec<(u64, u64)>,
    /// R² goodness of fit for the best model.
    pub r_squared: f64,
    /// Final score (0-100) adjusted by fit quality.
    pub score: f64,
}

/// Classify scalability based on latency measurements at different workspace sizes.
///
/// Analyzes the growth pattern of latency as workspace size increases.
/// Returns the classification and a score based on how well the tool scales.
pub fn classify_scalability(
    measurements: &[(u64, u64)], // (workspace_size_kb, latency_ms)
) -> ScalabilityResult {
    if measurements.len() < 2 {
        // Not enough data points - assume constant
        return ScalabilityResult {
            classification: ScalabilityClass::Constant,
            latency_at_sizes: measurements.to_vec(),
            r_squared: 1.0,
            score: 100.0,
        };
    }

    // Filter out zero or negative latencies for fitting
    let valid_measurements: Vec<(f64, f64)> = measurements
        .iter()
        .filter(|(size, lat)| *size > 0 && *lat > 0)
        .map(|(size, lat)| (*size as f64, *lat as f64))
        .collect();

    if valid_measurements.len() < 2 {
        return ScalabilityResult {
            classification: ScalabilityClass::Constant,
            latency_at_sizes: measurements.to_vec(),
            r_squared: 1.0,
            score: 100.0,
        };
    }

    // First, check if latency is essentially constant (variance near zero)
    let latency_variance = compute_variance(valid_measurements.iter().map(|(_, y)| *y));
    let latency_std_dev = latency_variance.sqrt();
    let latency_mean =
        valid_measurements.iter().map(|(_, y)| *y).sum::<f64>() / valid_measurements.len() as f64;

    // If coefficient of variation (std_dev / mean) is small, it's constant
    if latency_mean > 0.0 && (latency_std_dev / latency_mean) < 0.1 {
        return ScalabilityResult {
            classification: ScalabilityClass::Constant,
            latency_at_sizes: measurements.to_vec(),
            r_squared: 1.0,
            score: 100.0,
        };
    }

    // Compute growth rates between consecutive measurements
    let mut growth_ratios: Vec<f64> = Vec::new();
    for window in valid_measurements.windows(2) {
        let (size1, lat1) = window[0];
        let (size2, lat2) = window[1];
        if lat1 > 0.0 && size2 > size1 {
            // Ratio of latency growth to size growth
            let size_ratio = size2 / size1;
            let lat_ratio = lat2 / lat1;
            growth_ratios.push(lat_ratio / size_ratio);
        }
    }

    // Analyze growth pattern
    let avg_growth_ratio = if growth_ratios.is_empty() {
        1.0
    } else {
        growth_ratios.iter().sum::<f64>() / growth_ratios.len() as f64
    };

    // Check for exponential: latency doubles while size grows linearly
    let mut is_exponential = false;
    for window in valid_measurements.windows(2) {
        let (size1, lat1) = window[0];
        let (size2, lat2) = window[1];
        if lat1 > 0.0 && size2 > size1 {
            let lat_double_ratio = lat2 / lat1;
            // If latency more than doubles for each size doubling, likely exponential
            let size_multiplier = size2 / size1;
            if size_multiplier >= 1.5 && lat_double_ratio >= size_multiplier * size_multiplier {
                is_exponential = true;
                break;
            }
        }
    }

    if is_exponential {
        return ScalabilityResult {
            classification: ScalabilityClass::Exponential,
            latency_at_sizes: measurements.to_vec(),
            r_squared: 0.5, // Poor fit for exponential
            score: 20.0,
        };
    }

    // Compute R² for different models to determine best fit
    let linear_r2 = fit_linear_r2(&valid_measurements);
    let log_r2 = fit_log_r2(&valid_measurements);

    // Determine classification based on growth pattern and fit
    let (classification, r_squared, base_score) = if avg_growth_ratio < 0.3 {
        // Very slow growth - likely sub-linear (logarithmic)
        (ScalabilityClass::SubLinear, log_r2, 85.0)
    } else if avg_growth_ratio < 1.5 {
        // Moderate growth - could be linear or sub-linear
        if log_r2 > linear_r2 {
            (ScalabilityClass::SubLinear, log_r2, 85.0)
        } else {
            (ScalabilityClass::Linear, linear_r2, 70.0)
        }
    } else if avg_growth_ratio < 3.0 {
        // Higher growth - likely linear or quadratic
        if linear_r2 > 0.9 {
            (ScalabilityClass::Linear, linear_r2, 70.0)
        } else {
            (ScalabilityClass::Quadratic, linear_r2, 40.0)
        }
    } else {
        // High growth rate - likely quadratic or worse
        (ScalabilityClass::Quadratic, linear_r2, 40.0)
    };

    // Adjust score based on R²
    let r2_adjustment = r_squared.max(0.0).min(1.0);
    let score = base_score * (0.7 + 0.3 * r2_adjustment);

    ScalabilityResult {
        classification,
        latency_at_sizes: measurements.to_vec(),
        r_squared,
        score: score.min(100.0).max(0.0),
    }
}

/// Compute variance of an iterator of f64 values.
fn compute_variance(values: impl Iterator<Item = f64>) -> f64 {
    let values: Vec<f64> = values.collect();
    let n = values.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    variance
}

/// Compute R² for a linear fit: latency = a * size + b
fn fit_linear_r2(data: &[(f64, f64)]) -> f64 {
    let n = data.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let sum_x: f64 = data.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = data.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = data.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = data.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-10 {
        return 0.0; // Singular matrix
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    // Compute R²
    let mean_y = sum_y / n;
    let mut ss_tot = 0.0;
    let mut ss_res = 0.0;

    for (x, y) in data {
        let y_pred = slope * x + intercept;
        ss_tot += (y - mean_y).powi(2);
        ss_res += (y - y_pred).powi(2);
    }

    if ss_tot.abs() < 1e-10 {
        return 1.0; // All values are the same
    }

    1.0 - ss_res / ss_tot
}

/// Compute R² for a logarithmic fit: latency = a * log(size) + b
fn fit_log_r2(data: &[(f64, f64)]) -> f64 {
    let n = data.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    // Transform to log space
    let log_data: Vec<(f64, f64)> = data
        .iter()
        .filter(|(x, _)| *x > 0.0)
        .map(|(x, y)| (x.ln(), *y))
        .collect();

    if log_data.len() < 2 {
        return 0.0;
    }

    let sum_x: f64 = log_data.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = log_data.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = log_data.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = log_data.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-10 {
        return 0.0;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    // Compute R²
    let mean_y = sum_y / n;
    let mut ss_tot = 0.0;
    let mut ss_res = 0.0;

    for (x, y) in &log_data {
        let y_pred = slope * x + intercept;
        ss_tot += (y - mean_y).powi(2);
        ss_res += (y - y_pred).powi(2);
    }

    if ss_tot.abs() < 1e-10 {
        return 1.0;
    }

    1.0 - ss_res / ss_tot
}

/// Compute scalability score from latency measurements and metrics.
///
/// This is the main entry point for computing the escalabilidad dimension score.
/// It takes the workspace size, latency, and metrics definition and returns
/// a score 0-100 for scalability.
pub fn compute_scalability_score(
    workspace_size_kb: u64,
    latency_ms: u64,
    metrics: &Option<MetricsDefinition>,
) -> f64 {
    // If we have a single measurement, use the expected classification from metrics
    if let Some(m) = metrics {
        if let Some(s) = &m.scalability {
            if let Some(class_str) = &s.classification {
                if let Some(class) = ScalabilityClass::from_str(class_str) {
                    // Single point - use expected classification
                    return class.base_score();
                }
            }
        }
    }

    // Single point without metrics - return neutral score
    if workspace_size_kb == 0 {
        return 75.0;
    }

    // Use a simple ratio-based score for single measurements
    // Lower is better: constant time should be independent of size
    let base_latency = 10.0; // Assume 10ms base overhead
    let expected_linear_ms = workspace_size_kb as f64 * 0.1; // 0.1ms per KB
    let actual_ratio = latency_ms as f64 / expected_linear_ms.max(base_latency);

    if actual_ratio <= 1.0 {
        100.0 // At or below linear expectation
    } else if actual_ratio <= 2.0 {
        85.0
    } else if actual_ratio <= 5.0 {
        70.0
    } else if actual_ratio <= 10.0 {
        50.0
    } else {
        30.0
    }
}

/// Compute scalability score from multiple measurements across different workspace sizes.
///
/// This function takes a series of (size, latency) measurements and computes
/// a comprehensive scalability score using curve fitting.
pub fn compute_scalability_score_from_measurements(
    measurements: &[(u64, u64)], // (workspace_size_kb, latency_ms)
    expected_classification: Option<&str>,
) -> ScalabilityResult {
    let result = classify_scalability(measurements);

    // If an expected classification was provided, adjust the score
    if let Some(class_str) = expected_classification {
        if let Some(expected) = ScalabilityClass::from_str(class_str) {
            if result.classification == expected {
                // Perfect match - full score
                return result;
            }

            // Score based on how far off the classification is
            let class_order = |c: &ScalabilityClass| -> i32 {
                match c {
                    ScalabilityClass::Constant => 0,
                    ScalabilityClass::SubLinear => 1,
                    ScalabilityClass::Linear => 2,
                    ScalabilityClass::Quadratic => 3,
                    ScalabilityClass::Exponential => 4,
                }
            };

            let diff = (class_order(&result.classification) - class_order(&expected)).abs();
            let penalty = match diff {
                0 => 1.0,
                1 => 0.8,
                2 => 0.5,
                _ => 0.3,
            };

            let adjusted_result = ScalabilityResult {
                classification: result.classification,
                latency_at_sizes: result.latency_at_sizes,
                r_squared: result.r_squared,
                score: result.score * penalty,
            };
            return adjusted_result;
        }
    }

    result
}

/// Compute consistency score from latency measurements.
///
/// For multiple samples: computes coefficient of variation (CV) and maps to score:
/// - CV < 10% → 95 (highly consistent)
/// - CV < 25% → 80 (consistent)
/// - CV < 50% → 60 (moderate)
/// - CV >= 50% → 30 (inconsistent)
///
/// For single samples: uses heuristic based on whether latency is reasonable
/// for the input size (workspace_size_kb). If latency is proportional to input size,
/// score is higher. Returns f64::NAN for incomplete (single sample with no size context).
pub fn compute_consistency_score(
    latency_ms: u64,
    workspace_size_kb: u64,
    latency_samples_ms: &[u64],
) -> f64 {
    // Multiple samples available: compute coefficient of variation
    if !latency_samples_ms.is_empty() && latency_samples_ms.len() >= 2 {
        return compute_consistency_from_cv(calculate_cv(latency_samples_ms));
    }

    // Single sample: check if we have workspace size context for heuristic
    if workspace_size_kb > 0 && latency_ms > 0 {
        // For single runs, score based on whether latency is reasonable for input size.
        // Use 1.0ms per KB as baseline — tools like semantic_search, find_usages,
        // and indexing need more than 0.1ms/KB due to tree-sitter parsing overhead.
        // The previous 0.1ms/KB threshold was too aggressive and penalized
        // correct-but-"slow" tools with CON=40.
        let expected_ms = workspace_size_kb as f64 * 1.0;
        let ratio = latency_ms as f64 / expected_ms;

        if ratio <= 1.0 {
            // At or below linear expectation - highly consistent
            return 95.0;
        } else if ratio <= 2.0 {
            return 90.0;
        } else if ratio <= 5.0 {
            return 80.0;
        } else if ratio <= 10.0 {
            return 60.0;
        } else {
            return 40.0;
        }
    }

    // Sub-ms tools: timer resolution is 1ms, so variance is measurement noise
    if latency_ms < 2 {
        return 95.0;
    }

    // Single sample without size context: cannot determine consistency meaningfully
    f64::NAN
}

/// Calculate coefficient of variation (CV) from latency samples.
fn calculate_cv(samples: &[u64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<u64>() as f64 / n;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = samples
        .iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / (n - 1.0);
    let std_dev = variance.sqrt();
    std_dev / mean
}

/// Compute consistency score from coefficient of variation.
/// Used with multi-run benchmark results.
pub fn compute_consistency_from_cv(cv: f64) -> f64 {
    if cv < 0.10 {
        95.0 // highly consistent
    } else if cv < 0.25 {
        80.0 // consistent
    } else if cv < 0.50 {
        60.0 // moderate
    } else {
        30.0 // inconsistent
    }
}

/// Compute robustness score based on error recovery ratio.
///
/// Score is derived from the ratio of successful operations to total operations:
/// - 100% success → 100.0 (perfect robustness)
/// - Recoverable errors lower the score proportionally
/// - If no operations were performed, returns f64::NAN (incomplete measurement)
pub fn compute_robustness_score(error_count: u32, total_operations: u32) -> f64 {
    if total_operations == 0 {
        // No operations tracked — cannot determine robustness
        return f64::NAN;
    }

    let success_ratio = 1.0 - (error_count as f64 / total_operations as f64);
    success_ratio * 100.0
}

/// Weights for MCP Health Score computation.
pub const HEALTH_WEIGHTS: (f64, f64, f64, f64, f64) = (0.35, 0.20, 0.15, 0.15, 0.15);

/// Compute the MCP Health Score from dimension scores.
/// Uses weighted average: CORR×0.35 + LAT×0.20 + ESC×0.15 + CON×0.15 + ROB×0.15
pub fn compute_health_score(scores: &DimensionScores) -> f64 {
    let (w_corr, w_lat, w_esc, w_con, w_rob) = HEALTH_WEIGHTS;

    let corr = scores.correctitud.unwrap_or(0.0);
    let lat = scores.latencia.unwrap_or(0.0);
    let esc = scores.escalabilidad.unwrap_or(0.0);
    let con = scores.consistencia.unwrap_or(0.0);
    let rob = scores.robustez.unwrap_or(0.0);

    // Count how many dimensions are available
    let mut count = 0u32;
    if scores.correctitud.is_some() {
        count += 1;
    }
    if scores.latencia.is_some() {
        count += 1;
    }
    if scores.escalabilidad.is_some() {
        count += 1;
    }
    if scores.consistencia.is_some() {
        count += 1;
    }
    if scores.robustez.is_some() {
        count += 1;
    }

    if count == 0 {
        return f64::NAN; // No scores available
    }

    let weighted_sum = corr * w_corr + lat * w_lat + esc * w_esc + con * w_con + rob * w_rob;
    let total_weight: f64 = if scores.correctitud.is_some() {
        w_corr
    } else {
        0.0
    } + if scores.latencia.is_some() {
        w_lat
    } else {
        0.0
    } + if scores.escalabilidad.is_some() {
        w_esc
    } else {
        0.0
    } + if scores.consistencia.is_some() {
        w_con
    } else {
        0.0
    } + if scores.robustez.is_some() {
        w_rob
    } else {
        0.0
    };

    if total_weight == 0.0 {
        return f64::NAN;
    }

    // Dimension scores are already 0-100, weights sum to 1.0 when all present.
    // Weighted average gives correct 0-100 result.
    weighted_sum / total_weight
}

/// Score a complete tool execution against ground truth and metrics.
pub fn score_scenario(
    tool_name: &str,
    language: &str,
    scenario_id: &str,
    tool_response: &Value,
    ground_truth: &Option<GroundTruth>,
    metrics: &Option<MetricsDefinition>,
    latency_ms: u64,
    exec_metadata: ExecutionMetadata,
) -> ToolScore {
    // Compute correctness score for non-call-graph tools
    let (correctitud, symbol_match, outline_match, code_match, complexity_match) =
        if let Some(gt) = ground_truth {
            compute_correctness_score(tool_response, gt, metrics)
        } else {
            // No ground truth - correctness is N/A
            (f64::NAN, None, None, None, None)
        };

    // Compute call graph specific scores
    let (edge_match, entry_point_match, leaf_function_match, hot_path_match, call_graph_score) =
        if let Some(gt) = ground_truth {
            compute_call_graph_score(tool_response, gt)
        } else {
            (None, None, None, None, f64::NAN)
        };

    // Dispatch to specialized scoring based on tool type
    let (
        final_correctitud,
        index_completeness,
        query_accuracy,
        per_file_edge_match,
        merge_accuracy,
        behavioral_preservation,
        usage_match,
        search_result_match,
    ) = match tool_name {
        "find_usages" => {
            if let Some(gt) = ground_truth {
                let (um, _, score) = compute_search_tools_score(tool_name, tool_response, gt);
                (score, None, None, None, None, None, um, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "semantic_search" => {
            if let Some(gt) = ground_truth {
                let (_, sm, score) = compute_search_tools_score(tool_name, tool_response, gt);
                (score, None, None, None, None, None, None, sm)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "build_lightweight_index" => {
            if let Some(gt) = ground_truth {
                let ic = compute_index_completeness(tool_response, gt);
                (
                    ic.completeness_score,
                    Some(ic),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "query_symbol_index" => {
            if let Some(gt) = ground_truth {
                let qa = compute_query_accuracy(tool_response, gt);
                (
                    qa.accuracy_score,
                    None,
                    Some(qa),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "get_per_file_graph" => {
            if let Some(gt) = ground_truth {
                let pfe = compute_per_file_edge_accuracy(tool_response, gt);
                (pfe.f1_score, None, None, Some(pfe), None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "merge_file_graphs" => {
            if let Some(gt) = ground_truth {
                let ma = compute_merge_accuracy(tool_response, gt);
                (
                    ma.accuracy_score,
                    None,
                    None,
                    None,
                    Some(ma),
                    None,
                    None,
                    None,
                )
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "safe_refactor" => {
            if let Some(gt) = ground_truth {
                let bp = compute_behavioral_preservation(tool_response, gt);
                (
                    bp.post_code_similarity,
                    None,
                    None,
                    None,
                    None,
                    Some(bp),
                    None,
                    None,
                )
            } else {
                // Even without ground truth, score based on response
                let bp = compute_behavioral_preservation(tool_response, &GroundTruth::default());
                (
                    bp.post_code_similarity,
                    None,
                    None,
                    None,
                    None,
                    Some(bp),
                    None,
                    None,
                )
            }
        }
        "check_architecture" => {
            if let Some(gt) = ground_truth {
                let score = score_cycles(tool_response, gt);
                (score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "trace_path" => {
            if let Some(gt) = ground_truth {
                let score = score_paths(tool_response, gt);
                (score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "analyze_impact" => {
            if let Some(gt) = ground_truth {
                let score = score_impacted_files(tool_response, gt);
                (score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "export_mermaid" => {
            if let Some(gt) = ground_truth {
                let score = score_mermaid(tool_response, gt);
                (score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "get_outline" => {
            if let Some(_gt) = ground_truth {
                // Use outline score from compute_correctness_score if available
                let recall_only = metrics
                    .as_ref()
                    .and_then(|m| m.correctness.as_ref())
                    .and_then(|c| c.recall_only)
                    .unwrap_or(false);
                let outline_score = if let Some(outline_match) = &outline_match {
                    outline_match.structure_score * 100.0
                } else if let Some(sym_match) = &symbol_match {
                    // Fallback: use symbol recall (if recall_only) or F1 when outline format not provided
                    if recall_only {
                        sym_match.recall * 100.0
                    } else {
                        sym_match.f1_score * 100.0
                    }
                } else {
                    // Fallback to NAN if no ground truth outline or symbols
                    f64::NAN
                };
                (outline_score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        "search_content" => {
            if let Some(gt) = ground_truth {
                let recall_only = metrics
                    .as_ref()
                    .and_then(|m| m.correctness.as_ref())
                    .and_then(|c| c.recall_only)
                    .unwrap_or(false);
                let score = score_search_content(tool_response, gt, recall_only);
                (score, None, None, None, None, None, None, None)
            } else {
                (f64::NAN, None, None, None, None, None, None, None)
            }
        }
        _ => {
            // Use call graph score if available, otherwise use symbol/outline/code/complexity score
            let base_score = if !call_graph_score.is_nan() {
                call_graph_score
            } else if !correctitud.is_nan() && correctitud > 0.0 {
                correctitud
            } else {
                correctitud
            };
            (base_score, None, None, None, None, None, None, None)
        }
    };

    // Compute latency score
    let latencia = compute_latency_score(latency_ms, metrics);

    // Compute dimension scores using execution metadata
    let escalabilidad =
        compute_scalability_score(exec_metadata.workspace_size_kb, latency_ms, metrics);
    let consistencia = compute_consistency_score(
        latency_ms,
        exec_metadata.workspace_size_kb,
        &exec_metadata.latency_samples_ms,
    );
    let robustez =
        compute_robustness_score(exec_metadata.error_count, exec_metadata.total_operations);

    let scores = DimensionScores {
        correctitud: Some(final_correctitud).filter(|&v| !v.is_nan()),
        latencia: Some(latencia),
        escalabilidad: Some(escalabilidad).filter(|&v| !v.is_nan()),
        consistencia: Some(consistencia).filter(|&v| !v.is_nan()),
        robustez: Some(robustez).filter(|&v| !v.is_nan()),
    };

    let health_score = compute_health_score(&scores);

    ToolScore {
        tool: tool_name.to_string(),
        language: language.to_string(),
        scenario_id: scenario_id.to_string(),
        correctitud: if final_correctitud.is_nan() {
            0.0
        } else {
            final_correctitud
        },
        latencia,
        escalabilidad,
        consistencia,
        robustez,
        health_score: if health_score.is_nan() {
            0.0
        } else {
            health_score
        },
        symbol_match,
        outline_match,
        code_match,
        complexity_match,
        edge_match,
        entry_point_match,
        leaf_function_match,
        hot_path_match,
        index_completeness,
        query_accuracy,
        per_file_edge_match,
        merge_accuracy,
        behavioral_preservation,
        usage_match,
        search_result_match,
    }
}

// =========================================================================
// Warmup Penalty Measurement (Phase B3)
// =========================================================================

/// Result of warm vs cold latency measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupResult {
    /// Latency of the first (cold) call in ms
    pub cold_latency_ms: u64,
    /// Median latency of warm calls in ms
    pub warm_median_ms: u64,
    /// Warmup penalty ratio (cold / warm_median)
    pub warmup_penalty: f64,
    /// Number of warm measurements taken
    pub warm_measurements: usize,
    /// Whether penalty exceeds 3x threshold
    pub penalty_flagged: bool,
}

/// Compute warmup penalty from a series of latency measurements.
/// First measurement is "cold", rest are "warm".
pub fn compute_warmup_penalty(latencies_ms: &[u64]) -> WarmupResult {
    if latencies_ms.is_empty() {
        return WarmupResult {
            cold_latency_ms: 0,
            warm_median_ms: 0,
            warmup_penalty: 1.0,
            warm_measurements: 0,
            penalty_flagged: false,
        };
    }

    let cold_latency_ms = latencies_ms[0];
    let warm_latencies: Vec<u64> = if latencies_ms.len() > 1 {
        let mut warm = latencies_ms[1..].to_vec();
        warm.sort();
        warm
    } else {
        vec![cold_latency_ms]
    };

    let warm_median_ms = warm_latencies[warm_latencies.len() / 2];
    let warmup_penalty = if warm_median_ms > 0 {
        cold_latency_ms as f64 / warm_median_ms as f64
    } else {
        1.0
    };

    WarmupResult {
        cold_latency_ms,
        warm_median_ms,
        warmup_penalty,
        warm_measurements: warm_latencies.len(),
        penalty_flagged: warmup_penalty > 3.0,
    }
}

// =========================================================================
// Session Benchmark Utilities (Phase B2)
// =========================================================================

use super::artifacts::{BenchmarkResult, BenchmarkStats, WarmupInfo};

/// Compute percentile from sorted data using nearest-rank method.
/// Returns 0 if data is empty.
pub fn percentile(sorted_data: &[u64], p: f64) -> u64 {
    if sorted_data.is_empty() {
        return 0;
    }
    if sorted_data.len() == 1 {
        return sorted_data[0];
    }
    let idx = ((p / 100.0) * (sorted_data.len() - 1) as f64) as usize;
    let idx = idx.min(sorted_data.len() - 1);
    sorted_data[idx]
}

/// Compute standard deviation of a slice of u64 values.
fn std_dev(data: &[u64], mean: f64) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let variance = data
        .iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Compute statistics from a series of latency measurements.
pub fn compute_benchmark_stats(latencies_ms: &[u64]) -> BenchmarkStats {
    if latencies_ms.is_empty() {
        return BenchmarkStats {
            min_ms: 0,
            max_ms: 0,
            mean_ms: 0.0,
            median_ms: 0,
            p50_ms: 0,
            p95_ms: 0,
            p99_ms: 0,
            std_dev_ms: 0.0,
            ops_per_second: 0.0,
            total_duration_ms: 0,
        };
    }

    let mut sorted = latencies_ms.to_vec();
    sorted.sort();

    let min_ms = sorted[0];
    let max_ms = *sorted.last().unwrap();
    let sum: u64 = sorted.iter().sum();
    let mean_ms = sum as f64 / sorted.len() as f64;
    let median_ms = percentile(&sorted, 50.0);
    let p50_ms = percentile(&sorted, 50.0);
    let p95_ms = percentile(&sorted, 95.0);
    let p99_ms = percentile(&sorted, 99.0);
    let std_dev_ms = std_dev(&sorted, mean_ms);
    let total_duration_ms: u64 = sorted.iter().sum();
    let ops_per_second = if total_duration_ms > 0 {
        (sorted.len() as f64) / (total_duration_ms as f64 / 1000.0)
    } else {
        0.0
    };

    BenchmarkStats {
        min_ms,
        max_ms,
        mean_ms,
        median_ms,
        p50_ms,
        p95_ms,
        p99_ms,
        std_dev_ms,
        ops_per_second,
        total_duration_ms,
    }
}

/// Compute warmup info from latency measurements.
pub fn compute_warmup_info(latencies_ms: &[u64]) -> WarmupInfo {
    if latencies_ms.is_empty() {
        return WarmupInfo {
            cold_latency_ms: 0,
            warm_median_ms: 0,
            warmup_penalty: 1.0,
            penalty_flagged: false,
        };
    }

    let cold_latency_ms = latencies_ms[0];
    let warm_latencies: Vec<u64> = if latencies_ms.len() > 1 {
        latencies_ms[1..].to_vec()
    } else {
        vec![cold_latency_ms]
    };

    let mut sorted_warm = warm_latencies.clone();
    sorted_warm.sort();
    let warm_median_ms = if sorted_warm.is_empty() {
        cold_latency_ms
    } else {
        sorted_warm[sorted_warm.len() / 2]
    };

    let warmup_penalty = if warm_median_ms > 0 {
        cold_latency_ms as f64 / warm_median_ms as f64
    } else {
        1.0
    };

    WarmupInfo {
        cold_latency_ms,
        warm_median_ms,
        warmup_penalty,
        penalty_flagged: warmup_penalty > 1.5,
    }
}

/// Build a BenchmarkResult from raw latency measurements.
pub fn build_benchmark_result(
    tool: String,
    iterations_requested: u32,
    latencies_ms: Vec<u64>,
) -> BenchmarkResult {
    let iterations_completed = latencies_ms.len() as u32;
    let completed = iterations_completed == iterations_requested;
    let stats = compute_benchmark_stats(&latencies_ms);
    let warmup = compute_warmup_info(&latencies_ms);
    let timestamp = iso8601_now();

    BenchmarkResult {
        tool,
        iterations_requested,
        iterations_completed,
        completed,
        latencies_ms,
        stats,
        warmup,
        timestamp,
    }
}

/// Get current timestamp in ISO 8601 format.
fn iso8601_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    let t = std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nanos);
    let datetime: chrono::DateTime<chrono::Utc> = t.into();
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// =========================================================================
// Indexing Tool Scoring Functions
// =========================================================================

/// Compute index completeness score for build_lightweight_index tool.
/// Compares actual symbols indexed vs expected symbols in ground truth.
pub fn compute_index_completeness(
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> IndexCompletenessResult {
    let expected_count = ground_truth
        .indexed_symbols
        .as_ref()
        .map(|s| s.len())
        .unwrap_or(0);

    // Unwrap MCP content wrapper if present
    let unwrapped = super::ground_truth::unwrap_response(tool_response);

    let actual_count = unwrapped
        .get("symbols_indexed")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0);

    let completeness_score = if expected_count == 0 {
        if actual_count == 0 {
            100.0
        } else {
            50.0 // Some found but none expected
        }
    } else {
        (actual_count as f64 / expected_count as f64) * 100.0
    };

    let missing_names: Vec<String> = ground_truth
        .indexed_symbols
        .as_ref()
        .map(|symbols| symbols.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default();

    IndexCompletenessResult {
        expected_count,
        actual_count,
        completeness_score: completeness_score.min(100.0),
        missing_symbols: missing_names,
    }
}

/// Compute query accuracy score for query_symbol_index tool.
/// Compares found locations vs expected locations.
pub fn compute_query_accuracy(
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> QueryAccuracyResult {
    let expected_locations = ground_truth
        .query_results
        .as_ref()
        .map(|r| r.len())
        .unwrap_or(0);

    // Unwrap MCP content wrapper if present
    let unwrapped = super::ground_truth::unwrap_response(tool_response);

    let found_locations = unwrapped
        .get("total")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0);

    let accuracy_score = if expected_locations == 0 {
        if found_locations == 0 {
            100.0
        } else {
            50.0
        }
    } else {
        (found_locations as f64 / expected_locations as f64) * 100.0
    };

    QueryAccuracyResult {
        expected_locations,
        found_locations,
        accuracy_score: accuracy_score.min(100.0),
        missing_locations: Vec::new(),
    }
}

/// Compute per-file edge accuracy score for get_per_file_graph tool.
pub fn compute_per_file_edge_accuracy(
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> PerFileEdgeMatchResult {
    let returned_edges = parse_returned_edges(tool_response);
    let expected_edges = ground_truth
        .per_file_edges
        .as_ref()
        .and_then(|pfe| pfe.first())
        .map(|pfe| pfe.edges.clone())
        .unwrap_or_default();

    let result = match_edges(&returned_edges, &expected_edges);

    PerFileEdgeMatchResult {
        file: ground_truth
            .per_file_edges
            .as_ref()
            .and_then(|pfe| pfe.first())
            .map(|pfe| pfe.file.clone())
            .unwrap_or_default(),
        true_positives: result.true_positives,
        false_positives: result.false_positives,
        false_negatives: result.false_negatives,
        f1_score: result.f1_score * 100.0,
    }
}

/// Compute merge accuracy score for merge_file_graphs tool.
pub fn compute_merge_accuracy(
    tool_response: &Value,
    ground_truth: &GroundTruth,
) -> MergeAccuracyResult {
    let returned_edges = parse_returned_edges(tool_response);
    let expected_edges = ground_truth
        .merged_edges
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let result = match_edges(&returned_edges, &expected_edges);

    MergeAccuracyResult {
        total_expected: expected_edges.len(),
        total_found: returned_edges.len(),
        accuracy_score: result.f1_score * 100.0,
        missing_edges: result.missing_edges,
        extra_edges: result.extra_edges,
    }
}

/// Compute behavioral preservation score for safe_refactor tools.
/// Compares pre/post code and checks if behavior is preserved.
pub fn compute_behavioral_preservation(
    tool_response: &Value,
    _ground_truth: &GroundTruth,
) -> BehavioralPreservationResult {
    // Unwrap MCP content wrapper if present
    let tool_response = super::ground_truth::unwrap_response(tool_response);

    // Check if refactoring was successful
    let success = tool_response
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        return BehavioralPreservationResult {
            pre_code_match: false,
            post_code_similarity: 0.0,
            behavioral_preserved: false,
            details: "Refactoring failed".to_string(),
        };
    }

    // For now, we score based on whether changes were made
    let has_changes = tool_response
        .get("changes")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if !has_changes {
        return BehavioralPreservationResult {
            pre_code_match: true,
            post_code_similarity: 100.0,
            behavioral_preserved: true,
            details: "No changes needed - code already in target form".to_string(),
        };
    }

    // Changes were made - estimate preservation based on validation result
    let is_valid = tool_response
        .get("validation_result")
        .and_then(|v| v.get("is_valid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let details = if is_valid {
        "Refactoring successful with valid result".to_string()
    } else {
        "Refactoring completed but validation has warnings".to_string()
    };

    BehavioralPreservationResult {
        pre_code_match: true,
        post_code_similarity: if is_valid { 100.0 } else { 75.0 },
        behavioral_preserved: is_valid,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_ground_truth() -> GroundTruth {
        GroundTruth {
            symbols: Some(vec![
                super::super::ground_truth::ExpectedSymbol {
                    name: "greet".to_string(),
                    kind: super::super::ground_truth::SymbolKind::Function,
                    location: None,
                },
                super::super::ground_truth::ExpectedSymbol {
                    name: "add".to_string(),
                    kind: super::super::ground_truth::SymbolKind::Function,
                    location: None,
                },
            ]),
            ..Default::default()
        }
    }

    fn make_sample_metrics() -> MetricsDefinition {
        MetricsDefinition {
            correctness: Some(CorrectnessMetrics {
                metric_type: Some("precision_recall".to_string()),
                min_score: Some(80.0),
                tolerance_pct: Some(0.05),
                recall_only: None,
            }),
            latency: Some(LatencyMetrics {
                target_ms: Some(100),
                max_ms: Some(500),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_compute_correctness_with_symbols() {
        use super::super::ground_truth::{
            match_symbols, ExpectedSymbol, ReturnedSymbol, SymbolKind,
        };

        let expected = vec![
            ExpectedSymbol {
                name: "greet".to_string(),
                kind: SymbolKind::Function,
                location: None,
            },
            ExpectedSymbol {
                name: "add".to_string(),
                kind: SymbolKind::Function,
                location: None,
            },
        ];

        let returned = vec![
            ReturnedSymbol {
                name: "greet".to_string(),
                kind: "function".to_string(),
                file: None,
                line: None,
                col: None,
            },
            ReturnedSymbol {
                name: "add".to_string(),
                kind: "function".to_string(),
                file: None,
                line: None,
                col: None,
            },
            ReturnedSymbol {
                name: "extra".to_string(),
                kind: "struct".to_string(),
                file: None,
                line: None,
                col: None,
            },
        ];

        let match_result = match_symbols(&returned, &expected, false);

        assert_eq!(match_result.true_positives, 2);
        assert_eq!(match_result.missing_symbols.len(), 0);
        assert_eq!(match_result.false_positives, 1); // "extra" is not in ground truth
        assert!(match_result.f1_score > 0.0);

        // Now test via compute_correctness_score
        let gt = make_sample_ground_truth();
        let response = serde_json::json!({
            "symbols": [
                {"name": "greet", "kind": "function"},
                {"name": "add", "kind": "function"},
                {"name": "extra", "kind": "struct"}
            ]
        });

        let (score, symbol_match, _, _, _) = compute_correctness_score(&response, &gt, &None);

        // Debug assertions
        assert!(!score.is_nan(), "Score should not be NaN");
        assert!(gt.symbols.is_some(), "gt.symbols should be Some");

        let (score2, symbol_match2, _, _, _) = compute_correctness_score(&response, &gt, &None);
        assert!(
            !score2.is_nan(),
            "Second call score should not be NaN, got {}",
            score2
        );

        assert!(score > 0.0, "Score should be > 0.0, got {}", score);
        assert!(symbol_match.is_some());
        let match_result = symbol_match.unwrap();
        assert_eq!(match_result.missing_symbols.len(), 0);
        assert_eq!(match_result.false_positives, 1); // "extra" is not in ground truth
    }

    #[test]
    fn test_compute_correctness_no_ground_truth() {
        let gt = GroundTruth::default();
        let response = serde_json::json!({"result": "anything"});

        let (score, _, _, _, _) = compute_correctness_score(&response, &gt, &None);

        assert!(score.is_nan());
    }

    #[test]
    fn test_compute_latency_score_under_target() {
        let metrics = make_sample_metrics();
        let score = compute_latency_score(50, &Some(metrics));
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_compute_latency_score_at_target() {
        let metrics = make_sample_metrics();
        let score = compute_latency_score(100, &Some(metrics));
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_compute_latency_score_over_target() {
        let metrics = make_sample_metrics();
        let score = compute_latency_score(300, &Some(metrics));
        // (500-300)/(500-100) * 100 = 200/400 * 100 = 50
        assert_eq!(score, 50.0);
    }

    #[test]
    fn test_compute_latency_score_at_max() {
        let metrics = make_sample_metrics();
        let score = compute_latency_score(500, &Some(metrics));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_compute_latency_score_over_max() {
        let metrics = make_sample_metrics();
        let score = compute_latency_score(1000, &Some(metrics));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_compute_health_score_all_dimensions() {
        let scores = DimensionScores {
            correctitud: Some(100.0),
            latencia: Some(100.0),
            escalabilidad: Some(100.0),
            consistencia: Some(100.0),
            robustez: Some(100.0),
        };

        let health = compute_health_score(&scores);
        assert_eq!(health, 100.0);
    }

    #[test]
    fn test_compute_health_score_partial_dimensions() {
        let scores = DimensionScores {
            correctitud: Some(100.0),
            latencia: Some(50.0),
            escalabilidad: None,
            consistencia: None,
            robustez: None,
        };

        let health = compute_health_score(&scores);
        // Only CORR (0.35) and LAT (0.20) available = 0.55 total weight
        // (100*0.35 + 50*0.20) / 0.55 = (35 + 10) / 0.55 = 45 / 0.55 ≈ 81.8
        assert!(health > 80.0 && health < 85.0);
    }

    #[test]
    fn test_compute_health_score_no_dimensions() {
        let scores = DimensionScores::default();
        let health = compute_health_score(&scores);
        assert!(health.is_nan());
    }

    #[test]
    fn test_score_scenario_complete() {
        let gt = make_sample_ground_truth();
        let metrics = make_sample_metrics();
        let response = serde_json::json!({
            "symbols": [
                {"name": "greet", "kind": "function"},
                {"name": "add", "kind": "function"}
            ]
        });

        let score = score_scenario(
            "get_file_symbols",
            "rust",
            "rust_symbols_test",
            &response,
            &Some(gt),
            &Some(metrics),
            50,
            ExecutionMetadata::default(),
        );

        assert_eq!(score.tool, "get_file_symbols");
        assert_eq!(score.language, "rust");
        assert!(score.correctitud > 90.0);
        assert_eq!(score.latencia, 100.0); // 50ms < 100ms target
        assert!(score.health_score > 0.0);
    }

    #[test]
    fn test_compute_index_completeness_full_match() {
        use super::super::ground_truth::{ExpectedSymbol, SymbolKind};

        let gt = GroundTruth {
            indexed_symbols: Some(vec![
                ExpectedSymbol {
                    name: "func1".to_string(),
                    kind: SymbolKind::Function,
                    location: None,
                },
                ExpectedSymbol {
                    name: "func2".to_string(),
                    kind: SymbolKind::Function,
                    location: None,
                },
            ]),
            ..Default::default()
        };

        let response = serde_json::json!({
            "success": true,
            "symbols_indexed": 2,
            "locations_indexed": 2
        });

        let result = compute_index_completeness(&response, &gt);

        assert_eq!(result.expected_count, 2);
        assert_eq!(result.actual_count, 2);
        assert_eq!(result.completeness_score, 100.0);
    }

    #[test]
    fn test_compute_query_accuracy() {
        let gt = GroundTruth {
            query_results: Some(vec![super::super::ground_truth::ExpectedQueryResult {
                symbol_name: "add".to_string(),
                locations: vec![],
            }]),
            ..Default::default()
        };

        let response = serde_json::json!({
            "symbol_name": "add",
            "total": 1,
            "locations": [
                {"file": "src/lib.rs", "line": 10, "column": 0, "symbol_kind": "Function"}
            ]
        });

        let result = compute_query_accuracy(&response, &gt);

        assert_eq!(result.expected_locations, 1);
        assert_eq!(result.found_locations, 1);
        assert_eq!(result.accuracy_score, 100.0);
    }

    #[test]
    fn test_compute_behavioral_preservation_success() {
        let gt = GroundTruth::default();

        let response = serde_json::json!({
            "success": true,
            "action": "rename",
            "changes": [
                {"file": "src/lib.rs", "old_text": "old_name", "new_text": "new_name"}
            ],
            "validation_result": {
                "is_valid": true,
                "warnings": [],
                "errors": []
            }
        });

        let result = compute_behavioral_preservation(&response, &gt);

        assert!(result.behavioral_preserved);
        assert_eq!(result.post_code_similarity, 100.0);
    }

    #[test]
    fn test_compute_behavioral_preservation_failure() {
        let gt = GroundTruth::default();

        let response = serde_json::json!({
            "success": false,
            "action": "rename",
            "changes": [],
            "validation_result": {
                "is_valid": false,
                "warnings": [],
                "errors": ["Symbol not found"]
            },
            "error_message": "Symbol not found"
        });

        let result = compute_behavioral_preservation(&response, &gt);

        assert!(!result.behavioral_preserved);
        assert_eq!(result.post_code_similarity, 0.0);
    }

    #[test]
    fn test_scalability_classifications() {
        // Test compute_scalability_score with expected classification from metrics
        let metrics = MetricsDefinition {
            scalability: Some(ScalabilityMetrics {
                classification: Some("constant".to_string()),
                breakpoint_kb: None,
            }),
            ..Default::default()
        };

        // Constant classification should give high score
        let score = compute_scalability_score(1000, 100, &Some(metrics.clone()));
        assert!(score >= 90.0);

        let mut sub_metrics = metrics.clone();
        sub_metrics.scalability.as_mut().unwrap().classification = Some("sub_linear".to_string());
        let score = compute_scalability_score(1000, 100, &Some(sub_metrics));
        assert!(score >= 80.0);

        let mut quad_metrics = metrics;
        quad_metrics.scalability.as_mut().unwrap().classification = Some("quadratic".to_string());
        let score = compute_scalability_score(1000, 100, &Some(quad_metrics));
        assert!(score >= 30.0);
    }

    #[test]
    fn test_health_weights_sum_to_one() {
        let (w_corr, w_lat, w_esc, w_con, w_rob) = HEALTH_WEIGHTS;
        let sum = w_corr + w_lat + w_esc + w_con + w_rob;
        assert!((sum - 1.0).abs() < 0.001);
    }

    // =========================================================================
    // Scalability Classification Tests
    // =========================================================================

    #[test]
    fn test_scalability_class_constant() {
        // Constant: same latency regardless of size
        let measurements = vec![(1, 10), (10, 10), (100, 10), (1000, 10)];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::Constant);
        assert!(result.score >= 90.0); // High score for constant
    }

    #[test]
    fn test_scalability_class_linear() {
        // Linear: latency grows proportionally with size
        let measurements = vec![(1, 10), (10, 100), (100, 1000), (1000, 10000)];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::Linear);
        assert!(result.score >= 60.0); // Decent score for linear
    }

    #[test]
    fn test_scalability_class_quadratic() {
        // Quadratic: latency grows with square of size (10x size -> 100x latency)
        let measurements = vec![(1, 10), (10, 1000), (100, 100000), (200, 400000)];
        let result = classify_scalability(&measurements);
        // Growth ratio is very high (>3), so classified as quadratic
        assert!(result.score <= 50.0); // Lower score for quadratic
    }

    #[test]
    fn test_scalability_class_exponential() {
        // True exponential: latency doubles while size grows linearly
        // 1->2: size 2x, latency 4x; 2->3: size 1.5x, latency 2x
        let measurements = vec![(1, 10), (2, 40), (3, 160), (4, 640), (5, 2560)];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::Exponential);
        assert!(result.score <= 30.0); // Very low score for exponential
    }

    #[test]
    fn test_scalability_class_sublinear() {
        // Sub-linear: latency grows slower than size (logarithmic pattern)
        let measurements = vec![
            (1, 10),
            (10, 15),   // size 10x, latency 1.5x
            (100, 18),  // size 10x, latency 1.2x
            (1000, 20), // size 10x, latency 1.1x
        ];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::SubLinear);
        assert!(result.score >= 70.0); // Good score for sub-linear
    }

    #[test]
    fn test_scalability_insufficient_data() {
        // Single point should return constant
        let measurements = vec![(100, 50)];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::Constant);
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_scalability_empty_data() {
        let measurements: Vec<(u64, u64)> = vec![];
        let result = classify_scalability(&measurements);
        assert_eq!(result.classification, ScalabilityClass::Constant);
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_scalability_class_from_str() {
        assert_eq!(
            ScalabilityClass::from_str("constant"),
            Some(ScalabilityClass::Constant)
        );
        assert_eq!(
            ScalabilityClass::from_str("linear"),
            Some(ScalabilityClass::Linear)
        );
        assert_eq!(
            ScalabilityClass::from_str("sub_linear"),
            Some(ScalabilityClass::SubLinear)
        );
        assert_eq!(
            ScalabilityClass::from_str("quadratic"),
            Some(ScalabilityClass::Quadratic)
        );
        assert_eq!(
            ScalabilityClass::from_str("exponential"),
            Some(ScalabilityClass::Exponential)
        );
        assert_eq!(ScalabilityClass::from_str("unknown"), None);
    }

    #[test]
    fn test_scalability_class_as_str() {
        assert_eq!(ScalabilityClass::Constant.as_str(), "constant");
        assert_eq!(ScalabilityClass::Linear.as_str(), "linear");
        assert_eq!(ScalabilityClass::SubLinear.as_str(), "sub_linear");
        assert_eq!(ScalabilityClass::Quadratic.as_str(), "quadratic");
        assert_eq!(ScalabilityClass::Exponential.as_str(), "exponential");
    }

    #[test]
    fn test_compute_scalability_score_from_measurements() {
        let measurements = vec![(1, 10), (10, 100), (100, 1000)];
        let result = compute_scalability_score_from_measurements(&measurements, Some("linear"));
        assert!(result.score > 0.0);
        assert_eq!(result.latency_at_sizes, measurements);
    }

    #[test]
    fn test_compute_scalability_score_from_measurements_with_expected() {
        // Measurements suggest linear, but we expect sub_linear
        let measurements = vec![(1, 10), (10, 100), (100, 1000)];
        let result = compute_scalability_score_from_measurements(&measurements, Some("sub_linear"));
        // Score should be penalized for mismatch
        assert!(result.score < 100.0);
    }

    #[test]
    fn test_single_measurement_scoring() {
        // Single measurement at small size should score well
        let score = compute_scalability_score(1, 10, &None);
        assert!(score >= 80.0);

        // Single measurement at large size with low latency should score well
        let score = compute_scalability_score(1000, 50, &None);
        assert!(score >= 70.0);
    }

    // =========================================================================
    // Warmup Penalty Tests (Phase B3)
    // =========================================================================

    #[test]
    fn test_warmup_penalty_no_warmup() {
        // Single measurement → penalty 1.0 (cold == warm since no warm measurements)
        let latencies = vec![100];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 100);
        assert_eq!(result.warm_median_ms, 100); // Falls back to cold value
        assert_eq!(result.warmup_penalty, 1.0);
        assert_eq!(result.warm_measurements, 1);
        assert!(!result.penalty_flagged);
    }

    #[test]
    fn test_warmup_penalty_low() {
        // cold=10ms, warm=[8,9,10,11,9] → penalty ~1.0-1.2
        let latencies = vec![10, 8, 9, 10, 11, 9];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 10);
        assert_eq!(result.warm_median_ms, 9); // median of [8,9,9,10,11]
        assert!(result.warmup_penalty > 1.0 && result.warmup_penalty < 1.5);
        assert_eq!(result.warm_measurements, 5);
        assert!(!result.penalty_flagged);
    }

    #[test]
    fn test_warmup_penalty_high() {
        // cold=100ms, warm=[5,6,5,7,6] → penalty ~17x, flagged
        let latencies = vec![100, 5, 6, 5, 7, 6];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 100);
        assert_eq!(result.warm_median_ms, 6); // median of [5,5,6,6,7]
        assert!(result.warmup_penalty > 15.0); // ~16.67x
        assert_eq!(result.warm_measurements, 5);
        assert!(result.penalty_flagged); // > 3x threshold
    }

    #[test]
    fn test_warmup_penalty_exact_3x() {
        // cold=30ms, warm=[10,10,10] → penalty 3.0, NOT flagged (threshold is > 3.0)
        let latencies = vec![30, 10, 10, 10];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 30);
        assert_eq!(result.warm_median_ms, 10);
        assert!((result.warmup_penalty - 3.0).abs() < 0.001);
        assert!(!result.penalty_flagged); // 3.0 is NOT > 3.0, so not flagged
    }

    #[test]
    fn test_warmup_penalty_empty() {
        let latencies: Vec<u64> = vec![];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 0);
        assert_eq!(result.warm_median_ms, 0);
        assert_eq!(result.warmup_penalty, 1.0);
        assert_eq!(result.warm_measurements, 0);
        assert!(!result.penalty_flagged);
    }

    #[test]
    fn test_warmup_penalty_warm_median_even_count() {
        // Even number of warm measurements - implementation uses upper middle value (index len/2)
        let latencies = vec![50, 10, 20];
        let result = compute_warmup_penalty(&latencies);
        assert_eq!(result.cold_latency_ms, 50);
        assert_eq!(result.warm_median_ms, 20); // upper median of [10, 20] = index 1 = 20
    }

    // =========================================================================
    // Benchmark Statistics Tests (Phase B2)
    // =========================================================================

    #[test]
    fn test_percentile_empty() {
        let data: Vec<u64> = vec![];
        assert_eq!(percentile(&data, 50.0), 0);
    }

    #[test]
    fn test_percentile_single() {
        let data = vec![42];
        assert_eq!(percentile(&data, 50.0), 42);
        assert_eq!(percentile(&data, 95.0), 42);
        assert_eq!(percentile(&data, 99.0), 42);
    }

    #[test]
    fn test_percentile_p50() {
        // Median of sorted data
        let data = vec![1, 5, 10, 15, 20];
        assert_eq!(percentile(&data, 50.0), 10); // middle element
    }

    #[test]
    fn test_percentile_p95_p99() {
        let mut data: Vec<u64> = (1..=100).collect();
        // p95 should be around 95th element (nearest-rank method)
        assert_eq!(percentile(&data, 95.0), 95);
        // p99 with 100 elements: floor(99/100 * 99) = floor(98.01) = 98
        assert_eq!(percentile(&data, 99.0), 99);
    }

    #[test]
    fn test_percentile_edge_cases() {
        // Test p0 and p100
        let data = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&data, 0.0), 10); // first element
        assert_eq!(percentile(&data, 100.0), 50); // last element
    }

    #[test]
    fn test_compute_benchmark_stats_constant() {
        // All same values - std_dev should be 0
        let latencies = vec![10, 10, 10, 10, 10];
        let stats = compute_benchmark_stats(&latencies);
        assert_eq!(stats.min_ms, 10);
        assert_eq!(stats.max_ms, 10);
        assert_eq!(stats.mean_ms, 10.0);
        assert_eq!(stats.median_ms, 10);
        assert_eq!(stats.p50_ms, 10);
        assert_eq!(stats.p95_ms, 10);
        assert_eq!(stats.p99_ms, 10);
        assert_eq!(stats.std_dev_ms, 0.0);
        assert!(stats.ops_per_second > 0.0);
    }

    #[test]
    fn test_compute_benchmark_stats_variable() {
        // Known values to verify mean/median/p95
        let latencies = vec![5, 10, 10, 15, 20, 25, 30];
        let stats = compute_benchmark_stats(&latencies);

        // Mean: (5+10+10+15+20+25+30)/7 = 115/7 ≈ 16.43
        assert!((stats.mean_ms - 16.428571).abs() < 0.01);

        // Median (p50): 4th element of sorted = 15
        assert_eq!(stats.median_ms, 15);
        assert_eq!(stats.p50_ms, 15);

        // p95: floor(95/100 * 6) = floor(5.7) = 5 → 6th element = 25
        assert_eq!(stats.p95_ms, 25);

        // p99: floor(99/100 * 6) = floor(5.94) = 5 → 6th element = 25
        assert_eq!(stats.p99_ms, 25);
    }

    #[test]
    fn test_compute_benchmark_stats_empty() {
        let latencies: Vec<u64> = vec![];
        let stats = compute_benchmark_stats(&latencies);
        assert_eq!(stats.min_ms, 0);
        assert_eq!(stats.max_ms, 0);
        assert_eq!(stats.mean_ms, 0.0);
        assert_eq!(stats.ops_per_second, 0.0);
    }

    #[test]
    fn test_compute_benchmark_stats_single() {
        let latencies = vec![100];
        let stats = compute_benchmark_stats(&latencies);
        assert_eq!(stats.min_ms, 100);
        assert_eq!(stats.max_ms, 100);
        assert_eq!(stats.mean_ms, 100.0);
        assert_eq!(stats.median_ms, 100);
        assert_eq!(stats.p50_ms, 100);
        assert_eq!(stats.p95_ms, 100);
        assert_eq!(stats.p99_ms, 100);
    }

    #[test]
    fn test_std_dev_calculation() {
        // Test standard deviation calculation
        let latencies = vec![10, 20, 30, 40, 50];
        let stats = compute_benchmark_stats(&latencies);
        // Mean = 30, variance = [(20)^2 + (10)^2 + 0 + 10^2 + 20^2] / 4 = 1000/4 = 250
        // std_dev = sqrt(250) ≈ 15.81
        assert!((stats.std_dev_ms - 15.81).abs() < 0.1);
    }

    #[test]
    fn test_ops_per_second_calculation() {
        // 10 calls in 100ms total = 100 ops/second
        let latencies = vec![10, 10, 10, 10, 10, 10, 10, 10, 10, 10];
        let stats = compute_benchmark_stats(&latencies);
        // total = 100ms, 10 calls, ops_per_second = 10 / (100/1000) = 100
        assert!((stats.ops_per_second - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_warmup_info_no_flag() {
        // Penalty < 1.5 should not be flagged
        let latencies = vec![10, 9, 10, 11, 10];
        let info = compute_warmup_info(&latencies);
        assert_eq!(info.cold_latency_ms, 10);
        assert!(info.warm_median_ms > 0);
        assert!(info.warmup_penalty < 1.5);
        assert!(!info.penalty_flagged);
    }

    #[test]
    fn test_warmup_info_flagged() {
        // Penalty > 1.5 should be flagged
        let latencies = vec![100, 10, 10, 10, 10];
        let info = compute_warmup_info(&latencies);
        assert_eq!(info.cold_latency_ms, 100);
        assert_eq!(info.warm_median_ms, 10);
        assert!(info.warmup_penalty > 1.5);
        assert!(info.penalty_flagged);
    }

    #[test]
    fn test_build_benchmark_result() {
        let latencies = vec![10, 20, 30];
        let result = build_benchmark_result("get_file_symbols".to_string(), 50, latencies);

        assert_eq!(result.tool, "get_file_symbols");
        assert_eq!(result.iterations_requested, 50);
        assert_eq!(result.iterations_completed, 3);
        assert!(!result.completed);
        assert_eq!(result.latencies_ms.len(), 3);
        assert!(!result.timestamp.is_empty());
    }
}
