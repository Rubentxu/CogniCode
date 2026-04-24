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
}
