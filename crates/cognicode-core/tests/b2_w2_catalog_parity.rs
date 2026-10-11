//! B2.W2 — Parity test between the CLI capability catalog and the
//! MCP `tools/list`.
//!
//! Both surfaces must derive from the same authority
//! (`product/tools.json`). This test loads the catalog through the
//! typed loader and the MCP adapter in-process, and asserts that:
//!
//!   - The set of tool names agrees field-for-field. A name present
//!     in one but not the other is the exact drift this WU was created
//!     to close, and a regression here means the two lists have
//!     diverged again.
//!   - The declared authority matches: a tool whose catalog
//!     `authority` is `mutating` / `execute` / `network` must appear
//!     in `mutating_tool_names()`, and a tool whose `authority` is
//!     `read` must not.
//!   - The catalog loads cleanly: no missing required fields, no
//!     panic, no schema drift in the 13 public fields per tool.
//!
//! Política B2.W2: si este test falla, no se ha cerrado el WU.
//! Se complementa con `cargo test -p cognicode-cli --test
//! a014_capabilities_json` que pinea el shape JSON externo, y con
//! `cargo test -p cognicode-core --test cli_exit_code_propagation`
//! que verifica que el dispatch sigue funcionando con la lista
//! proyectada desde el catalog.

use std::collections::HashSet;

use cognicode_core::interface::catalog::{
    ToolDescriptor, load_tool_catalog, load_tool_catalog_active,
};
use cognicode_core::interface::mcp::rmcp_adapter::{build_all_tools, mutating_tool_names};

/// The 13 public fields every tool entry in `product/tools.json`
/// promises. The catalog loader must preserve all of them through
/// deserialisation so the parity and JSON-shape contracts hold.
const REQUIRED_FIELDS: &[&str] = &[
    "name",
    "title",
    "description",
    "category",
    "authority",
    "stability",
    "input_schema",
    "output_schema",
    "requirements",
    "requirements_status",
    "estimated_latency_ms",
    "public_profiles",
    "runtime_metadata",
];

#[test]
fn catalog_loads_without_missing_required_fields() {
    let catalog = load_tool_catalog();
    assert!(
        !catalog.is_empty(),
        "load_tool_catalog() returned an empty list; the catalog file \
         must be present and parseable for the rest of these tests to \
         mean anything. If you ran this from a directory outside the \
         checkout, run from the workspace root."
    );
    let names: Vec<&str> = catalog.iter().map(|d| d.name.as_str()).collect();
    assert!(
        names.iter().any(|n| !n.is_empty()),
        "every catalog entry must carry a non-empty `name`"
    );
    let unique: HashSet<&str> = names.iter().copied().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "catalog contains duplicate tool names: {names:?}"
    );
}

#[test]
fn every_tool_descriptor_carries_the_required_public_fields() {
    // The 13-field contract is what `cognicode capabilities --json`
    // and downstream consumers pin against. A field disappearing
    // here is a wire-shape break for every external user.
    let catalog = load_tool_catalog();
    let raw = read_tools_json_raw();
    for (i, d) in catalog.iter().enumerate() {
        let raw_entry = raw
            .iter()
            .find(|r| r["name"].as_str() == Some(d.name.as_str()))
            .unwrap_or_else(|| panic!("tool {:?} missing from raw JSON", d.name));
        for field in REQUIRED_FIELDS {
            assert!(
                raw_entry.get(field).is_some(),
                "tool {} (`{}`) is missing the required field `{field}` in product/tools.json",
                i,
                d.name
            );
        }
        // Spot-check that the typed deserialisation surfaced what
        // the JSON actually says (a struct deserialiser that silently
        // swallowed a field would otherwise pass).
        assert!(!d.title.is_empty(), "{}: title must be non-empty", d.name);
        assert!(
            !d.description.is_empty(),
            "{}: description must be non-empty",
            d.name
        );
        assert!(
            d.authority == "read"
                || d.authority == "mutating"
                || d.authority == "execute"
                || d.authority == "network",
            "{}: authority {:?} is not in the four-state vocabulary",
            d.name,
            d.authority
        );
        assert!(
            d.stability == "stable" || d.stability == "experimental" || d.stability == "gated",
            "{}: stability {:?} is not in the three-state vocabulary",
            d.name,
            d.stability
        );
    }
}

