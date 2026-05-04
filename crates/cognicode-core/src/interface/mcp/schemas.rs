//! MCP Tool Schemas for CogniCode Interface Layer
//!
//! This module defines all input/output schemas for MCP tools following
//! the JSON-RPC 2.0 specification.

use serde::{Deserialize, Serialize};
use crate::application::dto::OverviewDetail;

/// Default depth for call hierarchy traversal
fn default_depth() -> u8 {
    1
}

/// Default value for include_external
fn default_false() -> bool {
    false
}

/// Default value for include_declaration
fn default_true() -> bool {
    true
}

/// Default value for compressed
fn default_compressed() -> bool {
    false
}

// ============================================================================
// Call Hierarchy
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCallHierarchyInput {
    /// Fully qualified name (e.g., 'module::function' or 'Class.method')
    pub symbol_name: String,

    /// Direction: incoming (who calls this) or outgoing (what this calls)
    #[serde(rename = "direction")]
    pub direction: CallDirection,

    /// Depth of traversal (default: 1, max: 10)
    #[serde(default = "default_depth")]
    pub depth: u8,

    /// Include external dependencies (crates/packages)
    #[serde(default = "default_false")]
    pub include_external: bool,

    /// Return compressed natural language summary instead of JSON (default: false)
    #[serde(default = "default_compressed")]
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCallHierarchyOutput {
    pub symbol: String,
    pub calls: Vec<CallEntry>,
    pub metadata: AnalysisMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub total_calls: usize,
    pub analysis_time_ms: u64,
}

// ============================================================================
// File Symbols
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetFileSymbolsInput {
    pub file_path: String,

    /// Return compressed natural language summary instead of JSON (default: false)
    #[serde(default = "default_compressed")]
    pub compressed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetFileSymbolsOutput {
    pub file_path: String,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub location: SourceLocation,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Field,
    Variable,
    Constant,
    Constructor,
    Interface,
    TypeAlias,
    Parameter,
}

/// Represents a location in source code for MCP protocol (1-indexed for display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

// ============================================================================
// Get All Symbols
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAllSymbolsInput {
    /// Maximum number of symbols to return (default: 100)
    #[serde(default)]
    pub limit: Option<usize>,

    /// Offset from which to start returning symbols (default: 0)
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAllSymbolsOutput {
    pub symbols: Vec<SymbolInfo>,
    pub total: usize,
    pub has_more: bool,
}

// ============================================================================
// Find Usages
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FindUsagesInput {
    pub symbol_name: String,

    #[serde(default = "default_true")]
    pub include_declaration: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindUsagesOutput {
    pub symbol: String,
    pub usages: Vec<UsageEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub is_definition: bool,
}

// ============================================================================
// Structural Search
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct StructuralSearchInput {
    pub pattern_type: PatternType,
    pub query: String,
    pub path: Option<String>,
    #[serde(default = "default_depth")]
    pub depth: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    FunctionCall,
    TypeDefinition,
    ImportStatement,
    Annotation,
    Custom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructuralSearchOutput {
    pub pattern: String,
    pub matches: Vec<MatchEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub matched_text: String,
    pub context: String,
}

// ============================================================================
// Analyze Impact
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeImpactInput {
    pub symbol_name: String,

    /// Return compressed natural language summary instead of JSON (default: false)
    #[serde(default = "default_compressed")]
    pub compressed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeImpactOutput {
    pub symbol: String,
    pub impacted_files: Vec<String>,
    pub impacted_symbols: Vec<String>,
    pub risk_level: RiskLevel,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

// ============================================================================
// Check Architecture
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckArchitectureInput {
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckArchitectureOutput {
    pub cycles: Vec<CycleInfo>,
    pub violations: Vec<ViolationInfo>,
    pub score: f32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleInfo {
    pub symbols: Vec<String>,
    pub length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationInfo {
    pub rule: String,
    pub from: String,
    pub to: String,
    pub severity: String,
}

// ============================================================================
// Safe Refactor
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SafeRefactorInput {
    pub action: RefactorAction,
    pub target: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorAction {
    Rename,
    Extract,
    Inline,
    Move,
    ChangeSignature,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafeRefactorOutput {
    pub action: RefactorAction,
    pub success: bool,
    pub changes: Vec<ChangeEntry>,
    pub validation_result: ValidationResult,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub file: String,
    pub old_text: String,
    pub new_text: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

// ============================================================================
// Validate Syntax
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateSyntaxInput {
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateSyntaxOutput {
    pub file_path: String,
    pub is_valid: bool,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<SyntaxWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxError {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxWarning {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: String,
}

// ============================================================================
// Get Complexity
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetComplexityInput {
    pub file_path: String,
    pub function_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetComplexityOutput {
    pub file_path: String,
    pub complexity: ComplexityMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub lines_of_code: u32,
    pub parameter_count: u32,
    pub nesting_depth: u32,
    pub function_name: Option<String>,
}

// ============================================================================
// Get Entry Points
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetEntryPointsInput {
    /// Return compressed natural language summary instead of JSON (default: false)
    #[serde(default = "default_compressed")]
    pub compressed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetEntryPointsOutput {
    pub entry_points: Vec<SymbolInfo>,
    pub total: usize,
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Get Leaf Functions
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetLeafFunctionsInput {
    /// Return compressed natural language summary instead of JSON (default: false)
    #[serde(default = "default_compressed")]
    pub compressed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetLeafFunctionsOutput {
    pub leaf_functions: Vec<SymbolInfo>,
    pub total: usize,
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Trace Path
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TracePathInput {
    /// Source symbol name (function or method)
    pub source: String,

    /// Target symbol name (function or method)
    pub target: String,

    /// Maximum depth for path search (default: 10)
    #[serde(default = "default_depth")]
    pub max_depth: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TracePathOutput {
    pub source: String,
    pub target: String,
    pub path_found: bool,
    pub path: Vec<PathEntry>,
    pub path_length: usize,
    pub metadata: AnalysisMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

// ============================================================================
// Export Mermaid
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportMermaidInput {
    /// Subgraph root symbol (optional - if not provided, exports entire graph)
    pub root_symbol: Option<String>,

    /// Maximum depth for traversal (default: 3)
    #[serde(default = "default_depth")]
    pub max_depth: u8,

    /// Include external dependencies (crates/packages)
    #[serde(default = "default_false")]
    pub include_external: bool,

    /// Theme for SVG rendering. If provided, renders the diagram as SVG.
    /// Available: catppuccin-mocha, catppuccin-latte, dracula, tokyo-night, tokyo-night-light,
    /// tokyo-night-storm, nord, nord-light, github-light, github-dark, solarized-light,
    /// solarized-dark, one-dark, zinc-dark. Default when rendering: tokyo-night-light.
    pub theme: Option<String>,

    /// Output format: "code" (default, returns mermaid source) or "svg" (renders to SVG).
    /// If omitted and theme is provided, defaults to "svg".
    pub format: Option<String>,

    /// Filter symbols by file path substring match (case-sensitive).
    /// Only symbols whose source file path contains this substring will be included.
    /// Example: "handlers" matches "src/handlers/auth.rs". None means no filtering.
    #[serde(default)]
    pub module_filter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportMermaidOutput {
    pub mermaid_code: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub metadata: AnalysisMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
}

// ============================================================================
// Get Hot Paths
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetHotPathsInput {
    /// Number of hot paths to return (default: 10)
    #[serde(default = "default_hot_paths_limit")]
    pub limit: usize,

    /// Minimum fan-in threshold (default: 2)
    #[serde(default = "default_hot_paths_min_fan_in")]
    pub min_fan_in: usize,
}

fn default_hot_paths_limit() -> usize {
    10
}

fn default_hot_paths_min_fan_in() -> usize {
    2
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetHotPathsOutput {
    pub hot_paths: Vec<HotPathEntry>,
    pub total: usize,
    pub metadata: AnalysisMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPathEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub fan_in: usize,
    pub fan_out: usize,
}

// ============================================================================
// Dead Code Detection
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FindDeadCodeInput {
    /// Maximum number of entries to return (default: 50)
    #[serde(default = "default_dead_code_limit")]
    pub limit: usize,
}

fn default_dead_code_limit() -> usize {
    50
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindDeadCodeOutput {
    pub dead_code: Vec<DeadCodeEntry>,
    pub total_dead: usize,
    pub total_symbols: usize,
    pub dead_code_percent: f32,
    pub metadata: AnalysisMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadCodeEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub kind: String,
    pub reason: String,
    pub confidence: f32,
}

// ============================================================================
// Module Dependencies
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetModuleDependenciesInput {
    /// Maximum number of modules to return (default: 100)
    #[serde(default = "default_module_limit")]
    pub limit: usize,
}

fn default_module_limit() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub module: String,
    pub depends_on: Vec<String>,
    pub depended_by: Vec<String>,
    pub coupling_score: usize,
    pub stability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependencyGraph {
    pub modules: Vec<ModuleDependency>,
    pub cycles: Vec<Vec<String>>,
    pub coupling_matrix: Vec<(String, String, usize)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetModuleDependenciesOutput {
    pub graph: ModuleDependencyGraph,
    pub total_modules: usize,
    pub total_cross_module_edges: usize,
    pub cycle_count: usize,
    pub metadata: AnalysisMetadata,
}

// ============================================================================
// Graph Strategy - Build Index
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildIndexInput {
    /// Directory to build the index for (default: current working directory)
    pub directory: Option<String>,

    /// Strategy to use: lightweight, on_demand, per_file, full
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_strategy() -> String {
    "lightweight".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildIndexOutput {
    pub success: bool,
    pub strategy: String,
    pub symbols_indexed: usize,
    pub locations_indexed: usize,
    pub message: String,
}

// ============================================================================
// Graph Strategy - Query Symbol Index
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct QuerySymbolInput {
    /// Symbol name to query (case-insensitive)
    pub symbol_name: String,

    /// Directory to search in (default: current working directory)
    pub directory: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuerySymbolOutput {
    pub symbol_name: String,
    pub locations: Vec<SymbolLocationEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocationEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub symbol_kind: String,
}

// ============================================================================
// Graph Strategy - Build Call Subgraph (On-Demand)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildSubgraphInput {
    /// Symbol name to build subgraph around
    pub symbol_name: String,

    /// Traversal depth (default: 3)
    #[serde(default = "default_subgraph_depth")]
    pub depth: u32,

    /// Direction: in, out, or both (default: both)
    #[serde(default = "default_subgraph_direction")]
    pub direction: SubgraphDirection,

    /// Directory to search in (default: current working directory)
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubgraphDirection {
    In,
    Out,
    Both,
}

fn default_subgraph_depth() -> u32 {
    3
}

fn default_subgraph_direction() -> SubgraphDirection {
    SubgraphDirection::Both
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildSubgraphOutput {
    pub symbol_name: String,
    pub root: HierarchySymbolInfo,
    pub entries: Vec<HierarchyEntryInfo>,
    pub total_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchySymbolInfo {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub symbol_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyEntryInfo {
    pub symbol: HierarchySymbolInfo,
    pub depth: u32,
    pub direction: String,
}

// ============================================================================
// Graph Strategy - Per-File Graph
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPerFileGraphInput {
    /// File path to get graph for
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPerFileGraphOutput {
    pub file_path: String,
    pub symbols: Vec<SymbolLocationEntry>,
    pub symbol_count: usize,
    pub dependencies: Vec<DependencyInfo>,
    pub dependency_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub caller: String,
    pub caller_file: String,
    pub caller_line: u32,
    pub callee: String,
}

// ============================================================================
// Graph Strategy - Merge Graphs
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeGraphsInput {
    /// List of file paths to merge
    pub file_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeGraphsOutput {
    pub file_count: usize,
    pub merged_symbol_count: usize,
    pub merged_dependency_count: usize,
    pub symbols: Vec<SymbolLocationEntry>,
    pub dependencies: Vec<DependencyInfo>,
}

// ============================================================================
// MCP Protocol Types
// ============================================================================

/// Standard JSON-RPC 2.0 request
/// Uses `Option<serde_json::Value>` for `id` to support string, number, and null IDs per the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
    pub id: Option<serde_json::Value>,
}

impl McpResponse {
    /// Creates a successful response.
    pub fn success(result: serde_json::Value, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Creates an error response.
    pub fn error_response(error: McpError, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl McpError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new(-32600, msg)
    }

    pub fn method_not_found(msg: impl Into<String>) -> Self {
        Self::new(-32601, msg)
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(-32602, msg)
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::new(-32603, msg)
    }

    /// Alias for `internal_error` for backward compatibility.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::internal_error(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_direction_serialization() {
        let incoming = CallDirection::Incoming;
        let json = serde_json::to_string(&incoming).unwrap();
        assert_eq!(json, "\"incoming\"");

        let outgoing = CallDirection::Outgoing;
        let json = serde_json::to_string(&outgoing).unwrap();
        assert_eq!(json, "\"outgoing\"");
    }

    #[test]
    fn test_call_hierarchy_input_defaults() {
        let json = r#"{"symbol_name": "test::func", "direction": "outgoing"}"#;
        let input: GetCallHierarchyInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.depth, 1);
        assert!(!input.include_external);
    }

    #[test]
    fn test_mcp_error_factory_methods() {
        let err = McpError::invalid_request("bad request");
        assert_eq!(err.code, -32600);

        let err = McpError::method_not_found("unknown method");
        assert_eq!(err.code, -32601);

        let err = McpError::invalid_params("wrong params");
        assert_eq!(err.code, -32602);

        let err = McpError::internal_error("oops");
        assert_eq!(err.code, -32603);
    }

    #[test]
    fn test_build_index_input_defaults() {
        let json = r#"{}"#;
        let input: BuildIndexInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.strategy, "lightweight");
        assert!(input.directory.is_none());
    }

    #[test]
    fn test_build_index_input_with_values() {
        let json = r#"{"directory": "/path/to/project", "strategy": "on_demand"}"#;
        let input: BuildIndexInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.directory, Some("/path/to/project".to_string()));
        assert_eq!(input.strategy, "on_demand");
    }

    #[test]
    fn test_query_symbol_input() {
        let json = r#"{"symbol_name": "my_function", "directory": "/path/to/project"}"#;
        let input: QuerySymbolInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.symbol_name, "my_function");
        assert_eq!(input.directory, Some("/path/to/project".to_string()));
    }

    #[test]
    fn test_build_subgraph_input_defaults() {
        let json = r#"{"symbol_name": "test_func"}"#;
        let input: BuildSubgraphInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.depth, 3);
        assert!(matches!(input.direction, SubgraphDirection::Both));
    }

    #[test]
    fn test_build_subgraph_input_directions() {
        let json_in = r#"{"symbol_name": "test", "direction": "in"}"#;
        let input_in: BuildSubgraphInput = serde_json::from_str(json_in).unwrap();
        assert!(matches!(input_in.direction, SubgraphDirection::In));

        let json_out = r#"{"symbol_name": "test", "direction": "out"}"#;
        let input_out: BuildSubgraphInput = serde_json::from_str(json_out).unwrap();
        assert!(matches!(input_out.direction, SubgraphDirection::Out));
    }

    #[test]
    fn test_merge_graphs_input() {
        let json = r#"{"file_paths": ["a.py", "b.py", "c.py"]}"#;
        let input: MergeGraphsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.file_paths.len(), 3);
    }

    #[test]
    fn test_dependency_info_serialization() {
        let dep = DependencyInfo {
            caller: "func_a".to_string(),
            caller_file: "test.py".to_string(),
            caller_line: 10,
            callee: "func_b".to_string(),
        };
        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("func_a"));
        assert!(json.contains("func_b"));
    }

    #[test]
    fn test_hierarchy_symbol_info_serialization() {
        let info = HierarchySymbolInfo {
            name: "test_func".to_string(),
            file: "test.py".to_string(),
            line: 5,
            column: 10,
            symbol_kind: "Function".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test_func"));
        assert!(json.contains("Function"));
    }
}

// ============================================================================
// LSP Navigation Operations
// ============================================================================

/// Input for go_to_definition tool
#[derive(Debug, Serialize, Deserialize)]
pub struct GoToDefinitionInput {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

/// Output for go_to_definition tool
#[derive(Debug, Serialize, Deserialize)]
pub struct GoToDefinitionOutput {
    pub found: bool,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub context: Option<String>,
    pub message: Option<String>,
}

/// Input for hover tool
#[derive(Debug, Serialize, Deserialize)]
pub struct HoverInput {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

/// Output for hover tool
#[derive(Debug, Serialize, Deserialize)]
pub struct HoverOutput {
    pub found: bool,
    pub content: Option<String>,
    pub documentation: Option<String>,
    pub kind: Option<String>,
}

/// Input for find_references tool
#[derive(Debug, Serialize, Deserialize)]
pub struct FindReferencesInput {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    #[serde(default = "default_true")]
    pub include_declaration: bool,
}

/// Output for find_references tool
#[derive(Debug, Serialize, Deserialize)]
pub struct FindReferencesOutput {
    pub symbol: String,
    pub references: Vec<ReferenceEntry>,
    pub total: usize,
}

/// A single reference entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub kind: String,
    pub context: String,
}

// ============================================================================
// Hierarchical Outline
// ============================================================================

/// Input for get_outline tool
#[derive(Debug, Serialize, Deserialize)]
pub struct OutlineInput {
    /// Path to the source file
    pub file_path: String,

    /// Include private symbols (starting with _) (default: true)
    #[serde(default = "default_true")]
    pub include_private: bool,

    /// Include test symbols (starting with test_) (default: true)
    #[serde(default = "default_true")]
    pub include_tests: bool,
}

/// Output for get_outline tool
#[derive(Debug, Serialize, Deserialize)]
pub struct OutlineOutput {
    pub file_path: String,
    pub nodes: Vec<OutlineNodeDto>,
    pub total_nodes: usize,
    pub generation_time_ms: u64,
}

/// DTO for outline node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNodeDto {
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub column: u32,
    pub signature: Option<String>,
    pub children: Vec<OutlineNodeDto>,
    pub is_private: bool,
}

// ============================================================================
// Symbol Code Retrieval
// ============================================================================

/// Input for get_symbol_code tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolCodeInput {
    /// Path to the source file
    pub file: String,

    /// Line number (1-indexed)
    pub line: u32,

    /// Column number (0-indexed)
    pub col: u32,
}

/// Output for get_symbol_code tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolCodeOutput {
    pub file: String,
    pub code: String,
    pub docstring: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub cached: bool,
}

// ============================================================================
// Semantic Search
// ============================================================================

/// Input for semantic_search tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticSearchInput {
    /// Search query string
    pub query: String,

    /// Optional filter for symbol kinds
    pub kinds: Option<Vec<String>>,

    /// Maximum number of results (default: 50)
    #[serde(default = "default_search_max_results")]
    pub max_results: usize,
}

fn default_search_max_results() -> usize {
    50
}

/// Output for semantic_search tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticSearchOutput {
    pub query: String,
    pub results: Vec<SearchResultDto>,
    pub total: usize,
    pub search_time_ms: u64,
}

/// DTO for search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultDto {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub score: f32,
    pub match_type: String,
}

// ============================================================================
// Find Usages with Context
// ============================================================================

/// Input for find_usages_with_context tool
#[derive(Debug, Serialize, Deserialize)]
pub struct FindUsagesWithContextInput {
    /// Symbol name to search
    pub symbol: String,

    /// Number of context lines around each reference (default: 3)
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,

    /// Include the declaration (default: true)
    #[serde(default = "default_true")]
    pub include_declaration: bool,
}

fn default_context_lines() -> u32 {
    3
}

/// Output for find_usages_with_context tool
#[derive(Debug, Serialize, Deserialize)]
pub struct FindUsagesWithContextOutput {
    pub symbol: String,
    pub usages: Vec<UsageWithContextEntry>,
    pub total: usize,
}

/// Entry with surrounding context lines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWithContextEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub context_lines: ContextLines,
    pub is_definition: bool,
}

/// Surrounding source lines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLines {
    pub before: Vec<String>,
    pub current: String,
    pub after: Vec<String>,
}

// ============================================================================
// File Operations (LLM File Tools)
// ============================================================================

/// Input for read_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileInput {
    /// Path to the file to read (required)
    pub path: String,

    /// Start line for partial read (1-indexed, default: 1)
    pub start_line: Option<u32>,

    /// End line for partial read (1-indexed, default: last line)
    pub end_line: Option<u32>,

    /// Read mode: raw, outline, symbols, compressed (default: raw)
    pub mode: Option<String>,

    /// Chunk size for streaming reads (optional)
    pub chunk_size: Option<usize>,

    /// Continuation token for pagination (optional)
    pub continuation_token: Option<String>,
}

#[allow(dead_code)]
fn default_read_mode() -> String {
    "raw".to_string()
}

/// Output for read_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileOutput {
    /// File content (may be truncated)
    pub content: String,

    /// Total number of lines in the file
    pub total_lines: u32,

    /// Whether the output was truncated
    pub truncated: bool,

    /// File metadata
    pub metadata: FileMetadata,

    /// The mode used (raw, outline, symbols, compressed)
    pub mode: String,

    /// The first line number returned (1-indexed)
    pub start_line: u32,

    /// The last line number returned (1-indexed)
    pub end_line: u32,

    /// Whether there is more content to read
    pub has_more: bool,

    /// Token for reading the next chunk (base64 encoded continuation token)
    pub next_token: Option<String>,

    /// Suggested chunk size for future reads
    pub suggested_chunk_size: Option<usize>,
}

/// File metadata for file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Absolute path to the file
    pub path: String,

    /// File size in bytes
    pub size: u64,

    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,

    /// Detected programming language (if applicable)
    pub language: Option<String>,
}

/// Input for edit_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct EditFileInput {
    /// Path to the file to edit (required)
    pub path: String,

    /// Edits to apply (required)
    pub edits: Vec<FileEdit>,
}

/// A single file edit operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    /// The exact text to replace (required)
    pub old_string: String,

    /// The replacement text (required)
    pub new_string: String,
}

/// Output for edit_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct EditFileOutput {
    /// Whether the edit was applied successfully
    pub applied: bool,

    /// Validation result
    pub validation: EditValidation,

    /// Optional preview of the change (for confirmation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,

    /// Number of bytes changed (absolute difference)
    pub bytes_changed: u64,

    /// Reason for rejection or non-application (if not applied)
    /// Possible values: "no_match", "syntax_rejected", or None if applied successfully
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Validation result for edit operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditValidation {
    /// Whether syntax validation passed
    pub passed: bool,

    /// List of syntax errors (empty if passed)
    #[serde(default)]
    pub syntax_errors: Vec<SyntaxError>,
}

/// Input for write_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileInput {
    /// Path to the file to write (required)
    pub path: String,

    /// Content to write (required)
    pub content: String,

    /// Whether to create parent directories if they don't exist (default: false)
    pub create_dirs: Option<bool>,
}

#[allow(dead_code)]
fn default_create_dirs() -> bool {
    false
}

/// Output for write_file tool
#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileOutput {
    /// Number of bytes written
    pub bytes_written: u64,

    /// File metadata
    pub metadata: FileMetadata,
}

/// Input for search_content tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchContentInput {
    /// Search pattern (required)
    pub pattern: String,

    /// Path to search within (optional, defaults to workspace root)
    pub path: Option<String>,

    /// Glob pattern to filter files (e.g., "*.rs")
    pub file_glob: Option<String>,

    /// Whether to treat pattern as regex (default: true)
    pub regex: Option<bool>,

    /// Case insensitive search (default: false)
    pub case_insensitive: Option<bool>,

    /// Maximum number of results to return (default: 50)
    pub max_results: Option<usize>,

    /// Number of context lines around matches (default: 2)
    pub context_lines: Option<u32>,
}

#[allow(dead_code)]
fn default_regex() -> bool {
    true
}

#[allow(dead_code)]
fn default_case_insensitive() -> bool {
    false
}

#[allow(dead_code)]
fn default_search_context_lines() -> u32 {
    2
}

/// Output for search_content tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchContentOutput {
    /// Matching lines
    pub matches: Vec<ContentMatch>,

    /// Total number of matches found
    pub total: usize,

    /// Number of files scanned
    pub files_scanned: usize,
}

/// A single content match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMatch {
    /// File path containing the match
    pub file: String,

    /// Line number (1-indexed)
    pub line: u32,

    /// Column number (1-indexed)
    pub col: u32,

    /// The matching text
    pub text: String,

    /// Surrounding context lines
    pub context: Vec<String>,
}

/// Input for list_files tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ListFilesInput {
    /// Path to list (optional, defaults to workspace root)
    pub path: Option<String>,

    /// Glob pattern to filter results (e.g., "**/*.rs")
    pub glob: Option<String>,

    /// Pagination offset (default: 0)
    pub offset: Option<usize>,

    /// Maximum number of results (default: 100)
    pub limit: Option<usize>,

    /// Whether to list files recursively (default: true)
    pub recursive: Option<bool>,

    /// Maximum depth for recursive traversal. None means unlimited.
    /// Only effective when recursive is true.
    pub max_depth: Option<usize>,
}

#[allow(dead_code)]
fn default_list_offset() -> usize {
    0
}

#[allow(dead_code)]
fn default_list_limit() -> usize {
    100
}

/// Output for list_files tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ListFilesOutput {
    /// List of file entries
    pub files: Vec<FileEntry>,

    /// Total number of files (before pagination)
    pub total: usize,

    /// The maximum depth reached during traversal
    pub depth_traversed: Option<usize>,
}

/// A single file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File path
    pub path: String,

    /// File size in bytes
    pub size: u64,

    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,

    /// Whether this is a directory
    pub is_dir: bool,

    /// Detected programming language (if applicable)
    pub language: Option<String>,
}

// ============================================================================
// AIX Tool Input Schemas
// ============================================================================

// AIX-1: Smart Overview & Ranked Symbols

/// Input for smart_overview tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SmartOverviewInput {
    /// Detail level: quick (~100 tokens), medium (~400 tokens), detailed (~800 tokens)
    #[serde(default)]
    pub detail: Option<OverviewDetail>,
}

fn default_ranked_limit() -> usize {
    50
}

/// Input for ranked_symbols tool
#[derive(Debug, Serialize, Deserialize)]
pub struct RankedSymbolsInput {
    /// Search query string
    pub query: String,
    /// Maximum number of results to return
    #[serde(default = "default_ranked_limit")]
    pub limit: usize,
}

fn default_hot_symbols_limit() -> usize {
    20
}

/// Input for get_hot_symbols tool (PL3)
#[derive(Debug, Serialize, Deserialize)]
pub struct GetHotSymbolsInput {
    /// Maximum number of hot symbols to return
    #[serde(default = "default_hot_symbols_limit")]
    pub limit: usize,
}

// AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan

/// Goal for onboarding plan
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingGoalDetail {
    Understand,
    Refactor,
    Debug,
    AddFeature,
    Review,
}

fn default_onboarding_goal() -> OnboardingGoalDetail {
    OnboardingGoalDetail::Understand
}

/// Input for suggest_onboarding_plan tool
#[derive(Debug, Serialize, Deserialize)]
pub struct OnboardingPlanInput {
    /// Goal for the onboarding plan
    #[serde(default = "default_onboarding_goal")]
    pub goal: OnboardingGoalDetail,
}

fn default_diagnose_target() -> Option<String> {
    None
}

fn default_min_severity() -> String {
    "important".to_string()
}

/// Input for auto_diagnose tool
#[derive(Debug, Serialize, Deserialize)]
pub struct AutoDiagnoseInput {
    /// Optional target directory to diagnose
    #[serde(default = "default_diagnose_target")]
    pub target: Option<String>,
    /// Minimum severity level to report
    #[serde(default = "default_min_severity")]
    pub min_severity: String,
}

fn default_refactor_goal() -> String {
    "reduce_complexity".to_string()
}

fn default_max_steps() -> usize {
    5
}

/// Input for suggest_refactor_plan tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SuggestRefactorPlanInput {
    /// Target symbol to refactor
    pub symbol: String,
    /// Goal for refactoring
    #[serde(default = "default_refactor_goal")]
    pub goal: String,
    /// Maximum number of steps in the plan
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
}

// AIX-3: NL to Symbol & Ask About Code & Find Pattern

fn default_nl_limit() -> usize {
    20
}

/// Input for nl_to_symbol tool
#[derive(Debug, Serialize, Deserialize)]
pub struct NlToSymbolInput {
    /// Natural language query
    pub query: String,
    /// Maximum number of results
    #[serde(default = "default_nl_limit")]
    pub limit: usize,
}

fn default_ask_limit() -> usize {
    10
}

/// Input for ask_about_code tool
#[derive(Debug, Serialize, Deserialize)]
pub struct AskAboutCodeInput {
    /// Question about code flow
    pub question: String,
    /// Maximum number of answers
    #[serde(default = "default_ask_limit")]
    pub limit: usize,
}

/// Input for find_pattern_by_intent tool
#[derive(Debug, Serialize, Deserialize)]
pub struct FindPatternByIntentInput {
    /// Natural language intent description (optional when listing all patterns)
    #[serde(default)]
    pub intent: String,
    /// Whether to list all available patterns
    #[serde(default)]
    pub list_patterns: Option<bool>,
}

// AIX-4: Compare Call Graphs & Detect API Breaks

/// Input for compare_call_graphs tool
#[derive(Debug, Serialize, Deserialize)]
pub struct CompareCallGraphsInput {
    /// Optional baseline directory to compare against
    #[serde(default)]
    pub baseline_dir: Option<String>,
}

fn default_api_break_severity() -> String {
    "minor".to_string()
}

/// Input for detect_api_breaks tool
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectApiBreaksInput {
    /// Optional baseline directory to compare against
    #[serde(default)]
    pub baseline_dir: Option<String>,
    /// Minimum severity to report
    #[serde(default = "default_api_break_severity")]
    pub min_severity: String,
}

/// Input for evaluate_refactor_quality tool (AIX-4.3)
/// No parameters needed - compares current graph state vs persisted baseline
#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluateRefactorQualityInput {}

// AIX-5: System Prompt Context & God Functions & Long Params

/// Format detail for system prompt context
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContextFormatDetail {
    Xml,
    Json,
    Markdown,
}

fn default_context_format() -> ContextFormatDetail {
    ContextFormatDetail::Xml
}

/// Input for generate_system_prompt_context tool
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPromptContextInput {
    /// Output format
    #[serde(default = "default_context_format")]
    pub format: ContextFormatDetail,
    /// Whether to include architecture info
    #[serde(default)]
    pub include_architecture: Option<bool>,
    /// Whether to include hot paths
    #[serde(default)]
    pub include_hot_paths: Option<bool>,
}

fn default_god_min_lines() -> usize {
    50
}

fn default_god_min_complexity() -> u32 {
    15
}

fn default_god_min_fan_in() -> usize {
    5
}

/// Input for detect_god_functions tool
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectGodFunctionsInput {
    /// Minimum lines of code threshold
    #[serde(default = "default_god_min_lines")]
    pub min_lines: usize,
    /// Minimum cyclomatic complexity threshold
    #[serde(default = "default_god_min_complexity")]
    pub min_complexity: u32,
    /// Minimum fan-in threshold
    #[serde(default = "default_god_min_fan_in")]
    pub min_fan_in: usize,
}

fn default_max_params() -> usize {
    5
}

/// Input for detect_long_parameter_lists tool
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectLongParamsInput {
    /// Maximum number of parameters allowed
    #[serde(default = "default_max_params")]
    pub max_params: usize,
}
