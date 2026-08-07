# Exploration: mcp-impact-tool

> **Change**: `mcp-impact-tool` — expose `ImpactAnalysisService` through the existing MCP tool surface
> **Project**: cognicode
> **Date**: 2026-06-09
> **Mode**: hybrid (Engram + OpenSpec; LogSeq unavailable)
> **User Override**: TDD ALWAYS

---

## 1. Current State

### Codebase Structure (MCP Layer)

The MCP tool surface lives in **`crates/cognicode-explorer/src/mcp.rs`** (~892 LOC). It follows a canonical pattern established in Phase 5 (`explorer-phase5`):

| Component | Role |
|-----------|------|
| `ExplorerMcpHandler` | Struct holding `Arc<ExplorerService>`. Implements `ServerHandler`. |
| `build_tool_schemas()` | Returns 8 `Tool` descriptors with hand-rolled JSON schemas. |
| `dispatch()` | `async fn` that matches on `request.name`, deserializes per-tool args, calls `ExplorerService` methods. |
| `ok()` / `err()` | Serialize `ExplorerResult<T>` to `CallToolResult::success` or `CallToolResult::error`. |

**Current 8 tools**: `explorer_open_workspace`, `explorer_spotter_search`, `explorer_inspect_object`, `explorer_get_views`, `explorer_get_view`, `explorer_get_lenses`, `explorer_apply_lens`, `explorer_query_moldql`.

### Core Service (ImpactAnalysisService)

`ImpactAnalysisService` lives at **`crates/cognicode-core/src/application/services/impact_analysis.rs`** (~550 LOC). It is:

- **Stateless**: zero-sized struct, `new() -> Self`, holds no fields.
- **Read-only**: every method takes `&CallGraph`, constructs a `CallGraphProjection`, delegates to algorithms.
- **5 public methods**: `impact_radius`, `has_path`, `shortest_path`, `detect_cycles`, `containing_component`.
- **25/25 tests green** (spec scenarios R1–R8 + edge cases E1–E10, all behavior-first).

All methods accept `&SymbolId` strings and return plain types or existing DTOs (`PathResultDto`, `SccDto`). Missing symbols / empty graphs / no-path return empty/none — **never panic**.

### DTOs (impact_dto.rs)

In **`crates/cognicode-core/src/application/dto/impact_dto.rs`**:
- `ImpactDto` — existing, for count-based impact (unused by `ImpactAnalysisService`).
- `CycleDto` — existing, for count-based cycle detection.
- **`PathResultDto`** — `{ path: Vec<String>, total_cost: f64, found: bool }` — used by `shortest_path`.
- **`SccDto`** — `{ members: Vec<String>, size: usize }` — used by `detect_cycles`.

Both are `Serialize + Deserialize`, ready for MCP wire transmission.

### Binary Path (--postgres)

The binary is **`crates/cognicode-explorer/src/bin/mcp.rs`** (~128 LOC). Construction flow:

```
Args::parse() → open_graph(SQLite) or open_graph_from_postgres(url)
              → Arc<CallGraph>
              → CallGraphRepository::new(graph)  // adapter: Arc<CallGraph> → SymbolRepository
              → ExplorerService::with_all(repo, reader, cwd, search, quality)
              → ExplorerMcpHandler::new(service)
              → rmcp::serve_server(handler, stdio)
```

The `Arc<CallGraph>` is consumed by `CallGraphRepository::new()` — the handler never sees it directly. For impact tools to work, the handler needs access to the same `Arc<CallGraph>`.

### Prior Slices — Metadata Envelope

The `mcp-postgres-envelope` slice (archived 2026-06-09) enriched MCP DTO `TypedRelation` with `provenance: Option<String>` and `confidence: Option<f64>`. Impact tools reuse existing DTOs (`PathResultDto` with `total_cost` already encodes confidence-weighted edges) — no additional envelope work needed for this slice.

---

## 2. Affected Areas

| File | Why |
|------|-----|
| `crates/cognicode-explorer/src/mcp.rs` | **Primary**: add 5 tool handlers, arg structs, schemas, dispatch arms |
| `crates/cognicode-explorer/src/bin/mcp.rs` | **Minor**: clone `Arc<CallGraph>` before it's consumed; pass to `ExplorerMcpHandler::new()` |
| `crates/cognicode-explorer/Cargo.toml` | **No change**: `cognicode-core` already depended; `ImpactAnalysisService` is public |
| `crates/cognicode-explorer/src/lib.rs` | **No change**: existing `pub use mcp::ExplorerMcpHandler` unchanged |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | **No change**: consumed read-only, no modifications |
| `crates/cognicode-core/src/application/dto/impact_dto.rs` | **No change**: DTOs already serializable |
| `crates/cognicode-explorer/src/adapters/call_graph_repository.rs` | **No change**: adapter unchanged |
| Tests in `mcp.rs` (unit) | **New**: ~10-12 tests for impact tool dispatch |
| `crates/cognicode-explorer/tests/integration.rs` | **New**: 1-2 integration tests for binary tool list contract |

