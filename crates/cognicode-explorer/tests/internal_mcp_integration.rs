//! Integration tests for the internal-MCP handlers
//! (`find_dead_code_v2`, `find_cycles`, `health_dashboard`).
//!
//! Unlike the unit tests in `mcp/handler/internal_mcp.rs` (which only
//! exercise arg parsing + DTO round-trips), these tests wire the **real**
//! `McpContext` + `CallGraph` and call each handler end-to-end through
//! the standard registry dispatch path.
//!
//! This catches integration bugs the unit tests miss:
//! - handler ↔ CallGraph API mismatches
//! - dispatch lookups in the registry
//! - filter logic (confidence threshold, min_scc_size) actually applied
//! - `graph_unavailable` error envelope shape when no graph is loaded
//! - envelope shape (`ok` vs `err`, payload fields) under realistic inputs
//!
//! Mirrors the pattern established by `lens_mcp_integration.rs` for the
//! lens-MCP handlers.

use std::sync::Arc;

use cognicode_core::domain::aggregates::{CallGraph, Symbol};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::handler::internal_mcp::register_internal_mcp_handlers;
use cognicode_explorer::mcp::{
    McpContext, TOOL_FIND_CYCLES, TOOL_FIND_DEAD_CODE_V2, TOOL_HEALTH_DASHBOARD,
};
use cognicode_explorer::session::SessionRegistry;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};

/// Extract the JSON payload from a `CallToolResult` and assert its shape.
/// Both `ok_envelope` and `err_envelope` produce a JSON object with
/// `tool_name`, `payload`/`error`, `version`, `timestamp` fields.
fn extract_env(result: &CallToolResult) -> Value {
    let text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .expect("CallToolResult should contain a text content");
    serde_json::from_str(text).expect("response text must be JSON")
}

/// Assert the result envelope is `ok` and return its `payload`.
fn ok_payload(result: &CallToolResult) -> Value {
    assert_eq!(
        result.is_error,
        Some(false),
        "expected ok envelope, got: {result:?}"
    );
    let env = extract_env(result);
    env.get("payload")
        .cloned()
        .expect("ok envelope must have a `payload` field")
}

/// Assert the result envelope is `err` and return the error code.
fn err_code(result: &CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(true),
        "expected err envelope, got: {result:?}"
    );
    let env = extract_env(result);
    env.get("payload")
        .and_then(|p| p.get("error_code"))
        .and_then(|c| c.as_str())
        .map(String::from)
        .expect("err envelope payload must have `error_code`")
}

// ============================================================================
// Fixture builders
// ============================================================================

/// Build a mixed fixture combining a live DAG with a dead cycle:
///
/// ```text
///      main       (root, entry point)
///     /    \
///    a      b     (live)
///    |      |
///    c      d     (live)
///    |      |
///    e  ←→  f     (DEAD CYCLE: e <-> f reference each other but have no
///                  reachable caller. Each has 1 incoming edge (from the
///                  other), so fan_in > 0 → confidence 0.5 in v2.)
/// ```
///
/// Total: 7 symbols, 5 live (reachable from main), 2 dead (e, f).
fn build_mixed_fixture() -> CallGraph {
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

    g.add_dependency(&main, &a, DependencyType::Calls).unwrap();
    g.add_dependency(&main, &b, DependencyType::Calls).unwrap();
    g.add_dependency(&a, &c, DependencyType::Calls).unwrap();
    g.add_dependency(&b, &d, DependencyType::Calls).unwrap();
    g.add_dependency(&e, &f, DependencyType::Calls).unwrap();
    g.add_dependency(&f, &e, DependencyType::Calls).unwrap();

    g
}

