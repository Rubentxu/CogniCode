# Kernel Tasks: `sddk/E1-renderer-consolidation`

> Sprint E1 — Consolidate View Model.
> Two deliverables in one PR: E1.4 (block-id registry) + E1.5 (graph renderer canonicalization).

---

## Router Context Used

- **Knowledge Coverage**: sufficient. ADR-008 (PROPOSED §240), ADR-040, ADR-042; engram #2592, #2626, #2628, #2629, #2630, #2631; CONTEXT.md vocabulary; spec deltas already authored.
- **Context Quality**: **C2** — durable artifacts (roadmap, ADRs, spec deltas) present; code verification complete (ViewBlock.tsx, PaneInspector.tsx, rendererRegistry.tsx, GraphView.tsx, backend `views.rs`, `facades/view.rs`).
- **Taxonomy**: switch-dispatch entropy (29 cases in ViewBlock.tsx) + renderer-backend fork (GraphView vs InteractiveGraph stub) + signature extension (RuntimeContext typed union).
- **Domain Language**: resolved — ViewBlock, RendererKind, RendererRegistry, ContextualView, ViewKind, RuntimeContext. Unresolved: none blocking.
- **Invariants Driving Tasks**: data-testid contract per block component; `onSelectObject` propagation on 4 interactive blocks (callers, callees, hotspots, quality_issue_detail); H5 invariant `viewId: viewId ?? display.view_id`; GraphView viewport capture + onClose.
- **Recommended Effort**: **deepen** (already done — proposal/design complete).

---

## D3 Verification Result (Critical)

**Answer**: ✅ **YES** — the backend stamps `renderer_kind: "graph"` for graph ViewKinds.

**Evidence**:

| Location | Line | Code | Meaning |
|----------|------|------|---------|
| `crates/cognicode-explorer/src/facades/view.rs` | 300 | `view.renderer_kind = executor.renderer_kind();` | Single seam where backend stamps descriptor metadata onto DTO |
| `crates/cognicode-explorer/src/domain/views.rs` | 1369 | `fn renderer_kind(&self) -> RendererKind { RendererKind::Graph }` | `CallGraphExecutor` returns `Graph` |
| `crates/cognicode-explorer/src/domain/views.rs` | 1540 | `fn renderer_kind(&self) -> RendererKind { RendererKind::Graph }` | `DependenciesExecutor` returns `Graph` |

**Caveat**: Only **2 of 5 graph ViewKinds** have built-in backend executors:

| ViewKind | Backend Executor | RendererKind stamped |
|----------|------------------|----------------------|
| `call_graph` | `CallGraphExecutor` | `Graph` |
| `dependency_graph` | `DependenciesExecutor` | `Graph` |
| `data_flow` | (none — ViewSpec only) | author must set `renderer_kind: "graph"` |
| `impact_radius` | (none — ViewSpec only) | author must set `renderer_kind: "graph"` |
| `seam_map` | (none — ViewSpec only) | author must set `renderer_kind: "graph"` |

**Impact on E1.5 implementation strategy** (validated design):

`resolveRenderStrategy()` MUST check BOTH paths defensively:

```typescript
function resolveRenderStrategy(view: ContextualView): RenderStrategy {
  // Primary: explicit renderer_kind (works for all ViewSpecs and 2/5 built-ins)
  if (view.renderer_kind === "graph") return { kind: "registry", rendererKind: "graph" };
  // Fallback: built-in graph views where view_kind is the source of truth
  if (isGraphViewKind(view.view_kind)) return { kind: "registry", rendererKind: "graph" };
  // Everything else: block dispatch
  return { kind: "blocks" };
}
```

This is robust because:
- Built-in `call_graph` / `dependency_graph` views hit BOTH branches (renderer_kind first).
- ViewSpec `data_flow` / `impact_radius` / `seam_map` views hit the fallback if the author forgot `renderer_kind: "graph"` — defensive safety net.
- All non-graph views reach `{ kind: "blocks" }`.

