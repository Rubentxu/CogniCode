# Exploration: forward-reach-impact

> **Date**: 2026-06-09
> **Mode**: hybrid (Engram + OpenSpec; LogSeq unavailable)
> **TDD**: User-overridden STRICT (red → green → refactor mandatory)

---

## Exploration Questions Answered

### Q1: What does `ImpactAnalysisService` expose for predecessor vs forward reach?

Current `ImpactAnalysisService` (`crates/cognicode-core/src/application/services/impact_analysis.rs`, ~650 LOC including tests) exposes **5 methods**, all predecessor-oriented or direction-agnostic:

| Method | Direction | Answers |
|--------|-----------|---------|
| `impact_radius(root, max_depth)` | **Predecessor-only** (reverse BFS) | "What depends on X?" |
| `has_path(from, to)` | Any direction | "Is there a path?" |
| `shortest_path(from, to)` | Any direction | "Cheapest path?" |
| `detect_cycles()` | Undirected (SCC) | "Any mutual cycles?" |
| `containing_component(id)` | Undirected | "What component is X in?" |

**There is NO forward reach method.** The spec explicitly states: "Forward reach (`forward_reach`) is a future slice." The mcp-impact-tool archive report lists it as a locked follow-on: "Forward impact reach (`forward_reach`) — awaiting `ImpactAnalysisService::forward_radius` method."

### Q2: What does `CallGraphProjection::find_impact_radius` do directionally?

It is a **reverse BFS** — walks `Direction::Incoming` edges exclusively (line 332 of `call_graph_projection.rs`):

```rust
for edge in self.graph.edges_directed(ni, Direction::Incoming) {
    let pred = edge.source();
    // ... follow predecessors ...
}
```

Answers "what breaks if I change X" by surfacing callers (predecessors). Root is excluded from results. `max_depth == 0` short-circuits. Missing root returns `vec![]`.

### Q3: Does `CallGraphProjection` already expose enough for forward reach?

`CallGraphProjection` has the **internals** needed but no **public method** for forward BFS:

- `has_path(from, to)` → yes, works in any direction
- `dijkstra(from, to)` → yes, works in any direction
- `find_impact_radius(root, max_depth)` → **predecessor-only**
- Private fields: `graph: StableGraph<...>`, `id_to_index: HashMap<SymbolId, NodeIndex>` → accessible only within `call_graph_projection.rs`

**Verdict**: A new public method is needed. Options:

1. **Add `find_forward_reach` to `CallGraphProjection`** (recommended) — forward BFS using `Direction::Outgoing`, symmetric with `find_impact_radius`. Keeps algorithm implementations cohesive in one place.
2. **Add a public accessor for raw graph nodes and iterate in `ImpactAnalysisService`** — exposes internals, breaks encapsulation.
3. **Use `has_path` iteratively** — O(n²) for each successor check, wasteful.

**Recommendation**: Option 1 — `CallGraphProjection::find_forward_reach(root, max_depth) -> Vec<SymbolId>`.

### Q4: Should this slice add core service only, MCP tool too, or both?

Prior SDD slices followed a layered approach (projection → service → MCP as separate slices). However, forward reach is a **single method** on each layer, and the patterns are already established. Adding all three layers in one slice:

| Layer | LOC (impl + tests) |
|-------|---------------------|
| `CallGraphProjection::find_forward_reach` | ~40 + ~60 |
| `ImpactAnalysisService::forward_radius` | ~30 + ~50 |
| MCP `impact_forward_radius` tool | ~40 + ~60 |
| **Total** | **~280 lines** |

Well within the 400-line review budget. The prior slices were separate because each was a substantial capability (8 algorithms, 5 service methods, 5 MCP tools). This is ONE method per layer.

**Recommendation**: Single slice — all three layers. This gives complete TDD coverage from projection through MCP exposure, and the change is small enough to review in one PR.

### Q5: What naming avoids confusion?

Existing naming conventions and recommendations:

