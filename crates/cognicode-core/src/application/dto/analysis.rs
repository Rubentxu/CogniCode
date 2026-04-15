//! Analysis DTOs - Transport-neutral types for code analysis
//!
//! These DTOs decouple the application layer from the MCP protocol.

use super::common::{AnalysisMetadata, RiskLevel, SourceLocation, SymbolKind, SymbolSummary};
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