No task in this plan is blocked by D3. The `resolveRenderStrategy` design in engram #2629 is correct as-is.

---

## Review Budget Forecast

- **Estimated changed lines**: ~350-400 LOC net change (refactor — removes more than it adds)
- **400-line budget risk**: **Low** (well under threshold for single PR)
- **Chained PRs recommended**: **No** — single PR is cleaner because E1.4 and E1.5 are interdependent (E1.5 short-circuit removal depends on E1.4 registry being live)
- **Decision needed before apply**: No — AUTO mode resolved all decision points

---

## Knowledge Traceability

- **Work item source artifacts**:
  - Proposal: engram #2628 (Option A adopted in AUTO mode)
  - Spec E1.4: engram #2630 (`openspec/changes/sddk-e1-renderer-consolidation/specs/block-renderer-registry/spec.md`)
  - Spec E1.5: engram #2631 (`openspec/changes/sddk-e1-renderer-consolidation/specs/graph-renderer-canonicalization/spec.md`)
  - Design: engram #2629
  - Exploration: engram #2626
- **Ownership source**: `apps/explorer-ui/` (single frontend owner), ADR-008 PROPOSED §240 (roadmap authority)
- **Open knowledge gaps affecting execution**: None. D1-D4 all resolved.

---

## Task Plan — Single PR with Atomic Commits

**Rationale for single PR over chained PRs**:
- E1.4 and E1.5 are interdependent (E1.5 short-circuit removal depends on the new dispatch path being live)
- ~350-400 LOC net change — well under the 400-line review threshold
- Tests are local (no API contract changes; all existing test contracts preserved)
- Atomic conventional commits within the PR keep each commit independently reviewable

**Commit sequence** (17 commits, ordered for safe bisect):

| Phase | Tasks | Purpose |
|-------|-------|---------|
| 1. Foundation | T1.1, T1.2, T1.3 | No behavior change — types, registry, tests scaffolding |
| 2. E1.4 block migration | T2.1 → T2.6 | Migrate 29 blocks; replace switch with lookup |
| 3. E1.5 graph canonicalization | T3.1 → T3.5 | Extend registry; route through `resolveRenderStrategy`; remove short-circuit |
| 4. Verification | T4.1 → T4.4 | Full test suite; lint; e2e |

---

## Tasks

### Phase 1 — Foundation (no behavior change)

#### T1.1: Extract `isGraphViewKind` and add `resolveRenderStrategy` to a new `viewKind.ts`