/// Build a pure DAG (no cycles, no dead code) — used for negative tests.
fn build_dag_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    let a = g.add_symbol(Symbol::new(
        "a",
        SymbolKind::Function,
        Location::new("a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "b",
        SymbolKind::Function,
        Location::new("b.rs", 1, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "c",
        SymbolKind::Function,
        Location::new("c.rs", 1, 0),
    ));
    g.add_dependency(&a, &b, DependencyType::Calls).unwrap();
    g.add_dependency(&b, &c, DependencyType::Calls).unwrap();
    g
}

/// Build a fixture with many standalone (zero-connection) symbols.
/// Used for health_dashboard stale-symbol detection.
fn build_stale_heavy_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    // 2 connected symbols (the live "tree")
    let root = g.add_symbol(Symbol::new(
        "root",
        SymbolKind::Function,
        Location::new("root.rs", 1, 0),
    ));
    let child = g.add_symbol(Symbol::new(
        "child",
        SymbolKind::Function,
        Location::new("child.rs", 1, 0),
    ));
    g.add_dependency(&root, &child, DependencyType::Calls)
        .unwrap();
    // 8 standalone symbols (no edges) → "stale"
    for i in 0..8 {
        g.add_symbol(Symbol::new(
            format!("orphan_{i}"),
            SymbolKind::Function,
            Location::new(format!("orphan_{i}.rs"), 1, 0),
        ));
    }
    g
}

// ============================================================================
// McpContext + registry fixtures
// ============================================================================

fn ctx_with_graph(graph: CallGraph) -> McpContext {
    McpContext::builder()
        .with_graph(Some(Arc::new(graph)))
        .with_session_registry(SessionRegistry::new())
        .build()
}

fn build_registry() -> ToolHandlerRegistry {
    let mut r = ToolHandlerRegistry::new();
    register_internal_mcp_handlers(&mut r);
    r
}

// ============================================================================
// find_dead_code_v2 — end-to-end dispatch
// ============================================================================

#[tokio::test]
async fn find_dead_code_v2_lists_all_dead_callables() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_DEAD_CODE_V2, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    // total_dead = 2 (e + f)
    assert_eq!(
        payload["total_dead"].as_u64(),
        Some(2),
        "mixed fixture has exactly 2 dead symbols: {payload}"
    );
    assert!(
        payload["dead_code_percent"].is_number(),
        "payload should include dead_code_percent: {payload}"
    );

    // Both dead_e and dead_f should appear in the dead_code array
    let dead_ids: Vec<String> = payload["dead_code"]
        .as_array()
        .expect("dead_code should be array")
        .iter()
        .filter_map(|entry| entry["symbol_id"].as_str().map(String::from))
        .collect();
    assert!(
        dead_ids.iter().any(|s| s.contains("dead_e")),
        "dead_e missing from dead_code: {dead_ids:?}"
    );
    assert!(
        dead_ids.iter().any(|s| s.contains("dead_f")),
        "dead_f missing from dead_code: {dead_ids:?}"
    );

    // Live symbols must NOT appear in dead_code
    for live in ["main", "alpha", "beta", "gamma", "delta"] {
        assert!(
            !dead_ids.iter().any(|s| s.contains(live)),
            "live symbol `{live}` should NOT appear in dead_code: {dead_ids:?}"
        );
    }
}

#[tokio::test]
async fn find_dead_code_v2_respects_limit() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_DEAD_CODE_V2, &ctx, json!({ "limit": 1 }))
        .await;
    let payload = ok_payload(&result);

    // total_dead must still report the underlying count, independent of limit.
    assert_eq!(
        payload["total_dead"].as_u64(),
        Some(2),
        "total_dead must reflect actual count, not limit: {payload}"
    );
    // dead_code array is capped to `limit` entries.
    assert_eq!(
        payload["dead_code"].as_array().unwrap().len(),
        1,
        "limit=1 should cap dead_code to 1 entry: {payload}"
    );
}

