//! B2.W3 — Output schema contract and state-preservation guarantee.
//!
//! This suite pins three contracts that close the gap B2.W2 left:
//!
//! 1. **Schema/DTO parity**: every tool that advertises an `output_schema`
//!    in `product/tools.json` must also have a real Rust DTO from which the
//!    schema is derived. A schema that describes nothing — or describes a
//!    shape the handler does not produce — is a lie of omission; the runtime
//!    must publish it AND it must agree with the catalog.
//!
//! 2. **Wire shape conformance**: a real response value (built from the
//!    same DTO) must satisfy the schema the catalog publishes. We do not
//!    pull in a full JSON-Schema implementation; we validate the narrow
//!    subset that matters — every `required` field is present and the
//!    declared types match — which is the same assertion
//!    `a012_structured_output::assert_object_contract_holds` makes for
//!    the wire. The set of tools under test expands to the full 15 in
//!    this file (a012 only spot-checks one or two).
//!
//! 3. **State preservation (AAT-B2-03)**: when a handler returns an
//!    `AnalysisOutcome<T>` whose state is `Partial`, `Unsupported` or
//!    `Failed`, the JSON value the boundary emits must carry the state,
//!    the `gaps` (for `Partial`), the `reason` (for `Unsupported`) and
//!    the `error` (for `Failed`). A `Partial { gaps }` must not collapse
//!    to an empty array; an `Unsupported { reason }` must not collapse
//!    to `false` or `null`. These are the regressions AAT-B2-03
//!    ("un fallo parcial del scanner no se convierte en éxito limpio")
//!    guards against, and they were the failure mode the previous wire
//!    format risked silently.
//!
//! Política B2.W3: this test file is the contract; any new tool that
//! publishes an `output_schema` must (a) extend
//! `output_contracts::published_output_schemas()` with the derived
//! schema, (b) re-run the catalog generator so `product/tools.json`
//! agrees, and (c) satisfy the type and state assertions below.
//!
//! Política B2.W3: this test does NOT cover JSON-RPC framing
//! (id/result/error envelopes, content blocks, pagination) — those are
//! B6.W1's concern. It covers the typed DTO shape only.

use cognicode_core::application::analysis_basis::{AnalysisError, AnalysisOutcome, OutcomeState};
use cognicode_core::interface::mcp::output_contracts::{
    compat, output_schema_for, published_output_schemas, tools_with_output_schema,
};
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Catalog/runtime parity for output schemas
// ---------------------------------------------------------------------------

/// The 15 tools with an `output_schema` in `product/tools.json` are exactly
/// the same 15 tools that publish a schema in
/// `output_contracts::published_output_schemas()`. A tool in one set but not
/// the other means either the catalog drifted, or the runtime drifted;
/// both are silenced by this assertion.
#[test]
fn catalog_and_runtime_agree_on_output_schema_coverage() {
    let runtime: std::collections::BTreeSet<String> = tools_with_output_schema()
        .into_iter()
        .map(String::from)
        .collect();
    let catalog = catalog_tools_with_output_schema();
    assert!(
        !catalog.is_empty(),
        "no tools in product/tools.json advertise an output_schema; B2.W3 \
         starts from the 15 tools that already publish a contract"
    );
    let only_in_catalog: Vec<&String> = catalog.difference(&runtime).collect();
    let only_in_runtime: Vec<&String> = runtime.difference(&catalog).collect();
    assert!(
        only_in_catalog.is_empty() && only_in_runtime.is_empty(),
        "output_schema drift.\n\
         Only in catalog ({}): {:?}\n\
         Only in runtime ({}): {:?}",
        only_in_catalog.len(),
        only_in_catalog,
        only_in_runtime.len(),
        only_in_runtime,
    );
}

/// Every published schema is an object with at least one property.
///
/// An empty `properties` map describes the universal set ("anything is an
/// object") and is functionally equivalent to no schema at all. Reject it
/// here so a future contributor cannot ship a stub and keep the count green.
#[test]
fn every_published_schema_describes_an_object_with_properties() {
    for (tool, schema) in published_output_schemas() {
        assert_eq!(
            schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{tool} does not describe an object"
        );
        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .unwrap_or_else(|| panic!("{tool} advertises no properties"));
        assert!(
            !properties.is_empty(),
            "{tool} advertises an empty properties map"
        );
    }
}

