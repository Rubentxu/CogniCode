# Tasks: MCP Impact Tools

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 280–360 (excluding tests); ~500 with tests |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr (user override → TDD-always) |
| Chain strategy | size-exception (auto) |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Add 5 impact tools + handler field + helpers + tests in one additive commit | PR 1 | Pure extension; pre-change test count unchanged; includes its own tests & integration rewrite |

> Even at the upper bound (~500 with tests) the change is a single additive diff against one crate. Splitting would force an interim "no graph" state that the spec explicitly forbids breaking. Keep as one PR; `size:exception` already pre-approved by user override.

---

## Phase 0: RED Gate (compile-fail first)

> The implementation **MUST NOT** begin until the RED-gate test below fails to compile. This anchors TDD ordering and prevents drift.

- [ ] 0.1 In `crates/cognicode-explorer/src/mcp.rs` `#[cfg(test)] mod tests`, add the RED-gate test exactly as specified:
  - Name: `test_handler_without_graph_returns_impact_unavailable`
  - Body uses `ExplorerMcpHandler::new(service)`, dispatches `dispatch(&handler.service(), &None, call_tool_args(TOOL_IMPACT_RADIUS, json!({"root":"x","max_depth":1})))`
  - Asserts `result.is_error == true` and `first_text(&result).contains("impact analysis unavailable")`
- [ ] 0.2 **RED check**: `cargo test -p cognicode-explorer --no-run` — must FAIL with compile error (missing `TOOL_IMPACT_RADIUS` const, wrong `dispatch` arity)
- [ ] 0.3 **GREEN check** (do not run yet — gate only): confirm the failure mode is exactly the expected compile error, not a runtime panic in unrelated code

> **Halt here.** Do not proceed to Phase 1 until the RED-gate test fails to compile as predicted. This is the user-mandated TDD checkpoint.

---

## Phase 1: Foundation (constants, handler field, dispatch signature)

> Smallest possible scaffolding to make the RED-gate test compile and turn GREEN.