#[tokio::test]
async fn find_dead_code_v2_confidence_threshold_filters() {
    // In the mixed fixture, dead_e and dead_f each have fan_in=1
    // (each other), so v2 assigns confidence = 0.5. With threshold=0.9
    // both should be filtered out → empty dead_code.
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_DEAD_CODE_V2,
            &ctx,
            json!({ "confidence_threshold": 0.9 }),
        )
        .await;
    let payload = ok_payload(&result);

    // total_dead still reports the underlying count.
    assert_eq!(
        payload["total_dead"].as_u64(),
        Some(2),
        "total_dead reports raw count regardless of threshold: {payload}"
    );

    // The dead_code array should be empty (filtered by threshold).
    assert_eq!(
        payload["dead_code"].as_array().unwrap().len(),
        0,
        "confidence_threshold=0.9 should filter both dead entries (conf=0.5): {payload}"
    );
    // Reported threshold should be clamped to ≤1.0.
    let reported_threshold = payload["confidence_threshold"].as_f64().unwrap();
    assert!(
        (0.0..=1.0).contains(&reported_threshold),
        "reported threshold must be in [0.0, 1.0]: {reported_threshold}"
    );
}

#[tokio::test]
async fn find_dead_code_v2_confidence_threshold_zero_keeps_all() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_DEAD_CODE_V2,
            &ctx,
            json!({ "confidence_threshold": 0.0 }),
        )
        .await;
    let payload = ok_payload(&result);

    let dead_count = payload["dead_code"].as_array().unwrap().len();
    assert_eq!(
        dead_count, 2,
        "threshold=0.0 must keep all entries: {payload}"
    );
}

#[tokio::test]
async fn find_dead_code_v2_invalid_args_returns_envelope() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // limit must be an integer; passing a string is invalid.
    let result = registry
        .dispatch(
            TOOL_FIND_DEAD_CODE_V2,
            &ctx,
            json!({ "limit": "not-a-number" }),
        )
        .await;
    assert_eq!(
        err_code(&result),
        "invalid_args",
        "invalid args should yield invalid_args error code"
    );
}

#[tokio::test]
async fn find_dead_code_v2_rejects_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_DEAD_CODE_V2, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "graph_unavailable",
        "missing graph should yield graph_unavailable"
    );
}

// ============================================================================
// find_cycles — end-to-end dispatch
// ============================================================================

#[tokio::test]
async fn find_cycles_detects_mutual_cycle() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_CYCLES, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    // Mutual e↔f cycle is the only SCC of size >= 2.
    assert_eq!(
        payload["total_cycles"].as_u64(),
        Some(1),
        "exactly one cycle expected: {payload}"
    );
    assert_eq!(
        payload["longest_cycle_length"].as_u64(),
        Some(2),
        "longest cycle has length 2: {payload}"
    );

    let cycle_ids: Vec<String> = payload["cycles"][0]["symbol_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    assert!(
        cycle_ids.iter().any(|s| s.contains("dead_e"))
            && cycle_ids.iter().any(|s| s.contains("dead_f")),
        "cycle payload should include both e and f: {cycle_ids:?}"
    );
}

#[tokio::test]
async fn find_cycles_min_scc_size_filters_small_cycles() {
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // The e↔f cycle has length 2. With min_scc_size=3 it must be filtered.
    let result = registry
        .dispatch(TOOL_FIND_CYCLES, &ctx, json!({ "min_scc_size": 3 }))
        .await;
    let payload = ok_payload(&result);

    assert_eq!(
        payload["total_cycles"].as_u64(),
        Some(0),
        "min_scc_size=3 should filter the only size-2 cycle: {payload}"
    );
    assert_eq!(
        payload["longest_cycle_length"].as_u64(),
        Some(0),
        "longest should be 0 when all cycles are filtered: {payload}"
    );
}

#[tokio::test]
async fn find_cycles_no_cycles_in_dag() {
    let g = build_dag_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_CYCLES, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    assert_eq!(
        payload["total_cycles"].as_u64(),
        Some(0),
        "a DAG has no SCCs of size >= 2: {payload}"
    );
}

#[tokio::test]
async fn find_cycles_rejects_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_CYCLES, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "graph_unavailable",
        "missing graph should yield graph_unavailable"
    );
}