- **Files**:
  - NEW: `apps/explorer-ui/src/components/ObjectInspector/viewKind.ts`
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (re-export from new module)
- **LOC delta**: +35 LOC new, -8 LOC removed (net +27)
- **Depends on**: None
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/PaneInspector.test.tsx` — all pre-existing tests pass
  - Visual check: `grep -n "isGraphViewKind" apps/explorer-ui/src/` shows two definitions (old in PaneInspector, new in viewKind.ts) — old will be removed in T3.3
- **Commit message**: `refactor(inspector): extract isGraphViewKind to viewKind module`
- **Risk**: **Low** — pure extraction, no logic change. PaneInspector keeps the local copy for now; T3.3 deletes it.
- **Rollback**: `git revert <commit>` restores inline function. No state change, no test changes.

#### T1.2: Create `blockRendererRegistry.ts` with typed registry

- **Files**:
  - NEW: `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.ts`
- **LOC delta**: +90 LOC new
- **Depends on**: None
- **Types to define**:
  ```typescript
  export interface BlockRendererProps<Extra = unknown> {
    block: ViewBlock | UnknownViewBlock;
    objectId: string;
    onSelectObject?: (objectId: string) => void;
    extra?: Extra;
  }
  export interface BlockRendererEntry<Extra = unknown> {
    component: React.ComponentType<BlockRendererProps<Extra>>;
    displayName: string;
  }
  export function registerBlockRenderer<Extra = unknown>(
    id: ViewBlock["id"],
    entry: BlockRendererEntry<Extra>,
  ): void;
  export function getBlockRenderer(
    id: string,
  ): BlockRendererEntry | undefined;
  ```
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0
  - File exists, exports `registerBlockRenderer`, `getBlockRenderer`, `BlockRendererEntry`, `BlockRendererProps`
  - No consumers yet — registry is empty (entries added in T2.x)
- **Commit message**: `feat(inspector): add blockRendererRegistry types`
- **Risk**: **Low** — additive only, no existing code references it.
- **Rollback**: Delete file. Zero downstream impact at this commit.

#### T1.3: Add exhaustiveness test scaffolding

- **Files**:
  - NEW: `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.test.ts`
- **LOC delta**: +40 LOC new
- **Depends on**: T1.2
- **Test content**:
  - Test that `getBlockRenderer("unknown_id")` returns `undefined`
  - Test that `registerBlockRenderer` adds an entry and `getBlockRenderer` retrieves it
  - Test that `registerBlockRenderer` overwrites a previous entry (returns the old one)
  - Registration-time assertion (separate from runtime test): assert at module load that all 29 `KNOWN_IDS` are registered. Initially this WILL fail (0/29 registered) — the test must be marked `it.todo()` or use a separate file. Place the assertion in a comment so T2.6 uncomments it.
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/blockRendererRegistry.test.ts` — all tests pass
  - The 29-id assertion is documented as `// TODO(T2.6): uncomment when all 29 blocks registered` with the assertion code shown
- **Commit message**: `test(inspector): add blockRendererRegistry scaffolding tests`
- **Risk**: **Low** — pure test scaffolding; no production code change.
- **Rollback**: Delete test file. Zero impact.

---

### Phase 2 — E1.4 block migration

