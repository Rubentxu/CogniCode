# Tasks: MCP Graph Primitives — Subgraph, Cluster, Explain

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 450–550 (production ~300 + tests ~150) |
| 400-line budget risk | Medium |
| Chained PRs recommended | Yes |
| Suggested split | PR-A: Projection + Service + DTOs (~280 LOC). PR-B: MCP constants/args/dispatch/schemas + dispatch tests (~200 LOC). |
| Delivery strategy | ask-on-risk |
| Chain strategy | stacked-to-main |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Projection layer + Service layer + DTOs green; 16 of 24 tests pass | PR-A | Foundation. Base branch: `main`. Production ~280 LOC. Includes 11 projection tests + 5 service tests. |
| 2 | MCP layer (constants, args, dispatch arms, schemas) + 8 dispatch tests green; all 24 tests pass; 17 tool schemas | PR-B | Wiring. Base branch: `main` (stacked after PR-A). Touches only `cognicode-explorer/src/mcp.rs`. ~200 LOC. |

### Dependency order rationale

The 3-layer delegation (Projection → Service → MCP) enforces strict compile-time dependencies. Writing the tests in RED-then-GREEN order (per spec §"TDD RED gate") means we MUST stage the layers in dependency order: any upper layer referencing a not-yet-existent lower layer will block the lower layer's tests from compiling in isolation. Two PRs is the smallest split that keeps each PR independently reviewable and rollable-back.

## Phase 0: RED Gate — Compile-Error Anchors (do FIRST, do ALL before any GREEN)

> Per spec §"TDD RED gate", each step below produces a deliberate compile error. Run all four `cargo build` commands; each MUST fail. This proves the test scaffolding references the not-yet-existing API.

- [ ] 0.1 **Add RED tests in `call_graph_projection.rs`** — Append 6 `#[test]` functions in the existing `#[cfg(test)] mod tests` block: `test_extract_subgraph_outgoing_two_hops`, `test_extract_subgraph_incoming_with_cycle`, `test_extract_subgraph_both_two_pass`, `test_extract_subgraph_unknown_root_returns_empty`, `test_extract_subgraph_max_depth_zero`, `test_extract_subgraph_dense_fanout_no_duplicates`. Each calls `CallGraphProjection::from_call_graph(&g).extract_subgraph(&SymbolId, SubgraphDirection, usize)`. **Verify**: `cargo test -p cognicode-core --lib extract_subgraph` → E0425 (cannot find method/type).
- [ ] 0.2 **Add RED tests in `call_graph_projection.rs`** — Append 5 `#[test]` functions: `test_explain_path_direct_edge_single_hop`, `test_explain_path_multi_hop_collects_metadata`, `test_explain_path_self_path_zero_hops`, `test_explain_path_unreachable_returns_none`, `test_explain_path_verb_mapping_all_eight_variants`. Each calls `CallGraphProjection::from_call_graph(&g).explain_path(&from, &to)`. **Verify**: `cargo test -p cognicode-core --lib explain_path` → E0425.
- [ ] 0.3 **Add RED tests in `impact_analysis.rs`** — Append 5 `#[test]` functions: `test_subgraph_service_mirrors_projection`, `test_cluster_components_scc_method`, `test_cluster_components_connected_method`, `test_explain_path_service_wraps_none_as_found_false`, `test_explain_path_service_found_true_carries_hops`. Each calls `ImpactAnalysisService::new(...).subgraph(...)`, `.cluster_components(...)`, `.explain_path(...)`. **Verify**: `cargo test -p cognicode-core --lib subgraph` (or `cluster_components`/`explain_path`) → E0425.
- [ ] 0.4 **Add RED MCP compile-anchor stubs in `mcp.rs`** — Add `TOOL_GRAPH_SUBGRAPH`, `TOOL_GRAPH_CLUSTER`, `TOOL_GRAPH_EXPLAIN` constants; `GraphSubgraphArgs`, `GraphClusterArgs`, `GraphExplainArgs` structs; 3 dispatch arm placeholders (`TOOL_GRAPH_SUBGRAPH => return err("not implemented")`); 3 schema entries in `build_tool_schemas()`. Extend `TOOL_NAMES` from 14→17 entries. **Verify**: `cargo build -p cognicode-explorer` → succeeds (anchors are stubs that compile). Then add 8 stub `#[tokio::test]` in the existing tests module that reference the new tool names + assert `is_error == true` because stubs always error. **Verify**: `cargo test -p cognicode-explorer --lib graph_subgraph graph_cluster graph_explain` → 8 tests compile and FAIL (RED). Total 24 RED tests across the workspace.
- [ ] 0.5 **Capture red-state summary** — Run `cargo build -p cognicode-core -p cognicode-explorer 2>&1 | grep -E "error\[E0" | wc -l` and record count. Expected: 0 errors after 0.4 (anchors compile), but 24 test failures. **Save snapshot to Engram observation**: title=`sdd/mcp-graph-primitives/red-gate` with RED test count + failing build outputs.