---

## 3. Approaches

### Approach A — Handler holds `Option<Arc<CallGraph>>` (Recommended)

**Description**: `ExplorerMcpHandler` gains `graph: Option<Arc<CallGraph>>`. The binary clones `Arc<CallGraph>` before passing it to `CallGraphRepository`, and gives the clone to the handler. Impact tools construct `ImpactAnalysisService::new()` on each call (stateless, zero-cost). When `graph` is `None`, tools return "impact analysis unavailable".

**Architecture**:
```
ExplorerMcpHandler {
    service: Arc<ExplorerService>,  // existing — 8 explorer tools
    graph:   Option<Arc<CallGraph>>, // new — impact tools
}
```

**Dispatch for impact tools**:
```rust
TOOL_IMPACT_RADIUS => {
    let graph = match &self.graph {
        Some(g) => g,
        None => return err("impact analysis unavailable: no call graph loaded".into()),
    };
    let svc = ImpactAnalysisService::new();
    let result: Vec<String> = svc.impact_radius(graph, &id, max_depth)
        .iter().map(|s| s.as_str().to_string()).collect();
    ok_direct(&result)
}
```

| Pros | Cons |
|------|------|
| Minimal change to binary (one `graph.clone()`) | Handler gains a new optional field |
| Zero new traits or adapters | Impact tools unavailable when graph is `None` (acceptable — errors are clear) |
| Respects explorer's stateless dispatch pattern | |
| `Option<Arc<CallGraph>>` is backward-compat — tests that build handlers with `TestRepo` keep working | |
| No cross-crate API changes | |

**Effort**: Low (~200 LOC new code + ~150 LOC tests).

---

### Approach B — `SymbolRepository` exposes `get_graph()`

**Description**: Add a default method `fn get_call_graph(&self) -> Option<&CallGraph> { None }` to the `SymbolRepository` trait. `CallGraphRepository` overrides it. The handler downcasts the trait object to get the graph.

| Pros | Cons |
|------|------|
| Handler doesn't need a new field | Leaks `CallGraph` through explorer port trait (coupling) |
| Graph stays single source of truth in the repo | Trait method with default impl is a breaking change for all implementors (mock repos, test repos) |
| | Only `CallGraphRepository` can provide the graph — not future DB-backed repos |

**Effort**: Medium. Trait change is invasive.

---

### Approach C — Impact tools in `cognicode-core` MCP handler

**Description**: Add impact tools to the existing `CogniCodeHandler` in `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` instead of the explorer.

| Pros | Cons |
|------|------|
| `CogniCodeHandler` already has `get_call_graph()` method (line 63) | The `CogniCodeHandler` is a separate binary (`cognicode`), not the explorer |
| Zero explorer changes | Users would need to run a different MCP server for impact analysis vs exploration |
| | Fragments the MCP surface — agents need two servers for one codebase |

**Effort**: Very Low. But architecturally wrong.

---

### Approach D — Single `impact_analyze` tool with `operation` enum

**Description**: One tool `impact_analyze` that takes `operation: "radius" | "has_path" | "shortest_path" | "cycles" | "component"` plus operation-specific args. Dispatch matches on `operation` internally.

| Pros | Cons |
|------|------|
| Keeps tool count lower (adds 1 vs 5) | Violates the explorer's "one tool = one operation" pattern |
| Single tool schema to maintain | Complex JSON schema with conditional required fields |
| | Agents need to read schema carefully to know what args are needed per operation |

**Effort**: Medium. Schema complexity adds risk.

---

## 4. Recommendation

**Approach A** — **Handler holds `Option<Arc<CallGraph>>`**.

This is the smallest, cleanest slice:

1. **No cross-crate API changes**: `cognicode-core` untouched. `SymbolRepository` trait unchanged.
2. **Matches existing pattern**: `ExplorerMcpHandler` already holds optional state (`ExplorerService` has `Option<Arc<dyn SearchRepository>>`, `Option<Arc<dyn QualityRepository>>`).
3. **`Option` is backward-compat**: unit tests that construct handlers without a graph (TestRepo-based) don't need `Arc<CallGraph>` — impact tools just return a clean error.
4. **Binary change is 2 lines**: clone `Arc<CallGraph>` before passing to repo adapter, then pass clone to handler.