- [ ] 1.1 In `crates/cognicode-explorer/src/mcp.rs` (top of file, after existing `TOOL_QUERY_MOLDQL`), add 5 `pub const TOOL_IMPACT_*` strings + `pub const DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5;`
- [ ] 1.2 Append the 5 constants to `pub const TOOL_NAMES: &[&str]` array (order: existing 8, then 5 impact)
- [ ] 1.3 Modify `pub struct ExplorerMcpHandler` (line ~117): add `graph: Option<Arc<CallGraph>>` field. Add `use cognicode_core::application::CallGraph;` import as needed
- [ ] 1.4 In `impl ExplorerMcpHandler`: keep existing `pub fn new(service: Arc<ExplorerService>) -> Self` unchanged in signature, but populate `graph: None` in the body
- [ ] 1.5 Add new constructor `pub fn with_graph(service: Arc<ExplorerService>, graph: Option<Arc<CallGraph>>) -> Self` that stores the graph as-is
- [ ] 1.6 Change `async fn dispatch(service: &Arc<ExplorerService>, ...)` → `async fn dispatch(service: &Arc<ExplorerService>, graph: &Option<Arc<CallGraph>>, request: CallToolRequestParams) -> CallToolResult`
- [ ] 1.7 Add `pub fn service(&self) -> &Arc<ExplorerService>` accessor (if not present) — needed by the RED-gate test (`handler.service()` is referenced)
- [ ] 1.8 Add private `fn require_graph<'a>(graph: &'a Option<Arc<CallGraph>>, tool: &str) -> Result<&'a Arc<CallGraph>, CallToolResult>` returning `Err(err(...))` on `None`
- [ ] 1.9 Add private `fn ok_direct<T: serde::Serialize>(value: &T) -> CallToolResult` per spec §R8
- [ ] 1.10 Update the 17 existing `dispatch(...)` call sites in tests: each must pass `&None` as the new middle argument
- [ ] 1.11 **GREEN check (Phase 1)**: `cargo test -p cognicode-explorer --no-run` compiles, `cargo test -p cognicode-explorer` passes all pre-change tests unchanged. RED-gate test should now **fail at runtime** (not compile-fail) — `require_graph` not yet wired into any arm, so `is_error` will be `false` (or text won't contain the message) and the assertion fails
- [ ] 1.12 Wire `require_graph` into a single dummy arm for `TOOL_IMPACT_RADIUS` that returns `require_graph(graph, "impact_radius")?` — minimal stub to make the RED-gate test pass. Do NOT implement the real radius logic yet
- [ ] 1.13 **GREEN check (RED-gate)**: `cargo test -p cognicode-explorer test_handler_without_graph_returns_impact_unavailable` passes. **First TDD cycle complete.**

> **Phase 1 exit criterion**: RED-gate test is GREEN; pre-change test suite has zero regressions.

---

## Phase 2: Handler field tests (R1 coverage)

- [ ] 2.1 Add `test_with_graph_some_makes_impact_arms_reachable` — build handler with `with_graph(service, Some(graph))`, assert `list_tools()` returns 13 entries, assert handler field is `Some`
- [ ] 2.2 Add `test_with_graph_none_matches_new_legacy` — build two handlers: `with_graph(service, None)` and `new(service)`. Assert both produce identical `list_tools()` length and identical dispatch behavior on all 5 impact tools
- [ ] 2.3 **GREEN check**: `cargo test -p cognicode-explorer test_with_graph_` — all pass

---

## Phase 3: Schema contract (R2 — 13 tools)

- [ ] 3.1 Add `test_tool_schemas_list_thirteen_tools` — call `build_tool_schemas()` directly, assert `len() == 13`, assert names are unique
- [ ] 3.2 Add `test_tool_names_contains_impact_constants` — call `tool_names()`, assert `len() == 13` and contains all 13 `TOOL_*` constants
- [ ] 3.3 In `build_tool_schemas()` (line ~341), append 5 new `Tool::new(...)` entries following the hand-rolled pattern (properties, required arrays)
  - `impact_radius`: `root` (string, required), `max_depth` (integer, optional)
  - `impact_has_path`: `from` (string, required), `to` (string, required)
  - `impact_shortest_path`: `from` (string, required), `to` (string, required)
  - `impact_detect_cycles`: no required args
  - `impact_component`: `id` (string, required)
- [ ] 3.4 **GREEN check**: `cargo test -p cognicode-explorer test_tool_schemas test_tool_names_` — all pass

---

## Phase 4: `ok_direct` helper (R8)

- [ ] 4.1 Add `test_ok_direct_serializes_pretty_json` — call `ok_direct(&vec!["a".to_string(),"b".to_string()])`, assert `is_error == false` and text is pretty-printed JSON array `["a","b"]`
- [ ] 4.2 (Optional but cheap) Add inline test for `ok_direct(&None::<PathResultDto>)` returning `"null"` — guards E6
- [ ] 4.3 **GREEN check**: `cargo test -p cognicode-explorer test_ok_direct` — passes
- [ ] 4.4 **Regression check**: `ok()` (existing helper) body must be byte-identical. Run pre-change `dispatch_*` tests; all must still pass

---

## Phase 5: `impact_radius` dispatch (R3, 5 tests)

- [ ] 5.1 Add arg struct `struct ImpactRadiusArgs { root: Option<String>, max_depth: Option<usize> }` in `mcp.rs`
- [ ] 5.2 Add match arm for `TOOL_IMPACT_RADIUS` in `dispatch`:
  - Call `require_graph(graph, "impact_radius")?`
  - Parse `ImpactRadiusArgs` from `arguments` (return `err("impact_radius: invalid args: ...")` on parse fail)
  - Extract `root` (return `err("impact_radius: missing required arg \`root\`")` if `None`)
  - Resolve `max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH)`
  - Build `ImpactAnalysisService::new()`, build `CallGraphProjection::new(graph)` per call, call `svc.impact_radius(proj.as_ref(), &root_id, max_depth)`
  - Map `Vec<SymbolId>` → `Vec<String>` (collect), wrap with `ok_direct(&result_vec)`
- [ ] 5.3 Add `test_impact_radius_returns_predecessors` (R3, graph `D→A→C`, `B→C`, root `C`, depth 2) — assert JSON parses to 3 strings
- [ ] 5.4 Add `test_impact_radius_missing_root_arg` (E2) — empty args → `is_error == true`, text contains `"missing required arg"` and `"impact_radius"`
- [ ] 5.5 Add `test_impact_radius_default_max_depth_is_5` (E4) — chain of 7, root at depth 6, no `max_depth` → result has exactly 5 entries
- [ ] 5.6 Add `test_impact_radius_zero_depth_returns_empty` (E3) — `A→B`, root `B`, depth 0 → `Vec::new()`
- [ ] 5.7 Add `test_impact_radius_unknown_root_returns_empty` (E6) — `A→B`, root `"missing"` → `Vec::new()`, no panic
- [ ] 5.8 **GREEN check**: `cargo test -p cognicode-explorer test_impact_radius` — all 5 pass

---

## Phase 6: `impact_has_path` dispatch (R4, 2 tests)

- [ ] 6.1 Add struct `struct ImpactHasPathArgs { from: Option<String>, to: Option<String> }`
- [ ] 6.2 Add struct `#[derive(Serialize)] struct HasPathResult { from: String, to: String, has_path: bool }` for response shape
- [ ] 6.3 Add match arm for `TOOL_IMPACT_HAS_PATH`:
  - `require_graph(graph, "impact_has_path")?`
  - Parse args, return missing-arg errors for `from` and `to` mentioning the tool name
  - Build projection, call `svc.has_path(proj.as_ref(), &from_id, &to_id)`
  - Construct `HasPathResult { from, to, has_path }`, `ok_direct(&result)`
- [ ] 6.4 Add `test_impact_has_path_direct_transitive_unreachable` (R4, E8) — graph `A→B→C` + `D`, three calls, verify `has_path` for direct, transitive, unreachable
- [ ] 6.5 Add `test_impact_has_path_self_path` (E9) — node `A` only, `from=A, to=A` → `has_path == true`
- [ ] 6.6 **GREEN check**: `cargo test -p cognicode-explorer test_impact_has_path` — all pass

---

## Phase 7: `impact_shortest_path` dispatch (R5, 3 tests)

- [ ] 7.1 Reuse arg struct shape (or new `ImpactPathArgs { from, to }`)
- [ ] 7.2 Add match arm for `TOOL_IMPACT_SHORTEST_PATH`:
  - `require_graph(graph, "impact_shortest_path")?`
  - Parse args with missing-arg errors
  - Call `svc.shortest_path(proj.as_ref(), &from_id, &to_id)` → returns `Option<PathResultDto>`
  - `ok_direct(&option_result)` — `None` serializes as JSON `null`
- [ ] 7.3 Add `test_impact_shortest_path_returns_cheapest` (R5) — graph `A→B` (conf 1.0) vs `A→C→B` (conf 0.5), assert `found=true`, `total_cost≈0.0`, `path=["A","B"]`
- [ ] 7.4 Add `test_impact_shortest_path_unreachable_returns_null` (E8) — `A→B` only, `to=C` → text equals `"null"`
- [ ] 7.5 Add `test_impact_shortest_path_self_path` (E9) — node `A` only, `from=A, to=A` → `path` length 1, `total_cost == 0.0`
- [ ] 7.6 **GREEN check**: `cargo test -p cognicode-explorer test_impact_shortest_path` — all pass

---

## Phase 8: `impact_detect_cycles` dispatch (R6, 2 tests)

- [ ] 8.1 Add match arm for `TOOL_IMPACT_DETECT_CYCLES`:
  - `require_graph(graph, "impact_detect_cycles")?`
  - No required args — accept empty `{}` or any blob
  - Build projection, call `svc.detect_cycles(proj.as_ref())` → `Vec<SccDto>`
  - `ok_direct(&result_vec)` — empty DAG serializes as `[]`
- [ ] 8.2 Add `test_impact_detect_cycles_returns_sccs` (R6, E13) — disjoint cycles `A↔B` and `X↔Y` → `Vec<SccDto>` length 2 with correct member sets and sizes
- [ ] 8.3 Add `test_impact_detect_cycles_dag_returns_empty` (E11) — `A→B→C` → `[]`
- [ ] 8.4 **GREEN check**: `cargo test -p cognicode-explorer test_impact_detect_cycles` — all pass

---

## Phase 9: `impact_component` dispatch (R7, 2 tests)

- [ ] 9.1 Add arg struct `struct ImpactComponentArgs { id: Option<String> }`
- [ ] 9.2 Add match arm for `TOOL_IMPACT_COMPONENT`:
  - `require_graph(graph, "impact_component")?`
  - Parse args, return missing-`id` error
  - Call `svc.containing_component(proj.as_ref(), &id_id)` → `Option<Vec<SymbolId>>`
  - Map `Some(vec)` → `Some(vec.into_iter().map(|s| s.to_string()).collect())`; `None` stays `None`
  - `ok_direct(&option_result)` — `None` serializes as JSON `null`
- [ ] 9.3 Add `test_impact_component_returns_members` (R7, E12) — disjoint components `A→B` and `C→D`, query `A` → `["A","B"]`
- [ ] 9.4 Add `test_impact_component_missing_id_returns_null` (E6) — `A→B`, `id="missing"` → text equals `"null"`
- [ ] 9.5 **GREEN check**: `cargo test -p cognicode-explorer test_impact_component` — all pass

---

## Phase 10: Binary wiring (R9)

- [ ] 10.1 In `crates/cognicode-explorer/src/bin/mcp.rs` SQLite branch: insert `let graph_for_handler = graph.clone();` **before** `CallGraphRepository::new(graph)`. Change `ExplorerMcpHandler::new(service)` → `ExplorerMcpHandler::with_graph(service, Some(graph_for_handler))`
- [ ] 10.2 Same change in the `--postgres` branch (symmetric: 1 `Arc::clone` line + 1 `with_graph` call)
- [ ] 10.3 **Diff sanity check**: `git diff crates/cognicode-explorer/src/bin/mcp.rs` — must show exactly 2 added `Arc::clone` lines (1 per path) and the rest of function bodies byte-identical
- [ ] 10.4 **Compile check**: `cargo build -p cognicode-explorer --bin cognicode-mcp` succeeds

---

## Phase 11: Integration test rewrite (R2 final)

- [ ] 11.1 In `crates/cognicode-explorer/tests/integration.rs` line ~1028, rewrite `mcp_tool_names_match_spec`:
  - Import all 13 `TOOL_*` constants (existing 8 + 5 new)
  - Update `expected` array to all 13 in canonical order
  - Change `assert_eq!(actual.len(), 8)` → `assert_eq!(actual.len(), 13)`
- [ ] 11.2 **GREEN check**: `cargo test -p cognicode-explorer --test integration mcp_tool_names_match_spec` passes

---

## Phase 12: Final verification (all acceptance criteria)

- [ ] 12.1 `cargo test -p cognicode-explorer` — **all 19+ new tests + zero pre-change regressions**
- [ ] 12.2 `cargo clippy --all-targets -p cognicode-explorer` — **zero warnings**
- [ ] 12.3 `cargo build --workspace` — clean
- [ ] 12.4 `git diff crates/cognicode-core/` — **must be empty** (AC10)
- [ ] 12.5 `git diff crates/cognicode-explorer/Cargo.toml` — **must be empty** (AC10)
- [ ] 12.6 Manual smoke: run `cognicode-mcp` against a real `.cognicode/cognicode.db`, send `tools/list`, confirm 13 tools
- [ ] 12.7 Manual smoke: call `impact_radius` on a known symbol — expect `is_error == false` and a `Vec<String>` payload
- [ ] 12.8 Confirm 21 new test functions exist: `cargo test -p cognicode-explorer -- --list | grep -E "test_(with_graph|tool_schemas|tool_names|impact_|ok_direct|handler_without)" | wc -l` → expect ≥21
- [ ] 12.9 **Spec scenario cross-walk**: every scenario in spec R1–R11 has at least one test covering it (no orphans)
- [ ] 12.10 **Edge case cross-walk**: every E1–E16 row is covered (most by the 21 tests above; E5, E10, E14–E16 may be implicit — verify)

---

## Dependency Graph

```
Phase 0  ──compile-fail──►  Phase 1  ──green gate──►  Phase 2 (handler field)
   │                            │                          │
   │                            ├──►  Phase 3 (schemas) ◄──┤
   │                            │                          │
   │                            ├──►  Phase 4 (ok_direct) ◄┘
   │                            │
   │                            ├──►  Phase 5 (radius)  ──► Phase 6 (has_path)
   │                            │                                │
   │                            │                                ▼
   │                            │                         Phase 7 (shortest_path)
   │                            │                                │
   │                            │                                ▼
   │                            │                         Phase 8 (detect_cycles)
   │                            │                                │
   │                            │                                ▼
   │                            │                         Phase 9 (component)
   │                            │                                │
   │                            ▼                                │
   │                       Phase 10 (binary wiring) ◄───────────┘
   │                            │
   │                            ▼
   │                       Phase 11 (integration test)
   │                            │
   │                            ▼
   └──────────────► Phase 12 (final verification)
```

**Strict ordering rules**:
- Phase 0 must compile-fail before Phase 1 begins.
- Phase 1 must end with RED-gate test GREEN before Phase 2.
- Phases 5–9 can be done in any order **after** Phase 4, but each must end with its own GREEN check.
- Phase 10 (binary) requires Phases 5–9 in place (binary wires all 5 tools).
- Phase 11 requires Phase 3 (schemas) complete.

---

## Test Inventory (target = 21 new tests)

| # | Test | Phase | Verifies |
|---|------|-------|----------|
| 1 | `test_handler_without_graph_returns_impact_unavailable` | 0/1 | R1, E1 (RED gate) |
| 2 | `test_with_graph_some_makes_impact_arms_reachable` | 2 | R1 |
| 3 | `test_with_graph_none_matches_new_legacy` | 2 | R1 |
| 4 | `test_tool_schemas_list_thirteen_tools` | 3 | R2, AC2 |
| 5 | `test_tool_names_contains_impact_constants` | 3 | R2 |
| 6 | `test_impact_radius_returns_predecessors` | 5 | R3 |
| 7 | `test_impact_radius_missing_root_arg` | 5 | E2 |
| 8 | `test_impact_radius_default_max_depth_is_5` | 5 | E4 |
| 9 | `test_impact_radius_zero_depth_returns_empty` | 5 | E3 |
| 10 | `test_impact_radius_unknown_root_returns_empty` | 5 | E6 |
| 11 | `test_impact_has_path_direct_transitive_unreachable` | 6 | R4, E8 |
| 12 | `test_impact_has_path_self_path` | 6 | E9 |
| 13 | `test_impact_shortest_path_returns_cheapest` | 7 | R5 |
| 14 | `test_impact_shortest_path_unreachable_returns_null` | 7 | E8 |
| 15 | `test_impact_shortest_path_self_path` | 7 | E9 |
| 16 | `test_impact_detect_cycles_returns_sccs` | 8 | R6, E13 |
| 17 | `test_impact_detect_cycles_dag_returns_empty` | 8 | E11 |
| 18 | `test_impact_component_returns_members` | 9 | R7, E12 |
| 19 | `test_impact_component_missing_id_returns_null` | 9 | E6 |
| 20 | `test_ok_direct_serializes_pretty_json` | 4 | R8 |
| 21 | `mcp_tool_names_match_spec` (integration, rewritten) | 11 | R2 |

**Total**: 21 new test functions (16 dispatch + 1 helper + 2 schema + 1 integration + 1 RED-gate shared).

---

## Validation Commands (reference)

```bash
# Compile gate (Phase 0 RED)
cargo test -p cognicode-explorer --no-run

# Unit + integration gate (Phases 1–11)
cargo test -p cognicode-explorer

# Lint gate (Phase 12)
cargo clippy --all-targets -p cognicode-explorer -- -D warnings

# Workspace compile gate
cargo build --workspace

# Diff invariants (Phase 12 — must be empty)
git diff crates/cognicode-core/
git diff crates/cognicode-explorer/Cargo.toml

# Binary smoke
cargo build -p cognicode-explorer --bin cognicode-mcp
./target/debug/cognicode-mcp --help
```

---

## Risk Notes

| Risk | Mitigation in tasks |
|------|---------------------|
| Forgetting to update pre-change tests' `dispatch` call sites | Phase 1.10 explicit, Phase 1.11 GREEN check enforces |
| `CallGraphProjection` API mismatch | Phase 5.2 documents call; if signature differs, fix in Phase 1.6 — caught by Phase 1.11 compile gate |
| `ImpactAnalysisService` method names diverge from spec | Tests 6/13/16/18 will fail to compile; fix in corresponding phase |
| `SymbolId` ↔ `String` conversion in 4 places | `Vec<SymbolId> → Vec<String>` shown in 5.2; reuse helper or inline `.into_iter().map(SymbolId::to_string).collect()` |
| Binary wiring breaks existing smoke | Phase 10.4 compile + manual smoke in 12.6 |
