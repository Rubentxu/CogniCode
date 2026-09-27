//! Output contracts for the public MCP tool surface.
//!
//! `product/tools.json` publishes an `output_schema` for every tool, and until
//! now it has been `null` for all 73. That is a promise gap in the same shape
//! as the ones A-009 and A-010 closed: the catalogue described a contract that
//! nothing published, enforced, or checked.
//!
//! # Why derive rather than write
//!
//! A hand-written `outputSchema` is a second description of a type that
//! already exists. It cannot drift without anyone noticing, which is the
//! failure mode of every hand-maintained schema in this repository before
//! A-011. Deriving from the Rust type that actually produces the bytes makes
//! drift impossible: change the struct, and the published contract changes
//! with it.
//!
//! # Why some tools are absent
//!
//! Only tools with a dedicated, concrete output type are listed. Two known
//! handler outputs — `GraphAnalyzeOutput` and `ProgramAnalysisToolOutput` —
//! carry a `serde_json::Value` field whose shape depends on a runtime mode
//! argument. No honest schema can be derived for those, and publishing an
//! empty or approximate one would be worse than publishing nothing: a
//! consumer that trusts a schema the server does not honour is worse off
//! than one that knows there is no contract.
//!
//! Tools whose handlers return a bare `String` are absent for the same reason.
//! Inventing a contract for them is explicitly out of scope; the catalogue
//! generator already refuses to do it.
//!
//! # How this stays honest
//!
//! [`output_schema_for`] is the single lookup, and
//! `crates/cognicode-mcp/tests/a012_structured_output.rs` checks three things
//! that no amount of schema writing can guarantee on its own:
//!
//! 1. every tool this module claims a contract for actually advertises it in
//!    `tools/list`;
//! 2. the advertised schema equals the derived one, so the runtime and the
//!    catalogue cannot disagree;
//! 3. a real response satisfies the schema it advertises, so a schema that
//!    describes nothing is caught rather than shipped.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::interface::mcp::handlers::BuildGraphOutput;
use crate::interface::mcp::handlers::consolidated_handlers::{
    CodebaseMapOutput, GraphCheckpointOutput, IacQueryOutput, ProjectInsightsOutput,
    ProjectOverviewOutput, ReviewPrOutput,
};
use crate::interface::mcp::handlers::graph_query_handlers::{
    ExportCallflowOutput, GetImplementorsOutput, GetImportsOutput, GetMembersOutput,
    GetTypeRefsOutput, GraphExplainOutput, GraphQueryFilteredOutput, GraphQueryOutput,
};

/// Derive the JSON Schema for `T` and return it as a JSON object.
///
/// `schemars` produces a draft 2020-12 document. MCP expects a bare schema
/// object, so the `$schema` key is dropped: a client that treats it as
/// informational is fine, and one that passes the whole thing to a strict
/// validator is happier without a dialect declaration it did not ask for.
fn schema_of<T: schemars::JsonSchema>() -> Map<String, Value> {
    let value = serde_json::to_value(schemars::schema_for!(T))
        .expect("schemars output is always serialisable");
    let mut object = value
        .as_object()
        .cloned()
        .expect("schemars produces a schema object");
    object.remove("$schema");
    object
}

/// Map a tool name to the JSON Schema its output is guaranteed to satisfy.
///
/// A `BTreeMap` so iteration order is stable; the map is part of a public
/// contract surface and a nondeterministic order would make diffs of
/// `tools/list` noise.
fn build_output_schemas() -> BTreeMap<&'static str, Map<String, Value>> {
    let mut m = BTreeMap::new();
    m.insert("build_graph", schema_of::<BuildGraphOutput>());
    m.insert("codebase_map", schema_of::<CodebaseMapOutput>());
    m.insert("export_callflow", schema_of::<ExportCallflowOutput>());
    m.insert("get_imports", schema_of::<GetImportsOutput>());
    m.insert("get_implementors", schema_of::<GetImplementorsOutput>());
    m.insert("get_members", schema_of::<GetMembersOutput>());
    m.insert("get_type_references", schema_of::<GetTypeRefsOutput>());
    m.insert("graph_checkpoint", schema_of::<GraphCheckpointOutput>());
    m.insert("graph_explain", schema_of::<GraphExplainOutput>());
    m.insert("graph_query", schema_of::<GraphQueryOutput>());
    m.insert(
        "graph_query_filtered",
        schema_of::<GraphQueryFilteredOutput>(),
    );
    m.insert("iac_query", schema_of::<IacQueryOutput>());
    m.insert("project_insights", schema_of::<ProjectInsightsOutput>());
    m.insert("project_overview", schema_of::<ProjectOverviewOutput>());
    m.insert("review_pr", schema_of::<ReviewPrOutput>());
    m
}

/// Every published tool output contract, built once per process.
///
/// Deriving sixteen schemas is cheap but not free, and `tools/list` can be
/// called repeatedly, so the result is cached rather than recomputed per call.
pub fn published_output_schemas() -> &'static BTreeMap<&'static str, Map<String, Value>> {
    static SCHEMAS: OnceLock<BTreeMap<&'static str, Map<String, Value>>> = OnceLock::new();
    SCHEMAS.get_or_init(build_output_schemas)
}

/// The output schema for `tool`, or `None` when none is published.
///
/// `None` is the honest answer for the majority of tools today. It means "no
/// contract", not "an empty contract", and the distinction matters: a client
/// must be able to tell that it cannot rely on the shape.
pub fn output_schema_for(tool: &str) -> Option<Map<String, Value>> {
    published_output_schemas().get(tool).cloned()
}

/// The tools that publish an output contract, sorted.
pub fn tools_with_output_schema() -> Vec<&'static str> {
    published_output_schemas().keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_schemas_are_objects_with_a_type() {
        for (tool, schema) in published_output_schemas() {
            assert_eq!(
                schema.get("type").and_then(|t| t.as_str()),
                Some("object"),
                "{tool} does not describe an object"
            );
            assert!(
                schema.contains_key("properties"),
                "{tool} has no properties: an empty schema promises nothing"
            );
        }
    }

    #[test]
    fn no_schema_carries_a_dialect_declaration() {
        for (tool, schema) in published_output_schemas() {
            assert!(
                !schema.contains_key("$schema"),
                "{tool} carries a $schema key; MCP expects a bare schema object"
            );
        }
    }

    #[test]
    fn an_unknown_tool_publishes_no_contract() {
        assert!(output_schema_for("definitely_not_a_tool").is_none());
    }

    #[test]
    fn the_table_is_not_empty_and_is_stable() {
        // If this ever drops toward zero the feature has been quietly undone.
        assert!(
            tools_with_output_schema().len() >= 10,
            "only {} tools publish an output contract",
            tools_with_output_schema().len()
        );
    }
}