**Validation (end of Phase 0)**:
```bash
cargo test -p cognicode-core --lib 2>&1 | grep -E "FAILED|error\[" | head -30
cargo test -p cognicode-explorer --lib 2>&1 | grep -E "FAILED|error\[" | head -30
```
Expected: 11 projection failures + 5 service failures + 8 dispatch failures = 24 failed tests, 0 compile errors.

## Phase 1: Projection Layer GREEN (PR-A foundation)

> Files: `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` only. ~140 LOC.

- [ ] 1.1 **Add `SubgraphDirection` enum + 4 DTO structs** — In `call_graph_projection.rs`, append: `pub enum SubgraphDirection { Outgoing, Incoming, Both }`, `pub struct SubgraphEdge { source: SymbolId, target: SymbolId, dependency_type: DependencyType, confidence: f64 }`, `pub struct SubgraphView { nodes: Vec<SymbolId>, edges: Vec<SubgraphEdge> }`, `pub struct ExplanationHop { from: SymbolId, to: SymbolId, dependency_type: DependencyType, confidence: f64, rationale: String }`, `pub struct ExplanationView { hops: Vec<ExplanationHop>, total_cost: f64 }`. Add `#[derive(Debug, Clone, PartialEq)]`. **Verify**: `cargo build -p cognicode-core` compiles.
- [ ] 1.2 **Add private `verb_for(dep_type) -> &'static str`** — `match` on all 8 `DependencyType` variants returning agent-readable verbs (`"calls"`, `"imports"`, `"inherits from"`, `"uses generic"`, `"references"`, `"defines"`, `"annotated by"`, `"contains"`). No wildcard needed because `DependencyType` is closed (per design.md). **Verify**: unit test on each variant via 8 inline asserts in `test_explain_path_verb_mapping_all_eight_variants` (already written in 0.2).
- [ ] 1.3 **Implement `extract_subgraph()` BFS** — BFS using a `VecDeque<(NodeIndex, usize)>`. Direction union: `Outgoing` walks `neighbors_directed(Outgoing)`, `Incoming` walks `neighbors_directed(Incoming)`, `Both` walks both via `Direction::Incoming` then `Direction::Outgoing` in one pass (using a `match`). Track visited via `HashSet<NodeIndex>`. Push each traversed edge (source, target, dep_type, confidence) into `edges`. Root always added to `nodes` first (even if unknown — caller decides if empty is OK). Cycle-safe (HashSet guard). `max_depth == 0` → return `SubgraphView { nodes: vec![root_index if known else empty], edges: vec![] }` after visiting root only. Returns `SubgraphView` by value. **Verify**: `cargo test -p cognicode-core --lib extract_subgraph` → 6 of 6 GREEN.
- [ ] 1.4 **Implement `explain_path()` using existing `dijkstra()`** — Reuse `self.dijkstra(from, to)` (line 294) to get `Option<(Vec<SymbolId>, f64)>`. If `None`, return `None`. If `from == to`, return `Some(ExplanationView { hops: vec![], total_cost: 0.0 })`. Otherwise walk adjacent pairs in the path, look up the edge in the underlying `StableGraph` for `(dep_type, confidence)`, build an `ExplanationHop` per pair, and accumulate `verb_for()` for `rationale`. **Verify**: `cargo test -p cognicode-core --lib explain_path` → 5 of 5 GREEN.
- [ ] 1.5 **Run full projection test suite** — `cargo test -p cognicode-core --lib call_graph_projection` → all 11 new tests + existing 22 tests pass (no regression in `strongly_connected_components`, `connected_components`, `dijkstra`, `impact_radius`).

