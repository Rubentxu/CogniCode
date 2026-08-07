# Design: MCP Impact Tools

## Technical Approach

Pure extension of the `ExplorerMcpHandler` surface: add `graph: Option<Arc<CallGraph>>` field, 5 tool schemas, 5 dispatch arms delegating to the stateless `ImpactAnalysisService`, and an `ok_direct` helper for unwrapped serialization. Zero changes to core, DB, existing tools, or DTOs.

## Architecture Decisions

### Decision: Handler holds `Option<Arc<CallGraph>>`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Handler field `Option<Arc<CallGraph>>` | Minimal: +1 field, +1 constructor, backward-compat | ✅ Chosen |
| `SymbolRepository` trait adds `get_call_graph()` | Leaks graph through port trait, breaks all impls | ❌ Invasive |
| Separate MCP binary | Fragments surface, agents need 2 servers | ❌ Wrong UX |

**Rationale**: `Option` defaults to `None` — every existing call site and test continues unchanged. The binary clones `Arc<CallGraph>` (atomic refcount, zero heap copy) before `CallGraphRepository::new()` consumes it.

### Decision: `dispatch` receives `graph` as parameter

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add `graph: &Option<Arc<CallGraph>>` param to `dispatch` | Existing 8 arms ignore it; 5 new arms use it | ✅ Chosen |
| Make `dispatch` a method on handler | Bigger refactor, breaks test pattern | ❌ Over-engineered |

**Rationale**: The existing `dispatch(service, request)` free-function pattern is followed by all 17 existing tests. Adding a second parameter is the smallest diff.

### Decision: `ok_direct<T: Serialize>` helper (not modifying `ok`)

**Rationale**: `ok()` wraps `ExplorerResult<T>` (Result-type). Impact tools return plain serializable values (`Vec<String>`, `Vec<SccDto>`, `Option<PathResultDto>`). A separate helper avoids changing the existing `ok` signature or wrapping impact results in a fake `ExplorerResult`.

### Decision: Shared `require_graph` guard for 5 tools

**Rationale**: All 5 tools share identical "graph unavailable" semantics. A single `fn require_graph -> Result<&Arc<CallGraph>, CallToolResult>` avoids 5 duplicated `match` blocks.

### Decision: Test count = 21 (spec test map)

| Category | Count | IDs |
|----------|-------|-----|
| Handler field | 3 | tests 1–3 |
| Schema contract | 2 | tests 4–5 |
| Dispatch (5 tools) | 14 | tests 6–19 |
| `ok_direct` helper | 1 | test 20 |
| Integration | 1 | test 21 |
| **Total** | **21** | |

Proposal says ≥19; spec maps 21. The extra 2 (handler parity + ok_direct) are not redundant — they guard distinct invariants. Target 21.

## Data Flow

```
bin/mcp.rs
  │
  ├── graph.clone() ─────────────────────┐
  │                                       │
  ├── CallGraphRepository::new(graph) ──► SymbolRepository (existing 8 tools)
  │
  └── ExplorerMcpHandler::with_graph(service, Some(graph_clone))
          │
          ├── call_tool() clones service + graph into async block
          │
          └── dispatch(service, &graph, request)
                  │
                  ├── TOOL_OPEN_WORKSPACE .. TOOL_QUERY_MOLDQL (existing)
                  │       └── uses service only
                  │
                  └── TOOL_IMPACT_RADIUS .. TOOL_IMPACT_COMPONENT (new)
                          └── require_graph(&graph, name)?
                               └── ImpactAnalysisService::new()
                                    └── svc.method(&**graph, args)
                                         └── ok_direct(&result)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/mcp.rs` | Modify | +5 tool constants, +5 arg structs, +1 `HasPathResult` struct, `graph` field on handler, `with_graph` constructor, `dispatch` gets `graph` param, `call_tool` clones graph, +5 schema entries in `build_tool_schemas`, +5 match arms, +`require_graph` guard, +`ok_direct` helper, +21 tests |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Modify | +2 lines: `let graph_clone = graph.clone();` before repo, change `new(service)` → `with_graph(service, Some(graph_clone))`. Both SQLite and `--postgres` paths. |
| `crates/cognicode-explorer/tests/integration.rs` | Modify | Rewrite `mcp_tool_names_match_spec` to assert 13 names (add 5 `TOOL_IMPACT_*` imports) |

## Interfaces / Contracts

### New constants (in `mcp.rs`)

```rust
pub const TOOL_IMPACT_RADIUS: &str = "impact_radius";
pub const TOOL_IMPACT_HAS_PATH: &str = "impact_has_path";
pub const TOOL_IMPACT_SHORTEST_PATH: &str = "impact_shortest_path";
pub const TOOL_IMPACT_DETECT_CYCLES: &str = "impact_detect_cycles";
pub const TOOL_IMPACT_COMPONENT: &str = "impact_component";
pub const DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5;
```

### Handler struct change

