//! B2.W2 — Single source of truth for the tool catalog.
//!
//! Both `cognicode capabilities --json` (CLI) and MCP `tools/list` derive
//! from [`load_tool_catalog`], which reads `product/tools.json` and
//! deserialises it into a typed [`ToolDescriptor`] vector. The JSON file
//! is the only on-disk authority; the Rust struct gives us typed access,
//! schema validation at load time, and a single point of change when a
//! tool is added or removed.
//!
//! Política B2.W2: la lista paralela (MCP `build_all_tools` inline)
//! queda eliminada. El módulo `mcp::rmcp_adapter` importa
//! [`load_tool_catalog`] y proyecta cada `ToolDescriptor` a un `Tool`
//! MCP. Si se añade una tool, va a `tools.json` primero, luego el
//! loader la expone, y el adapter MCP la recoge sin tocar Rust.
//!
//! ## Feature gating
//!
//! Some tools are only available when a Cargo feature is enabled
//! (e.g. `evidence-cli-ladybug` for `list_evidence` / `search_evidence`,
//! `program-analysis-server` for `interproc_summary`,
//! `persistence` for `detect_drift` / `reparse_on_edit`). The optional
//! [`ToolDescriptor::feature`] field records which feature, if any,
//! gates the tool. [`tool_feature_enabled`] resolves the gate against
//! the *current* build via `cfg!(feature = ...)` so the MCP adapter
//! can filter before emitting `tools/list`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// One entry from `product/tools.json`.
///
/// The shape mirrors the JSON 1:1. Fields use `serde_json::Value` for
/// the bag-like entries (`requirements`, `runtime_metadata`,
/// `input_schema`, `output_schema`) because they are not all shaped
/// the same way across tools and we want the loader to be tolerant.
///
/// `authority` carries the canonical four-state vocabulary
/// (`read` | `mutating` | `execute` | `network`) declared by PRF-MCP-05;
/// `mutating_tool_names` derives from this field, not a hand-kept list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDescriptor {
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub authority: String,
    pub stability: String,
    pub estimated_latency_ms: u32,
    #[serde(default)]
    pub requirements: Value,
    #[serde(default = "default_requirements_status")]
    pub requirements_status: String,
    #[serde(default)]
    pub public_profiles: Vec<String>,
    #[serde(default)]
    pub runtime_metadata: Value,
    #[serde(default)]
    pub input_schema: Value,
    #[serde(default)]
    pub output_schema: Value,
    /// B3.W2 — Affordances for `cognicode inspect`. The set of
    /// `Intent` variants (`Locate`, `Summarize`, `Verify`, `Read`)
    /// that route to this tool. Agents introspect this field to
    /// discover which intent each tool resolves without reading
    /// the routing function source. Parity with
    /// `interface::intent::route_intent` is pinned by the test
    /// `b3_w2_progressive_context::intent_tags_in_catalog_match_route_intent`.
    #[serde(default)]
    pub intent_tags: Vec<String>,
    /// B3.W1 — declared precision floor. One of `"lsp"`,
    /// `"ast-heuristic"`, or `"index"`. Tools without an explicit
    /// declaration default to `"ast-heuristic"` (the conservative
    /// choice — never silently upgrade an absent field to LSP-grade,
    /// AAT-B3-01). The 8 navigation tools consolidated in B3.W1
    /// (see `NAVIGATION_TOOLS` in `symbol_identity`) MUST declare
    /// this field; the test
    /// `catalog_declares_precision_floor_for_every_navigation_tool`
    /// pins the invariant.
    #[serde(default = "default_precision_floor")]
    pub precision_floor: String,
    /// Cargo feature this tool is gated behind. `None` means always
    /// available. The MCP filter uses this to drop tools whose feature
    /// is not enabled in the current build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// B3.W3 — tool names that must run first (e.g. `build_graph`
    /// for navigation/graph tools). Identifiers match `name`. Empty
    /// means no dependency; absent means not yet annotated. Used
    /// by the "what analysis is required first" affordance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerequisites: Option<Vec<String>>,
    /// B3.W3 — tool names that the result commonly leads to (e.g.
    /// `query_symbol_index` → `["get_symbol_code", "find_references"]`).
    /// Chains form a DAG; the test `catalog_follow_ups_chain_is_acyclic`
    /// asserts there is no cycle. Used by the "what follow-up result
    /// can I query" affordance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_ups: Option<Vec<String>>,
    /// B3.W3 — argument names that MUST be present in the call. Mirrors
    /// `input_schema.required` so a caller can ask the catalog
    /// declaratively which fields are missing without parsing the JSON
    /// Schema. Used by the "what arguments are missing" affordance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_args: Option<Vec<String>>,
    /// B3.W3 — human-readable strings describing what blocks this tool
    /// (e.g. `"requires build_graph to have run first"`,
    /// `"may miss dynamic dispatch"`, `"mutates workspace"`). Free-form;
    /// rendered as-is by the agent runtime. Used by the "what limitation
    /// blocks progress" affordance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitations: Option<Vec<String>>,
    /// B4.W1 — minimum provenance tier the tool's edges are guaranteed
    /// to carry. One of `"observed"`, `"inferred"`, `"heuristic"`.
    /// Required for the 5 impact tools consolidated by B4.W1
    /// (`analyze_impact`, `graph_explain`, `trace_path`, `graph_query`,
    /// `graph_query_filtered`). AAT-B4-03 disallows silently upgrading
    /// an absent field to `"observed"`.
    #[serde(default = "default_impact_direction_floor")]
    pub impact_direction_floor: String,
    // Note: the spec's `intent_routing` (question 1 / 2 — "what can I
    // do" / "which intent resolves here") is covered by B3.W2's
    // `intent_tags` field above. The two were planned as separate
    // work units; both end up answering the same question, so we keep
    // the canonical B3.W2 name (`intent_tags`) and document that B3.W3
    // does not duplicate it. See `b3_w2_progressive_context` tests
    // for the B3.W2 contract and `b3_w3_agent_discovery` for the
    // B3.W3 fields.
}

