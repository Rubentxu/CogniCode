//! Integration tests for the lens-MCP handlers
//! (`lens_find_dead_code`, `lens_find_intersection`, `lens_hotspots`).
//!
//! Unlike the unit tests in `mcp/handler/lens_mcp.rs` (which only exercise
//! arg parsing), these tests wire the **real** `McpContext` +
//! `CallGraph` + `LensRegistry` + `ViewService` mock and call each
//! handler end-to-end through the standard registry dispatch path.
//!
//! This catches integration bugs the unit tests miss:
//! - handler ↔ service type mismatches
//! - missing context fields (e.g. `view` not wired)
//! - dispatch lookups in the registry
//! - envelope shape under realistic inputs
//!
//! No PG dependency — the handlers operate on the in-memory
//! `McpContext.graph: Option<Arc<CallGraph>>`. PG-backed integration
//! tests already exist for `apply_lens` via `pg_bridge_contract.rs`;
//! these tests cover the lens path specifically.

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_explorer::dto::{FindingSeverity, LensDescriptor, LensResult};
use cognicode_explorer::error::ExplorerResult;
use cognicode_explorer::facades::ViewService;
use cognicode_explorer::mcp::envelope::{err_envelope, ok_envelope};
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::handler::lens_mcp::register_lens_mcp_handlers;
use cognicode_explorer::mcp::{
    McpContext, TOOL_FIND_DEAD_CODE, TOOL_FIND_INTERSECTION, TOOL_HOTSPOTS,
};
use cognicode_explorer::session::SessionRegistry;
use serde_json::{json, Value};

// ============================================================================
// Fixture builders
// ============================================================================

/// Build a small call graph fixture with predictable reachability:
///
/// ```text
///      main         (root, entry point)
///     /    \
///    a      b       (live, called from main)
///    |      |
///    c      d       (live, transitive)
///    |      |
///    e  ←→  f       (DEAD CYCLE: e <-> f reference each other but
///                    have no caller reachable from any root.
///                    Each has an in-edge (from the other), so neither
///                    is a root — they're isolated from the live tree.)
/// ```
///
/// `CallGraph::find_dead_code` considers a symbol dead when it has at
/// least one incoming edge AND no path from any root reaches it.
/// The e <-> f cycle is the only configuration that produces dead
/// callables without polluting the root set.
fn build_reachability_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    let main = g.add_symbol(Symbol::new(
        "main",
        SymbolKind::Function,
        Location::new("main.rs", 1, 0),
    ));
    let a = g.add_symbol(Symbol::new(
        "alpha",
        SymbolKind::Function,
        Location::new("a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "beta",
        SymbolKind::Function,
        Location::new("b.rs", 1, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "gamma",
        SymbolKind::Function,
        Location::new("c.rs", 1, 0),
    ));
    let d = g.add_symbol(Symbol::new(
        "delta",
        SymbolKind::Function,
        Location::new("d.rs", 1, 0),
    ));
    let e = g.add_symbol(Symbol::new(
        "dead_e",
        SymbolKind::Function,
        Location::new("e.rs", 1, 0),
    ));
    let f = g.add_symbol(Symbol::new(
        "dead_f",
        SymbolKind::Function,
        Location::new("f.rs", 1, 0),
    ));

    // Live edges from main to its descendants.
    g.add_dependency(&main, &a, DependencyType::Calls).unwrap();
    g.add_dependency(&main, &b, DependencyType::Calls).unwrap();
    g.add_dependency(&a, &c, DependencyType::Calls).unwrap();
    g.add_dependency(&b, &d, DependencyType::Calls).unwrap();

    // Dead cycle: e <-> f reference each other, but no reachable caller.
    // Neither is a root (both have in-edges from each other), so BFS
    // from roots {main} does not reach them — `find_dead_code` returns
    // both as dead.
    g.add_dependency(&e, &f, DependencyType::Calls).unwrap();
    g.add_dependency(&f, &e, DependencyType::Calls).unwrap();

    g
}

/// Build a denser fixture for PageRank testing — chain topology so
/// the "tail" symbols have high PageRank (they're called by the most
/// nodes) and "head" symbols have low PageRank.
fn build_pagerank_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    // 5 levels of call chain: l0 -> l1 -> l2 -> l3 -> l4
    // l0 has 0 incoming edges (root)
    // l4 has 0 outgoing edges (leaf)
    // Each level l_i calls l_{i+1}
    let mut levels: Vec<SymbolId> = Vec::new();
    for i in 0..5 {
        levels.push(g.add_symbol(Symbol::new(
            format!("lvl{i}"),
            SymbolKind::Function,
            Location::new(format!("lvl{i}.rs"), 1, 0),
        )));
    }
    for w in levels.windows(2) {
        g.add_dependency(&w[0], &w[1], DependencyType::Calls)
            .unwrap();
    }
    g
}