| Layer | Predecessor (existing) | Forward (proposed) | Rationale |
|-------|----------------------|--------------------|-----------|
| Projection | `find_impact_radius` | `find_forward_reach` | Symmetric: verb + direction + noun |
| Service | `impact_radius` | `forward_radius` | Symmetric: direction + "radius" |
| MCP tool | `impact_radius` | `impact_forward_radius` | Follows `impact_*` prefix convention |

**Rejected alternatives**:
- `downstream_impact` → too long, asymmetric with `impact_radius`
- `reachable_from` → graph-theory term, good but doesn't mirror `impact_radius`
- `dependents` → ambiguous (dependents of X = those that X depends on, or those that depend on X?)
- `affected_by` → means "what affects this" (i.e., predecessors), already covered by `impact_radius`
- `impacts` → verb form, inconsistent with noun-form `impact_radius`

### Q6: What behavior-first TDD tests should be written first?

Following strict RED → GREEN → REFACTOR, tests at each layer:

**Phase 1 — Projection (RED gate: `find_forward_reach` doesn't exist yet)**

| Test | Verifies |
|------|----------|
| `test_find_forward_reach_direct_successor` | A→B, depth 1 from A = {B} |
| `test_find_forward_reach_transitive_chain` | A→B→C, depth 2 from A = {B, C} |
| `test_find_forward_reach_root_not_included` | A→B, A not in result |
| `test_find_forward_reach_zero_depth_empty` | depth 0 → vec![] |
| `test_find_forward_reach_missing_root_empty` | unknown id → vec![] |
| `test_find_forward_reach_max_sentinel` | `usize::MAX` = all reachable |
| `test_find_forward_reach_cycle_terminates` | A→B→A, visited set prevents infinite loop |
| `test_find_forward_reach_disconnected_no_cross` | A→B, C→D, forward from A = {B} only |

**Phase 2 — Service (RED gate: `forward_radius` doesn't exist yet)**

Follows exact `impact_radius` test pattern:
- `test_forward_radius_bounded_successors`
- `test_forward_radius_zero_depth`
- `test_forward_radius_missing_root`
- `test_forward_radius_empty_graph`
- `test_forward_radius_max_sentinel`
- `test_forward_radius_non_mutating` (100x repeat, graph unchanged)

**Phase 3 — MCP tool (RED gate: dispatch arm for `impact_forward_radius` doesn't exist)**

- `test_impact_forward_radius_returns_successors` (happy path)
- `test_impact_forward_radius_missing_root_arg` (error message)
- `test_impact_forward_radius_default_max_depth` (omitted → 5)
- `test_impact_forward_radius_zero_depth_returns_empty`
- `test_impact_forward_radius_unknown_root_returns_empty`
- `test_impact_forward_radius_without_graph_unavailable` (graph=None)
- Integration: update `mcp_tool_names_match_spec` for 14 tools

**RED gate test (first test to write — must fail to compile)**:

```rust
#[test]
fn test_find_forward_reach_direct_successor() {
    // A -> B; forward from A at depth 1 should return {B}
    let g = make_graph(|g| {
        g.add_symbol(sym("A"));
        g.add_symbol(sym("B"));
        add_edge(g, "A", "B", DependencyType::Calls);
    });
    let proj = CallGraphProjection::from_call_graph(&g);
    let result = proj.find_forward_reach(&id("A"), 1);
    assert_eq!(result, vec![id("B")]);
}
```

### Q7: How should edge cases behave?

Following the exact same contract as `find_impact_radius` / `impact_radius`:

| Edge Case | Expected Behavior | Reason |
|-----------|-------------------|--------|
| `max_depth == 0` | `vec![]` | No hops taken; short-circuit |
| Missing symbol | `vec![]` (no panic) | Consistent with impact_radius |
| Empty graph | `vec![]` | Consistent with impact_radius |
| Cycles (A→B→A) | `{B}` at depth ≥ 1 | Visited set prevents re-entry; A is root (excluded) |
| Disconnected graph | Only successors of queried node | BFS doesn't cross disconnected components |
| Self-path / root | Root excluded from results | Consistent with impact_radius |
| `usize::MAX` | All reachable successors | Sentinel for unbounded |

### Q8: Does this require any PostgreSQL/DB/UI changes?

**No.** This is purely:
- `cognicode-core::infrastructure::graph` — new method on `CallGraphProjection`
- `cognicode-core::application::services` — new method on `ImpactAnalysisService`
- `cognicode-explorer::mcp` — new MCP dispatch arm + constant + schema

No DB schema changes. No new repositories. No UI changes. No new dependencies in either crate's `Cargo.toml`.

---

## Current State

The codebase currently answers **"what depends on X?"** (predecessors) but NOT **"what does X affect?"** (successors/forward reach). The gap is:

```
Current:  impact_radius(X) → {callers of X}        (reverse BFS, predecessors)
Missing:  forward_radius(X) → {things X calls}      (forward BFS, successors)
```

`has_path(A, B)` and `shortest_path(A, B)` work in both directions, but require the caller to already know which symbol to check — they don't answer "give me everything downstream."

## Affected Areas

| File | Impact | LOC delta (est.) |
|------|--------|-------------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | **Modified** — new `find_forward_reach` method + tests | +100 |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` | **Modified** — new `forward_radius` method + tests | +80 |
| `crates/cognicode-explorer/src/mcp.rs` | **Modified** — new constant, arg struct, schema, dispatch arm, tests | +100 |
| `crates/cognicode-explorer/tests/integration.rs` | **Modified** — update `mcp_tool_names_match_spec` | +5 |
| `crates/cognicode-core/Cargo.toml` | **None** | 0 |
| `crates/cognicode-explorer/Cargo.toml` | **None** | 0 |
| PostgreSQL / repositories | **None** | 0 |
| `CallGraph` aggregate | **None** (consumed read-only) | 0 |
| `ImpactAnalyzer` domain service | **None** | 0 |

## Approaches

### 1. Single Slice — All Three Layers (Recommended)

Add `find_forward_reach` to `CallGraphProjection`, `forward_radius` to `ImpactAnalysisService`, and `impact_forward_radius` MCP tool — all in one SDD cycle.

- **Pros**: Complete feature delivered. All TDD coverage from projection through MCP exposure. ~280 LOC, well within 400-line review budget. Patterns already established by prior slices. Follows `impact_radius` naming symmetry exactly.
- **Cons**: Touches 3 files across 2 crates. 3 phases of RED→GREEN→REFACTOR needed.
- **Effort**: Low-Medium

### 2. Two Slices — Projection+Service, then MCP

Slice 1: `find_forward_reach` + `forward_radius`. Slice 2: MCP tool.

- **Pros**: Smaller individual slices (~180 + ~100 LOC). Follows prior SDD pattern (service before MCP).
- **Cons**: Two full SDD cycles for what is fundamentally one capability. Slice 1 has no user-facing exposure (no MCP tool to exercise it from outside).
- **Effort**: Medium (two cycles)

### 3. Core-Only — No MCP Tool

Slice 1: `find_forward_reach` + `forward_radius` only. Defer MCP.

- **Pros**: Smallest change (~180 LOC). Core capability usable programmatically.
- **Cons**: No user-facing tool. Requires another SDD cycle for MCP exposure. Breaks the pattern of delivering complete, testable features.
- **Effort**: Low (but incomplete)

## Recommendation

**Approach 1 — Single Slice, All Three Layers.**

Rationale:
- This is ONE new method per layer, not a new service or new tool surface. The prior slices were separate because each introduced substantial new capabilities (8 algorithms, 5 service methods, 5 MCP tools). This is symmetric: one algorithm, one service method, one MCP tool.
- The 400-line review budget comfortably accommodates ~280 lines.
- TDD is stronger when all layers are tested together — the MCP tool test exercises the full stack.
- Naming symmetry with `impact_radius`/`find_impact_radius` makes the feature intuitive: `impact_radius` = "what breaks me?", `forward_radius` = "what do I break?"
- The RED gate is cleaner: one test at a time, one layer at a time (projection first, then service, then MCP).

## Entropy Analysis (Qualitative)

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | ~0.5 bits | < 1.0 | ✅ Low |
| H(Δ_new) | ~1.0 bits | > 0 | ✅ |
| New connascence pairs | 2 (projection→StableGraph direction, service→projection method) | < 3 | ✅ |
| OCP compliant? | Yes — extends, doesn't modify | yes | ✅ |
| Enthalpy budget | Low — one new BFS method, one new delegation, one new dispatch arm | — | ✅ |

**Existing files modified**: `call_graph_projection.rs` (new method appended), `impact_analysis.rs` (new method appended), `mcp.rs` (new constant + arg struct + dispatch arm + tests appended). All changes are additive — no existing code is refactored, no signatures changed.

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Naming confusion between `impact_radius` (predecessors) and `forward_radius` (successors) | Low | Document direction semantics in rustdoc. The word "radius" in both establishes a family: both are BFS-bounded reachable-set queries. |
| Cycle in forward BFS causing stack overflow | Low | Visited `HashSet<NodeIndex>` prevents re-entry. Same pattern as `find_impact_radius` which already handles cycles in the reverse direction. |
| `CallGraphProjection` grows beyond single-responsibility | Low | One new method (~30 lines) doesn't change the struct's responsibility — it's still "algorithmic queries over the call graph projection." |
| MCP tool count grows (13→14) requiring integration test update | Low | `mcp_tool_names_match_spec` update is trivial (~5 lines). |
| Forward BFS for deep/large graphs (O(V+E)) | Low | Same complexity as `find_impact_radius`. No new performance concern. |

## Follow-On Slices Now Unblocked (after this change)

1. **`impact_forward_has_path` / `impact_forward_shortest_path`** — already work; just expose new MCP tool names if needed (but `has_path` and `shortest_path` are already direction-agnostic).
2. **Multi-root forward impact** — "what do I break if I change these 3 symbols together?"
3. **Bidirectional impact view** — combine `impact_radius` + `forward_radius` into a single "impact zone" view.
4. **Confidence-weighted forward scoring** — aggregate confidence along all forward paths.

## Answers to Additional Explore Questions Implicit in the Brief

**"What is the smallest clean TDD slice?"** → Single slice with all three layers. ~280 LOC, well within budget.

**"Should this add core service only, MCP tool too, or both?"** → Both + projection method. One slice.

**"What naming avoids confusion?"** → `find_forward_reach` (projection), `forward_radius` (service), `impact_forward_radius` (MCP). Symmetric with existing `impact_radius` family.

**"What behavior-first tests should be written first?"** → Projection `find_forward_reach` tests first (RED gate), then service `forward_radius`, then MCP `impact_forward_radius`.

**"How should edge cases behave?"** → Identical contract to `impact_radius`: empty on missing/zero-depth, no panic, visited-set cycle protection, root excluded.

**"Does this require PostgreSQL/DB/UI changes?"** → No.

## Ready for Proposal

**Yes.** The exploration confirms:
- `CallGraphProjection` needs a new `find_forward_reach` method (forward BFS via `Direction::Outgoing`)
- `ImpactAnalysisService` needs a new `forward_radius` method (delegates to projection)
- MCP handler needs a new `impact_forward_radius` tool (follows exact `impact_radius` dispatch pattern)
- Single SDD slice delivers all three layers; ~280 LOC, within 400-line budget
- No DB/UI changes; no new dependencies
- Naming symmetry: `impact_radius` (predecessors) ↔ `forward_radius` (successors)
- TDD RED-gate: write `test_find_forward_reach_direct_successor` first, watch it fail to compile, then implement

The orchestrator should launch `sdd-propose` for `forward-reach-impact` with the approach: single slice adding forward reach across projection, service, and MCP layers.
