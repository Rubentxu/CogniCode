# Proposal: Forward Reach Impact

## Intent

`impact_radius` answers "what depends on X?" (predecessors, reverse BFS). Missing: "what does X affect?" (successors, forward BFS). This gap forces agents to guess downstream impact manually. Add `find_forward_reach` (outgoing BFS) as symmetric counterpart — same layers, same semantics.

## Scope

### In Scope
- `CallGraphProjection::find_forward_reach(root, max_depth)` — outgoing BFS via `Direction::Outgoing`, visited-set guard, max_depth boundary, root excluded (~30 LOC)
- `ImpactAnalysisService::forward_radius(graph, root, max_depth)` — delegating wrapper (~15 LOC)
- MCP tool `impact_forward_radius` — constant `TOOL_IMPACT_FORWARD_RADIUS`, args struct, schema entry, dispatch arm, tests (~100 LOC)
- Unit tests: projection (7), service (3), MCP dispatch (5)
- Integration: `mcp_tool_names_match_spec` → 14 tools

### Out of Scope
- No DB/UI changes. No new dependencies. No protocol overhaul.
- No modification to existing `impact_radius`, `has_path`, or `shortest_path` signatures.
- No caching or performance optimizations beyond existing per-call rebuild.

## Capabilities

### New Capabilities
- `explorer-forward-reach`: MCP tool exposing forward-successor lookup to agents

### Modified Capabilities
- `callgraph-petgraph-projection`: new `find_forward_reach` method
- `impact-analysis-service`: new `forward_radius` method
- `explorer-impact-tools`: new tool constant (`TOOL_IMPACT_FORWARD_RADIUS`), 13→14 tool count, schema + dispatch arm

## Approach

Single slice — projection + service + MCP in one SDD cycle (~280 LOC). Follows existing patterns exactly:
- **Projection**: forward BFS mirroring `find_impact_radius` but on `Direction::Outgoing` edges
- **Service**: zero-field stateless wrapper matching `impact_radius` delegation pattern
- **MCP**: constant → args struct → schema → dispatch arm with graph guard, same `ok_direct` serializer

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/…/call_graph_projection.rs` | +1 method | `find_forward_reach` (~30 LOC) |
| `crates/cognicode-core/…/impact_analysis.rs` | +1 method | `forward_radius` (~15 LOC) |
| `crates/cognicode-explorer/src/mcp.rs` | +1 tool | constant, args, schema, dispatch (~100 LOC) |
| `crates/cognicode-explorer/tests/integration.rs` | Modified | 13→14 tool count |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Naming confusion: `impact_radius` vs `forward_radius` | Medium | Rustdoc: "predecessor" vs "successor" direction explicit in every docstring |
| Infinite loop on cycles | Low | Visited `HashSet<NodeIndex>` at projection level (same guard as `find_impact_radius`) |
| `max_depth == usize::MAX` saturation | Low | Same sentinel contract as `impact_radius` — pass-through to BFS |
| Tool count mismatch in older integrations | Low | Integration test `mcp_tool_names_match_spec` asserts 14 |

## Rollback Plan

Revert: remove `TOOL_IMPACT_FORWARD_RADIUS` from constants/names/schemas/dispatch. Revert tool count to 13. Delete `find_forward_reach` and `forward_radius`. No data migration.

## Dependencies

None. Uses existing `CallGraphProjection`, `ImpactAnalysisService`, `ok_direct`, `require_graph`.

## Success Criteria

- [ ] **RED GATE**: `test_find_forward_reach_direct_successor` compiles but FAILS — no implementation yet
- [ ] `find_forward_reach` passes 7 edge-case tests (zero depth, missing symbol, empty graph, cycle, disconnected, `usize::MAX`, direct successor)
- [ ] `forward_radius` passes 3 delegation tests (mirrors projection results, empty graph, missing symbol)
- [ ] `impact_forward_radius` MCP tool passes 5 dispatch tests (success, missing root, max_depth omitted→default 5, graph unavailable, invalid args)
- [ ] `mcp_tool_names_match_spec` asserts 14 tools with `TOOL_IMPACT_FORWARD_RADIUS` present
- [ ] 0 regressions: `cargo test --all-targets -p cognicode-core -p cognicode-explorer` green
- [ ] `cargo clippy --all-targets -p cognicode-core -p cognicode-explorer` clean
