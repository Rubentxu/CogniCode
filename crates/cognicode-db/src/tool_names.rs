//! Shared tool-name constants for MCP tool telemetry
//!
//! Prevents hidden meaning connascence by using shared constants
//! rather than raw strings across Phase 3A (telemetry) and Phase 3B (dashboard).

/// Tool name for suggest_context MCP tool
pub const TOOL_SUGGEST_CONTEXT: &str = "suggest_context";

/// Tool name for reparse_on_edit MCP tool
pub const TOOL_REPARSE_ON_EDIT: &str = "reparse_on_edit";

/// Tool name for build_graph MCP tool
pub const TOOL_BUILD_GRAPH: &str = "build_graph";

/// Tool name for get_call_hierarchy MCP tool
pub const TOOL_GET_CALL_HIERARCHY: &str = "get_call_hierarchy";

/// Tool name for analyze_impact MCP tool
pub const TOOL_ANALYZE_IMPACT: &str = "analyze_impact";

/// Tool name for check_architecture MCP tool
pub const TOOL_CHECK_ARCHITECTURE: &str = "check_architecture";

/// Tool name for find_usages MCP tool
pub const TOOL_FIND_USAGES: &str = "find_usages";

/// Tool name for get_complexity MCP tool
pub const TOOL_GET_COMPLEXITY: &str = "get_complexity";

/// Tool name for semantic_search MCP tool
pub const TOOL_SEMANTIC_SEARCH: &str = "semantic_search";

/// Tool name for smart_overview MCP tool
pub const TOOL_SMART_OVERVIEW: &str = "smart_overview";

/// Tool name for ranked_symbols MCP tool
pub const TOOL_RANKED_SYMBOLS: &str = "ranked_symbols";

/// Tool name for generate_contract MCP tool
pub const TOOL_GENERATE_CONTRACT: &str = "generate_contract";

/// Tool name for validate_contract MCP tool
pub const TOOL_VALIDATE_CONTRACT: &str = "validate_contract";

/// Tool name for detect_drift MCP tool
pub const TOOL_DETECT_DRIFT: &str = "detect_drift";

/// All known tool names as a slice (useful for validation)
pub const ALL_TOOL_NAMES: &[&str] = &[
    TOOL_SUGGEST_CONTEXT,
    TOOL_REPARSE_ON_EDIT,
    TOOL_BUILD_GRAPH,
    TOOL_GET_CALL_HIERARCHY,
    TOOL_ANALYZE_IMPACT,
    TOOL_CHECK_ARCHITECTURE,
    TOOL_FIND_USAGES,
    TOOL_GET_COMPLEXITY,
    TOOL_SEMANTIC_SEARCH,
    TOOL_SMART_OVERVIEW,
    TOOL_RANKED_SYMBOLS,
    TOOL_GENERATE_CONTRACT,
    TOOL_VALIDATE_CONTRACT,
    TOOL_DETECT_DRIFT,
];

/// Check if a tool name is a known/valid tool name
pub fn is_known_tool_name(name: &str) -> bool {
    ALL_TOOL_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_tool_names() {
        assert!(is_known_tool_name(TOOL_SUGGEST_CONTEXT));
        assert!(is_known_tool_name(TOOL_REPARSE_ON_EDIT));
        assert!(is_known_tool_name("build_graph"));
        assert!(!is_known_tool_name("unknown_tool"));
    }

    #[test]
    fn test_tool_name_constants_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for name in ALL_TOOL_NAMES {
            assert!(seen.insert(*name), "Duplicate tool name: {}", name);
        }
    }
}