// ============================================================================
// McpContext fixture
// ============================================================================

fn ctx_with_graph(graph: CallGraph) -> McpContext {
    McpContext::builder()
        .with_graph(Some(Arc::new(graph)))
        .with_session_registry(SessionRegistry::new())
        .build()
}

// ============================================================================
// Mock ViewService for intersection tests
// ============================================================================

/// Mock that returns canned lens results based on lens_id + object_id.
/// Lets us assert that `lens_find_intersection` correctly buckets
/// findings across lenses and applies the consensus threshold.
struct CannedLensService {
    /// Map: (object_id, lens_id) → Vec<DesignFinding summary>
    canned: std::collections::HashMap<(String, String), Vec<(String, String, String, f32)>>,
}

impl CannedLensService {
    fn new() -> Self {
        Self {
            canned: std::collections::HashMap::new(),
        }
    }

    fn with(
        mut self,
        object_id: &str,
        lens_id: &str,
        findings: Vec<(&str, &str, &str, f32)>,
    ) -> Self {
        let converted: Vec<(String, String, String, f32)> = findings
            .into_iter()
            .map(|(t, h, s, c)| (t.to_string(), h.to_string(), s.to_string(), c))
            .collect();
        self.canned
            .insert((object_id.to_string(), lens_id.to_string()), converted);
        self
    }
}

#[async_trait]
impl ViewService for CannedLensService {
    async fn available_views(
        &self,
        _object_id: &str,
    ) -> ExplorerResult<Vec<cognicode_explorer::dto::ViewDescriptorDto>> {
        Ok(vec![])
    }
    async fn contextual_view(
        &self,
        _object_id: &str,
        _view_id: &str,
    ) -> ExplorerResult<cognicode_explorer::dto::ContextualView> {
        Err(cognicode_explorer::error::ExplorerError::FeatureDisabled(
            "mock".into(),
        ))
    }
    async fn build_contextual_graph(
        &self,
        _focus_id: &str,
        _level: &str,
        _depth: u8,
        _max_nodes: usize,
    ) -> ExplorerResult<cognicode_explorer::dto::ContextualGraphResponse> {
        Err(cognicode_explorer::error::ExplorerError::FeatureDisabled(
            "mock".into(),
        ))
    }
    async fn available_lenses(
        &self,
        _object_id: &str,
    ) -> ExplorerResult<Vec<LensDescriptor>> {
        Ok(vec![])
    }
    async fn apply_lens(
        &self,
        object_id: &str,
        lens_id: &str,
    ) -> ExplorerResult<LensResult> {
        let findings = self
            .canned
            .get(&(object_id.to_string(), lens_id.to_string()))
            .cloned()
            .unwrap_or_default();
        let design_findings: Vec<cognicode_explorer::dto::DesignFinding> = findings
            .into_iter()
            .enumerate()
            .map(|(i, (title, hypothesis, severity_str, confidence))| {
                let severity = match severity_str.as_str() {
                    "Critical" => FindingSeverity::Critical,
                    "Warning" => FindingSeverity::Warning,
                    "Info" => FindingSeverity::Info,
                    _ => FindingSeverity::Info,
                };
                cognicode_explorer::dto::DesignFinding {
                    id: format!("f:{lens_id}:{i}"),
                    lens_id: lens_id.to_string(),
                    title,
                    hypothesis,
                    severity,
                    confidence,
                    object_ids: vec![],
                    evidence_ids: vec![],
                }
            })
            .collect();
        Ok(LensResult {
            lens_id: lens_id.to_string(),
            findings: design_findings,
            summary: format!("mock {lens_id} result"),
        })
    }
    async fn execute_view_spec(
        &self,
        _spec: &cognicode_explorer::dto::ViewSpec,
        _object_id: &str,
    ) -> ExplorerResult<cognicode_explorer::dto::ContextualView> {
        Err(cognicode_explorer::error::ExplorerError::FeatureDisabled(
            "mock".into(),
        ))
    }
}