#### T2.1: Migrate `callers` and `callees` to registry (highest risk — `onSelectObject` consumers)

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` (add 2 `registerBlockRenderer` calls at module bottom; do NOT remove switch yet)
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.ts` (still empty at this commit — populated by T2.x)
- **LOC delta**: +12 LOC (2 register calls + comments)
- **Depends on**: T1.2
- **Pattern** (do NOT remove the switch yet — both paths coexist):
  ```typescript
  // At bottom of ViewBlock.tsx — registers for the registry, but switch still handles dispatch
  registerBlockRenderer("callers", {
    component: CallListView,
    displayName: "CallListView (callers)",
  });
  registerBlockRenderer("callees", {
    component: CallListView,
    displayName: "CallListView (callees)",
  });
  ```
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/ViewBlock.test.tsx` — all callers/callees tests still pass via the switch (no behavior change)
  - `getBlockRenderer("callers").component === CallListView` — verified via a console.log in dev or manual `node -e` import check
  - ViewBlock.test.tsx `onSelectObject` propagation tests for callers/callees still pass
- **Commit message**: `refactor(inspector): register callers and callees in blockRendererRegistry`
- **Risk**: **Medium** — both paths now exist; risk is drift if T2.5 (switch removal) is delayed. Mitigated by atomic follow-up commits.
- **Rollback**: `git revert <commit>`. Switch continues to handle dispatch; registry entries are no-ops.

#### T2.2: Migrate interactive blocks `hotspots` and `quality_issue_detail`

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` (add 2 register calls)
- **LOC delta**: +12 LOC
- **Depends on**: T2.1
- **Pattern**: identical to T2.1
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/ViewBlock.test.tsx` — all 4 interactive block tests pass via switch
  - Registry contains 4 entries: `callers`, `callees`, `hotspots`, `quality_issue_detail`
- **Commit message**: `refactor(inspector): register hotspots and quality_issue_detail in blockRendererRegistry`
- **Risk**: **Medium** — same as T2.1, plus this completes the interactive set.
- **Rollback**: `git revert <commit>`.

#### T2.3: Migrate identity blocks (8 of them)

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx`
- **LOC delta**: +48 LOC (8 register calls)
- **Depends on**: T2.2
- **Block IDs**: `identity`, `symbol_quality_identity`, `file_quality_identity`, `scope_quality_identity`, `issue_identity`, `file_identity`, `scope_identity`, `rule_identity`
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/ViewBlock.test.tsx` — identity block tests still pass via switch
  - Registry now has 12 entries
- **Commit message**: `refactor(inspector): register identity blocks in blockRendererRegistry`
- **Risk**: **Low** — identity blocks have no `onSelectObject`; pure render.
- **Rollback**: `git revert <commit>`.

#### T2.4: Migrate remaining 15 blocks

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx`
- **LOC delta**: +90 LOC (15 register calls)
- **Depends on**: T2.3
- **Block IDs**: `call_metrics`, `signature`, `source_slice`, `symbol_quality_issues`, `file_quality_issues`, `file_quality_gate`, `scope_quality_gate`, `scope_quality_issues`, `issue_location`, `issue_message`, `rule_related`, `kinds`, `symbols`, `scope_kinds`, `scope_files`, `cross_scope`, `quality_summary` (17 in this commit — final count 29)

  > Correction: 29 total = 4 (T2.1-T2.2) + 8 (T2.3) + 17 (T2.4) = 29 ✅
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/ViewBlock.test.tsx` — all tests pass via switch
  - Registry now has 29 entries (all `KNOWN_IDS` from types.ts)
- **Commit message**: `refactor(inspector): register remaining blocks in blockRendererRegistry`
- **Risk**: **Low** — bulk registration; no logic change.
- **Rollback**: `git revert <commit>`.

#### T2.5: Remove switch from `ViewBlock.tsx` — replace with registry lookup

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` (242 → ~30 LOC)
- **LOC delta**: -200 LOC (net for the file)
- **Depends on**: T2.4
- **Replacement**:
  ```typescript
  export function ViewBlock({ block, onSelectObject }: ViewBlockProps) {
    const id = (block as { id: string }).id;
    if (!isKnownBlockId(id)) {
      return <UnknownBlockView block={block as UnknownViewBlock} />;
    }
    const entry = getBlockRenderer(id);
    if (!entry) {
      return <UnknownBlockView block={block as UnknownViewBlock} />;
    }
    const Component = entry.component;
    return <Component block={block} objectId="" onSelectObject={onSelectObject} />;
  }
  ```
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/ViewBlock.test.tsx` — all tests pass via registry
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/PaneStackView.test.tsx` — all tests pass
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/blockRendererRegistry.test.ts` — exhaustiveness test passes (uncommented from T1.3)
  - File `ViewBlock.tsx` < 50 LOC
- **Commit message**: `refactor(inspector): replace ViewBlock switch with blockRendererRegistry lookup`
- **Risk**: **Medium** — large surface change in a single file. Mitigated by T2.1-T2.4 having pre-registered all entries, so the switch→lookup swap is behaviorally a no-op.
- **Rollback**: `git revert <commit>`. Switch is restored.

#### T2.6: Delete obsolete imports and clean up

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx`
- **LOC delta**: -10 LOC
- **Depends on**: T2.5
- **Actions**:
  - Remove imports of `typed`, individual block components (`IdentityView`, `CallListView`, etc.) — they are now only referenced inside `registerBlockRenderer` calls (move those to a separate `blockRegistrations.ts` for cleanliness)
  - NEW: `apps/explorer-ui/src/components/ObjectInspector/blockRegistrations.ts` — contains all 29 `registerBlockRenderer` calls (imported once by `blockRendererRegistry.ts` for side-effect)
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/` — all tests pass
  - `cd apps/explorer-ui && npm run lint` — no NEW errors (existing 39 tolerated)
- **Commit message**: `refactor(inspector): extract block registrations to dedicated module`
- **Risk**: **Low** — file moves only; behavior identical to T2.5.
- **Rollback**: `git revert <commit>`.

---

### Phase 3 — E1.5 graph renderer canonicalization

#### T3.1: Add `RuntimeContext` type and extend `RendererEntry.render` signature

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/rendererRegistry.tsx`
- **LOC delta**: +20 LOC
- **Depends on**: None (parallel to Phase 2; can be developed independently)
- **Changes**:
  ```typescript
  // New type — additive, backward-compatible
  export interface RuntimeContext {
    view?: ContextualView;
    objectId?: string;
    paneId?: string;
    viewId?: string;
    dispatch?: React.Dispatch<InspectorAction>;
    onClose?: () => void;
    onSelectObject?: (objectId: string, viewId?: string) => void;
  }

  // Extend signature — all existing entries continue to work
  export interface RendererEntry {
    label: string;
    render: (body: unknown, extra?: RuntimeContext | Record<string, unknown>) => ReactNode;
  }
  ```
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0 (signature is widened, not narrowed — all existing entries compile)
  - `cd apps/explorer-ui && npm run test -- --run src/components/rendererRegistry.test.tsx` — all tests pass