// ============================================================================
// health_dashboard — end-to-end dispatch
// ============================================================================

#[tokio::test]
async fn health_dashboard_clean_dag_returns_high_score() {
    let g = build_dag_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    // DAG: no dead code (all reachable from roots), no cycles.
    let score = payload["health_score"].as_f64().unwrap();
    assert!(
        (0.99..=1.0).contains(&score),
        "clean DAG should have health_score near 1.0, got {score}: {payload}"
    );

    let findings = payload["findings"].as_array().unwrap();
    assert!(
        findings.is_empty(),
        "clean DAG should have no findings: {payload}"
    );

    // Basic stats
    assert_eq!(payload["symbols"]["total"].as_u64(), Some(3));
    assert_eq!(payload["symbols"]["indexed"].as_u64(), Some(3));
    assert_eq!(payload["symbols"]["stale"].as_u64(), Some(0));
    assert_eq!(payload["edges"]["total"].as_u64(), Some(2));
}

#[tokio::test]
async fn health_dashboard_with_dead_code_emits_finding() {
    // Mixed fixture: 7 symbols, 2 dead → 28.6% dead rate → critical finding.
    let g = build_mixed_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    let findings = payload["findings"].as_array().unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f["severity"].as_str() == Some("critical")),
        "mixed fixture (28.6% dead) should emit critical finding: {payload}"
    );
    let titles: Vec<&str> = findings
        .iter()
        .filter_map(|f| f["title"].as_str())
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("dead-code")),
        "finding should mention dead-code: {titles:?}"
    );
}

#[tokio::test]
async fn health_dashboard_with_cycles_emits_finding() {
    // 3-cycle, all reachable from itself (no dead code), cycles present.
    let mut g = CallGraph::new();
    let a = g.add_symbol(Symbol::new(
        "a",
        SymbolKind::Function,
        Location::new("a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "b",
        SymbolKind::Function,
        Location::new("b.rs", 1, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "c",
        SymbolKind::Function,
        Location::new("c.rs", 1, 0),
    ));
    g.add_dependency(&a, &b, DependencyType::Calls).unwrap();
    g.add_dependency(&b, &c, DependencyType::Calls).unwrap();
    g.add_dependency(&c, &a, DependencyType::Calls).unwrap();

    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    let titles: Vec<&str> = payload["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["title"].as_str())
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("cyclic dependency")),
        "3-cycle should emit cycle finding: {titles:?}"
    );
}

#[tokio::test]
async fn health_dashboard_stale_symbols_emits_warning() {
    // 10 symbols, 2 indexed, 8 stale (80% stale > 50% threshold) → warning.
    let g = build_stale_heavy_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    let titles: Vec<&str> = payload["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["title"].as_str())
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("stale")),
        "8/10 stale symbols should emit stale finding: {titles:?}"
    );
}

#[tokio::test]
async fn health_dashboard_health_score_in_unit_interval() {
    // Both extremes: clean DAG (near 1.0) and stale-heavy (near 0).
    // Run both and assert the score is always in [0.0, 1.0].

    let cases: Vec<(&str, CallGraph)> = vec![
        ("dag", build_dag_fixture()),
        ("mixed", build_mixed_fixture()),
        ("stale_heavy", build_stale_heavy_fixture()),
    ];

    for (label, g) in cases {
        let ctx = ctx_with_graph(g);
        let registry = build_registry();
        let result = registry
            .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
            .await;
        let payload = ok_payload(&result);
        let score = payload["health_score"].as_f64().unwrap();
        assert!(
            (0.0..=1.0).contains(&score),
            "[{label}] health_score must be in [0.0, 1.0], got {score}: {payload}"
        );
    }
}

#[tokio::test]
async fn health_dashboard_rejects_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_HEALTH_DASHBOARD, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "graph_unavailable",
        "missing graph should yield graph_unavailable"
    );
}