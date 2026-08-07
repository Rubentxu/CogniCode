# Kernel Proposal: moldable-view-call-graph

**Status:** Proposed — pending sdd-kernel-spec
**Date:** 2026-06-20
**Context quality:** C2
**Author:** sdd-kernel-propose

## Intent

The `call_graph` view renders a blank SVG despite correct metadata (fan in/out, signature).
`PaneInspector` always renders `<Blocks>`; no routing to a graph renderer exists. This change
fixes the bug AND introduces the Moldable Development exploration experience (Pane Stack
click-to-explore + Exploration Snapshot persistence) so developers navigate the knowledge graph
with GtPager-like patterns. Per grill Decision 13: NO backward compatibility.

## Context Gate

| Knowledge Coverage | Quality | Taxonomy | Extra Effort |
|--------------------|---------|----------|--------------|
| sufficient | C2 | routing-gap, schema-stamp, persistence | deepen |

## Knowledge Alignment

- Roadmap / Backlog: `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md` ✅
- Work Items / Specs: delta spec ❌ — owned by sdd-kernel-spec
- ADR / Architecture Sources: `docs/adr/ADR-040-graph-view-renderer.md` ✅
- Ownership Source: grill session 2026-06-20 (13/13 decisions resolved)
- Prior Learnings: explore-report.md ✅; Engram: CogniCode architecture decisions

## Knowledge Decisions

- Stays memory-only: None — all 13 grill decisions are ADR-backed.
- Promote to durable knowledge: **ADR-040 addendum required** — schema gap correction (see below).

### ⚠ Contradiction Surfaced — Option A is NOT frontend-only

The task input claims: *"backend Rust already sends `view_kind` (dto.rs:195)"*. **Verified FALSE.**

| Claim | Code Evidence | Reality |
|-------|--------------|---------|
| `ContextualView` has `view_kind` | dto.rs:194-213 | **No such field.** Struct has `renderer_kind` only. |
| Backend sends `view_kind` for call graph | views.rs:178-187 `build_callgraph` | Uses `..Default::default()` → `renderer_kind = Json`. No `view_kind`. |
| Service stamps descriptor metadata | view.rs:297 `executor.build(&ctx).await` | Returns DTO **directly** — no stamp from `ViewDescriptor`. |

The `ViewDescriptor` trait (registry.rs:130) **does** know `view_kind = CallGraph` and
`renderer_kind = Graph` — but the service layer never transfers this to the `ContextualView` DTO.

**Consequence:** Adding `view_kind: viewKindSchema.optional()` to the frontend Zod schema alone
validates a field the backend never sends → always `undefined` → routing never fires → bug persists.

**Corrected Task 0 (backend + frontend):**
1. **Backend** — stamp `view_kind` + `renderer_kind` from `executor` descriptor onto the
   `ContextualView` in `facades/view.rs:297` after `build()`. Add `view_kind: ViewKind` field to
   `ContextualView` struct (dto.rs). ~1.5h.
2. **Frontend** — add `view_kind: viewKindSchema.optional()` to `contextualViewSchema`
   (schemas.ts:784). ~30 min.
3. **MSW** — update fixtures to include `view_kind`. ~30 min.

This changes Task 0 from "frontend-only" to "backend + frontend" and adds ~1.5h.

## Lens Routing

| Lens | Delegation | Status | Proposal Impact |
|------|------------|--------|-----------------|
| base-discipline | kernel | applied | Context/domain verified against code; contradiction surfaced |
| entropy-sdd | skill | deepened | GraphViewRenderer ↔ SvgGraph coupling (connascence of name); routing early-return is 1 new branch in PaneInspector — low entropy |
| cognicode-sdd | skill | verified | Confirmed schema gap is backend, not frontend; MSW handlers have no `view_kind` (grep: 0 matches) |

## Scope

### In Scope
- **Epic 1 — GraphViewRenderer** (bug fix): `GraphViewRenderer.tsx` + routing early-return in `PaneInspector` + empty state + Vitest + Playwright golden image `call-graph-ready`
- **Epic 2 — Exploration Snapshot**: `ViewportState` on `Pane`; viewport capture in `SvgGraph`; `PaneSnapshot` + `panes` field on backend `ExplorationSession`; localStorage cache + manual save
- **Task 0 (corrected)**: backend stamp `view_kind`/`renderer_kind` on `ContextualView` + frontend schema + MSW fixtures
- Edge label highlight-only fix (Decision 6)