- **Commit message**: `feat(renderer): add RuntimeContext type to RendererEntry signature`
- **Risk**: **Low** — purely additive API change.
- **Rollback**: `git revert <commit>`. Signature reverts; no consumer affected yet.

#### T3.2: Replace `graph` registry entry with GraphView adapter

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/rendererRegistry.tsx`
- **LOC delta**: +30 LOC (adapter), -50 LOC (delete `GraphRenderer` stub), -3 LOC (delete `InteractiveGraph` lazy import — moved to bench)
- **Depends on**: T3.1
- **Replacement**:
  ```typescript
  // Old:
  // this.register("graph", { label: "Graph", render: (body) => <GraphRenderer body={body} /> });
  // New:
  this.register("graph", {
    label: "Graph (SvgGraph)",
    render: (body, extra) => {
      const ctx = (extra ?? {}) as RuntimeContext;
      const view = (body as ContextualView) ?? ctx.view;
      if (!view) return <JsonRenderer body={body} />;
      return (
        <GraphView
          view={view}
          objectId={ctx.objectId ?? ""}
          paneId={ctx.paneId}
          onClose={ctx.onClose}
        />
      );
    },
  });
  ```
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0
  - `cd apps/explorer-ui && npm run test -- --run src/components/rendererRegistry.test.tsx` — `registry.get("graph")` returns the new entry; existing tests still pass (registry path is still dead in production — PaneInspector short-circuits)
- **Commit message**: `feat(renderer): replace graph entry with GraphView adapter`
- **Risk**: **Medium** — replaces a stub with the real GraphView. The registry path is still not the production path (short-circuit in PaneInspector) so this commit is observationally a no-op. Risk manifests when T3.3 flips the dispatch.
- **Rollback**: `git revert <commit>`. InteractiveGraph stub is restored.

#### T3.3: Update PaneInspector to use `resolveRenderStrategy` and remove `isGraphViewKind` short-circuit

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (279 → ~255 LOC)
- **LOC delta**: +20 LOC (resolver call + RuntimeContext build), -24 LOC (delete local `isGraphViewKind` + ternary at L237-256)
- **Depends on**: T1.1, T3.2
- **Replacement at L237-256**:
  ```typescript
  {display ? (
    (() => {
      const strategy = resolveRenderStrategy(display);
      if (strategy.kind === "registry") {
        return rendererRegistry.render(strategy.rendererKind, display, {
          view: display,
          objectId,
          paneId: undefined, // GraphView falls back to activePaneId internally
          viewId: display.view_id,
          dispatch,
          onClose,
          onSelectObject: (objId, vId) =>
            dispatch({
              type: "SELECT_OBJECT",
              payload: { objectId: objId, viewId: vId ?? viewId ?? display.view_id },
            }),
        });
      }
      return (
        <Blocks
          view={display}
          onSelectObject={(objId) =>
            dispatch({
              type: "SELECT_OBJECT",
              payload: { objectId: objId, viewId: viewId ?? display.view_id },
            })
          }
        />
      );
    })()
  ) : (...)}
  ```
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/PaneStackView.test.tsx` — H5 invariant preserved; all tests pass
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/PaneInspector.test.tsx` (if exists) — all tests pass
  - `cd apps/explorer-ui && npm run test -- --run src/components/GraphView/` — GraphViewRenderer tests pass (now via registry path)
  - Manual: `pnpm dev` + open `/inspector` — click on a call-graph node, observe SELECT_OBJECT fires
- **Commit message**: `refactor(inspector): route PaneInspector through resolveRenderStrategy`
- **Risk**: **High** — this is the load-bearing commit. Mitigated by:
  - `resolveRenderStrategy` validated against D3 (backend stamps `renderer_kind: "graph"`)
  - All E2E tests in `call-graph-rendering.spec.ts` and `exploration.spec.ts` must pass
  - H5 invariant (`viewId: viewId ?? display.view_id`) explicitly preserved in the new dispatch
- **Rollback**: `git revert <commit>`. Short-circuit restored; GraphView used directly as before.

#### T3.4: Add regression test for `resolveRenderStrategy` and registry graph dispatch

- **Files**:
  - NEW: `apps/explorer-ui/src/components/ObjectInspector/viewKind.test.ts`
  - MODIFIED: `apps/explorer-ui/src/components/rendererRegistry.test.tsx`
- **LOC delta**: +80 LOC new test, +30 LOC extended test
- **Depends on**: T3.3
- **Test cases** (viewKind.test.ts):
  - `resolveRenderStrategy({ renderer_kind: "graph", view_kind: "call_graph", ... })` → `{ kind: "registry", rendererKind: "graph" }`
  - `resolveRenderStrategy({ renderer_kind: "json", view_kind: "call_graph", ... })` → `{ kind: "registry", rendererKind: "graph" }` (defensive fallback)
  - `resolveRenderStrategy({ renderer_kind: "json", view_kind: "vertical_slice", ... })` → `{ kind: "blocks" }`
  - `resolveRenderStrategy({ renderer_kind: "graph", view_kind: "data_flow", ... })` → `{ kind: "registry", rendererKind: "graph" }` (ViewSpec case)
  - `isGraphViewKind("data_flow")` → true; `isGraphViewKind("vertical_slice")` → false

  Test cases (rendererRegistry.test.tsx):
  - `rendererRegistry.get("graph").render(view, { objectId, viewId, onClose })` returns a `<GraphView>` instance (verify via `data-testid="graph-view-renderer"`)
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/ObjectInspector/viewKind.test.ts` — all pass
  - `cd apps/explorer-ui && npm run test -- --run src/components/rendererRegistry.test.tsx` — all pass including new graph entry test
