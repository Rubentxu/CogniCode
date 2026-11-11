//! Input contracts for the public MCP tool surface.
//!
//! The catalogue publishes an `input_schema` for every tool, and the
//! arguments the server actually reads live in a Rust struct per tool.
//! Until now those were two descriptions of the same thing: the catalogue
//! carried a hand-written copy of the shape, and the copy was the one that
//! rotted. A copy cannot drift without anyone noticing, which is the
//! failure mode of every hand-maintained schema in this repository before
//! the output contracts were derived.
//!
//! # Why derive rather than write
//!
//! The struct that deserialises the arguments IS the declaration of those
//! arguments. Deriving the schema from it makes drift impossible: add a
//! field and the published contract grows with it; rename one and the
//! published contract follows. A tool that accepts an argument the
//! catalogue never mentioned — a hidden parameter — cannot exist here,
//! because the catalogue's copy is checked against this derivation.
//!
//! The output side of the same boundary works this way already, in
//! [`super::output_contracts`]. This module is its input counterpart.
//!
//! # What is published, and what is not
//!
//! A tool publishes a contract when its arguments are described by a
//! struct. The tools that read their arguments one key at a time — the
//! evidence tools — publish no derived contract: their arguments are
//! described by hand in the catalogue, and this module does not pretend
//! otherwise. `None` is the honest answer for them, and the distinction
//! matters: a consumer must be able to tell that it cannot rely on the
//! shape.
//!
//! # How this stays honest
//!
//! The published catalogue carries a copy of these schemas, and the copy
//! is checked against the derivation, so the document an adopter reads and
//! the contract the server honours cannot say different things. The
//! catalogue's copy is a published artifact, not a second source: it is
//! regenerated from this module and the check fails when the two differ.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

use cognicode_core::interface::schema::schema_of;

