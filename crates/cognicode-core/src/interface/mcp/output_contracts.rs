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
    m.insert(
        "composite_impact_query",
        // Hand-rolled schema: CompositeImpactResult in
        // crates/cognicode-core/src/impact/composite.rs does not
        // (yet) derive schemars::JsonSchema, and cascading the derive
        // through Finding and ToolRef is out of scope for a hotfix
        // that unblocks compilation. The shape documented here
        // mirrors the struct's serde annotations (Option<Value>
        // fields, Vec<Finding>, verdict_refused invariant).
        serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "impact": { "type": ["object", "null"] },
                "architecture": { "type": ["object", "null"] },
                "findings": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "suggested_follow_ups": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "suggested_prerequisites": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "verdict_refused": { "type": "boolean" },
                "verdict_refused_reason": { "type": "string" }
            },
            "required": ["verdict_refused", "verdict_refused_reason"]
        }))
        .expect("hand-rolled composite_impact_query schema is a valid JSON object"),
    );
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

/// B2.W3 — compat layer for the output contract.
///
/// Every schema this module publishes is derived from the Rust DTO
/// that serialises the response, so a future change to the DTO can
/// only land in lockstep with a schema bump. When that happens, an
/// existing MCP client that has hard-coded the old shape still wants
/// to work; the compat layer is where that contract lives.
///
/// Today there is no legacy wire shape — every tool is at `v1` — but
/// the API is here so the next shape change has a place to go. The
/// catalogue does not declare schema versions yet; `schema_version_for`
/// returns `"v1"` uniformly, and `legacy_schema_for` returns `None`
/// uniformly. Populating either is a deliberate, observable act.
pub mod compat {
    use std::collections::BTreeMap;

    use serde_json::{Map, Value};

    use crate::application::analysis_basis::{AnalysisError, AnalysisOutcome, OutcomeState};

    /// Current published schema version.
    ///
    /// Bumping this is the single change that tells consumers "the
    /// shape is different now, consult `legacy_schema_for`". The
    /// version is a `&'static str` rather than a typed enum because
    /// versions are deliberately open-ended: a future `v2` or
    /// `v2025-04` is a string the runtime has not heard of yet, and
    /// that is fine.
    pub const CURRENT_SCHEMA_VERSION: &str = "v1";