- **Commit message**: `test(inspector): add resolveRenderStrategy and registry graph dispatch tests`
- **Risk**: **Low** — pure tests; no production code change.
- **Rollback**: Delete test files.

#### T3.5: Update `GraphViewRenderer.test.tsx` to use registry path

- **Files**:
  - MODIFIED: `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.test.tsx`
- **LOC delta**: +15 LOC
- **Depends on**: T3.3
- **Changes**: One existing test in `GraphViewRenderer.test.tsx` asserts GraphView mounts directly. Add a parallel test that asserts GraphView mounts via `rendererRegistry.render("graph", view, runtimeContext)`. Both must pass — proves the registry path is behaviorally equivalent.
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run src/components/GraphView/` — all tests pass (existing + new)
- **Commit message**: `test(graph): verify registry path matches direct GraphView mount`
- **Risk**: **Low** — additive test.
- **Rollback**: Revert test addition.

---

### Phase 4 — Verification

#### T4.1: Type-check + workspace compile

- **Files**: None (verification only)
- **Depends on**: All previous tasks
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` — exit 0
  - `cargo check --workspace` — exit 0 (2 pre-existing warnings tolerated)
  - No NEW warnings introduced by this change
- **Commit message**: `chore(ci): verify tsc and cargo check pass` (only if manual fixups needed; otherwise no commit)
- **Risk**: **Low** — read-only verification.
- **Rollback**: N/A (no commit if clean).