**Validation (end of Phase 1)**:
```bash
cargo build -p cognicode-core 2>&1 | grep -c "^error"
cargo test -p cognicode-core --lib extract_subgraph explain_path 2>&1 | tail -5
```
Expected: 0 build errors; 11 tests pass.

## Phase 2: Service Layer GREEN (PR-A continuation)

> Files: `crates/cognicode-core/src/application/services/impact_analysis.rs` and `crates/cognicode-core/src/application/dto/impact_dto.rs`. ~60 LOC production + DTO conversions.

- [ ] 2.1 **Add 3 result DTOs to `impact_dto.rs`** — Append `pub struct SubgraphEdgeDto { source, target, dependency_type, confidence }` (all `String`/`f64`), `pub struct SubgraphResultDto { nodes: Vec<String>, edges: Vec<SubgraphEdgeDto> }`, `pub struct ClusterDto { members: Vec<String>, size: usize }`, `pub struct ClusterResultDto(pub Vec<ClusterDto>)`, `pub struct ExplainHopDto { from, to, dependency_type, confidence, rationale }`, `pub struct ExplainResultDto { found: bool, hops: Vec<ExplainHopDto>, total_cost: f64, summary: String }`. Add `#[derive(Debug, Clone, Serialize, Deserialize)]` to each. Add constructor `SubgraphResultDto::from_view(view: SubgraphView)`, `ClusterResultDto::from_sccs(sccs: Vec<Vec<SymbolId>>)` and `::from_components(comps)`, `ExplainResultDto::from_view(view: &ExplanationView, found: bool)`. **Verify**: `cargo build -p cognicode-core` compiles.
- [ ] 2.2 **Implement `ImpactAnalysisService::subgraph()`** — Thin wrapper: `CallGraphProjection::from_call_graph(graph).extract_subgraph(root, direction, max_depth)` → `SubgraphResultDto::from_view(view)`. ~6 LOC. **Verify**: `cargo test -p cognicode-core --lib test_subgraph_service_mirrors_projection` → GREEN.
- [ ] 2.3 **Implement `ImpactAnalysisService::cluster_components(method: &str)`** — Match on `method`: `"scc"` → `CallGraphProjection::from_call_graph(graph).strongly_connected_components()`; `"connected"` → `.connected_components()`. Convert to `ClusterResultDto`. ~10 LOC. **Verify**: both `test_cluster_components_scc_method` and `test_cluster_components_connected_method` GREEN.
- [ ] 2.4 **Implement `ImpactAnalysisService::explain_path()`** — Call `CallGraphProjection::from_call_graph(graph).explain_path(from, to)`. Map `None` → `Some(ExplainResultDto { found: false, hops: vec![], total_cost: 0.0, summary: "no path".into() })`. Map `Some(view)` → `Some(ExplainResultDto::from_view(&view, true))` with `summary = format!("{} hop(s)", view.hops.len())`. Return type stays `Option<ExplainResultDto>` (MCP layer unwraps the Option). ~12 LOC. **Verify**: `test_explain_path_service_wraps_none_as_found_false` + `test_explain_path_service_found_true_carries_hops` GREEN.
- [ ] 2.5 **Run full service test suite** — `cargo test -p cognicode-core --lib impact_analysis` → all 5 new tests + existing 28 tests pass.

**Validation (end of Phase 2)**:
```bash
cargo test -p cognicode-core --lib 2>&1 | tail -3
```
Expected: `test result: ok`. 16/24 TDD tests green (projection 11 + service 5).

## Phase 3: MCP Layer GREEN (PR-B)

> File: `crates/cognicode-explorer/src/mcp.rs` only. ~200 LOC (constants + args + dispatch arms + schemas + 8 tests).