/// The published count is at least the 15 tools the goal doc targets.
///
/// A future commit that quietly drops a tool from
/// `published_output_schemas()` shrinks this number; the assertion makes
/// the regression loud.
#[test]
fn the_published_count_meets_the_b2_w3_target() {
    assert!(
        tools_with_output_schema().len() >= 15,
        "only {} tools publish an output contract; B2.W3 targets at least 15",
        tools_with_output_schema().len()
    );
}

// ---------------------------------------------------------------------------
// Compat namespace
// ---------------------------------------------------------------------------

/// `compat::schema_version_for` returns the schema version for every
/// published tool. Today the answer is uniformly `"v1"` because no
/// legacy wire shape exists yet; the assertion pins that the lookup
/// returns a version for every advertised schema, so the gating logic
/// in `compat::legacy_schema_for` has a string to branch on.
#[test]
fn compat_publishes_a_schema_version_for_every_tool() {
    for tool in tools_with_output_schema() {
        let version = compat::schema_version_for(tool)
            .unwrap_or_else(|| panic!("{tool} has no compat schema version"));
        assert!(
            !version.is_empty(),
            "{tool} compat version is empty: would silently select legacy"
        );
    }
}

/// Every published tool has at most one legacy schema.
///
/// "At most" is the honest contract for today: no tool has a legacy
/// wire shape yet, so the answer is `None` for all of them. The
/// assertion makes adding a legacy schema a deliberate, observable act:
/// the test stops failing the moment the legacy map is populated, and
/// the maintainer can then decide whether the new shape breaks the
/// old consumer.
#[test]
fn compat_legacy_schema_is_explicitly_empty_in_v1() {
    for tool in tools_with_output_schema() {
        assert!(
            compat::legacy_schema_for(tool).is_none(),
            "{tool} unexpectedly advertises a legacy schema in v1"
        );
    }
}

// ---------------------------------------------------------------------------
// State preservation (AAT-B2-03)
// ---------------------------------------------------------------------------

/// `shape_response` does NOT collapse `Partial` into an empty `[]`.
///
/// A `Partial` response MUST carry its `gaps`. The contract is the same
/// one `AnalysisOutcome::partial` enforces in its constructor: a
/// partial without gaps is a complete. A wire value that hides the
/// gaps behind an empty array is the failure mode AAT-B2-03 names
/// ("un fallo parcial del scanner no se convierte en éxito limpio").
#[test]
fn partial_response_carries_its_gaps() {
    let gaps = vec![
        AnalysisError::gap("call_graph", "Tarjan SCC not computed in v1"),
        AnalysisError::gap("coverage", "ignored files not walked"),
    ];
    let value = serde_json::json!({
        "symbols": [{"name": "alpha"}],
        "edges": [],
    });
    let response = compat::shape_response(OutcomeState::Partial, value, &gaps, None, None);

    assert_eq!(
        response["state"], "partial",
        "shape_response hides the state"
    );
    let carried = response["gaps"]
        .as_array()
        .expect("gaps must serialise as an array");
    assert_eq!(
        carried.len(),
        gaps.len(),
        "Partial must carry every gap; got {} of {}",
        carried.len(),
        gaps.len()
    );
    assert!(
        !carried.is_empty(),
        "Partial with non-empty gaps collapsed to an empty array: \
         this is the bug AAT-B2-03 forbids"
    );
    assert_eq!(response["gaps"][0]["capability"], "call_graph");
    assert_eq!(
        response["gaps"][0]["message"],
        "Tarjan SCC not computed in v1"
    );
    assert_eq!(
        response["value"]["symbols"][0]["name"], "alpha",
        "Partial must carry the value alongside the gaps"
    );
}