    /// Return the schema version for `tool`, or `None` when the tool
    /// does not publish a contract. The lookup is the same one
    /// `published_output_schemas` uses; the two never disagree.
    pub fn schema_version_for(tool: &str) -> Option<&'static str> {
        if super::output_schema_for(tool).is_some() {
            Some(CURRENT_SCHEMA_VERSION)
        } else {
            None
        }
    }

    /// Return the *legacy* wire schema for `tool`, if any.
    ///
    /// Legacy schemas are the shapes published before the current
    /// `CURRENT_SCHEMA_VERSION`. A consumer that pins the old shape
    /// can ask for it here; the boundary emits that shape instead of
    /// the new one. Today no tool has a legacy shape, so the answer
    /// is uniformly `None`.
    pub fn legacy_schema_for(tool: &str) -> Option<Map<String, Value>> {
        // Deliberately empty in v1: no tool has a legacy wire shape.
        // Adding a legacy entry is a one-line addition to
        // `legacy_schemas()` and a corresponding catalogue bump.
        legacy_schemas().get(tool).cloned()
    }

    /// The legacy wire schemas, keyed by tool name.
    ///
    /// Empty today; populated the moment a tool's shape changes in a
    /// way that would break an existing consumer. Keep the map
    /// static so the answer is deterministic and does not require
    /// filesystem access.
    static LEGACY_SCHEMAS: std::sync::OnceLock<BTreeMap<&'static str, Map<String, Value>>> =
        std::sync::OnceLock::new();

    fn legacy_schemas() -> &'static BTreeMap<&'static str, Map<String, Value>> {
        LEGACY_SCHEMAS.get_or_init(BTreeMap::new)
    }

    /// Shape an outcome into the wire `Value` the boundary emits.
    ///
    /// The contract this function enforces is the one AAT-B2-03 names:
    ///
    ///   - A `Partial` response MUST carry its `gaps`. Empty `[]` is
    ///     the regression B2.W3 forbids.
    ///   - An `Unsupported` response MUST carry its `reason`.
    ///     Collapsing to `false` or `null` is the regression B2.W3
    ///     forbids.
    ///   - A `Complete` response MUST carry the full value; gaps,
    ///     reason and error are absent because they are absent.
    ///   - A `Failed` response MUST carry its `error`. The `value` is
    ///     dropped because the run did not produce one.
    ///   - A `Unknown` response carries none of the four optional
    ///     fields. Emitting any would be a lie.
    ///
    /// The function does not validate the inner `value` against any
    /// advertised schema: that is the responsibility of the schema
    /// validator (see `assert_object_contract_holds` in
    /// `tests/b2_w3_output_schema_contract.rs`). The boundary is
    /// responsible only for carrying the *envelope* faithfully; the
    /// `value` is opaque here.
    pub fn shape_response(
        state: OutcomeState,
        value: Value,
        gaps: &[AnalysisError],
        reason: Option<&str>,
        error: Option<&AnalysisError>,
    ) -> Value {
        let mut object = Map::new();
        // The state is the discriminator; it is always present so a
        // consumer never has to guess.
        object.insert(
            "state".to_string(),
            Value::String(state_discriminant(state).to_string()),
        );

        // Only `Complete` and `Partial` carry a value. The other three
        // states have nothing useful to say, and emitting one would
        // imply success that does not exist.
        if matches!(state, OutcomeState::Complete | OutcomeState::Partial) {
            object.insert("value".to_string(), value);
        }

        // Partial carries gaps, and the gaps are required to be
        // non-empty. We assert the same invariant
        // `AnalysisOutcome::partial` enforces, because the wire shape
        // is a second description of the same state.
        if state == OutcomeState::Partial {
            assert!(
                !gaps.is_empty(),
                "Partial response must carry at least one gap; \
                 collapsing to an empty value is the bug AAT-B2-03 forbids"
            );
            let gap_values: Vec<Value> = gaps
                .iter()
                .map(|g| serde_json::to_value(g).expect("AnalysisError serialises"))
                .collect();
            object.insert("gaps".to_string(), Value::Array(gap_values));
        }

        if state == OutcomeState::Unsupported {
            let reason = reason.unwrap_or_else(|| {
                panic!(
                    "Unsupported response must carry a reason; \
                     collapsing to false is the bug AAT-B2-03 forbids"
                )
            });
            assert!(
                !reason.trim().is_empty(),
                "Unsupported response must carry a non-empty reason"
            );
            object.insert("reason".to_string(), Value::String(reason.to_string()));
        }

        if state == OutcomeState::Failed {
            let err = error.unwrap_or_else(|| panic!("Failed response must carry an error"));
            object.insert(
                "error".to_string(),
                serde_json::to_value(err).expect("AnalysisError serialises"),
            );
        }

        Value::Object(object)
    }

    /// Convenience helper: shape an `AnalysisOutcome<T>` directly.
    ///
    /// The wire shape this produces is identical to
    /// `serde_json::to_value(&outcome)`. The helper exists so the
    /// boundary code does not have to remember which serialiser to
    /// call when it has an outcome in hand.
    pub fn shape_response_from_outcome<T: serde::Serialize>(outcome: &AnalysisOutcome<T>) -> Value {
        // Round-trip through serde so the helper and the direct path
        // agree byte-for-byte. The performance cost is one allocation
        // per response, which the boundary already does many of; not a
        // hot path.
        serde_json::to_value(outcome).expect("AnalysisOutcome serialises")
    }

    /// The string discriminator emitted on the wire.
    ///
    /// Same lowercase string `OutcomeState` already uses; repeated
    /// here so callers that hand-roll a `Value` do not have to know
    /// the convention.
    fn state_discriminant(state: OutcomeState) -> &'static str {
        match state {
            OutcomeState::Complete => "complete",
            OutcomeState::Partial => "partial",
            OutcomeState::Unknown => "unknown",
            OutcomeState::Unsupported => "unsupported",
            OutcomeState::Failed => "failed",
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn current_version_is_v1() {
            assert_eq!(CURRENT_SCHEMA_VERSION, "v1");
        }

        #[test]
        fn every_published_tool_has_a_version() {
            for tool in super::super::tools_with_output_schema() {
                assert!(
                    schema_version_for(tool).is_some(),
                    "{tool} is published but has no compat version"
                );
            }
        }

        #[test]
        fn no_tool_has_a_legacy_schema_yet() {
            for tool in super::super::tools_with_output_schema() {
                assert!(
                    legacy_schema_for(tool).is_none(),
                    "{tool} unexpectedly carries a legacy schema in v1"
                );
            }
        }

        #[test]
        fn an_unknown_tool_has_no_version() {
            assert!(schema_version_for("definitely_not_a_tool").is_none());
        }

        #[test]
        fn partial_carries_non_empty_gaps() {
            let gaps = vec![AnalysisError::gap("a", "b")];
            let response = shape_response(OutcomeState::Partial, Value::Null, &gaps, None, None);
            let arr = response["gaps"].as_array().unwrap();
            assert_eq!(arr.len(), 1);
        }

        #[test]
        fn unsupported_carries_reason() {
            let response = shape_response(
                OutcomeState::Unsupported,
                Value::Null,
                &[],
                Some("nope"),
                None,
            );
            assert_eq!(response["reason"], "nope");
        }

        #[test]
        fn failed_carries_error() {
            let err = AnalysisError::io_failure("disk gone");
            let response = shape_response(OutcomeState::Failed, Value::Null, &[], None, Some(&err));
            assert_eq!(response["error"]["message"], "disk gone");
        }

        #[test]
        #[should_panic(expected = "Partial response must carry at least one gap")]
        fn partial_rejects_empty_gaps() {
            // Mirrors `AnalysisOutcome::partial`'s own panic so the
            // boundary cannot accidentally emit a Partial-without-gaps.
            let _ = shape_response(OutcomeState::Partial, Value::Null, &[], None, None);
        }

        #[test]
        #[should_panic(expected = "Unsupported response must carry a reason")]
        fn unsupported_rejects_missing_reason() {
            let _ = shape_response(OutcomeState::Unsupported, Value::Null, &[], None, None);
        }

        #[test]
        #[should_panic(expected = "Failed response must carry an error")]
        fn failed_rejects_missing_error() {
            let _ = shape_response(OutcomeState::Failed, Value::Null, &[], None, None);
        }
    }
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