fn default_requirements_status() -> String {
    "not_declared".to_string()
}

/// B3.W1 — default precision floor when the catalog omits the field.
/// Chosen to be the conservative tier: an absent field must NOT be
/// silently upgraded to LSP-grade (AAT-B3-01).
fn default_precision_floor() -> String {
    "ast-heuristic".to_string()
}

/// B4.W1 — default impact direction floor when the catalog omits the
/// field. Conservative: `"inferred"`. Tools that genuinely observe
/// every edge declare `"observed"` explicitly in `product/tools.json`.
fn default_impact_direction_floor() -> String {
    "inferred".to_string()
}

/// Resolve the gate for a `feature` value declared in the catalog.
///
/// Returns `true` when the feature is enabled in the *current* build,
/// `false` otherwise (including for any feature name this build cannot
/// recognise — fail-closed so a typo never silently exposes a tool).
pub fn tool_feature_enabled(feature: &Option<String>) -> bool {
    match feature.as_deref() {
        None => true,
        Some("persistence") => cfg!(feature = "persistence"),
        Some("program-analysis-server") => cfg!(feature = "program-analysis-server"),
        Some("evidence-cli-ladybug") => cfg!(feature = "evidence-cli-ladybug"),
        Some(_) => false,
    }
}

/// Load the full catalog from disk, with feature filtering applied.
///
/// Equivalent to `load_tool_catalog()` filtered by
/// [`tool_feature_enabled`]. Both the CLI `build_capabilities_doc`
/// and the MCP `build_all_tools` are expected to consume this view
/// so the parity test can compare them on equal footing.
pub fn load_tool_catalog_active() -> Vec<ToolDescriptor> {
    load_tool_catalog()
        .into_iter()
        .filter(|d| tool_feature_enabled(&d.feature))
        .collect()
}

/// Load the tool catalog from `product/tools.json`.
///
/// Walks the same anchors as the private
/// `resolve_product_assets_root` helper in
/// `crate::interface::cli::commands`: cwd and its parents first,
/// then `CARGO_MANIFEST_DIR` and its parents. Returns an empty
/// vector when the file cannot be found or fails to deserialise;
/// callers that need a fail-closed signal should log their own
/// diagnostics.
pub fn load_tool_catalog() -> Vec<ToolDescriptor> {
    for path in catalog_candidate_paths() {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(parsed) = serde_json::from_str::<CatalogFile>(&text)
        {
            return parsed.tools;
        }
    }
    Vec::new()
}

#[derive(Deserialize)]
struct CatalogFile {
    tools: Vec<ToolDescriptor>,
}

fn catalog_candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        walk_up(cwd.as_path(), &mut out);
    }

    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        walk_up(Path::new(&manifest), &mut out);
    }

    out
}