- [ ] 3.1 **Add 3 tool constants + `DEFAULT_SUBGRAPH_DEPTH` const** — Append after `TOOL_IMPACT_COMPONENT` (line 53): `pub const TOOL_GRAPH_SUBGRAPH: &str = "graph_subgraph"; pub const TOOL_GRAPH_CLUSTER: &str = "graph_cluster"; pub const TOOL_GRAPH_EXPLAIN: &str = "graph_explain"; pub const DEFAULT_SUBGRAPH_DEPTH: usize = 3;`. **Verify**: `cargo build -p cognicode-explorer` compiles.
- [ ] 3.2 **Add 3 arg structs** — Append after `ImpactIdArgs` (line 158): `GraphSubgraphArgs { root: Option<String>, direction: Option<String>, max_depth: Option<usize> }`, `GraphClusterArgs { method: Option<String> }`, `GraphExplainArgs { from: Option<String>, to: Option<String> }`. All `#[derive(Debug, Default, Deserialize)] #[serde(default)]`. **Verify**: compiles.
- [ ] 3.3 **Extend `TOOL_NAMES` from 14→17** — Replace the 14-entry `TOOL_NAMES` slice with 17 entries, appending the 3 new constants. Order: existing 14 then `TOOL_GRAPH_SUBGRAPH, TOOL_GRAPH_CLUSTER, TOOL_GRAPH_EXPLAIN`. **Verify**: `cargo test -p cognicode-explorer --lib tool_names` (existing test on length) must update; add assertion `assert_eq!(TOOL_NAMES.len(), 17)`. **Verify test**: `test_tool_names_contains_all_seventeen` GREEN.
- [ ] 3.4 **Add 3 schema entries to `build_tool_schemas()`** — Append 3 `Tool { name, description, input_schema }` entries for `graph_subgraph` (root required, direction enum, max_depth optional), `graph_cluster` (method optional, enum scc/connected), `graph_explain` (from + to required). Use the existing `serde_json::json!({"type":"object",...})` pattern from lines 728–780. **Verify**: a new test `test_build_tool_schemas_returns_seventeen` asserts `len == 17` and each new name is present.
- [ ] 3.5 **Add 3 dispatch arms** — Insert after the `TOOL_IMPACT_COMPONENT` arm (~line 547). Each follows the same shape:
  - `TOOL_GRAPH_SUBGRAPH =>`: `require_graph` → parse `GraphSubgraphArgs` → validate `root.is_some()` → parse `direction` (default `"both"`; reject unknown with `err(...)`) → default `max_depth` to `DEFAULT_SUBGRAPH_DEPTH` → `service.subgraph(g, &root_id, dir, depth)` → `ok_direct(&dto)`. ~25 LOC.
  - `TOOL_GRAPH_CLUSTER =>`: `require_graph` → parse `GraphClusterArgs` (empty `{}` valid) → default method `"scc"`; reject unknown with `err(...)` → `service.cluster_components(g, method)` → `ok_direct(&dto)`. ~10 LOC.
  - `TOOL_GRAPH_EXPLAIN =>`: `require_graph` → parse `GraphExplainArgs` → validate `from.is_some()` + `to.is_some()` → `service.explain_path(g, &from_id, &to_id)` → `ok_direct(&dto)` (unwraps the `Some`, never `None` because service guarantees `Some`). ~12 LOC.
  - **Verify**: `cargo build -p cognicode-explorer` compiles; `cargo test -p cognicode-explorer --lib graph_subgraph graph_cluster graph_explain` → 8 dispatch tests GREEN.
- [ ] 3.6 **Run full MCP test suite** — `cargo test -p cognicode-explorer --lib` → all tests pass; `test_tool_names_contains_all_seventeen` + `test_build_tool_schemas_returns_seventeen` + 8 dispatch tests GREEN.

**Validation (end of Phase 3)**:
```bash
cargo build -p cognicode-explorer 2>&1 | grep -c "^error"
cargo test -p cognicode-explorer --lib 2>&1 | tail -3
```
Expected: 0 build errors; 8 new dispatch tests pass + all existing tests still pass.

## Phase 4: Workspace-Wide Validation