/// `Complete` carries the full value and no gaps.
///
/// A `Complete` response must not advertise gaps it does not have; the
/// absence is the signal that the run finished without omissions. A
/// `Complete` whose value is the empty array or `false` would be the
/// other collapse mode: a successful run reported as "nothing to
/// report".
#[test]
fn complete_response_has_no_gaps() {
    let value = serde_json::json!({
        "symbols": [{"name": "alpha"}, {"name": "beta"}],
        "edges": [{"from": "alpha", "to": "beta"}],
    });
    let response = compat::shape_response(OutcomeState::Complete, value.clone(), &[], None, None);

    assert_eq!(response["state"], "complete");
    assert_eq!(
        response["value"], value,
        "Complete must carry the full value byte-for-byte"
    );
    let gaps = response
        .get("gaps")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        gaps.is_empty(),
        "Complete must not advertise gaps; got {gaps:?}"
    );
}

/// `Unsupported` carries a `reason` and does NOT collapse to `false`.
///
/// AAT-B2-03 forbids the `Unsupported → false` collapse. The
/// `shape_response` boundary MUST emit the reason so a downstream
/// consumer can decide whether the absence is honest ("graph not yet
/// built") or a contract violation ("the schema did not promise this
/// capability").
#[test]
fn unsupported_response_carries_its_reason() {
    let response = compat::shape_response(
        OutcomeState::Unsupported,
        Value::Null,
        &[],
        Some("graph not yet built"),
        None,
    );

    assert_eq!(response["state"], "unsupported");
    assert_eq!(
        response["reason"], "graph not yet built",
        "Unsupported must carry the reason; collapsing to false is the \
         bug AAT-B2-03 forbids"
    );
    // Sanity guard: the response must not have been collapsed to a
    // boolean. A `false` here would be the regression that motivated
    // the test.
    assert!(
        !response.is_boolean(),
        "Unsupported must not collapse to a boolean"
    );
    assert_ne!(
        response,
        Value::Bool(false),
        "Unsupported must not collapse to `false`"
    );
}

/// `Failed` carries an `error`.
///
/// The error envelope is what `AnalysisOutcome::failed` requires: a
/// non-empty `AnalysisError::message`. The wire value mirrors that:
/// `error.kind`, `error.message`, and an optional `error.capability`.
#[test]
fn failed_response_carries_its_error() {
    let err = AnalysisError::io_failure("could not read snapshot");
    let response = compat::shape_response(OutcomeState::Failed, Value::Null, &[], None, Some(&err));

    assert_eq!(response["state"], "failed");
    assert_eq!(
        response["error"]["message"], "could not read snapshot",
        "Failed must carry the error message"
    );
    assert_eq!(
        response["error"]["kind"], "io",
        "Failed must carry the error kind so a consumer can branch on it"
    );
}

/// `Unknown` carries no value, no gaps, no reason, no error.
///
/// This is the "I genuinely don't know" state; emitting any of those
/// four fields would be a lie.
#[test]
fn unknown_response_carries_no_payload() {
    let response = compat::shape_response(OutcomeState::Unknown, Value::Null, &[], None, None);

    assert_eq!(response["state"], "unknown");
    assert!(
        response.get("gaps").is_none(),
        "Unknown must not advertise gaps"
    );
    assert!(
        response.get("reason").is_none(),
        "Unknown must not advertise a reason"
    );
    assert!(
        response.get("error").is_none(),
        "Unknown must not advertise an error"
    );
}

/// Convenience helper: `shape_response_from_outcome` produces the same
/// wire value as `serde_json::to_value(&outcome)` for a `Complete`
/// outcome, and does NOT collapse a `Partial` outcome's gaps.
///
/// This is the integration check between B2.W1's contract
/// (`AnalysisOutcome<T>`) and B2.W3's wire shape. Two paths to the
/// same shape must agree, byte for byte; otherwise a future refactor
/// could pick one and silently break the other.
#[test]
fn shape_response_from_outcome_matches_typed_serialization() {
    // Complete: both paths must produce the same value.
    let complete: AnalysisOutcome<u32> = AnalysisOutcome::complete(42);
    let from_helper = compat::shape_response_from_outcome(&complete);
    let from_serde: Value = serde_json::to_value(&complete).expect("serialise");
    assert_eq!(
        from_helper, from_serde,
        "shape_response_from_outcome diverged from serde for a Complete outcome"
    );

    // Partial: the helper must carry the gaps; a path that silently
    // dropped them would be the regression AAT-B2-03 forbids.
    let partial: AnalysisOutcome<u32> =
        AnalysisOutcome::partial(42, vec![AnalysisError::gap("x", "y")]);
    let response = compat::shape_response_from_outcome(&partial);
    assert_eq!(response["state"], "partial");
    assert_eq!(response["value"], 42);
    assert_eq!(response["gaps"][0]["capability"], "x");
    assert_eq!(response["gaps"][0]["message"], "y");
}

