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

/// Load the full catalog, with feature filtering applied.
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

/// The catalogue baked into the binary at compile time.
///
/// `product/tools.json` is versioned and hand-authored; `include_str!`
/// compiles that exact file in, so there is still **one** source of truth —
/// it is simply also *inside* the binary, where no install layout can lose
/// it. The filesystem lookup above stays as the first choice so a checkout
/// can be inspected and edited without a rebuild.
///
/// This is the third time the same failure has been reached from a different
/// direction: `env!("CARGO_MANIFEST_DIR")` named the build machine's tree, the
/// runtime resolver then named the process CWD, and the installer shipped no
/// product data at all. Each of those produced the *same* observable symptom —
/// a healthy server, exit code 0, valid JSON-RPC, and an empty `tools/list` —
/// and each one was invisible to the suite because the suite ran where the
/// data happened to be. A catalogue that cannot be found is indistinguishable
/// from a product with no tools, so the binary carries its own answer.
const EMBEDDED_TOOLS: &str = include_str!("../../../../product/tools.json");

/// The catalogue as it ships inside the binary, parsed.
///
/// Falls back to an empty vector if `tools.json` ever stops being valid
/// JSON — a malformed catalogue must not abort the process, and an empty
/// advertisement is the honest failure (the parity tests catch a drifted or
/// broken file at build time, not at runtime).
pub fn embedded_tool_catalog() -> Vec<ToolDescriptor> {
    serde_json::from_str::<CatalogFile>(EMBEDDED_TOOLS)
        .map(|parsed| parsed.tools)
        .unwrap_or_default()
}

/// Where a loaded catalogue came from, and whether it is usable.
///
/// The bare `Vec<ToolDescriptor>` that used to cross this boundary could not
/// distinguish "the product genuinely declares zero tools" from "the catalogue
/// could not be found or parsed". Those are the same wire response and
/// opposite meanings, and conflating them is exactly what made the MCP
/// regression invisible: a server that had lost its catalogue reported the
/// same `{"tools": []}` as a healthy one, with exit code 0 and no diagnostic.
/// An empty vector is a *state*, not an outcome; the outcome is this enum.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogReport {
    pub tools: Vec<ToolDescriptor>,
    pub source: CatalogSource,
}

/// The provenance of a loaded catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSource {
    /// Read and parsed from `product/tools.json` at this path.
    Disk(PathBuf),
    /// Served from the catalogue compiled into the binary. Reached when no
    /// anchor carried one on disk — the normal state of a packaged install.
    Embedded,
    /// Nothing could be loaded: no catalogue on disk *and* the compiled-in
    /// copy failed to parse. This is the state that must never be reported
    /// as "the product has no tools", and the one the callers warn about.
    Unavailable,
}

impl CatalogReport {
    /// Whether the catalogue describes the product, as opposed to failing to.
    pub fn is_usable(&self) -> bool {
        !matches!(self.source, CatalogSource::Unavailable)
    }