/// Mirror `commands::walk_up`: walk parents, bounded so a symlink loop
/// cannot turn discovery into a hang. 32 hops matches the CLI anchor.
fn walk_up(start: &Path, out: &mut Vec<PathBuf>) {
    let mut cursor: Option<&Path> = Some(start);
    let mut hops = 0usize;
    while let Some(dir) = cursor {
        out.push(dir.join("product").join("tools.json"));
        cursor = dir.parent();
        hops += 1;
        if hops > 32 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_status_defaults_to_not_declared() {
        // The JSON in this repo uses "not_declared" for every tool;
        // tools without the field still deserialise cleanly and the
        // default reflects that.
        let json = r#"{
            "name": "x",
            "title": "X",
            "description": "X",
            "category": "graph",
            "authority": "read",
            "stability": "stable",
            "estimated_latency_ms": 10
        }"#;
        let d: ToolDescriptor = serde_json::from_str(json).expect("parses");
        assert_eq!(d.requirements_status, "not_declared");
        assert!(d.feature.is_none());
    }

    #[test]
    fn feature_field_round_trips_when_set() {
        let d = ToolDescriptor {
            name: "t".into(),
            title: "T".into(),
            description: "T".into(),
            category: "graph".into(),
            authority: "read".into(),
            stability: "stable".into(),
            estimated_latency_ms: 1,
            requirements: Value::Null,
            requirements_status: "satisfied".into(),
            public_profiles: vec![],
            runtime_metadata: Value::Null,
            input_schema: Value::Null,
            output_schema: Value::Null,
            intent_tags: Vec::new(),
            precision_floor: "ast-heuristic".to_string(),
            feature: Some("evidence-cli-ladybug".into()),
            prerequisites: None,
            follow_ups: None,
            required_args: None,
            limitations: None,
            impact_direction_floor: "observed".into(),
        };
        let v = serde_json::to_value(&d).expect("serialise");
        assert_eq!(v["feature"], "evidence-cli-ladybug");
    }

    #[test]
    fn feature_field_omitted_when_none() {
        let d = ToolDescriptor {
            name: "t".into(),
            title: "T".into(),
            description: "T".into(),
            category: "graph".into(),
            authority: "read".into(),
            stability: "stable".into(),
            estimated_latency_ms: 1,
            requirements: Value::Null,
            requirements_status: "satisfied".into(),
            public_profiles: vec![],
            runtime_metadata: Value::Null,
            input_schema: Value::Null,
            output_schema: Value::Null,
            intent_tags: Vec::new(),
            precision_floor: "ast-heuristic".to_string(),
            feature: None,
            prerequisites: None,
            follow_ups: None,
            required_args: None,
            limitations: None,
            impact_direction_floor: "observed".into(),
        };
        let v = serde_json::to_value(&d).expect("serialise");
        assert!(
            v.get("feature").is_none(),
            "feature=None must not add a null key to the JSON output"
        );
    }

    #[test]
    fn b3_w3_discovery_fields_round_trip_when_set() {
        // B3.W3 — every new affordance field round-trips when populated
        // and is omitted from the JSON when None (skip_serializing_if).
        let d = ToolDescriptor {
            name: "t".into(),
            title: "T".into(),
            description: "T".into(),
            category: "graph".into(),
            authority: "read".into(),
            stability: "stable".into(),
            estimated_latency_ms: 1,
            requirements: Value::Null,
            requirements_status: "satisfied".into(),
            public_profiles: vec![],
            runtime_metadata: Value::Null,
            input_schema: Value::Null,
            output_schema: Value::Null,
            intent_tags: Vec::new(),
            precision_floor: "ast-heuristic".to_string(),
            feature: None,
            prerequisites: Some(vec!["build_graph".into()]),
            follow_ups: Some(vec!["get_symbol_code".into()]),
            required_args: Some(vec!["symbol_name".into()]),
            limitations: Some(vec!["requires build_graph to have run first".into()]),
            impact_direction_floor: "observed".to_string(),
        };
        let v = serde_json::to_value(&d).expect("serialise");
        assert_eq!(v["prerequisites"][0], "build_graph");
        assert_eq!(v["follow_ups"][0], "get_symbol_code");
        assert_eq!(v["required_args"][0], "symbol_name");
        assert_eq!(
            v["limitations"][0],
            "requires build_graph to have run first"
        );
    }

    #[test]
    fn b3_w3_discovery_fields_omitted_when_none() {
        // B3.W3 — None-valued affordances must NOT emit null keys.
        // The catalog stays byte-shape compatible with pre-B3.W3
        // consumers (`b2_w2_catalog_parity`).
        let d = ToolDescriptor {
            name: "t".into(),
            title: "T".into(),
            description: "T".into(),
            category: "graph".into(),
            authority: "read".into(),
            stability: "stable".into(),
            estimated_latency_ms: 1,
            requirements: Value::Null,
            requirements_status: "satisfied".into(),
            public_profiles: vec![],
            runtime_metadata: Value::Null,
            input_schema: Value::Null,
            output_schema: Value::Null,
            intent_tags: Vec::new(),
            precision_floor: "ast-heuristic".to_string(),
            feature: None,
            prerequisites: None,
            follow_ups: None,
            required_args: None,
            limitations: None,
            impact_direction_floor: "observed".into(),
        };
        let v = serde_json::to_value(&d).expect("serialise");
        for key in [
            "prerequisites",
            "follow_ups",
            "required_args",
            "limitations",
        ] {
            assert!(
                v.get(key).is_none(),
                "field `{key}` with None must not emit a JSON key"
            );
        }
    }

    #[test]
    fn feature_filter_matches_current_cargo_config() {
        // Persistence is a default feature, so it must be enabled.
        assert!(tool_feature_enabled(&Some("persistence".into())));
        // The two opt-in features are *not* in the default build, so
        // the filter must reject them. (If this test runs with those
        // features enabled, the assertion would still pass; the
        // contract is "matches what cargo enabled".)
        assert_eq!(
            tool_feature_enabled(&Some("program-analysis-server".into())),
            cfg!(feature = "program-analysis-server")
        );
        assert_eq!(
            tool_feature_enabled(&Some("evidence-cli-ladybug".into())),
            cfg!(feature = "evidence-cli-ladybug")
        );
        // No feature: always available.
        assert!(tool_feature_enabled(&None));
        // Unknown feature: fail-closed.
        assert!(!tool_feature_enabled(&Some(
            "definitely-not-a-feature".into()
        )));
    }
}