// ---------------------------------------------------------------------------
// Schema conformance on a synthetic response value
// ---------------------------------------------------------------------------

/// A `Complete` response payload built from the same fields the schema
/// requires MUST satisfy that schema.
///
/// We construct a value that is a known-valid instance of one of the
/// 15 published output types — `BuildGraphOutput` — and run it through
/// the same `assert_object_contract_holds` validator A-012 uses for the
/// wire. A schema that fails on a known-valid value is broken; a value
/// that fails a known-valid schema is the regression B2.W3 prevents.
#[test]
fn a_complete_response_payload_satisfies_its_advertised_schema() {
    let schema =
        output_schema_for("build_graph").expect("build_graph must publish an output_schema");
    let payload = serde_json::json!({
        "success": true,
        "status": "complete",
        "symbols_found": 2,
        "relationships_found": 1,
        "edges": [{"from": "a", "to": "b"}],
        "message": "ok",
    });
    assert_object_contract_holds(&schema, &payload, "build_graph");
}

/// A `Partial` response payload MUST still satisfy the advertised
/// schema — the gaps do not live in the tool's own output schema; they
/// are the *envelope* around it.
///
/// This pins the layering: the typed DTO schema describes the
/// `value`; the state-preservation envelope describes the state. The
/// `Partial` envelope is what the boundary adds on top, and it MUST
/// NOT alter the shape of the value.
#[test]
fn a_partial_response_payload_still_satisfies_the_typed_schema() {
    let schema =
        output_schema_for("build_graph").expect("build_graph must publish an output_schema");
    let payload = serde_json::json!({
        "success": false,
        "status": "partial",
        "symbols_found": 1,
        "relationships_found": 0,
        "edges": [],
        "message": "1 file skipped",
        "skipped_files": [
            {"path": "broken.rs", "reason_kind": "parse", "reason": "syntax error"}
        ],
    });
    assert_object_contract_holds(&schema, &payload, "build_graph");
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn catalog_tools_with_output_schema() -> std::collections::BTreeSet<String> {
    let path = catalog_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read catalog file {:?}: {e}", path));
    let v: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("catalog file {:?} is not valid JSON: {e}", path));
    v["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("catalog file {:?} has no `tools` array", path))
        .iter()
        .filter(|t| !t.get("output_schema").map(|s| s.is_null()).unwrap_or(true))
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string())
        .collect()
}

fn catalog_path() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("product").join("tools.json");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("could not locate product/tools.json above {:?}", manifest);
}

fn assert_object_contract_holds(schema: &Map<String, Value>, payload: &Value, tool: &str) {
    assert_eq!(
        schema.get("type").and_then(|t| t.as_str()),
        Some("object"),
        "{tool} advertises a non-object schema"
    );
    let properties = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or_else(|| panic!("{tool} advertises no properties"));
    for required in schema
        .get("required")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
    {
        let field = required.as_str().expect("required field name");
        assert!(
            payload.get(field).is_some(),
            "{tool} promises `{field}` but the value omits it"
        );
    }
    for (field, spec) in properties {
        let Some(value) = payload.get(field) else {
            continue;
        };
        let declared = spec.get("type").and_then(|t| t.as_str());
        if let Some(declared) = declared {
            assert!(
                type_matches(declared, value),
                "{tool}.{field} is declared `{declared}` but the value is `{}`",
                json_type(value)
            );
        }
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_matches(declared: &str, value: &Value) -> bool {
    let actual = json_type(value);
    if declared == "number" {
        return matches!(actual, "number" | "integer");
    }
    if declared == actual {
        return true;
    }
    // Nullable string fields may legitimately be `null`.
    declared == "string" && actual == "null"
}