#### T4.2: Run frontend unit tests

- **Files**: None (verification only)
- **Depends on**: All previous tasks
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run` — 526 tests total, 521 must pass + 5 pre-existing failures tolerated (NOT newly broken)
  - Specifically verify:
    - `ViewBlock.test.tsx` (29 data-testid assertions + 4 onSelectObject propagation tests)
    - `rendererRegistry.test.tsx` (registry lookup + new graph entry)
    - `PaneStackView.test.tsx` (H5 invariant + close propagation)
    - `GraphViewRenderer.test.tsx` (viewport + onClose + new registry path test)
    - `RationaleView.test.tsx`, `InteractiveGraph.test.tsx`, `context.test.ts` — pre-existing failures tolerated
- **Commit message**: None (verification only).
- **Risk**: **Low**.
- **Rollback**: N/A.

#### T4.3: Run lint

- **Files**: None (verification only)
- **Depends on**: All previous tasks
- **Verification**:
  - `cd apps/explorer-ui && npm run lint` — 39 pre-existing errors tolerated, NO new errors introduced
  - Specifically verify no NEW `no-explicit-any`, `no-unused-vars`, or `set-state-in-effect` errors in the modified files
- **Commit message**: None unless fixups needed.
- **Risk**: **Low**.
- **Rollback**: N/A.

#### T4.4: Run Playwright E2E (relevant specs)

- **Files**: None (verification only)
- **Depends on**: All previous tasks
- **Verification**:
  - `cd apps/explorer-ui && npm run test:e2e -- call-graph-rendering.spec.ts` — must pass
  - `cd apps/explorer-ui && npm run test:e2e -- exploration.spec.ts` — must pass
  - `cd apps/explorer-ui && npm run test:e2e -- pane-stack.spec.ts` — must pass
  - `cd apps/explorer-ui && npm run test:e2e -- graph.spec.ts` — must pass
- **Commit message**: None (verification only).
- **Risk**: **Medium** — E2E exercises the production path; T3.3 changes the dispatch surface. If E2E fails, do NOT merge; either fix T3.3 implementation or expand the spec.
- **Rollback**: `git revert` the failing commit (likely T3.3).

---

## Summary

- **Total tasks**: 17 (T1.1-T1.3, T2.1-T2.6, T3.1-T3.5, T4.1-T4.4)
- **Estimated LOC delta**: +350-400 net (refactor — bulk is deletion of 200-line switch)
- **Estimated commits**: 17 atomic conventional commits (Phase 4 verification may not produce commits)
- **PR strategy**: single PR, atomic commits, fast-forward merge after CI passes
- **Unresolved blockers**: none — D3 verified, all decision points resolved in AUTO mode
- **Knowledge persistence**: engram #<id> with `topic_key: sddk/E1-renderer-consolidation/tasks`

---

## Post-Merge Follow-ups (NOT in scope of this change)

These are intentionally excluded per the proposal:

1. **ADR-043**: SvgGraph vs Cytoscape reconciliation. Write AFTER this lands.
2. **InteractiveGraph deprecation**: Keep as bench-only per E7 WebGL plans. ADR-043 will decide.
3. **ADR-008 status promotion**: PROPOSED → ACCEPTED (governance).
4. **E1.2** (`rendererRegistry["code"]` syntax highlighting) — separate sprint.
5. **Vega-Lite wiring** — Phase 4.