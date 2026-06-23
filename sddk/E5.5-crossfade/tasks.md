# Kernel Tasks: sddk/E5.5-crossfade

## Router Context Used
- Knowledge Coverage: sufficient — spec REQs (`docs/changes/sddk-E5.5-crossfade/spec.md`, engram #2708) + design snippets (engram #2709) + root-cause memory (#2707) + code-level verification of `InteractiveGraph.tsx:90-196,354-358` and `Shell.tsx:46-88,90-120` all in context
- Context Quality: **C3** (full durable knowledge; React-derived-state pattern documented in spec; test mocks at `InteractiveGraph.test.tsx:71-129` and `Shell.test.tsx:48-65` confirmed available)
- Taxonomy: dual-remount root cause (effect dep + cold-cache loading gate); CSS transition vs imperative morph tradeoff; SWR cache hit behavior under perspective toggle
- Invariants Driving Tasks:
  - `cytoscape.destroy()` MUST NOT fire on warm-cache perspective swap (T1.2 + T1.4a)
  - InteractiveGraphPanel MUST NOT render GRAPH_LOADING when stale data is available (T1.3 + T1.4c)
  - `prefers-reduced-motion: reduce` MUST disable opacity transition (T1.1)
  - React rules of hooks — both `useSubgraph` + `useArchitecture` remain unconditional (T1.3 does not change hook order)
  - ISP preserved — no new props on `InteractiveGraph` (state is derived from existing `data` prop)
- Recommended Effort: **verify** — design specifies single commit, ~40-60 LOC code + tests; pattern proven (React docs "adjusting state when props change")

## Review Budget Forecast
- Estimated changed lines: **~120** (≈15 CSS + ≈15 code InteractiveGraph + ≈10 code Shell + ≈70 tests + ≈10 docs)
- 400-line budget risk: **Low**
- Chained PRs recommended: **No** — design mandates single atomic commit; budget well under 400 lines
- Decision needed before apply: **No**

## Knowledge Traceability
- Work item source artifacts:
  - Proposal: engram #2707 (Option C vs B tradeoff, dual-remount root cause)
  - Spec: engram #2708 (6 REQs, 19 Given/When/Then scenarios)
  - Design: engram #2709 (Option C chosen — stale-data hold + opacity fade, single commit ~40-60 LOC, 0 new connascence pairs)
- Ownership source: `apps/explorer-ui` (frontend-owned); verified against `InteractiveGraph.tsx:90-196` (mount effect dep `[root, data, layoutAlgorithm]`), `InteractiveGraph.tsx:297-358` (canvas wrapper at lines 297-305 outer / 354-358 canvas div), `Shell.tsx:46-88` (InteractiveGraphPanel), `Shell.tsx:90-120` (GRAPH_LOADING + GRAPH_ERROR constants), `InteractiveGraph.test.tsx:114` (cytoscape mock `destroy() { /* no-op */ }` → must convert to spy)
- Open knowledge gaps affecting execution: **None**

## Pre-Apply Baseline (tolerated failures)
Confirmed at HEAD `a096d7f` (v0.11.5):
- `tsc --noEmit` → clean (exit 0)
- `vitest run` → 4 failed tests + 1 uncaught error (`cy.nodes is not a function` in `RationaleView.test.tsx` unmount race, unrelated to E5.5)
- `eslint src` → 30 errors + 3 warnings (spec cites "38 lint errors" — actual is 33 problems; all pre-existing)

These are tolerated. T1.4 verification asserts **delta only** — no new failures introduced by this change.

## Execution Strategy

**Single atomic commit** containing all 5 sub-tasks. Rollback = `git revert <sha>`. Sub-tasks are presented in dependency order for reviewer readability; the diff lands as one commit because:
- T1.1 must land before T1.2 (CSS module must exist for the import to resolve).
- T1.2 and T1.3 are independent (they fix different remount triggers — effect dep vs cold-cache loading gate — and either change is partial without the other).
- T1.4 is the regression net that proves T1.2 + T1.3 work.
- T1.5 is the post-implementation truth update; it documents that the crossfade mitigation has shipped.

**Commit message (single):**

```text
feat(inspector): add crossfade between graph/C4 perspectives (E5.5)

Roadmap E5.5 (crossfade) was ⚠️ Partial after Sprint E5: cytoscape
is destroyed/recreated on perspective toggle (`InteractiveGraph.tsx:196`
deps `[root, data, layoutAlgorithm]`) AND `InteractiveGraphPanel` shows
GRAPH_LOADING during warm-cache revalidation (`Shell.tsx:75`
`if (isLoading) return GRAPH_LOADING`). The two compounding remounts
produce the visible flash.

Mitigation per design #2709 (Option C — stale-data hold + opacity fade,
frontend-only, 0 new dependencies, 0 new connascence pairs):

1. Stale-data hold in `InteractiveGraphPanel` — keep last good
   SubgraphResponse across revalidation, suppress GRAPH_LOADING when
   stale data is available, clear the hold on hard error.

2. Opacity fade in `InteractiveGraph` — React-derived-state-from-props
   pattern watches `data` identity and toggles a `.igCanvasFading` class
   on the canvas wrapper for 200ms. CSS module owns the transition so
   `prefers-reduced-motion` can short-circuit it.

3. Cytoscape fiber persists across warm-cache data swap — the mount
   effect's `data` dep still triggers, but the stale-data hold means
   `InteractiveGraph` never unmounts during revalidation.

The `Shell.tsx` rationale branch and `GraphLanding.tsx:119` still
expose the same flash; those are out of scope per spec REQ-E5.5-3
(follow-up tracked in roadmap).

Ref: #2707 (root cause), #2708 (spec), #2709 (design).
```

---

## Tasks

### T1.1: Create `InteractiveGraph.module.css` (new file)
- **Files**: `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.module.css` (NEW)
- **LOC delta**: ~15
- **Depends on**: — (root of the chain)
- **Verification**:
  - `ls apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.module.css` → exits 0, prints the file path
  - `cat .../InteractiveGraph.module.css` → output contains three blocks in this order: `.igCanvas` (with `transition: opacity 200ms ease-out`), `.igCanvasFading` (with `opacity: 0.15`), and `@media (prefers-reduced-motion: reduce)` containing both `.igCanvas { transition: none }` and `.igCanvasFading { opacity: 1 }`
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit 0 (CSS modules do not affect TS, but this is the cheapest smoke test that nothing else broke at file-resolve time)
- **Commit message**: included in the single atomic commit above
- **Risk**: **low** — pure declarative CSS, no runtime behavior, follows `ContextualPanel.module.css` precedent (header comment + scoped class names), 0 new connascence pairs
- **Rollback**: `git rm apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.module.css`; T1.2 will fail to compile (import path missing) — fine because the whole commit is reverted together

### T1.2: Modify `InteractiveGraph.tsx` — opacity fade wrapper
- **Files**: `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`
- **LOC delta**: ~15 (add `prevDataRef`, `canvasFading` state, derived-state-from-props block, double-rAF effect, fade class on canvas div, CSS module import)
- **Depends on**: T1.1 (CSS module must exist for import to resolve)
- **Implementation anchor** (line 90-196 area + line 297-358 render block):
  - Line ~85: add `import styles from "./InteractiveGraph.module.css";`
  - Line ~95: add `const prevDataRef = useRef(data);` and `const [canvasFading, setCanvasFading] = useState(false);`
  - Line ~95 (after refs/state): React-idiomatic "adjusting state when props change":
    ```tsx
    if (prevDataRef.current !== data) {
      prevDataRef.current = data;
      setCanvasFading(true);
    }
    ```
  - New `useEffect` listening on `[canvasFading]` only: when true, schedule `setCanvasFading(false)` after double-rAF (`requestAnimationFrame(() => requestAnimationFrame(() => setCanvasFading(false)))`) so the browser paints the faded state before clearing it
  - Line ~354: change `<div ref={containerRef} data-testid="interactive-graph-canvas" style={...} />` to add `className={canvasFading ? styles.igCanvasFading : styles.igCanvas}`
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit 0
  - `cd apps/explorer-ui && npx vitest run src/components/InteractiveGraph/InteractiveGraph.test.tsx --reporter=basic 2>&1 | tail -3` → output ends with `Tests  X passed` where X matches the pre-T1.2 count of InteractiveGraph tests (12 today). T1.2 is additive — no existing assertion should regress.
- **Commit message**: included in the single atomic commit above
- **Risk**: **low** — additive derived state, no new props (ISP preserved), no API surface change, derived-state-from-props is the React-blessed pattern for "respond to prop change"; double-rAF is the established workaround for "browser must paint the intermediate state"
- **Rollback**: `git checkout -- apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`; T1.4a / T1.4b will fail (they assert behavior introduced here) — acceptable because the whole commit reverts together

### T1.3: Modify `Shell.tsx` — `InteractiveGraphPanel` stale-data hold
- **Files**: `apps/explorer-ui/src/components/Shell.tsx`
- **LOC delta**: ~10 (add `lastGoodDataRef`, `displayData` derivation, suppress GRAPH_LOADING, pass `displayData` to InteractiveGraph)
- **Depends on**: — (independent of T1.2; fixes a DIFFERENT remount trigger — the cold-cache loading gate)
- **Implementation anchor** (line 46-88):
  - Line 15: add `import { useRef } from "react";` to the existing React import (currently `{ Suspense, lazy }`)
  - Line ~53: add `const lastGoodDataRef = useRef<SubgraphResponse | null>(null);`
  - Line ~73 (after `const { data, isLoading, error } = ...`):
    ```tsx
    if (data && !error) {
      lastGoodDataRef.current = data;
    } else if (error) {
      lastGoodDataRef.current = null;
    }
    const displayData = data ?? lastGoodDataRef.current;
    const showLoading = isLoading && !displayData;
    ```
  - Line ~75: change `if (isLoading) return GRAPH_LOADING;` → `if (showLoading) return GRAPH_LOADING;`
  - Line ~78-87: pass `displayData` (not `data`) to `<InteractiveGraph data={...} />`
  - Note: do NOT change hook order. Both `useSubgraph` and `useArchitecture` remain unconditional. `useRef` is added at the top, before any conditional return — safe under rules of hooks.
- **Verification**:
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit 0
  - `cd apps/explorer-ui && npx vitest run src/components/Shell.test.tsx --reporter=basic 2>&1 | tail -3` → existing E5.3 perspective wire-up tests (lines 337-438 of `Shell.test.tsx`) still pass; pre-existing `Shell.test.tsx` count unchanged. New T1.4c / T1.4d land in T1.4.
- **Commit message**: included in the single atomic commit above
- **Risk**: **low** — derived state with ref guard, no API surface change, no new props threaded down, only modifies what happens INSIDE `InteractiveGraphPanel`. The cold-cache first paint (no stale data yet) still shows GRAPH_LOADING exactly as before — REQ-E5.5-1 scenario "cold-cache first paint" preserved.
- **Rollback**: `git checkout -- apps/explorer-ui/src/components/Shell.tsx`; T1.4c / T1.4d will fail (they assert behavior introduced here) — acceptable because the whole commit reverts together

### T1.4: Add 4 regression tests (2 InteractiveGraph + 2 Shell)
- **Files**:
  - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx` (modify: convert `destroy()` no-op at line 114 to a `vi.fn()`, append 2 new tests)
  - `apps/explorer-ui/src/components/Shell.test.tsx` (append a new `describe("crossfade (E5.5)...")` block with 2 new tests)
- **LOC delta**: ~70 (5-8 lines per test + spy plumbing + helper)
- **Depends on**: T1.2 + T1.3 (tests validate the implementation)
- **Sub-tasks**:

  **T1.4a** — `InteractiveGraph.test.tsx`: warm-cache data swap does NOT destroy cytoscape fiber
  - Convert mock at line 114 from `destroy() { /* no-op */ }` to `destroy = vi.fn();` and expose the spy on the `Cy` instance (e.g. `Cy.prototype.destroy = vi.fn()` or assign on the constructor)
  - Test: render with `data = fixtureA`, capture `cy.destroy.mock.calls.length`, rerender with `data = fixtureB`, assert `cy.destroy.mock.calls.length === 0` (or unchanged from initial). This is the PRIMARY regression assertion for E5.5 — if T1.2 is missing, the effect dep at line 196 triggers and `destroy` fires.
  - Note: vitest must NOT clear mocks between render and rerender — call `vi.clearAllMocks()` in `beforeEach`, NOT `afterEach`, or wrap the assertion in a way that survives cleanup. Inspect existing `beforeEach` / `afterEach` (lines 134-141) and align.

  **T1.4b** — `InteractiveGraph.test.tsx`: fade state toggles on data identity change
  - Render with `data = fixtureA`, capture the `data-testid="interactive-graph-canvas"` element's `className`, assert it includes `igCanvas` and NOT `igCanvasFading`.
  - Rerender with a NEW identity (`data = { ...fixtureA }` — different object reference, same content).
  - Assert className includes `igCanvasFading` immediately after rerender.
  - After double-rAF advances (use `vi.useFakeTimers()` + `vi.runAllTimers()`), assert className flips back to `igCanvas`.
  - Validates the derived-state-from-props block + double-rAF effect.

  **T1.4c** — `Shell.test.tsx`: stale data suppresses GRAPH_LOADING during revalidation
  - Setup: `useSubgraphSpy.mockReturnValue({ data: SUBGRAPH_FIXTURE, isLoading: true, error: null })` AND prime `lastGoodDataRef` via an initial render with `isLoading: false, data: SUBGRAPH_FIXTURE`.
  - Pattern follows `InteractiveGraphPanel perspective wire-up (E5.3)` block at lines 337-438. Use the `AppContext.Provider` + `stateWithSymbol` recipe.
  - Assert `screen.queryByTestId("interactive-graph-loading")` is `null`.
  - Assert `screen.getByTestId("interactive-graph")` IS present (InteractiveGraph rendered with stale data).

  **T1.4d** — `Shell.test.tsx`: hard error clears stale hold
  - Setup: prime `lastGoodDataRef` (initial render with valid data), then re-render with `useSubgraphSpy.mockReturnValue({ data: null, isLoading: false, error: new Error("boom") })`.
  - Assert `screen.getByTestId("interactive-graph-error")` IS present (not the loading state, not InteractiveGraph — the error UI takes precedence per the existing `if (error) return GRAPH_ERROR;` branch).
  - Validates the `else if (current.error) lastGoodDataRef.current = null` clearing path that was the latent bug called out in design #2709's "Learned" section.

- **Verification** (per-sub-task and aggregate):
  - `cd apps/explorer-ui && npx vitest run src/components/InteractiveGraph/InteractiveGraph.test.tsx --reporter=basic 2>&1 | tail -3` → `Tests  14 passed` (12 existing + T1.4a + T1.4b). Pre-existing baseline was 12 passes; we add 2.
  - `cd apps/explorer-ui && npx vitest run src/components/Shell.test.tsx --reporter=basic 2>&1 | tail -3` → existing E5.3 tests still pass; new T1.4c + T1.4d pass.
  - Aggregate: `cd apps/explorer-ui && npx vitest run src/components/InteractiveGraph/InteractiveGraph.test.tsx src/components/Shell.test.tsx --reporter=basic 2>&1 | tail -3` → exit 0, no NEW failures introduced. The 4 pre-existing failures + 1 uncaught error in OTHER test files (notably `RationaleView.test.tsx`) remain tolerated.
- **Commit message**: included in the single atomic commit above
- **Risk**: **low** — tests use existing `vi.fn()` / `vi.spyOn` infrastructure; mock spy pattern (`Cy.prototype.destroy = vi.fn()`) is the standard vitest approach; no new test framework introduced
- **Rollback**: `git checkout -- apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx apps/explorer-ui/src/components/Shell.test.tsx`; T1.4 is the regression net — without T1.4 there is no automated proof T1.2 + T1.3 work, but the production code change still ships. Acceptable because the whole commit reverts together.

### T1.5: Doc sync — roadmap + ADR-039
- **Files**:
  - `docs/explorer-roadmap.md` (line 127 — E5.5 row status flip)
  - `docs/adr/ADR-039-explorer-navigation-model.md` (line 132 — E5 row Evidence note)
- **LOC delta**: ~10 (2 single-line edits + a sentence in ADR)
- **Depends on**: T1.4 (docs reflect verified implementation)
- **Sub-tasks**:

  **T1.5a** — `docs/explorer-roadmap.md:127`
  - Change: `| E5.5 | Add smooth transition between perspectives (data swap + re-layout) | ⚠️ Partial | Data swap now works; crossfade still ❌ — cytoscape instance is destroyed and remounted on perspective change |`
  - To: `| E5.5 | Add smooth transition between perspectives (data swap + re-layout) | ✅ | Stale-data hold + opacity fade shipped (E5.5 crossfade, `Shell.tsx` + `InteractiveGraph.tsx`); cytoscape fiber persists across warm-cache swap. `GraphLanding.tsx` parity deferred — tracked as follow-up. |`

  **T1.5b** — `docs/adr/ADR-039-explorer-navigation-model.md:132`
  - The E5 row already reads `✅ Complete` — do NOT change the status cell. UPDATE the Evidence cell to add a note about the crossfade mitigation that was retrofitted.
  - Current: `| E5 — Perspective toggle | ✅ Complete | Toggle wired into \`InteractiveGraphPanel\` (\`Shell.tsx:45-88\`); canvas morphs between graph (useSubgraph) and C4 (useArchitecture) perspectives after object selection |`
  - New: append the crossfade note within the same cell: `| E5 — Perspective toggle | ✅ Complete | Toggle wired into \`InteractiveGraphPanel\` (\`Shell.tsx:46-88\`); canvas morphs between graph (\`useSubgraph\`) and C4 (\`useArchitecture\`) perspectives after object selection. **E5.5 crossfade:** stale-data hold + opacity fade ensures warm-cache perspective swap does not destroy the cytoscape fiber (\`InteractiveGraph.tsx\`). \`Shell.tsx\` rationale branch and \`GraphLanding.tsx\` still flash — follow-up. |`

- **Verification**:
  - `grep -n "E5.5" docs/explorer-roadmap.md` → line 127 shows `| E5.5 | ... | ✅ |` (status flipped from ⚠️ to ✅)
  - `grep -n "E5.5 crossfade\|crossfade" docs/adr/ADR-039-explorer-navigation-model.md` → matches the E5 row at line 132
  - `git diff --stat docs/explorer-roadmap.md docs/adr/ADR-039-explorer-navigation-model.md` → both files show 1 line changed each (or however many lines the replacement spans)
- **Commit message**: included in the single atomic commit above
- **Risk**: **low** — text-only edits; no semantic content shift beyond stating the new state
- **Rollback**: `git checkout -- docs/explorer-roadmap.md docs/adr/ADR-039-explorer-navigation-model.md`; documents return to the pre-shipment state, acceptable because the whole commit reverts together

---

## Aggregate Verification (after all 5 sub-tasks land)

Run from repo root in this exact order:

1. `cd apps/explorer-ui && npx tsc --noEmit` → **expect exit 0** (no output, clean)
2. `cd apps/explorer-ui && npx vitest run src/components/InteractiveGraph/InteractiveGraph.test.tsx src/components/Shell.test.tsx --reporter=basic 2>&1 | tail -5` → **expect**:
   ```
   Test Files  X passed (X)
   Tests  Y passed
   ```
   where Y = 12 (existing InteractiveGraph) + 7+ (existing Shell) + 4 (new T1.4a-d) — verify the count grew by exactly 4 vs pre-change baseline. NO new failures introduced. The pre-existing `RationaleView.test.tsx` unmount-race failure is tolerated (unrelated to E5.5).
3. `cd apps/explorer-ui && npx eslint src/components/InteractiveGraph src/components/Shell.tsx --max-warnings=9999 2>&1 | tail -3` → **expect** the lint error count for these two files to remain at 0 (both files were clean at baseline). New errors anywhere else are tolerated.
4. `grep -n "E5.5" docs/explorer-roadmap.md` → **expect line 127** to show `| E5.5 | ... | ✅ |`
5. `git diff --stat` → **expect** the diff to be confined to exactly 7 files:
   - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.module.css` (new)
   - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`
   - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx`
   - `apps/explorer-ui/src/components/Shell.tsx`
   - `apps/explorer-ui/src/components/Shell.test.tsx`
   - `docs/explorer-roadmap.md`
   - `docs/adr/ADR-039-explorer-navigation-model.md`

Total diff: ~120 lines (≈15 CSS + ≈15 code IG + ≈10 code Shell + ≈70 tests + ≈10 docs). Well under the 400-line review budget.

---

## Rollback Notes

Single-commit rollback: `git revert <sha>` after the change lands. Each sub-task's individual rollback is listed above; in practice the design's "single atomic commit" decision means all 5 sub-tasks revert together. The 0-new-connascence-pairs guarantee means no out-of-scope files need touching during revert.

If a partial rollback is ever needed (e.g. the CSS fade works but Shell stale-data hold regresses something), the sub-tasks are independently revertible because:
- T1.1 / T1.2 are coupled (CSS module → component import) but self-contained.
- T1.3 is fully independent of T1.2.
- T1.4 is the test net — reverting tests without reverting code leaves tests failing but code unchanged; reverting code without reverting tests leaves tests failing.
- T1.5 is independent of everything else.

**Recommended rollback ordering** if multi-step: revert T1.5 first (docs only — no functional impact), then T1.4, then T1.3 / T1.2 / T1.1 as a unit.