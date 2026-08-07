# Kernel Tasks: Fix Pre-Existing TypeScript Build Failure

**Change**: `sddk/fix-pre-existing-ts-build-failure`
**Strategy**: Single PR, 13 atomic commits (one per cluster), Cluster A first.

## Router Context Used
- **Knowledge Coverage**: sufficient — explore #2674 (59 errors, 12 clusters), proposal #2676 (4 schema values Rust emits vs TS rejects), spec #2677 (10 REQs, 33 scenarios), design #2678 (added Cluster M, verified PaneInspector hidden functional bug)
- **Context Quality**: C2 (durable knowledge + direct code read of api.rs:42-106, facades/graph.rs:271-402, schemas.ts:962-995, PaneInspector.tsx:247-251, rendererRegistry.tsx:127-141)
- **Taxonomy**: schema-sync (URGENT runtime bug), type-drift, dead-code, mechanical-strict-mode
- **Invariants Driving Tasks**: Rust style_class output set ⊆ TS schema enum (cross-layer invariant, will be promoted to ADR post-merge); `npm run build` exits 0; no NEW test failures
- **Recommended Effort**: verify (DONE — code confirmed by direct read in proposal/design phases)

## Review Budget Forecast
- **Estimated changed lines**: ~70-90 LOC across 13 commits (additive + mechanical)
- **400-line budget risk**: **Low** — no commit exceeds ~40 LOC (Cluster K is largest, ~30-40 LOC for 28 errors)
- **Chained PRs recommended**: **No** — single PR with atomic commits is the right granularity; all errors are independent and share no review dependencies
- **Decision needed before apply**: **No** — Rust backend already emits all 4 divergent values (verified in design #2678); schema widening is OCP-safe additive

## Knowledge Traceability
- **Work item source artifacts**: engram #2674 (explore), #2676 (proposal), #2677 (spec), #2678 (design)
- **Ownership source**: `apps/explorer-ui/` is the sole affected crate; ownership confirmed by explore
- **Open knowledge gaps affecting execution**: None blocking. Rust backend emission set verified at api.rs:42-106 and facades/graph.rs:271-402.

## Pre-existing State to Tolerate
- 5 pre-existing unit test failures (unrelated, do NOT regress)
- 38 pre-existing lint errors (unrelated, do NOT regress)
- Cluster M (`handlers.ts:93`) was missing from explore's affected-files list — design #2678 added it. Without Cluster M, the "all 59 errors resolved" gate fails by 1.

## Cluster Ordering Rationale
- **Commit 1 (A) FIRST**: URGENT runtime bug — Rust emits `node-code` + 3 C4 edge variants that TS Zod rejects. Fixes 7 errors including 3 in `NeighborMinigraph`, `GraphLanding`, `InteractiveGraph`. Unblocks downstream test runs.
- **Commits 2-5 (B, C, D, E)**: Mechanical cleanup, independent, no risk.
- **Commit 6 (F)**: HIDDEN FUNCTIONAL BUG — flagged prominently. `rendererRegistry.render(id, body)` is a 2-arg wrapper that drops the 3rd arg silently, so GraphView runs with `objectId=""` and no `onClose` today. Fix MUST use `getOrJson(kind).render(display, ctx)` to restore runtimeContext propagation.
- **Commits 7-10 (G, H, I, J)**: Local type fixes, low risk.
- **Commit 11 (K)**: Largest mechanical batch (28 errors) — late because it touches many files and is the largest diff.
- **Commits 12-13 (L, M)**: Final cleanup; M required to reach 0 errors.

## Cluster → Task Map

| Commit | Cluster | Errors | Tasks | Risk |
|--------|---------|--------|-------|------|
| 1 | A — Schema Sync | 7 | T1.1, T1.2 | Medium |
| 2 | B — Navigation Export | 4 | T2.1, T2.2, T2.3 | Low |
| 3 | C — Set Type | 1 | T3.1 | Low |
| 4 | D — null vs undefined | 3 | T4.1, T4.2, T4.3 | Low |
| 5 | E — Import Path | 2 | T5.1, T5.2 | Low |
| 6 | F — PaneInspector (HIDDEN BUG) | 3 | T6.1, T6.2, T6.3 | Medium-High |
| 7 | G — ViewBlock narrowing | 1 | T7.1 | Medium |
| 8 | H — GraphView dispatch | 1 | T8.1 | Low |
| 9 | I — Test mocks | 3 | T9.1 | Low |
| 10 | J — CytoscapeOptions renderer | 2 | T10.1 | Low |
| 11 | K — Bench strict mode | ~27-28 | T11.1 | Low-Medium |
| 12 | L — Test unused vars | 2 | T12.1, T12.2 | Low |
| 13 | M — handlers.ts last_scan_at | 1 | T13.1 | Low |
| **Total** | **13 clusters** | **~59 errors** | **22 tasks** | — |

---

## Tasks

### T1.1: Add 4 new style class values to Zod schemas + multimodal Record
- **Files**:
  - `apps/explorer-ui/src/api/schemas.ts` (L962-995) — add `"node-code"` to `graphNodeStyleClassSchema` enum, add `"edge-part-of"`, `"edge-deployed-as"`, `"edge-in-system"` to `graphEdgeStyleClassSchema` enum
  - `apps/explorer-ui/src/multimodal/multimodal.ts` (L51) — widen the Exclude or Record type to cover all `GraphNodeStyleClass` values that are NOT multimodal inspector kinds (`entry-point`, `hot`, `god` need entries or the type must exclude them)
- **LOC delta**: +5 lines (4 enum values + 1 type adjustment)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep -E "(node-code|edge-part-of|edge-deployed-as|edge-in-system)" || echo "OK: no errors mentioning new schema values"
  ```
  Expected: prints "OK: no errors mentioning new schema values"
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep -c "multimodal.ts" || echo "0"
  ```
  Expected: "0"
- **Commit message**: `fix(schema): sync Zod schema with Rust backend (node-code + 3 C4 edges)`
- **Risk**: Medium — confirmed in design #2678 that Rust api.rs:65,101,102,103 + facades/graph.rs:271,313,362,395,402 all emit these values; the schema currently rejects valid payloads (runtime bug). Fix is OCP-safe additive enum widening (no existing value modified).
- **Rollback**: `git revert <sha>` — removes 4 enum values; TS will re-reject backend payloads but no data loss (backend is the source of truth and will keep emitting).

### T1.2: Add schemas.test.ts assertions for all 4 new values
- **Files**: `apps/explorer-ui/src/api/schemas.test.ts`
- **LOC delta**: +8-12 lines (4 `safeParse` assertions with expected success)
- **Depends on**: T1.1 (the values must be added before tests can assert them)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npm run test -- --run -- schemas.test
  ```
  Expected: all tests pass, including new assertions for `node-code`, `edge-part-of`, `edge-deployed-as`, `edge-in-system`
  ```bash
  cd apps/explorer-ui
  npm run test -- --run -- schemas.test 2>&1 | grep -E "(node-code|edge-part-of|edge-deployed-as|edge-in-system)" | grep -i "pass\|✓"
  ```
  Expected: at least 4 matching lines, one per new value
- **Commit message**: `test(schema): assert all 4 new style class values parse successfully`
- **Risk**: Low — pure test additions; if T1.1 is correct, tests pass; if T1.1 missed a value, test fails loudly
- **Rollback**: `git revert <sha>` — removes test assertions; no production behavior change

---

### T2.1: Re-export ViewportState from navigation/index.ts barrel
- **Files**: `apps/explorer-ui/src/state/navigation/index.ts` (L12)
- **LOC delta**: +1 line (add `ViewportState` to the `export type {...}` list)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "context.ts.*TS2305" || echo "OK: no TS2305 in context.ts"
  ```
  Expected: prints "OK: no TS2305 in context.ts"
- **Commit message**: `fix(navigation): re-export ViewportState from barrel`
- **Risk**: Low — additive re-export, no consumer signature changes
- **Rollback**: `git revert <sha>` — removes the re-export line; `context.ts` will fail to resolve again but no other code is affected

### T2.2: Remove unused ViewportState import from slices/navigation.ts
- **Files**: `apps/explorer-ui/src/state/slices/navigation.ts` (L11)
- **LOC delta**: -1 line (remove `ViewportState` from import list)
- **Depends on**: T2.1 (the import was importing from a missing re-export; after T2.1 the import resolves but remains unused)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "slices/navigation.ts.*TS6196" || echo "OK: no TS6196 in slices/navigation.ts"
  ```
  Expected: prints "OK: no TS6196 in slices/navigation.ts"
- **Commit message**: `fix(navigation): remove unused ViewportState import from slice`
- **Risk**: Low — local import removal; verified `ViewportState` is consumed via `context.ts` not directly here
- **Rollback**: `git revert <sha>` — restores the import; TS6196 error returns but no functional impact

### T2.3: Remove unused imports from slices/index.ts
- **Files**: `apps/explorer-ui/src/state/slices/index.ts` (L11-12)
- **LOC delta**: -2 lines (remove `makeInitialNavigationState` and `makeInitialNavigationSliceState` from imports)
- **Depends on**: none (independent of T2.1, T2.2)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "slices/index.ts.*TS6133" || echo "OK: no TS6133 in slices/index.ts"
  ```
  Expected: prints "OK: no TS6133 in slices/index.ts"
  ```bash
  cd apps/explorer-ui
  grep -E "makeInitialNavigationState|makeInitialNavigationSliceState" apps/explorer-ui/src/state/rootReducer.ts || echo "OK: rootReducer does not use these symbols"
  ```
  Expected: prints "OK: rootReducer does not use these symbols" (confirms no other consumer)
- **Commit message**: `fix(state): remove unused initial-state imports from slices barrel`
- **Risk**: Low — local import removal; `rootReducer.ts` calls `navigationReducer` directly (verified in design context)
- **Rollback**: `git revert <sha>` — restores imports; TS6133 errors return but no functional impact

---

### T3.1: Fix viewKind.ts GRAPH_KINDS Set indexing pattern
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/viewKind.ts` (L31,50)
- **LOC delta**: +3 lines (extract `const GRAPH_KIND_ARRAY = [...] as const`, derive `GraphViewKind` from it, then construct `Set<string>` from the array)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "viewKind.ts.*TS2537" || echo "OK: no TS2537 in viewKind.ts"
  ```
  Expected: prints "OK: no TS2537 in viewKind.ts"
  ```bash
  cd apps/explorer-ui
  node -e "
    const { GRAPH_KINDS } = await import('./apps/explorer-ui/src/components/ObjectInspector/viewKind.ts');
  " 2>&1 | head -1 || echo "OK: module loads"
  # Or if .ts import via ts-node not available, skip and rely on tsc check above
  ```
  Expected: tsc check passes; if runtime check available, `.has("call_graph")` returns true
- **Commit message**: `fix(viewKind): derive GraphViewKind from array literal, not Set`
- **Risk**: Low — pure type-level refactor; `GRAPH_KINDS.has(...)` callers continue to work because Set is constructed from the same array
- **Rollback**: `git revert <sha>` — restores broken Set[number] pattern; TS2537 returns

---

### T4.1: Align useSWR generic in useViews.ts
- **Files**: `apps/explorer-ui/src/hooks/useViews.ts` (L78)
- **LOC delta**: 0 lines (type-only fix; may need a `| undefined` or generic narrowing)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "useViews.ts.*TS2345" || echo "OK: no TS2345 in useViews.ts"
  ```
  Expected: prints "OK: no TS2345 in useViews.ts"
- **Commit message**: `fix(useViews): align SWR generic with mergeAvailableViews signature`
- **Risk**: Low — SWR's generic only affects type inference; runtime behavior unchanged
- **Rollback**: `git revert <sha>` — restores generic mismatch; TS2345 returns

### T4.2: Widen ViewDescriptorPlus.source to match schema
- **Files**: `apps/explorer-ui/src/hooks/useViews.ts` (L134, plus the `ViewDescriptorPlus` type definition)
- **LOC delta**: 0-1 lines (widen type literal from `"runtime" | null` to `string | null`)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "useViews.ts.*TS2322" || echo "OK: no TS2322 in useViews.ts"
  ```
  Expected: prints "OK: no TS2322 in useViews.ts"
- **Commit message**: `fix(useViews): widen ViewDescriptorPlus.source to schema type`
- **Risk**: Low — schema is source of truth (`z.string().nullable()`); widening is OCP-safe
- **Rollback**: `git revert <sha>` — restores narrow literal type; TS2322 returns

### T4.3: Null-coalesce viewId at PaneInspector dispatch sites
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (L130, L136)
- **LOC delta**: 0 lines (replace `viewId` with `viewId ?? undefined` at both dispatch sites)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "PaneInspector.tsx.*TS2322" || echo "OK: no TS2322 in PaneInspector.tsx"
  ```
  Expected: prints "OK: no TS2322 in PaneInspector.tsx"
- **Commit message**: `fix(PaneInspector): coalesce viewId null to undefined at dispatch sites`
- **Risk**: Low — `?? undefined` is semantically equivalent for the Redux action payload (which expects `viewId?: string`); runtime behavior unchanged
- **Rollback**: `git revert <sha>` — restores `viewId` direct pass; TS2322 returns

---

### T5.1: Correct import path depth in ViewBlocks/types.ts
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts` (L8)
- **LOC delta**: 0 lines (change `../../api/types` to `../../../api/types`)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "ViewBlocks/types.ts.*TS2307" || echo "OK: no TS2307 in ViewBlocks/types.ts"
  ```
  Expected: prints "OK: no TS2307 in ViewBlocks/types.ts"
- **Commit message**: `fix(ViewBlocks): correct relative import path depth`
- **Risk**: Low — single path correction; the target module (`api/types`) exists at the corrected depth
- **Rollback**: `git revert <sha>` — restores broken path; TS2307 returns

### T5.2: Remove unused UnknownViewBlock import from ViewBlocks/types.ts
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts` (L7)
- **LOC delta**: -1 line (remove `UnknownViewBlock` from import list)
- **Depends on**: T5.1 (verify the import is genuinely unused after path correction; if it becomes used, keep it)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "ViewBlocks/types.ts.*TS6196" || echo "OK: no TS6196 in ViewBlocks/types.ts"
  ```
  Expected: prints "OK: no TS6196 in ViewBlocks/types.ts"
  ```bash
  cd apps/explorer-ui
  grep -n "UnknownViewBlock" apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts || echo "OK: symbol no longer referenced in this file"
  ```
  Expected: prints "OK: symbol no longer referenced in this file"
- **Commit message**: `fix(ViewBlocks): remove unused UnknownViewBlock import`
- **Risk**: Low — symbol truly unused in this file (only its type is used in the union elsewhere)
- **Rollback**: `git revert <sha>` — restores import; TS6196 returns

---

### ⚠️ T6.1: Fix PaneInspector render dispatch (HIDDEN FUNCTIONAL BUG)
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (L247-251)
- **LOC delta**: +1-2 lines (replace 3-arg `render(kind, body, ctx)` call with `getOrJson(kind).render(display, runtimeContext)`)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "PaneInspector.tsx.*TS2554" || echo "OK: no TS2554 in PaneInspector.tsx"
  ```
  Expected: prints "OK: no TS2554 in PaneInspector.tsx"
  ```bash
  cd apps/explorer-ui
  grep -n "rendererRegistry.render" apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx || echo "OK: no more 3-arg render() calls"
  ```
  Expected: prints "OK: no more 3-arg render() calls" — confirms the convenience wrapper is no longer used at this site
- **Commit message**: `fix(PaneInspector): use getOrJson().render() to preserve runtimeContext (functional bug)`
- **Risk**: **Medium-High** — this is NOT just a type fix. The current code `rendererRegistry.render(strategy.rendererKind, display.body ?? display, runtimeContext)` calls a 2-arg convenience wrapper (rendererRegistry.tsx:141) that **silently drops the 3rd argument** (`runtimeContext`). Result: GraphView receives `objectId=""` and no `onClose` callback in production TODAY. The type error (TS2554) was masking this bug. Fix MUST call `getOrJson(kind).render(display, runtimeContext)` to pass `runtimeContext` properly. **Reviewer must verify the call passes `display` (full ContextualView) and `runtimeContext` (not `display.body`).**
- **Rollback**: `git revert <sha>` — restores 3-arg call; TS2554 returns AND the runtime bug reappears (GraphView loses `objectId` and `onClose`). DO NOT roll back without coordination.

### T6.2: Remove `display.body` access (dead code)
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (L249)
- **LOC delta**: 0 lines (already covered by T6.1 — `display.body ?? display` is replaced with `display`)
- **Depends on**: T6.1 (combined into the same edit)
- **Verification**: Same as T6.1 — `display.body` is no longer referenced at this site
- **Commit message**: `fix(PaneInspector): remove dead display.body access`
- **Risk**: Low — combined with T6.1; `ContextualView` schema has no `body` field, so this access was dead code
- **Rollback**: Same as T6.1

### T6.3: Remove unused useApp import from PaneInspector.tsx
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` (L9)
- **LOC delta**: -1 line (remove `useApp` from the import)
- **Depends on**: none (independent of T6.1, T6.2)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "PaneInspector.tsx.*TS6133" || echo "OK: no TS6133 in PaneInspector.tsx"
  ```
  Expected: prints "OK: no TS6133 in PaneInspector.tsx"
- **Commit message**: `fix(PaneInspector): remove unused useApp import`
- **Risk**: Low — `useAppDispatch` is still used; only `useApp` is unused
- **Rollback**: `git revert <sha>` — restores import; TS6133 returns

---

### T7.1: Narrow ViewBlock type via typed() helper
- **Files**: `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` (L86)
- **LOC delta**: +1-2 lines (cast `block` via `typed()` helper or refactor component prop type)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "ViewBlock.tsx.*TS2769" || echo "OK: no TS2769 in ViewBlock.tsx"
  ```
  Expected: prints "OK: no TS2769 in ViewBlock.tsx"
- **Commit message**: `fix(ViewBlock): narrow block type via typed() helper`
- **Risk**: Medium — narrowing via cast or type guard must be sound; verify the runtime block shape matches the narrow target before committing
- **Rollback**: `git revert <sha>` — restores unsound union; TS2769 returns

---

### T8.1: Fix GraphView dispatch type
- **Files**: `apps/explorer-ui/src/components/GraphView/GraphView.tsx` (L52)
- **LOC delta**: 0-1 lines (change param type to `React.Dispatch<Action>` or use a properly aligned type)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "GraphView.tsx.*TS2345" || echo "OK: no TS2345 in GraphView.tsx"
  ```
  Expected: prints "OK: no TS2345 in GraphView.tsx"
- **Commit message**: `fix(GraphView): align dispatch parameter type`
- **Risk**: Low — type-only fix; `Dispatch<Action>` is the standard React pattern
- **Rollback**: `git revert <sha>` — restores mismatched type; TS2345 returns

---

### T9.1: Fix GraphViewRenderer test mock setup
- **Files**: `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.test.tsx` (L74, L93, L106)
- **LOC delta**: +5-10 lines (refactor mock to use `vi.mock(...)` or `vi.spyOn(...)` with proper typing)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "GraphViewRenderer.test.tsx.*TS2339" || echo "OK: no TS2339 in GraphViewRenderer.test.tsx"
  ```
  Expected: prints "OK: no TS2339 in GraphViewRenderer.test.tsx"
  ```bash
  cd apps/explorer-ui
  npm run test -- --run -- GraphViewRenderer.test
  ```
  Expected: test suite passes (no NEW failures vs. pre-change baseline)
- **Commit message**: `test(GraphViewRenderer): use vi.mock for layout function`
- **Risk**: Low — test infrastructure fix; does not alter production code
- **Rollback**: `git revert <sha>` — restores broken mock setup; TS2339 returns and tests may fail to run

---

### T10.1: Extend CytoscapeOptions or assert renderer property
- **Files**:
  - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` (L123)
  - `apps/explorer-ui/src/components/InteractiveGraph/cytoscape-shared.ts` (L178) — if file exists, otherwise just InteractiveGraph.tsx
- **LOC delta**: +1-2 lines (type assertion `as CytoscapeOptions` OR extend the type declaration with `renderer?: string`)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep -E "InteractiveGraph.*TS2353|cytoscape-shared.*TS2353" || echo "OK: no TS2353 in InteractiveGraph"
  ```
  Expected: prints "OK: no TS2353 in InteractiveGraph"
- **Commit message**: `fix(InteractiveGraph): allow renderer property in CytoscapeOptions`
- **Risk**: Low — preferred approach is type extension (safer than assertion); `renderer` is a real cytoscape.js option
- **Rollback**: `git revert <sha>` — restores type mismatch; TS2353 returns

---

### T11.1: Resolve bench strict mode violations (~27-28 errors)
- **Files**:
  - `apps/explorer-ui/src/bench/runner.test.ts` (~16 errors — indexed access guards)
  - `apps/explorer-ui/src/bench/renderers/types.ts` (3 errors — `erasableSyntaxOnly` parameter properties)
  - `apps/explorer-ui/src/bench/report.test.ts` (3 errors — indexed access guards)
  - `apps/explorer-ui/src/bench/runner.ts` (2 errors — unused param, string|undefined)
  - `apps/explorer-ui/src/bench/cytoscape-canvas.test.ts` (~1-2 errors — indexed access)
  - `apps/explorer-ui/src/bench/cytoscape-webgl.test.ts` (~1-2 errors — indexed access)
  - `apps/explorer-ui/src/bench/sigma-poc.test.ts` (~1-2 errors — indexed access)
- **LOC delta**: +30-40 lines (mechanical: `?.`, `!`, `?? ""`, param prefix `_`, explicit class fields for parameter properties)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "src/bench/" || echo "OK: no errors in src/bench/"
  ```
  Expected: prints "OK: no errors in src/bench/"
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep -c "src/bench/" || true
  ```
  Expected: "0"
  ```bash
  cd apps/explorer-ui
  npm run test -- --run -- bench
  ```
  Expected: all bench tests pass (no NEW failures vs. pre-change baseline; 5 pre-existing failures tolerated)
- **Commit message**: `fix(bench): resolve strict mode violations (noUncheckedIndexedAccess, erasableSyntaxOnly)`
- **Risk**: Low-Medium — mechanical type-level fixes; `erasableSyntaxOnly` param-property refactor is the only structural change but is a standard pattern (move `public readonly x` to explicit `readonly x = ...` field). Verify no behavioral change in bench code.
- **Rollback**: `git revert <sha>` — restores 27-28 errors; entire bench directory fails to type-check

---

### T12.1: Remove unused JobStatus import from ScanBar.tsx
- **Files**: `apps/explorer-ui/src/components/ScanBar.tsx` (L13)
- **LOC delta**: -1 line (remove `JobStatus` from import list)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "ScanBar.tsx.*TS6133" || echo "OK: no TS6133 in ScanBar.tsx"
  ```
  Expected: prints "OK: no TS6133 in ScanBar.tsx"
- **Commit message**: `fix(ScanBar): remove unused JobStatus import`
- **Risk**: Low — local import removal; `JobStatus` was unused
- **Rollback**: `git revert <sha>` — restores import; TS6133 returns

### T12.2: Remove unused variable from GraphLanding.test.tsx
- **Files**: `apps/explorer-ui/src/components/GraphLanding/GraphLanding.test.tsx` (L32)
- **LOC delta**: -1 line (remove unused `a` variable or use it)
- **Depends on**: none (independent of T12.1)
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "GraphLanding.test.tsx.*TS6133" || echo "OK: no TS6133 in GraphLanding.test.tsx"
  ```
  Expected: prints "OK: no TS6133 in GraphLanding.test.tsx"
- **Commit message**: `test(GraphLanding): remove unused variable`
- **Risk**: Low — local variable removal in test code
- **Rollback**: `git revert <sha>` — restores variable; TS6133 returns

---

### T13.1: Add `last_scan_at` to WorkspaceSummary schema + fixture
- **Files**:
  - `apps/explorer-ui/src/api/schemas.ts` (L115-122) — add `last_scan_at: z.string().nullable()` to `workspaceSummarySchema`
  - `apps/explorer-ui/src/mocks/fixtures.ts` (L33-40) — add `last_scan_at: "2026-06-07T10:11:12Z"` (or `null`) to `workspaceSummaryFixture`
- **LOC delta**: +2 lines (one per file)
- **Depends on**: none
- **Verification**:
  ```bash
  cd apps/explorer-ui
  npx tsc --noEmit 2>&1 | grep "handlers.ts.*TS2339" || echo "OK: no TS2339 in handlers.ts"
  ```
  Expected: prints "OK: no TS2339 in handlers.ts"
  ```bash
  cd apps/explorer-ui
  npm run test -- --run -- schemas.test
  ```
  Expected: `workspaceSummarySchema.parse(workspaceSummaryFixture)` still passes (test at schemas.test.ts:72)
- **Commit message**: `fix(workspace): add last_scan_at to WorkspaceSummary schema and fixture`
- **Risk**: Low — additive schema field; `handlers.ts:93` already references `last_scan_at` via `?? new Date().toISOString()`, so the field is expected. If the Rust backend also emits this field (verify at apply time), this aligns with reality.
- **Rollback**: `git revert <sha>` — removes `last_scan_at` from both schema and fixture; TS2339 returns in handlers.ts

---

## Verification

### Per-cluster verification
Each task has its own verification block above. The single most useful per-cluster check is:
```bash
cd apps/explorer-ui && npx tsc --noEmit 2>&1 | grep -c "error TS" || true
```

### Cumulative gate (run after ALL 13 commits)
```bash
cd /var/home/rubentxu/Proyectos/rust/CogniCode
cd apps/explorer-ui

# Primary gate: 0 TypeScript errors
npm run build
# Expected: exit 0, no TS errors

# Regression gate: no NEW test failures (5 pre-existing tolerated)
npm run test -- --run
# Expected: exit 0 OR only the 5 pre-existing failures documented in session #2673

# Lint gate: no NEW lint errors (38 pre-existing tolerated)
npm run lint
# Expected: exit 0 OR only the 38 pre-existing errors

# Schema regression gate (validates Cluster A)
npm run test -- --run -- schemas.test
# Expected: all assertions pass, including 4 new ones for node-code, edge-part-of, edge-deployed-as, edge-in-system
```

### Cluster M critical check (often forgotten)
```bash
cd apps/explorer-ui
# Confirm handlers.ts:93 compiles AND fixture matches schema
npx tsc --noEmit 2>&1 | grep "handlers.ts" || echo "OK"
grep "last_scan_at" src/mocks/fixtures.ts src/api/schemas.ts src/mocks/handlers.ts
# Expected: all 3 files mention last_scan_at
```

---

## Rollback Notes

### Per-commit rollback
Each commit is independently revertable via `git revert <sha>`. No commit depends on a later commit (all dependencies are on earlier commits in the sequence).

### Full PR rollback
```bash
git revert <merge-sha>..HEAD~13..HEAD  # if merged as a single PR
# OR
git reset --hard <sha-before-pr>  # if not yet merged (destructive, use carefully)
```

### Critical rollback caveat — Cluster F (T6.1)
Reverting T6.1 (Commit 6) restores BOTH the TS2554 type error AND the runtime bug where GraphView receives `objectId=""` and no `onClose`. **Do not roll back Commit 6 without coordination** — the runtime bug pre-dates this PR and is masked by the type error, but reverting reintroduces it.

### Cluster A rollback caveat
Reverting Commit 1 (T1.1) re-introduces the runtime bug where the TS frontend rejects valid C4 payloads from the Rust backend. **Do not roll back Commit 1 without coordination** — this is a production-affecting regression.

---

## Risks and Unknowns

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Schema widening misses a Rust-emitted value | Low | Full divergence table verified at api.rs:42-106 in design #2678 |
| PaneInspector dispatch regression (T6.1) | Medium | T6.1 fixes a HIDDEN functional bug; reviewer must verify the call passes `display` and `runtimeContext`, NOT `display.body` |
| Bench fixes brittle to upstream | Low | Mechanical type assertions only; no behavioral change |
| Cluster M forgotten in apply | Low | Design #2678 explicitly added it; verification gate includes `last_scan_at` check |
| Rust backend doesn't actually emit `last_scan_at` | Low | handlers.ts:93 already references it (existing behavior); if Rust doesn't emit it, the `??` fallback returns current time |

---

## Pre-existing Failures to Tolerate

These failures exist BEFORE this change and are out of scope. Do NOT regress them:

- **5 pre-existing unit test failures** — documented in session #2673; unrelated to TS build
- **38 pre-existing lint errors** — unrelated to TS build
- **Core::schemas::ViewDescriptor vs explorer::dto::ViewDescriptor divergence** — v2 follow-up, not in this scope
- **ContextualView.body schema extension** — Cluster F removes dead `display.body` access; does NOT add a `body` field

---

## Next Steps

- Apply in order: T1.1 → T1.2 → T2.1 → T2.2 → T2.3 → T3.1 → T4.1 → T4.2 → T4.3 → T5.1 → T5.2 → T6.1 → T6.2 → T6.3 → T7.1 → T8.1 → T9.1 → T10.1 → T11.1 → T12.1 → T12.2 → T13.1
- Recommended work-unit commits (per `work-unit-commits` skill): group T1.1+T1.2 (Schema+Test), T2.1+T2.2+T2.3 (Navigation), T6.1+T6.2+T6.3 (PaneInspector atomic), T12.1+T12.2 (Test cleanup) — but the 13-cluster structure already enforces this grouping
- Post-merge: consider ADR for "Rust style_class output set ⊆ TS schema enum" invariant (recommended by proposal #2676)
- Post-merge: confirm `cargo check` still green (should be — TS-only changes)