**Tool surface: 5 new tools** (following the explorer's "one operation = one tool" convention):

| Tool Name | Operation | Args | Returns |
|-----------|-----------|------|---------|
| `impact_radius` | `ImpactAnalysisService::impact_radius` | `symbol_id` (required), `max_depth` (optional, default `usize::MAX`) | `Vec<String>` |
| `impact_has_path` | `ImpactAnalysisService::has_path` | `from` (required), `to` (required) | `{ from, to, has_path: bool }` |
| `impact_shortest_path` | `ImpactAnalysisService::shortest_path` | `from` (required), `to` (required) | `PathResultDto` (or `{ found: false }`) |
| `impact_detect_cycles` | `ImpactAnalysisService::detect_cycles` | none | `Vec<SccDto>` |
| `impact_component` | `ImpactAnalysisService::containing_component` | `symbol_id` (required) | `Vec<String>` (or `null` if missing) |

**Total tool count**: 8 existing + 5 new = **13 tools**. Well under the 20-tool page size used in `CogniCodeHandler`.

---

## 5. Risk Analysis

| Risk | Severity | Mitigation |
|------|----------|------------|
| `Arc<CallGraph>` clone is extra allocation | None — `Arc::clone` is atomic increment, zero heap copy | |
| `CallGraphProjection` rebuild per call | Low — `from_call_graph` costs O(V+E) but projection is read-only and graphs are small (< 10K nodes) | Accept for MVP; future slice can cache the projection |
| Missing symbols produce empty results (not errors) | Low — documented behavior matches service contract | Tool descriptions must mention this behavior |
| `SymbolId` string format mismatch | Low — `SymbolId::new()` is infallible; the service does a lookup that returns empty on miss | No validation needed at MCP boundary |
| Test repo (`TestRepo`) doesn't provide `Arc<CallGraph>` | None — handler field is `Option`, tests skip impact tools gracefully | |
| `Option<Arc<CallGraph>>` is `None` when graph loading fails | Medium — users who pass `--postgres` and get a connect error would get "impact analysis unavailable" for all impact tools | Clear error message; the explorer tools (8) still work |

---

## 6. TDD Strategy (mandatory)

**Always-on TDD rule**: write failing tests FIRST, then implement until green, then refactor. No implementation before a failing test.

### Test plan (red → green → refactor, one tool at a time):

```markdown
Phase 1 — Handler field addition (no tool logic yet)
  [ ] R1: test_handler_without_graph_returns_impact_unavailable
         → Construct handler with graph=None, call dispatch with TOOL_IMPACT_RADIUS
         → Assert is_error=true, text contains "impact analysis unavailable"

Phase 2 — impact_radius
  [ ] R2: test_impact_radius_returns_predecessors
         → Build graph: D→A→C, B→C. Dispatch impact_radius(symbol_id="C", max_depth=2)
         → Assert is_error=false, result contains ["A","B","D"]
  [ ] R3: test_impact_radius_missing_symbol_returns_empty
  [ ] R4: test_impact_radius_zero_depth_returns_empty
  [ ] R5: test_impact_radius_missing_max_depth_uses_sentinel

Phase 3 — impact_has_path
  [ ] R6: test_has_path_direct_edge_returns_true
  [ ] R7: test_has_path_transitive_returns_true
  [ ] R8: test_has_path_reverse_returns_false
  [ ] R9: test_has_path_missing_endpoint_returns_false

Phase 4 — impact_shortest_path
  [ ] R10: test_shortest_path_returns_path_dto
  [ ] R11: test_shortest_path_unreachable_returns_not_found
  [ ] R12: test_shortest_path_self_path

Phase 5 — impact_detect_cycles
  [ ] R13: test_detect_cycles_dag_returns_empty
  [ ] R14: test_detect_cycles_mutual_returns_scc

Phase 6 — impact_component
  [ ] R15: test_component_returns_members
  [ ] R16: test_component_missing_returns_none

Phase 7 — Integration
  [ ] R17: test_tool_schemas_includes_all_five_impact_tools (list_tools contract)
  [ ] R18: test_impact_tool_schemas_have_required_args_marked

Phase 8 — Binary integration test
  [ ] R19: integration test verifies binary tool list includes impact tools (mirrors existing mcp_binary_tool_list_matches_spec)
```

**Total: ~19 tests**. All follow the existing test patterns in `mcp.rs` — dispatch via `call_tool_args()`, assert `CallToolResult` structure, extract `first_text()`.

---

## 7. Entropy Analysis (Connascence Landscape)

**Method**: Heuristic (CogniCode build_graph optional for quantitative I(A;B))

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `ExplorerMcpHandler` | `Arc<CallGraph>` | Identity | ~1.0 | ✅ OK (same Arc cloned) |
| `dispatch()` (impact arms) | `ImpactAnalysisService` | Name | ~0.58 (1 call site) | ✅ OK |
| `bin/mcp.rs` graph construction | `ExplorerMcpHandler::new()` | Position | ~0.0 | ✅ OK (clone before consume) |
| Impact tool schemas | `PathResultDto` / `SccDto` fields | Meaning | ~1.0 | ✅ OK (field names in DTOs) |

**Critical Pairs**: None.
**Hidden Connascence**: None.
**SOLID-Entropy Violations**: None. OCP compliant — H(Δ_existing) ≈ 0 bits.
**DQS Impact**: Neutral — pure extension.

---

## 8. Response Shape Detail

For each tool, the MCP response is `Content::text(json)` where `json` is `serde_json::to_string_pretty(&result)`:

| Tool | Success Response | Empty/Missing Response |
|------|-----------------|----------------------|
| `impact_radius` | `["A","B","C"]` | `[]` |
| `impact_has_path` | `{"from":"A","to":"B","has_path":true}` | `{"from":"A","to":"X","has_path":false}` |
| `impact_shortest_path` | `{"path":["A","B"],"total_cost":0.0,"found":true}` | `{"path":[],"total_cost":0.0,"found":false}` |
| `impact_detect_cycles` | `[{"members":["A","B"],"size":2}]` | `[]` |
| `impact_component` | `["A","B","C"]` | `null` (JSON null, not string "null") |

**Error responses** (graph unavailable, invalid args): `CallToolResult::error` with `Content::text(msg)`.

---

## 9. Binary Construction Diff (bin/mcp.rs)

Minimal change — 2 lines added:

```rust
// Existing:
let graph = open_graph_from_postgres(url).await?;
let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));

// New: clone before consume
let graph_clone = graph.clone();  // ADD
let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));

// Existing + new:
let handler = ExplorerMcpHandler::new(service);
// becomes:
let handler = ExplorerMcpHandler::with_graph(service, graph_clone);  // CHANGE
```

Same pattern needed for the SQLite path (non-`--postgres`). `open_graph(&db_path)?` already returns `Arc<CallGraph>`; clone it before passing to repo.

---

## 10. Integration with Prior Slices

| Prior Slice | How This Integrates |
|-------------|-------------------|
| `explorer-graph-foundation` | `CallGraph` edges have `Provenance + Confidence` — these flow through `CallGraphProjection` → `PathResultDto.total_cost` (confidence-weighted) |
| `explorer-graph-postgres-graphstore` | `--postgres` loads `CallGraph` with full metadata via `save_call_graph` / `load_call_graph` |
| `explorer-bridge-postgres` | `open_graph_from_postgres` returns `Arc<CallGraph>` — impact tools consume it |
| `mcp-postgres-envelope` | `PathResultDto` with `total_cost` already embeds confidence; no new envelope work needed |
| `petgraph-postgres-projection` | `CallGraphProjection` is built by `ImpactAnalysisService` — stable, tested, read-only |
| `impact-analysis-service` | Consumed directly — no modification, no new DTOs |

---

## 11. Explore Questions — Answered

| # | Question | Answer |
|---|----------|--------|
| 1 | Where are MCP tools registered? | `crates/cognicode-explorer/src/mcp.rs` — `build_tool_schemas()` + `dispatch()` in `ExplorerMcpHandler` |
| 2 | Existing MCP shapes/tests? | 8 tools, `ExplorerResult<T>` → `CallToolResult` via `ok()`/`err()`, 16 unit + 3 integration tests |
| 3 | Live in explorer or core? | **Explorer MCP layer** — calls core service. Core MCP handler (`CogniCodeHandler`) is a different binary |
| 4 | One tool with enum or multiple? | **5 separate tools** — matches explorer's "one operation = one tool" convention |
| 5 | Arguments? | `symbol_id`/`from`/`to` as String, `max_depth` as usize (optional, sentinel default) |
| 6 | Response shapes? | Reuse `PathResultDto`, `SccDto` from core; `Vec<String>` for lists; lightweight `HasPathResult` for bool |
| 7 | Missing/empty representation? | Empty results (`[]`, `null`, `found: false`) — no errors. Graph-unavailable → text error |
| 8 | TDD tests? | 19 tests across 8 phases, following existing `mcp.rs` test patterns |
| 9 | `--postgres` integration? | Clone `Arc<CallGraph>` in `bin/mcp.rs` before repo adapter consumes it; same Arc serves both paths |

---

## Ready for Proposal

**Yes** — the exploration is complete. All 9 questions answered. Recommended approach (A) is well-defined, minimal, and follows existing patterns. No unresolved blockers.

**Next recommended phase**: `sdd-propose`