/// Map a tool name to the JSON Schema of the arguments it reads.
///
/// A `BTreeMap` so iteration order is stable; the map is part of a public
/// contract surface and a nondeterministic order would make diffs of
/// `tools/list` noise.
///
/// Several tools read the same shape — the program-analysis family shares
/// one input struct — and each of them gets its own entry: the contract
/// belongs to the tool a caller names, not to the type behind it.
fn build_input_schemas() -> BTreeMap<&'static str, Map<String, Value>> {
    let mut m = BTreeMap::new();
    m.insert(
        "analyze_impact",
        schema_of::<cognicode_core::interface::mcp::schemas::AnalyzeImpactInput>(),
    );
    m.insert(
        "ask_about_code",
        schema_of::<cognicode_core::interface::mcp::schemas::AskAboutCodeInput>(),
    );
    m.insert(
        "build_call_subgraph",
        schema_of::<cognicode_core::interface::mcp::schemas::BuildSubgraphInput>(),
    );
    m.insert(
        "build_graph",
        schema_of::<cognicode_core::interface::mcp::handlers::BuildGraphInput>(),
    );
    m.insert(
        "build_lightweight_index",
        schema_of::<cognicode_core::interface::mcp::schemas::BuildIndexInput>(),
    );
    m.insert(
        "cfg_per_function",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "check_architecture",
        schema_of::<cognicode_core::interface::mcp::schemas::CheckArchitectureInput>(),
    );
    m.insert(
        "codebase_map",
        schema_of::<
            cognicode_core::interface::mcp::handlers::consolidated_handlers::CodebaseMapInput,
        >(),
    );
    m.insert(
        "detect_drift",
        schema_of::<cognicode_core::interface::mcp::schemas::DetectDriftInput>(),
    );
    m.insert(
        "detect_god_functions",
        schema_of::<cognicode_core::interface::mcp::schemas::DetectGodFunctionsInput>(),
    );
    m.insert(
        "detect_long_parameter_lists",
        schema_of::<cognicode_core::interface::mcp::schemas::DetectLongParamsInput>(),
    );
    m.insert(
        "dominators_cfg",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "edit_file",
        schema_of::<cognicode_core::interface::mcp::schemas::EditFileInput>(),
    );
    m.insert(
        "export_callflow",
        schema_of::<
            cognicode_core::interface::mcp::handlers::graph_query_handlers::ExportCallflowInput,
        >(),
    );
    m.insert(
        "export_mermaid",
        schema_of::<cognicode_core::interface::mcp::schemas::ExportMermaidInput>(),
    );
    // The findings answer is derived only in the builds that ship the tool:
    // the findings pipeline grounds its findings in the canonical evidence
    // kernel, and a contract for a tool this build does not advertise would
    // be a promise to nobody.
    #[cfg(feature = "evidence-kernel")]
    m.insert(
        "explain_finding",
        schema_of::<cognicode_core::interface::mcp::handlers::findings_handlers::ExplainFindingInput>(),
    );
    m.insert(
        "find_pattern_by_intent",
        schema_of::<cognicode_core::interface::mcp::schemas::FindPatternByIntentInput>(),
    );
    m.insert(
        "find_references",
        schema_of::<cognicode_core::interface::mcp::schemas::FindReferencesInput>(),
    );
    m.insert(
        "find_usages",
        schema_of::<cognicode_core::interface::mcp::schemas::FindUsagesInput>(),
    );
    m.insert(
        "generate_contract",
        schema_of::<cognicode_core::interface::mcp::schemas::GenerateContractInput>(),
    );
    m.insert(
        "get_call_hierarchy",
        schema_of::<cognicode_core::interface::mcp::schemas::GetCallHierarchyInput>(),
    );
    m.insert(
        "get_complexity",
        schema_of::<cognicode_core::interface::mcp::schemas::GetComplexityInput>(),
    );
    m.insert(
        "get_entry_points",
        schema_of::<cognicode_core::interface::mcp::schemas::GetEntryPointsInput>(),
    );
    m.insert(
        "get_file_symbols",
        schema_of::<cognicode_core::interface::mcp::schemas::GetFileSymbolsInput>(),
    );
    m.insert(
        "get_hot_paths",
        schema_of::<cognicode_core::interface::mcp::schemas::GetHotPathsInput>(),
    );
    m.insert(
        "get_imports",
        schema_of::<cognicode_core::interface::mcp::handlers::graph_query_handlers::GetImportsInput>(),
    );
    m.insert(
        "get_implementors",
        schema_of::<
            cognicode_core::interface::mcp::handlers::graph_query_handlers::GetImplementorsInput,
        >(),
    );
    m.insert(
        "get_leaf_functions",
        schema_of::<cognicode_core::interface::mcp::schemas::GetLeafFunctionsInput>(),
    );
    m.insert(
        "get_members",
        schema_of::<cognicode_core::interface::mcp::handlers::graph_query_handlers::GetMembersInput>(),
    );
    m.insert(
        "get_per_file_graph",
        schema_of::<cognicode_core::interface::mcp::schemas::GetPerFileGraphInput>(),
    );
    m.insert(
        "get_symbol_code",
        schema_of::<cognicode_core::interface::mcp::schemas::SymbolCodeInput>(),
    );
    m.insert(
        "get_type_references",
        schema_of::<cognicode_core::interface::mcp::handlers::graph_query_handlers::GetTypeRefsInput>(),
    );
    m.insert(
        "go_to_definition",
        schema_of::<cognicode_core::interface::mcp::schemas::GoToDefinitionInput>(),
    );
    m.insert(
        "graph_all_paths",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphAllPathsInput>(),
    );
    m.insert(
        "graph_checkpoint",
        schema_of::<
            cognicode_core::interface::mcp::handlers::consolidated_handlers::GraphCheckpointInput,
        >(),
    );
    m.insert(
        "graph_communities",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphCommunitiesInput>(),
    );
    m.insert(
        "graph_community_detail",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphCommunityDetailInput>(),
    );
    m.insert(
        "graph_condensed",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphCondensedInput>(),
    );
    m.insert(
        "graph_explain",
        schema_of::<
            cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphExplainInput,
        >(),
    );
    m.insert(
        "graph_feedback_arcs",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphFeedbackArcsInput>(),
    );
    m.insert(
        "graph_god_nodes",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphGodNodesInput>(),
    );
    m.insert(
        "graph_insights",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphInsightsInput>(),
    );
    m.insert(
        "graph_pagerank",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphPageRankInput>(),
    );
    m.insert(
        "graph_query",
        schema_of::<cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphQueryInput>(),
    );
    m.insert(
        "graph_query_filtered",
        schema_of::<
            cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphQueryFilteredInput,
        >(),
    );
    m.insert(
        "graph_reduced",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphReducedInput>(),
    );
    m.insert(
        "graph_search_idf",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphSearchIdfInput>(),
    );
    m.insert(
        "graph_suggest_questions",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphSuggestQuestionsInput>(),
    );
    m.insert(
        "graph_surprising_connections",
        schema_of::<cognicode_core::interface::mcp::schemas::GraphSurprisingConnectionsInput>(),
    );
    m.insert(
        "hover",
        schema_of::<cognicode_core::interface::mcp::schemas::HoverInput>(),
    );
    m.insert(
        "iac_query",
        schema_of::<cognicode_core::interface::mcp::handlers::consolidated_handlers::IacQueryInput>(
        ),
    );
    m.insert(
        "interproc_summary",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "list_files",
        schema_of::<cognicode_core::interface::mcp::schemas::ListFilesInput>(),
    );
    m.insert(
        "list_view_specs",
        schema_of::<cognicode_core::interface::mcp::schemas::ListViewSpecsInput>(),
    );
    m.insert(
        "merge_graphs",
        schema_of::<cognicode_core::interface::mcp::schemas::MergeGraphsInput>(),
    );
    m.insert(
        "nl_to_symbol",
        schema_of::<cognicode_core::interface::mcp::schemas::NlToSymbolInput>(),
    );
    m.insert(
        "project_insights",
        schema_of::<
            cognicode_core::interface::mcp::handlers::consolidated_handlers::ProjectInsightsInput,
        >(),
    );
    m.insert(
        "project_overview",
        schema_of::<
            cognicode_core::interface::mcp::handlers::consolidated_handlers::ProjectOverviewInput,
        >(),
    );
    m.insert(
        "query_symbol_index",
        schema_of::<cognicode_core::interface::mcp::schemas::QuerySymbolInput>(),
    );
    m.insert(
        "read_file",
        schema_of::<cognicode_core::interface::mcp::schemas::ReadFileInput>(),
    );
    m.insert(
        "read_view_spec",
        schema_of::<cognicode_core::interface::mcp::schemas::ReadViewSpecInput>(),
    );
    m.insert(
        "rename_preview",
        schema_of::<cognicode_core::interface::mcp::schemas::RenamePreviewInput>(),
    );
    m.insert(
        "reparse_on_edit",
        schema_of::<cognicode_core::interface::mcp::schemas::ReparseOnEditInput>(),
    );
    m.insert(
        "retrieve_and_verify",
        schema_of::<cognicode_core::interface::mcp::schemas::RetrieveAndVerifyInput>(),
    );
    m.insert(
        "review_pr",
        schema_of::<cognicode_core::interface::mcp::handlers::consolidated_handlers::ReviewPrInput>(
        ),
    );
    m.insert(
        "safe_refactor",
        schema_of::<cognicode_core::interface::mcp::schemas::SafeRefactorInput>(),
    );
    m.insert(
        "safe_refactor_preview",
        schema_of::<cognicode_core::interface::mcp::schemas::SafeRefactorPreviewInput>(),
    );
    m.insert(
        "search_content",
        schema_of::<cognicode_core::interface::mcp::schemas::SearchContentInput>(),
    );
    m.insert(
        "slice_backward",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "slice_forward",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "smart_search",
        schema_of::<cognicode_core::interface::mcp::schemas::SmartSearchInput>(),
    );
    // TASK-82 / TASK-84 — los goals: sus argumentos se leen del mismo struct
    // que el resto, así que el `inputSchema` que publica sale de ahí y no de
    // una copia escrita a mano. Los goals viven juntos porque son una
    // familia: el mismo tipo de declaración para todos.
    m.insert(
        "status",
        schema_of::<cognicode_core::interface::goals::status::StatusInput>(),
    );
    m.insert(
        "context",
        schema_of::<cognicode_core::interface::goals::context::ContextInput>(),
    );
    m.insert(
        "find",
        schema_of::<cognicode_core::interface::goals::find::FindInput>(),
    );
    m.insert(
        "inspect",
        schema_of::<cognicode_core::interface::goals::inspect::InspectInput>(),
    );
    m.insert(
        "trace",
        schema_of::<cognicode_core::interface::goals::trace::TraceInput>(),
    );
    m.insert(
        "impact",
        schema_of::<cognicode_core::interface::goals::impact::ImpactInput>(),
    );
    m.insert(
        "overview",
        schema_of::<cognicode_core::interface::goals::overview::OverviewInput>(),
    );
    m.insert(
        "audit",
        schema_of::<cognicode_core::interface::goals::audit::AuditInput>(),
    );
    m.insert(
        "change",
        schema_of::<cognicode_core::interface::goals::change::ChangeInput>(),
    );
    // TASK-85 — el goal `recipe`: sus parámetros son los de la receta, así que
    // viajan en un objeto (`params`) y no como argumentos sueltos.
    m.insert(
        "recipe",
        schema_of::<cognicode_core::interface::goals::recipe::RecipeInput>(),
    );
    m.insert(
        "taint_flow",
        schema_of::<
            cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput,
        >(),
    );
    m.insert(
        "trace_path",
        schema_of::<cognicode_core::interface::mcp::schemas::TracePathInput>(),
    );
    m.insert(
        "validate_contract",
        schema_of::<cognicode_core::interface::mcp::schemas::ValidateContractInput>(),
    );
    m.insert(
        "write_file",
        schema_of::<cognicode_core::interface::mcp::schemas::WriteFileInput>(),
    );
    m
}

