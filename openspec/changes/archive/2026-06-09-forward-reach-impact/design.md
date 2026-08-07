# Design: forward-reach-impact

## Technical Approach

Single vertical slice adding forward-reach (successor) BFS across projection → service → MCP. Mirrors the existing predecessor chain (`find_impact_radius` / `impact_radius` / `impact_radius`) but on `Direction::Outgoing`. No new dependencies, no new DTOs, no DB/UI changes.

## Architecture Decisions

### Decision: Naming convention

| Layer | Predecessor (existing) | Forward (new) |
|-------|----------------------|--------------------|
| Projection | `find_impact_radius` | `find_forward_reach` |
| Service | `impact_radius` | `forward_radius` |
| MCP tool | `impact_radius` | `impact_forward_radius` |

**Choice**: "forward" prefix at projection/service, "forward" infix at MCP.
**Alternatives**: `find_successors`, `outgoing_radius`.
**Rationale**: MCP uses `impact_forward_radius` to sort alphabetically next to `impact_radius`. Rustdoc disambiguates with explicit direction semantics ("successors", "what does X affect?").

### Decision: Reuse DEFAULT_IMPACT_RADIUS_DEPTH

**Choice**: Share `DEFAULT_IMPACT_RADIUS_DEPTH = 5` for both directions.
**Alternatives**: New `DEFAULT_FORWARD_DEPTH` constant.
**Rationale**: Same semantic role. Two constants with the same value = DRY violation.

### Decision: BFS ordering unspecified

**Choice**: Result `Vec<SymbolId>` is unsorted (same as `find_impact_radius`).
**Rationale**: Symmetry with predecessor chain. Tests sort before comparing.

## Data Flow

```
MCP request -> dispatch -> ImpactForwardRadiusArgs
                                |
                                v
                 ImpactAnalysisService::forward_radius
                                |
                                v
                 CallGraphProjection::from_call_graph(graph)
                                |
                                v
                 find_forward_reach (BFS Direction::Outgoing)
                                |
                                v
                 Vec<SymbolId> -> Vec<String> -> ok_direct
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modify | Add `find_forward_reach` (~25 LOC) + 9 tests (~90 LOC) |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | Modify | Add `forward_radius` (~10 LOC) + 3 tests (~40 LOC) |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | Add constant, args struct, dispatch arm, schema, update TOOL_NAMES (~80 LOC) + 6 tests (~100 LOC), update 5 existing tests 13->14 |
| `crates/cognicode-explorer/tests/integration.rs` | Modify | Update `mcp_tool_names_match_spec`: assert 14, add import (~5 LOC) |

## Interfaces / Contracts

### CallGraphProjection::find_forward_reach

```rust
/// Compute the forward reach of `root`: successors reachable within
/// `max_depth` outgoing hops (Direction::Outgoing). Symmetric
/// counterpart of find_impact_radius (Direction::Incoming).
/// Root excluded. Returns vec![] for missing root, depth 0, or
/// empty projection.
pub fn find_forward_reach(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>
```

Implementation mirrors `find_impact_radius` exactly: same early-return structure, same `(NodeIndex, usize)` BFS queue, same `HashSet<NodeIndex>` visited-set. Only differences: `Direction::Outgoing` and `edge.target()` for the neighbor.

### ImpactAnalysisService::forward_radius

```rust
/// Forward impact radius. Delegates to find_forward_reach.
pub fn forward_radius(&self, graph: &CallGraph, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>
```

Mirrors `impact_radius` signature and delegation pattern.

### MCP dispatch arm

```rust
TOOL_IMPACT_FORWARD_RADIUS => {
    // require_graph -> parse ImpactForwardRadiusArgs ->
    // ImpactAnalysisService::forward_radius -> ok_direct(&strings)
}
```

`ImpactForwardRadiusArgs` has same shape as `ImpactRadiusArgs`: `root: Option<String>`, `max_depth: Option<usize>`.

## Testing Strategy

### TDD Sequence (STRICT RED-first per layer)

**Step 1 - Projection RED->GREEN:**
RED: `test_find_forward_reach_direct_successor` (A->B, depth 1 -> {B})
GREEN: implement `find_forward_reach`
Then add 8 more projection tests.

**Step 2 - Service RED->GREEN:**
RED: `test_forward_radius_mirrors_find_forward_reach`
GREEN: implement `forward_radius`
Then add empty-graph and missing-symbol tests.

**Step 3 - MCP RED->GREEN:**
RED: `test_impact_forward_radius_returns_successors`
GREEN: add constant, args, schema, dispatch arm, update TOOL_NAMES
Then add 5 more dispatch tests + update 5 existing tests.

**Step 4 - Integration GREEN:**
Update `mcp_tool_names_match_spec`: assert 14, add `TOOL_IMPACT_FORWARD_RADIUS`.

### Test Names

| # | Test Name | Expected |
|---|-----------|----------|
| **Projection** | | |
| 1 | `test_find_forward_reach_direct_successor` (RED gate) | A->B, d=1 -> {B} |
| 2 | `test_find_forward_reach_transitive` | A->B->C, A->D, d=2 -> {B,C,D} |
| 3 | `test_find_forward_reach_zero_depth` | d=0 -> vec![] |
| 4 | `test_find_forward_reach_missing_root` | unknown -> vec![] |
| 5 | `test_find_forward_reach_cycle` | A->B->C->A, MAX -> {B,C} |
| 6 | `test_find_forward_reach_disconnected` | no successors -> vec![] |
| 7 | `test_find_forward_reach_empty_projection` | empty graph -> vec![] |
| 8 | `test_find_forward_reach_max_sentinel` | usize::MAX -> all reachable |
| 9 | `test_find_forward_reach_depth_boundary` | multi-fanout exact boundary |
| **Service** | | |
| 1 | `test_forward_radius_mirrors_find_forward_reach` (RED gate) | identical sets |
| 2 | `test_forward_radius_empty_graph_returns_empty` | vec![] |
| 3 | `test_forward_radius_missing_symbol_returns_empty` | vec![] |
| **MCP** | | |
| 1 | `test_impact_forward_radius_returns_successors` (RED gate) | JSON array |
| 2 | `test_impact_forward_radius_missing_root_arg` | is_error, "missing required arg" |
| 3 | `test_impact_forward_radius_default_depth_is_5` | 7-chain -> 5 successors |
| 4 | `test_impact_forward_radius_zero_depth` | [] |
| 5 | `test_impact_forward_radius_unknown_root` | [] |
| 6 | `test_impact_forward_radius_graph_unavailable` | is_error, "impact analysis unavailable" |

### Existing Tests Requiring Changes (13->14)

| Test | Change |
|------|--------|
| `tool_schemas_list_thirteen_tools` | Rename -> `_fourteen_tools`, assert 14 |
| `tool_names_exposed_via_back_compat_helper` | Assert 14, add forward constant |
| `test_with_graph_some_makes_impact_arms_reachable` | Assert 14 tools |
| `test_with_graph_none_matches_new_legacy` | Assert 14, add TOOL to unavailable loop |
| `mcp_tool_names_match_spec` (integration) | Assert 14, add import + expected entry |

## Migration / Rollout

No migration required. Pure additive — no signature changes, no DB schema changes, no feature flags.

## Open Questions

None.
