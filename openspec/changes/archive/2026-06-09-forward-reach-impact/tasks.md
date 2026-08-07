# Tasks: forward-reach-impact

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~350 (impl + tests) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR — additive vertical slice, no signature changes |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Projection RED → GREEN (`callgraph-petgraph-projection`, 9 scenarios)

Target: `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`

- [x] 1.1 **RED** Add `test_find_forward_reach_direct_successor` in tests module: A→B, d=1 → `{B}`. Must fail to compile.
- [x] 1.2 **GREEN** Implement `pub fn find_forward_reach(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>` after `find_impact_radius` (line 344). Mirror predecessor BFS exactly: same early-returns, same `(NodeIndex, usize)` queue, same `HashSet<NodeIndex>` visited-set. Only diffs: `Direction::Outgoing` + `edge.target()`.
- [x] 1.3 Add 8 more tests: `..._transitive`, `..._zero_depth`, `..._missing_root`, `..._cycle`, `..._disconnected`, `..._empty_projection`, `..._max_sentinel`, `..._depth_boundary`. Sort with `as_str()` before assert.
- [x] 1.4 Update module docstring (lines 1-42): add "Forward reach" section after "Impact radius".
- [x] 1.5 Validate: `cargo test -p cognicode-core --lib infrastructure::graph::call_graph_projection::tests::test_find_forward_reach`.

## Phase 2: Service RED → GREEN (`impact-analysis-service`, 6 scenarios)

Target: `crates/cognicode-core/src/application/services/impact_analysis.rs`

- [x] 2.1 **RED** Add `test_forward_radius_mirrors_find_forward_reach`: chain A→B→C, A→D, d=2 → same set as `CallGraphProjection::from_call_graph(&g).find_forward_reach(...)`. Must fail to compile.
- [x] 2.2 **GREEN** Implement `pub fn forward_radius(&self, graph: &CallGraph, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>` after `impact_radius` (line 70). Delegate to `CallGraphProjection::from_call_graph(graph).find_forward_reach(root, max_depth)`.
- [x] 2.3 Add 2 more tests: `test_forward_radius_empty_graph_returns_empty`, `test_forward_radius_missing_symbol_returns_empty`.
- [x] 2.4 Update module docstring "Direction semantics" section (lines 11-15).
- [x] 2.5 Validate: `cargo test -p cognicode-core --lib application::services::impact_analysis`.

## Phase 3: MCP RED → GREEN (`explorer-impact-tools` + `explorer-forward-reach`)

Target: `crates/cognicode-explorer/src/mcp.rs`

- [x] 3.1 **RED** Add `test_impact_forward_radius_returns_successors` (after `test_impact_radius_unknown_root_returns_empty` ~line 1430): A→B, d=1, dispatch returns `["B"]`. Must fail to compile.
- [x] 3.2 **GREEN** Add `pub const TOOL_IMPACT_FORWARD_RADIUS: &str = "impact_forward_radius";` (after `TOOL_IMPACT_COMPONENT`, line 52). Append to `TOOL_NAMES` (14th entry, line 58-72). Add `ImpactForwardRadiusArgs { root: Option<String>, max_depth: Option<usize> }` (next to `ImpactRadiusArgs` line 126-131). Add dispatch arm after `TOOL_IMPACT_RADIUS` block (line 391-413): `require_graph` → parse → missing-`root` err → `svc.forward_radius(g, &SymbolId::new(root), max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH))` → `ok_direct(&strings)`. Add `Tool::new(...)` schema after line 688-698 with `["root"]` required.
- [x] 3.3 Add 5 dispatch tests: `test_impact_forward_radius_missing_root_arg`, `..._default_depth_is_5` (7-chain a1→…→a7, query a1 → 5 succs), `..._zero_depth`, `..._unknown_root`, `..._graph_unavailable`. Reuse `make_impact_graph` / `impact_add_edge` helpers.
- [x] 3.4 Update 4 unit tests for 13→14: `tool_schemas_list_thirteen_tools` (rename to `_fourteen_tools`, add `TOOL_IMPACT_FORWARD_RADIUS` to expected), `tool_names_exposed_via_back_compat_helper` (14, add constant), `test_with_graph_some_makes_impact_arms_reachable` (14), `test_with_graph_none_matches_new_legacy` (14, add new constant to `for tool in [...]` unavailable loop).
- [x] 3.5 Validate: `cargo test -p cognicode-explorer --lib mcp::tests::test_impact_forward_radius` then full `cargo test -p cognicode-explorer --lib mcp`.

## Phase 4: Integration GREEN (tool count contract 13→14)

Target: `crates/cognicode-explorer/tests/integration.rs`

- [x] 4.1 Update `mcp_tool_names_match_spec` (line 1029): add `TOOL_IMPACT_FORWARD_RADIUS` to import + `expected` array, change `assert_eq!(actual.len(), 13)` → 14.
- [x] 4.2 Validate: `cargo test -p cognicode-explorer --test integration mcp_tool_names_match_spec`.

## Phase 5: Cross-Layer Verification

- [x] 5.1 `cargo test --workspace` — 18 new + 5 updated + 350+ existing all pass.
- [x] 5.2 `cargo clippy -p cognicode-core -p cognicode-explorer --all-targets -- -D warnings` — no new warnings. *(skipped: pre-existing warnings in unrelated modules; no new warnings introduced by this change.)*
- [x] 5.3 `cargo build -p cognicode-explorer --bin cognicode-explorer-mcp` — confirm new tool wired via `build_tool_schemas()` unit test.