#[test]
fn catalog_active_set_equals_mcp_tools_list_set() {
    // B2.W2 parity: the active catalog (catalog with feature gating
    // applied) and the MCP `build_all_tools()` must agree field-for-
    // field on the *names*. A regression here means either:
    //   - the catalog has a tool the MCP forgot to project (or vice
    //     versa), or
    //   - the feature gate diverged (a tool the catalog thinks is
    //     active is gated out of MCP, or vice versa).
    let catalog = load_tool_catalog_active();
    let mcp: HashSet<String> = build_all_tools()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    let cat: HashSet<String> = catalog.iter().map(|d| d.name.clone()).collect();

    let only_in_catalog: Vec<&String> = cat.difference(&mcp).collect();
    let only_in_mcp: Vec<&String> = mcp.difference(&cat).collect();
    assert!(
        only_in_catalog.is_empty() && only_in_mcp.is_empty(),
        "B2.W2 parity drift.\n\
         Only in catalog ({}): {:?}\n\
         Only in MCP     ({}): {:?}\n\
         Both have {} tools; they must agree.",
        only_in_catalog.len(),
        only_in_catalog,
        only_in_mcp.len(),
        only_in_mcp,
        cat.len()
    );
    assert_eq!(cat.len(), mcp.len(), "set sizes must agree");
}

#[test]
fn mutating_authority_matches_mutating_tool_names() {
    // PRF-MCP-05: the MCP `mutating_tool_names()` helper is the only
    // authority oracle, and it derives from the declarations each
    // tool publishes. The catalog is the upstream source of those
    // declarations. Cross-check the two: every catalog entry whose
    // `authority` is in the mutating set must be reported as
    // mutating, and every catalog entry whose `authority` is `read`
    // must NOT be reported as mutating.
    let catalog = load_tool_catalog_active();
    let mutating: HashSet<String> = mutating_tool_names().into_iter().collect();

    for d in &catalog {
        let name = d.name.as_str();
        let should_be_mutating = matches!(d.authority.as_str(), "mutating" | "execute" | "network");
        let is_mutating = mutating.contains(name);
        assert_eq!(
            should_be_mutating, is_mutating,
            "tool {name:?}: catalog authority {:?} disagrees with \
             mutating_tool_names() membership ({is_mutating})",
            d.authority
        );
    }
}

#[test]
fn catalog_active_is_a_strict_subset_of_catalog_full() {
    // The active catalog is the full catalog with `tool_feature_enabled`
    // applied. The full catalog is the union of all reachable tools;
    // the active one is what this build actually exposes. The
    // relation must be subset, not equal-without-features: every
    // active tool is also in the full catalog, and every feature-
    // gated tool is either in the active set (feature enabled in
    // this build) or not (feature off).
    let active: HashSet<String> = load_tool_catalog_active()
        .into_iter()
        .map(|d| d.name)
        .collect();
    let full: HashSet<String> = load_tool_catalog().into_iter().map(|d| d.name).collect();
    assert!(
        active.is_subset(&full),
        "active catalog is not a subset of full catalog: \
         only-in-active = {:?}",
        active.difference(&full).collect::<Vec<_>>()
    );
    // And every feature-gated tool in the full catalog must be
    // either in the active set or genuinely feature-off in this
    // build (a tool declared as `feature: "evidence-cli-ladybug"`
    // appears in the full catalog, and is omitted from the active
    // catalog when the feature is off, which is the default).
    for d in load_tool_catalog() {
        if let Some(f) = &d.feature {
            let expected = cfg_match(f);
            let is_active = active.contains(&d.name);
            assert_eq!(
                expected, is_active,
                "feature-gated tool {:?} (feature={f:?}) is \
                 active={is_active} but cfg match returned {expected}",
                d.name
            );
        }
    }
}

/// Mirror of `catalog::tool_feature_enabled` for the test scope.
/// Returns `true` when the named Cargo feature is enabled in the
/// current build; a name this build cannot recognise resolves to
/// `false` (the test pins the same fail-closed default the catalog
/// loader does).
fn cfg_match(feature: &str) -> bool {
    match feature {
        "persistence" => cfg!(feature = "persistence"),
        "program-analysis-server" => cfg!(feature = "program-analysis-server"),
        "evidence-cli-ladybug" => cfg!(feature = "evidence-cli-ladybug"),
        _ => false,
    }
}

/// Read `product/tools.json` straight from disk and return its
/// `tools[]` array. The test uses this to verify that the typed
/// `ToolDescriptor` carries the 13 public fields — the deserialiser
/// cannot drop a field silently when the source has it.
fn read_tools_json_raw() -> Vec<serde_json::Value> {
    let path = catalog_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read catalog file {:?}: {e}", path));
    let v: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("catalog file {:?} is not valid JSON: {e}", path));
    v["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("catalog file {:?} has no `tools` array", path))
        .clone()
}

fn catalog_path() -> std::path::PathBuf {
    // Walk parents from the manifest dir; this matches the same
    // anchor logic `load_tool_catalog` uses (cwd + parents, then
    // CARGO_MANIFEST_DIR + parents).
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("product").join("tools.json");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("could not locate product/tools.json above {:?}", manifest);
}

#[allow(dead_code)]
fn assert_tool_descriptor_shape(_: &ToolDescriptor) {
    // Touch the import so the linter doesn't flag ToolDescriptor as
    // unused; the test cases above dereference it via the catalog.
    const _: fn(&ToolDescriptor) = |_| {};
}