```rust
#[derive(Clone)]
pub struct ExplorerMcpHandler {
    service: Arc<ExplorerService>,
    graph: Option<Arc<CallGraph>>,
}

impl ExplorerMcpHandler {
    pub fn new(service: Arc<ExplorerService>) -> Self {
        Self { service, graph: None }
    }
    pub fn with_graph(
        service: Arc<ExplorerService>,
        graph: Option<Arc<CallGraph>>,
    ) -> Self {
        Self { service, graph }
    }
}
```

### Dispatch signature

```rust
async fn dispatch(
    service: &Arc<ExplorerService>,
    graph: &Option<Arc<CallGraph>>,
    request: CallToolRequestParams,
) -> CallToolResult
```

### Response shapes per tool

| Tool | Success type | Empty/missing |
|------|-------------|---------------|
| `impact_radius` | `Vec<String>` | `[]` |
| `impact_has_path` | `HasPathResult { from, to, has_path }` | `{ from, to, has_path: false }` |
| `impact_shortest_path` | `Option<PathResultDto>` | `null` (JSON null) |
| `impact_detect_cycles` | `Vec<SccDto>` | `[]` |
| `impact_component` | `Option<Vec<String>>` | `null` (JSON null) |

### Key helpers

```rust
fn require_graph<'a>(
    graph: &'a Option<Arc<CallGraph>>,
    tool: &str,
) -> Result<&'a Arc<CallGraph>, CallToolResult> {
    graph.as_ref().ok_or_else(||
        err(format!("{tool}: impact analysis unavailable — no call graph loaded"))
    )
}

fn ok_direct<T: serde::Serialize>(value: &T) -> CallToolResult {
    match serde_json::to_string_pretty(value) {
        Ok(json) => CallToolResult::success(vec![Content::text(json)]),
        Err(e) => err(format!("failed to serialize tool result: {e}")),
    }
}
```

### Binary diff (bin/mcp.rs) — both paths

```rust
// Before:
let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
let handler = ExplorerMcpHandler::new(service);

// After:
let graph_for_handler = graph.clone();                                    // +1 line
let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
let handler = ExplorerMcpHandler::with_graph(service, Some(graph_for_handler)); // changed
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit (dispatch) | 5 tools × (happy + edge) = 14 tests | `dispatch()` called directly with `call_tool_args`; graph built via `CallGraph::new()` + `add_symbol` + `add_dependency` |
| Unit (handler) | `new()` defaults None, `with_graph(Some)` works, `with_graph(None)` ≡ `new()` | Construct handlers, check `list_tools` count |
| Unit (helper) | `ok_direct` serializes `Vec<String>` and `Option<T>` where T=None | Direct call, assert text content |
| Schema contract | 13 tools, 13-name set | `build_tool_schemas().len()`, `tool_names().len()` |
| Integration | Binary tool list = 13 | Rewrite `mcp_tool_names_match_spec` |

### TDD Sequence (RED-first)

**Batch 1 — RED gate** (1 test, must fail to compile):
- `test_handler_without_graph_returns_impact_unavailable` — needs `TOOL_IMPACT_RADIUS` constant + `dispatch` with graph param

**Batch 2 — Infrastructure** (4 tests):
- `test_with_graph_some_makes_impact_arms_reachable`, `test_with_graph_none_matches_new_legacy`
- `test_tool_schemas_list_thirteen_tools`, `test_tool_names_contains_impact_constants`

**Batch 3 — Helper** (1 test):
- `test_ok_direct_serializes_pretty_json`

**Batch 4 — impact_radius** (5 tests):
- Predecessors, missing root, default depth=5, zero depth, unknown root

**Batch 5 — impact_has_path** (2 tests):
- Direct/transitive/unreachable, self-path

**Batch 6 — impact_shortest_path** (3 tests):
- Cheapest path, unreachable→null, self-path

**Batch 7 — impact_detect_cycles** (2 tests):
- Returns SCCs (multi-cycle), DAG→empty

**Batch 8 — impact_component** (2 tests):
- Returns members, missing→null

**Batch 9 — Integration** (1 test):
- `mcp_tool_names_match_spec` rewritten for 13

### First failing test (RED gate)

```rust
#[tokio::test]
async fn test_handler_without_graph_returns_impact_unavailable() {
    let (service, _dir) = build_test_service();
    let handler = ExplorerMcpHandler::new(service);
    let result = dispatch(
        &handler.service(),
        &None,
        call_tool_args(TOOL_IMPACT_RADIUS, json!({"root": "x", "max_depth": 1})),
    ).await;
    assert!(result.is_error());
    assert!(first_text(&result).contains("impact analysis unavailable"));
}
```

This fails to compile because `dispatch` currently takes 2 args, not 3.

## Migration / Rollout

No migration required. Pure additive change — `Option` defaults to `None`, existing behavior preserved.

## Open Questions

- [ ] None — all 11 design points resolved against the actual codebase.