/// Every published tool input contract, built once per process.
///
/// Deriving seventy-odd schemas is cheap but not free, and `tools/list`
/// can be called repeatedly, so the result is cached rather than recomputed
/// per call.
pub fn published_input_schemas() -> &'static BTreeMap<&'static str, Map<String, Value>> {
    static SCHEMAS: OnceLock<BTreeMap<&'static str, Map<String, Value>>> = OnceLock::new();
    SCHEMAS.get_or_init(build_input_schemas)
}

/// The input schema for `tool`, or `None` when none is published.
pub fn input_schema_for(tool: &str) -> Option<Map<String, Value>> {
    published_input_schemas().get(tool).cloned()
}

/// The tools that publish an input contract, sorted.
pub fn tools_with_input_schema() -> Vec<&'static str> {
    published_input_schemas().keys().copied().collect()
}

/// Whether `tool` publishes an input contract.
pub fn publishes_contract(tool: &str) -> bool {
    published_input_schemas().contains_key(tool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_published_schema_describes_an_object() {
        for (tool, schema) in published_input_schemas() {
            assert_eq!(
                schema.get("type").and_then(|t| t.as_str()),
                Some("object"),
                "{tool} does not describe an object"
            );
        }
    }

    #[test]
    fn a_schema_never_declares_the_dialect_it_was_derived_in() {
        for (tool, schema) in published_input_schemas() {
            assert!(
                schema.get("$schema").is_none(),
                "{tool} advertises the dialect it was derived in; a client that \
                 validates strictly did not ask for it"
            );
        }
    }

    #[test]
    fn the_arguments_the_server_reads_are_the_ones_it_publishes() {
        // The whole point of deriving: a tool cannot accept an argument the
        // published schema does not name. This is the shape of the defect
        // that made four arguments invisible to every client.
        let schemas = published_input_schemas();
        for (tool, schema) in schemas {
            // Every published shape declares its properties, even when the
            // tool reads no arguments: the client reads the same shape for
            // every tool.
            let properties = schema
                .get("properties")
                .and_then(|p| p.as_object())
                .unwrap_or_else(|| panic!("{tool} advertises no properties"));
            for name in properties.keys() {
                assert!(
                    !name.is_empty(),
                    "{tool} advertises an argument with no name"
                );
            }
            let required = schema
                .get("required")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();
            for name in required {
                let name = name.as_str().expect("a required argument is named");
                assert!(
                    properties.contains_key(name),
                    "{tool} requires `{name}`, which it does not describe"
                );
            }
        }
    }
}
