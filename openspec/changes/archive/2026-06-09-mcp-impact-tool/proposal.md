# Proposal: MCP Impact Tools

## Intent

Expose `ImpactAnalysisService` through 5 new MCP tools in the explorer, so agents can query impact radius, path existence, shortest path, cycle detection, and component membership without switching MCP servers.

## Scope

**In**: 5 tools (`impact_radius`, `impact_has_path`, `impact_shortest_path`, `impact_detect_cycles`, `impact_component`); `Option<Arc<CallGraph>>` field on `ExplorerMcpHandler`; binary clones `Arc<CallGraph>` before repo adapter consumes it; new `ok_direct()` response helper; 19 TDD tests.

**Out**: No core service changes, no UI, no DB schema, no protocol overhaul, no separate MCP binary, no forward-reach (future slice).

## Capabilities

### New
- **`explorer-impact-tools`**: 5 MCP tools delegating to `ImpactAnalysisService` (stateless coordinator). Each tool constructs `CallGraphProjection` per call. When graph is unavailable, tools return clear error message. Tool responses reuse existing `PathResultDto` and `SccDto`; `impact_radius`/`impact_component` return `Vec<String>`; `impact_has_path` returns `{from, to, has_path}`; `impact_detect_cycles` returns `Vec<SccDto>`.

### Modified
None. Pure extension — zero changes to existing specs.

## Approach

**Handler holds `Option<Arc<CallGraph>>`** (Approach A per exploration). Minimal, backward-compatible:

1. `ExplorerMcpHandler` gains `graph: Option<Arc<CallGraph>>` field and `with_graph(service, graph)` constructor.
2. `call_tool` clones both `service` and `graph` into async block.
3. `dispatch` receives `graph`; impact arms construct `ImpactAnalysisService::new()`, build projection per call, delegate.
4. Binary clones `Arc<CallGraph>` before `CallGraphRepository::new(graph)` consumes it — one extra line per path (SQLite + `--postgres`).
5. New `ok_direct(&result)` helper serializes any `Serialize` directly (existing `ok()` wraps `ExplorerResult<T>`).
6. `build_tool_schemas()` returns 13 tools total. 5 new schemas follow existing hand-rolled pattern.

## Affected Areas

| File | Impact |
|------|--------|
| `crates/cognicode-explorer/src/mcp.rs` | +5 tools, arg structs, schemas, dispatch arms, `ok_direct()`, handler field |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Clone `Arc<CallGraph>` before repo adapter (2 lines) |
| `crates/cognicode-explorer/tests/integration.rs` | Tool list contract test |

No changes to `cognicode-core`, `Cargo.toml`, or existing DTOs.

## Risks

| Risk | Mitigation |
|------|-----------|
| Graph unavailable → errors on impact tools | Clear "impact analysis unavailable" message; 8 explorer tools unaffected |
| `CallGraphProjection` rebuilt per call O(V+E) | Acceptable for <10K-node graphs; caching deferred |
| Backward compatibility | `Option` defaults to `None`; all existing tests pass unchanged |
| Tool naming collision | `impact_*` prefix unique across 13-tool surface |

## Rollback

Remove `graph` field from handler, delete 5 dispatch arms and tool schemas, revert binary clone. Single commit revert, zero data loss.

## Dependencies

- **`impact-analysis-service`** (archived, 25/25 tests green): consumed read-only.
- **`petgraph-postgres-projection`** (archived): `CallGraphProjection` is stable, read-only.

## Success Criteria (TDD-First)

**Red-before-green gate: implementation starts only after these fail.**

1. **RED**: `test_handler_without_graph_returns_impact_unavailable` — construct handler with `graph=None`, dispatch any impact tool → `is_error=true`, text contains "impact analysis unavailable"
2. **RED**: All 5 per-tool tests fail before dispatch arms written
3. **GREEN**: 19 tests pass (16 unit dispatch + 2 schema contract + 1 integration binary tool list)
4. **GREEN**: `list_tools` returns 13 tools (8 + 5)
5. `cargo clippy --all-targets -p cognicode-explorer` clean
6. `cargo test -p cognicode-explorer` zero regressions
7. `git diff` on `cognicode-core` is empty

## Entropy Budget

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0.0 bits | < 1.0 | ✅ pure extension |
| H(Δ_new) | ~2.5 bits | > 0 | ✅ (log2(5 tools) + schema overhead) |
| New connascence pairs | 2 (handler→graph, handler→service) | < 3 | ✅ |
| OCP compliant | Yes | — | ✅ |

**Method**: Heuristic (CogniCode graph not built for this phase). Confidence: estimated.
**Verdict**: Green — zero entropy introduced to existing components. All new coupling is at the MCP wiring layer.