### Out Of Scope
- Real backend layout endpoint (still `layoutFromContextualView` mock)
- Other graph ViewKinds wired (dependency_graph, seam_map — generic renderer supports them, only call_graph routed)
- Multi-user sharing; conflict resolution; remote renderer plugins

## Invariants

- `MAX_PANES = 8` cap enforced — paneStack.ts
- `SELECT_OBJECT` deduplicates by `objectId` — paneStack.ts:116-129
- `ExplorationSession.panes` has NO `#[serde(default)]` — old sessions fail 422 (Decision 13)

## Domain Language

- Resolved Terms: GraphViewRenderer, ViewKind routing, Pane Stack, Exploration Snapshot, ViewportState, PaneSnapshot
- Unresolved Ambiguities: None remaining (schema gap resolved — corrected Option A)

## Capabilities

### New Capabilities
- `graph-view-routing`: `ContextualView.view_kind` drives pane rendering (graph vs blocks)
- `exploration-snapshot`: persist + restore pane stack with per-pane viewport (scroll/zoom/pan)

### Modified Capabilities
- `contextual-view-dto`: now carries `view_kind` stamped from `ViewDescriptor`
- `exploration-session`: gains `panes: Vec<PaneSnapshot>` (breaking, no default)

## Approach

1. **Task 0 first** — backend stamp + frontend schema + MSW (unblocks routing)
2. **TDD** — failing Vitest for routing → implement GraphViewRenderer → green
3. **Visual regression** — Playwright golden image validates SVG renders
4. **Persistence** — viewport capture → backend DTO → save/load round-trip

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `facades/view.rs` | **Modified** | Stamp `view_kind`/`renderer_kind` from descriptor after `build()` |
| `dto.rs` (ContextualView) | **Modified** | Add `view_kind: ViewKind` field |
| `PaneInspector.tsx` | **Modified** | Early-return routing to GraphViewRenderer |
| `GraphView/` (new) | **Created** | Renderer + empty state + tests |
| `schemas.ts` | **Modified** | Add `view_kind` to contextualViewSchema |
| `SvgGraph.tsx` | **Modified** | Expose `onViewportChange` |
| `navigation/types.ts` | **Modified** | Add `ViewportState`, `pane.viewport` |
| `dto.rs` (ExplorationSession) | **Modified** | Add `panes: Vec<PaneSnapshot>` (breaking) |
| `facades/persistence.rs` | **Modified** | Store panes |

## Entropy Budget

| Metric | Estimate | Status |
|--------|----------|--------|
| Existing change entropy | low | OK — 1 new branch + 1 new component, well-isolated |
| New connascence | 3 (name: GraphViewRenderer↔SvgGraph, PaneInspector↔GraphViewRenderer, ContextualView↔view_kind) | OK |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Early-return changes pane markup → existing pane-stack tests fail | Medium | Run `pane-stack.spec.ts` after Task 1.2; regenerate snapshots |
| `useMemo([object_id, blocks])` stale layout on SWR refetch | Medium | Derive `layoutKey` from canonicalized blocks; Vitest for stability |
| Breaking `ExplorationSession.panes` (no default) → data loss | High (intentional) | CHANGELOG entry; user-facing warning; Decision 13 |
| Backend stamp forgotten in non-executor paths (MCP, ViewSpec) | Medium | Audit all `ContextualView` construction sites; integration test |

## Rollback Plan

Revert Task 0 + Task 1.2 (routing early-return). `ContextualView` without `view_kind` falls back
to `<Blocks>` rendering — restores pre-change behavior. Backend `panes` field removal requires
DB column drop if migrated (check migration status before deploy).

## Success Criteria

- [ ] `call_graph` view renders SVG with nodes (no longer blank)
- [ ] `ContextualView` JSON payload includes `view_kind` (verified via network inspect)
- [ ] Click on graph node opens new pane in stack (dedup by objectId)
- [ ] Empty state shown for objects without callers/callees
- [ ] Save snapshot persists all panes with viewports; load restores exactly
- [ ] localStorage cache provides instant restore on page load
- [ ] Vitest + Playwright tests pass; golden image `call-graph-ready` validates fix

## Effort Estimate

~13-15.5h (roadmap 11-14h + ~1.5h backend schema stamp correction)

## Related Documentation

- ADR: `docs/adr/ADR-040-graph-view-renderer.md` (+ addendum for schema gap correction)
- Roadmap: `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md`
- UX Workflow: `docs/wireframes/MOLDABLE-VIEW-UX-WORKFLOW.md`
- Explore Report: `openspec/changes/moldable-view-call-graph/explore-report.md`