- [ ] 4.1 **Run full workspace test suite** — `cargo test --workspace --lib`. Expected: all 24 new tests GREEN; no regression in existing tests.
- [ ] 4.2 **Run clippy on both modified crates** — `cargo clippy -p cognicode-core -p cognicode-explorer -- -D warnings`. Fix any lints introduced by the new code.
- [ ] 4.3 **Verify TOOL_NAMES count + dispatch routing** — `cargo test -p cognicode-explorer` with the two `length == 17` assertions and the dispatch-arm test suite. Confirms the 14→17 upgrade and that all 3 new arms are reachable from `dispatch()`.
- [ ] 4.4 **Verify graph-unavailable guard** — The 3 RED tests written in 0.4 that pass `graph: &None` MUST still error with a consistent message. Confirm via existing `test_dispatch_*_without_graph` patterns.
- [ ] 4.5 **Final LOC accounting** — `git diff --stat main` to confirm ~300 production + ~150 test = ~450 LOC total (under the 550 high-watermark but above 400 budget, justifying the chained-PR recommendation).

**Validation (end of Phase 4)**:
```bash
cargo test --workspace --lib 2>&1 | tail -3
cargo clippy -p cognicode-core -p cognicode-explorer -- -D warnings 2>&1 | tail -3
git diff --stat main | tail -5
```
Expected: 24 new tests green + 0 clippy warnings + diff stats within budget.

## Line Count Estimates (per phase)

| Phase | File(s) | Production LOC | Test LOC | Notes |
|-------|---------|---------------:|---------:|-------|
| 0 | (test scaffolding only) | 0 | 200 | All tests in existing `mod tests` blocks + 8 MCP dispatch tests |
| 1 | `call_graph_projection.rs` | 140 | 0 | 4 DTOs (~30) + 2 methods (~80) + verb map (~30) |
| 2 | `impact_analysis.rs` + `impact_dto.rs` | 60 | 0 | 3 service methods + 6 DTOs + constructors |
| 3 | `mcp.rs` | 200 | 0 | 3 constants + 3 args + 3 arms + 3 schemas + 8 dispatch tests |
| 4 | (validation only) | 0 | 0 | No code changes |
| **Total** | | **~400** | **~200** | **~600 diff** (still warrants chained PRs) |

## Dependencies Between Tasks

```
Phase 0 (RED) ── blocks ──> Phase 1 (Projection GREEN)
Phase 1      ── blocks ──> Phase 2 (Service GREEN)
Phase 2      ── blocks ──> Phase 3 (MCP GREEN)
Phase 3      ── blocks ──> Phase 4 (Validation)
```

- Phase 0.1 blocks Phase 1.3 (test must exist before method)
- Phase 0.2 blocks Phase 1.4
- Phase 0.3 blocks Phase 2.2, 2.3, 2.4
- Phase 0.4 blocks Phase 3.1–3.5
- Phase 1.1 (DTOs) blocks Phase 1.3 + 1.4 (method signatures)
- Phase 2.1 (DTOs) blocks Phase 2.2, 2.3, 2.4
- Phase 3.1 (constants) blocks Phase 3.2 (args) and Phase 3.5 (dispatch arms)
- Phase 3.3 (`TOOL_NAMES`) blocks Phase 3.4 (schemas) — both must be done before Phase 4 schema-count test

## PR Split Guidance

**PR-A** (Phase 0.1 + 0.2 + 0.3 + 1 + 2): foundation. Touches only `cognicode-core`. After merge, `cognicode-explorer` still builds because Phase 0.4 anchors compile independently. Reviewers see projection tests + service tests + DTOs all GREEN. ~280 LOC, ~16 tests added.

**PR-B** (Phase 3 + 4): MCP wiring. Touches only `cognicode-explorer/src/mcp.rs`. No new logic, just thin dispatch + schema glue + dispatch tests. ~200 LOC, ~8 tests added. Stacks cleanly after PR-A.

Decision needed before apply: **Yes** — confirm `stacked-to-main` chain strategy (vs `feature-branch-chain` or `size:exception`).

## Out of Scope (deferred)

- Provenance/EvidenceBlock on edges (proposal §"Out of Scope")
- `max_paths` multi-path for `graph_explain`
- `max_nodes` cap for `graph_subgraph`
- Caching of projection results
- Subgraph DTO node deduplication / sorting (open question in design.md — preserved BFS order)