    /// One line naming the failure, for stderr. Empty when usable.
    ///
    /// Diagnostics never belong on stdout: MCP speaks JSON-RPC there, and a
    /// human-readable line in the protocol stream corrupts the session.
    pub fn diagnostic(&self) -> Option<String> {
        match &self.source {
            CatalogSource::Unavailable => Some(
                "the tool catalogue could not be loaded from disk or from the \
                 copy compiled into this binary; advertising an empty tool \
                 list, which is NOT the same as this product having no tools"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

/// [`load_tool_catalog`], reporting where the catalogue came from.
pub fn load_tool_catalog_report() -> CatalogReport {
    for path in catalog_candidate_paths() {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(parsed) = serde_json::from_str::<CatalogFile>(&text)
        {
            return CatalogReport {
                tools: parsed.tools,
                source: CatalogSource::Disk(path),
            };
        }
    }
    match serde_json::from_str::<CatalogFile>(EMBEDDED_TOOLS) {
        Ok(parsed) => CatalogReport {
            tools: parsed.tools,
            source: CatalogSource::Embedded,
        },
        // A malformed compiled-in catalogue is a build defect, not a runtime
        // condition. Saying so, rather than quietly serving nothing, is the
        // only way anyone finds out.
        Err(_) => CatalogReport {
            tools: Vec::new(),
            source: CatalogSource::Unavailable,
        },
    }
}

/// Load the tool catalog of the running product.
///
/// The catalogue says which tools the product offers and, for each of them,
/// whether it changes something. The read-only posture filters on that second
/// answer, so the catalogue is what decides whether a mutating tool can run —
/// and that makes it a security surface, not a convenience.
///
/// The catalogue is therefore the product's own: the copy that shipped inside
/// this binary, or the one that sits next to this binary in its installation.
/// It is never the copy that happens to sit in the tree the user asked us to
/// analyse. A workspace is data the product reads; data does not get to
/// redefine the product, and least of all to lower its posture.
///
/// Resolution is delegated to [`product_assets_root`], which is the
/// **single** product-asset resolver for the whole workspace: the CLI
/// capabilities document and the MCP `tools/list` catalogue must never be able
/// to disagree about where the shipped data lives.
///
/// Returns an empty vector when the file cannot be found or fails to
/// deserialise; callers that need a fail-closed signal should log
/// their own diagnostics.
pub fn load_tool_catalog() -> Vec<ToolDescriptor> {
    load_tool_catalog_report().tools
}

#[derive(Deserialize)]
struct CatalogFile {
    tools: Vec<ToolDescriptor>,
}

fn catalog_candidate_paths() -> Vec<PathBuf> {
    product_assets_root()
        .map(|root| vec![root.join("product").join("tools.json")])
        .unwrap_or_default()
}

/// Resolve the directory that holds the running product's own data.
///
/// **This is the single source of truth for product-asset location.**
/// `crate::interface::cli::commands::resolve_product_assets_root`
/// delegates here; so does the MCP catalogue. Two resolvers with two
/// answers is how an installed server ends up advertising an empty
/// tool catalogue while the CLI works perfectly against the same
/// binary.
///
/// The anchor is the executable's own location: the tree this binary was
/// installed into, where the product's data ships next to the product.
/// That is the only anchor, and the absence of the others is the point.
///
/// It used to be resolved by walking up from the directory the server was
/// pointed at, then from the process working directory. Both of those are
/// the *workspace*: the tree the user opened. A workspace that ships a
/// `product/` directory of its own was therefore able to state which tools
/// the product has and which of them change something — and the read-only
/// posture believed it. The tree under analysis is data. Data must not be
/// able to lower a posture.
///
/// The copy compiled into the binary is the other source, and the one that
/// always exists: a published binary cannot lose it, whatever the install
/// layout looks like.
///
/// `CARGO_MANIFEST_DIR` is deliberately **not** an anchor. It is a
/// build-time constant: `cargo` sets it while compiling and while
/// running tests, so a test process passes it down to every child it
/// spawns. A published binary never has it. Depending on it makes the
/// test suite green while the deployed server has no catalogue at all
/// — the exact shape of the regression this resolver exists to close,
/// and the reason `a014_product_asset_resolution` forbids it.
pub fn product_assets_root() -> Option<PathBuf> {
    fn holds(root: &Path) -> bool {
        root.join("product").join("tools.json").is_file()
            || root.join("product").join("profiles.json").is_file()
    }

    fn walk_up(start: &Path) -> Option<PathBuf> {
        let mut cursor: Option<&Path> = Some(start);
        // Bounded so a symlink loop cannot turn discovery into a hang.
        let mut hops = 0usize;
        while let Some(dir) = cursor {
            if holds(dir) {
                return Some(dir.to_path_buf());
            }
            cursor = dir.parent();
            hops += 1;
            if hops > 32 {
                break;
            }
        }
        None
    }

    // The installation this binary belongs to.
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    walk_up(parent)
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