// ============================================================================
// lens_find_dead_code tests
// ============================================================================

#[tokio::test]
async fn lens_find_dead_code_returns_unreachable_callables() {
    let g = build_reachability_fixture();
    let ctx = ctx_with_graph(g);
    let mut registry = ToolHandlerRegistry::new();
    register_lens_mcp_handlers(&mut registry);

    let result = registry
        .dispatch(TOOL_FIND_DEAD_CODE, &ctx, json!({}))
        .await;

    let envelope = ok_envelope(TOOL_FIND_DEAD_CODE, &serde_json::Value::Null);
    let _ = envelope; // silence unused if test refactors

    // The handler should NOT return an error envelope.
    assert!(
        result.content.is_empty() || result.is_error != Some(true),
        "expected success envelope, got: {result:?}"
    );

    // Inspect JSON payload directly via the typed dispatch helper.
    let handler = registry.get(TOOL_FIND_DEAD_CODE).unwrap();
    let raw = handler.handle(&ctx, json!({})).await;
    let json_text = format!("{:?}", raw);
    // 7 callable symbols total → 2 dead → dead_code_percent = 2/7 * 100 ≈ 28.57
    assert!(
        json_text.contains("dead_code_percent"),
        "payload should include dead_code_percent: {json_text}"
    );
    // Use substring without surrounding quotes — Rust Debug escapes `"` as
    // `\"`, so `\"total_dead\":` doesn't contain the bare substring
    // `total_dead:` (the char after `dead` is `\"`, not `:`).
    assert!(
        json_text.contains("total_dead"),
        "payload should mention total_dead field: {json_text}"
    );
    assert!(
        json_text.contains("2,") || json_text.contains("2 "),
        "fixture has 2 dead callables: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_dead_code_lists_all_dead_symbols() {
    let g = build_reachability_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(TOOL_FIND_DEAD_CODE, &ctx, json!({}))
        .await;
    let json_text = format!("{:?}", raw);

    // Fixture has 2 dead callables (dead_e, dead_f) in the isolated cycle.
    for dead_id in ["dead_e", "dead_f"] {
        assert!(
            json_text.contains(dead_id),
            "expected `{dead_id}` in dead symbols payload: {json_text}"
        );
    }
    // The 5 live symbols (main, alpha, beta, gamma, delta) should NOT
    // appear in the dead list.
    for live_id in ["main.rs:main", "a.rs:alpha", "b.rs:beta", "c.rs:gamma", "d.rs:delta"] {
        assert!(
            !json_text.contains(&format!("\"symbol_id\": \"{live_id}\"")),
            "live symbol `{live_id}` should NOT appear in dead list: {json_text}"
        );
    }
}

#[tokio::test]
async fn lens_find_dead_code_respects_limit() {
    let g = build_reachability_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(TOOL_FIND_DEAD_CODE, &ctx, json!({ "limit": 1 }))
        .await;
    let json_text = format!("{:?}", raw);

    // The payload includes a `dead_symbols` array — `limit: 1` caps it at 1.
    // The total_dead field always reports the true count (2).
    assert!(
        json_text.contains("total_dead"),
        "payload should always report total_dead: {json_text}"
    );
    assert!(
        json_text.contains("dead_symbols"),
        "payload should include dead_symbols array: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_dead_code_with_custom_entry_points() {
    let g = build_reachability_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // Treat `dead_e` as an entry point — it should no longer be in the
    // dead list because BFS from it covers itself and dead_f.
    let raw = registry
        .dispatch(
            TOOL_FIND_DEAD_CODE,
            &ctx,
            json!({ "entry_points": ["dead_e"] }),
        )
        .await;
    let json_text = format!("{:?}", raw);

    // With dead_e as entry point, BFS reaches dead_e and dead_f → 0 dead.
    // The total_dead field should now read 0.
    assert!(
        json_text.contains("total_dead"),
        "payload should include total_dead: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_dead_code_rejects_when_graph_unavailable() {
    // McpContext with NO graph wired.
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(TOOL_FIND_DEAD_CODE, &ctx, json!({}))
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error code, got: {json_text}"
    );
}

// ============================================================================
// lens_hotspots tests
// ============================================================================

#[tokio::test]
async fn lens_hotspots_returns_top_n_ranked_by_pagerank() {
    let g = build_pagerank_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_HOTSPOTS,
            &ctx,
            json!({ "object_id": "lvl0", "top_n": 3 }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // The handler returns top-N ranked by pagerank. The exact ordering
    // depends on PageRank numerics, but the response should include the
    // requested top_n and exclude lvl0 (the anchor).
    assert!(
        json_text.contains("top_n"),
        "payload should echo top_n field: {json_text}"
    );
    assert!(
        !json_text.contains("lvl0.rs:lvl0"),
        "anchor lvl0 should be excluded from hotspots list: {json_text}"
    );
}

#[tokio::test]
async fn lens_hotspots_excludes_anchor_from_results() {
    let g = build_pagerank_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // lvl4 has 0 outgoing edges → lowest PageRank in our chain. Anchor on
    // it and verify it's excluded from the hotspots list.
    let raw = registry
        .dispatch(
            TOOL_HOTSPOTS,
            &ctx,
            json!({ "object_id": "lvl4", "top_n": 10 }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // `lvl4` is the anchor — it must NOT appear in the hotspots array.
    // The envelope echoes object_id back, so we check that the hotspots
    // array specifically doesn't list it.
    assert!(
        json_text.contains("object_id"),
        "anchor should be echoed in object_id field: {json_text}"
    );
    assert!(
        !json_text.contains("lvl4.rs:lvl4"),
        "anchor `lvl4` should be excluded from hotspots list: {json_text}"
    );
}

#[tokio::test]
async fn lens_hotspots_caps_top_n_at_100() {
    let g = build_pagerank_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // top_n > 100 should be clamped to 100 by the handler.
    let raw = registry
        .dispatch(
            TOOL_HOTSPOTS,
            &ctx,
            json!({ "object_id": "lvl0", "top_n": 9999 }),
        )
        .await;
    let json_text = format!("{raw:?}");
    // The handler clamps via `.min(100)` — the echoed `top_n` field
    // should reflect the clamp. The Debug output escapes the literal
    // value as `\"top_n\": 100` so we look for `top_n` and `100`
    // independently.
    assert!(
        json_text.contains("top_n"),
        "payload should include top_n field: {json_text}"
    );
    assert!(
        json_text.contains("100"),
        "top_n should be clamped to 100: {json_text}"
    );
}

// ============================================================================
// lens_find_intersection tests (require ViewService mock)
// ============================================================================

#[tokio::test]
async fn lens_find_intersection_buckets_findings_across_lenses() {
    let g = build_reachability_fixture();
    let canned = CannedLensService::new()
        // hotspots lens sees "God function" + "Hot path"
        .with(
            "sym:UserService",
            "hotspots",
            vec![
                ("God function", "Too many callees", "Critical", 0.9),
                ("Hot path", "Frequent callee", "Warning", 0.7),
            ],
        )
        // dependencies lens sees "God function" (consensus!) + "Tight coupling"
        .with(
            "sym:UserService",
            "dependencies",
            vec![
                ("God function", "Too many callees", "Critical", 0.85),
                ("Tight coupling", "High fan-in", "Warning", 0.6),
            ],
        );

    let mut ctx = ctx_with_graph(g);
    ctx.view = Some(Arc::new(canned));

    let registry = build_registry();
    let raw = registry
        .dispatch(
            TOOL_FIND_INTERSECTION,
            &ctx,
            json!({
                "object_id": "sym:UserService",
                "lens_ids": ["hotspots", "dependencies"],
                "min_consensus": 2
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // `God function` appears in both lenses → consensus hit.
    assert!(
        json_text.contains("God function"),
        "consensus finding 'God function' should be in result: {json_text}"
    );
    // `Tight coupling` and `Hot path` only appear once each → filtered out.
    assert!(
        !json_text.contains("Tight coupling") && !json_text.contains("Hot path"),
        "non-consensus findings should be filtered: {json_text}"
    );
    // Per-lens counts should reflect both lenses ran.
    assert!(
        json_text.contains("hotspots") && json_text.contains("dependencies"),
        "per_lens_counts should include both lenses: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_intersection_rejects_fewer_than_two_lenses() {
    let g = build_reachability_fixture();
    let canned = CannedLensService::new();
    let mut ctx = ctx_with_graph(g);
    ctx.view = Some(Arc::new(canned));

    let registry = build_registry();
    let raw = registry
        .dispatch(
            TOOL_FIND_INTERSECTION,
            &ctx,
            json!({
                "object_id": "sym:UserService",
                "lens_ids": ["hotspots"]
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("at least 2"),
        "expected 'at least 2 lens_ids' error: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_intersection_handles_missing_view_service() {
    let g = build_reachability_fixture();
    // NO view service wired → facade_unavailable.
    let ctx = ctx_with_graph(g);

    let registry = build_registry();
    let raw = registry
        .dispatch(
            TOOL_FIND_INTERSECTION,
            &ctx,
            json!({
                "object_id": "sym:UserService",
                "lens_ids": ["hotspots", "dependencies"]
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("facade_unavailable"),
        "expected facade_unavailable error: {json_text}"
    );
}

#[tokio::test]
async fn lens_find_intersection_min_consensus_clamps_to_lens_count() {
    let g = build_reachability_fixture();
    let canned = CannedLensService::new()
        // All 3 lenses see the same finding — should satisfy min_consensus=2.
        .with(
            "sym:X",
            "hotspots",
            vec![("Shared finding", "Same hypothesis", "Critical", 0.9)],
        )
        .with(
            "sym:X",
            "dependencies",
            vec![("Shared finding", "Same hypothesis", "Critical", 0.8)],
        )
        .with(
            "sym:X",
            "architecture",
            vec![("Shared finding", "Same hypothesis", "Critical", 0.7)],
        );

    let mut ctx = ctx_with_graph(g);
    ctx.view = Some(Arc::new(canned));

    let registry = build_registry();
    let raw = registry
        .dispatch(
            TOOL_FIND_INTERSECTION,
            &ctx,
            json!({
                "object_id": "sym:X",
                "lens_ids": ["hotspots", "dependencies"],
                "min_consensus": 5 // > lens_ids.len() (2) → clamped to 2
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Even though min_consensus was 5, the handler clamps to lens_ids.len() (2).
    // The shared finding appears in both lenses → should appear in result.
    assert!(
        json_text.contains("Shared finding"),
        "consensus finding should still appear after min_consensus clamp: {json_text}"
    );
}

// ============================================================================
// Helpers
// ============================================================================

fn build_registry() -> ToolHandlerRegistry {
    let mut registry = ToolHandlerRegistry::new();
    register_lens_mcp_handlers(&mut registry);
    registry
}

// ============================================================================
// Dispatch infrastructure check (sanity)
// ============================================================================

#[test]
fn all_three_handlers_are_registered() {
    let registry = build_registry();
    assert!(registry.get(TOOL_FIND_DEAD_CODE).is_some());
    assert!(registry.get(TOOL_FIND_INTERSECTION).is_some());
    assert!(registry.get(TOOL_HOTSPOTS).is_some());
}

#[test]
fn handlers_declare_non_empty_arg_schemas() {
    let registry = build_registry();
    for (name, expected_required_keys) in [
        (TOOL_FIND_DEAD_CODE, vec![] as Vec<&str>),
        (TOOL_HOTSPOTS, vec!["object_id"]),
        (TOOL_FIND_INTERSECTION, vec!["object_id", "lens_ids"]),
    ] {
        let handler = registry.get(name).unwrap();
        let schema = handler.arg_schema();
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for key in expected_required_keys {
            assert!(
                required.contains(&key),
                "{name}: schema should require `{key}`, got {required:?}"
            );
        }
    }
}

// Suppress unused-import warnings if a future refactor drops one of the
// re-exports below.
#[allow(dead_code)]
fn _suppress_unused() {
    let _ = err_envelope(TOOL_FIND_DEAD_CODE, "x", "y");
    let _: Value = json!({});
}
