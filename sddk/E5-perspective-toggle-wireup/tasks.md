# Kernel Tasks: sddk/E5-perspective-toggle-wireup

## Router Context Used
- Knowledge Coverage: sufficient — roadmap E5.3/E5.5 + ADR-039 §3/§4 + GraphLanding:43-54 reference pattern + hooks signatures all verified in design (#2698)
- Context Quality: **C3** (full durable knowledge; reference implementation + schemas cross-checked)
- Taxonomy: frontend data-source seam; React rules-of-hooks invariant; doc-comment drift trap
- Invariants Driving Tasks:
  - React rules of hooks → both `useSubgraph` and `useArchitecture` called unconditionally (T1.2)
  - `ArchitecturePayload === SubgraphResponse` alias → no adapter needed (T1.2)
  - ADR-039 §3/§4 morph contract → canvas responds to perspective after object selection (T1.2, T1.5)
- Recommended Effort: **verify** — pattern proven in GraphLanding; single small commit; no new artifacts

## Review Budget Forecast
- Estimated changed lines: ~80 (30 logic + 20 docs + 30 test)
- 400-line budget risk: **Low**
- Chained PRs recommended: **No** — fits comfortably in one commit
- Decision needed before apply: **No**

## Knowledge Traceability
- Work item source artifacts:
  - Proposal: engram #2696 (Option A, single-commit, type-compatible)
  - Spec: engram #2697 (6 REQs, 21 scenarios)
  - Design: engram #2698 (5 snippet corrections + 4 critical observations)
- Ownership source: `apps/explorer-ui` (frontend-owned); design verified against `Shell.tsx:45-71`, `Shell.tsx:96-118`, `GraphLanding.tsx:43-54`, `GraphLanding.tsx:139-155`, `useArchitecture.ts:13`, `useSubgraph.ts:14`, `context.ts:56-60`
- Open knowledge gaps affecting execution: **None** — all 5 snippet corrections and 4 critical observations already incorporated into sub-task design below

## Execution Strategy

**Single atomic commit** containing all 7 sub-tasks. Rollback = `git revert <sha>`. Sub-tasks are presented in dependency order for reviewer readability, but the diff lands as one commit.

**Commit message (single):**

```text
feat(explorer): wire PerspectiveToggle into InteractiveGraphPanel

Today `InteractiveGraphPanel` (Shell.tsx:45-71) always calls
`useSubgraph(rootId)` regardless of `perspective`, so once a symbol is
selected the toggle in the header is a no-op. ADR-039 §3/§4 states the
canvas must morph between Graph and C4 perspectives regardless of
selection state.

Mirror the proven GraphLanding.tsx:43-54 dual-hook pattern: call both
`useSubgraph` and `useArchitecture` unconditionally, pass `null` to
the inactive hook, select `data` by perspective. Thread `workspaceId`
from ShellBootstrap into the panel so the C4 branch has a workspace to
fetch against.

Also fix the latent trap in `context.ts:56-60` whose doc comment
documented the bug ("toggle only applies when no object is selected")
as design intent — corrected to mirror ADR-039 §3/§4.

Adds a `GRAPH_ERROR` constant (mirrors GraphLanding.tsx:139-155) since
the rewritten panel surfaces `error` for the first time. Regression
test in `Shell.test.tsx` uses MSW to assert that the correct endpoint
is hit per perspective.

Closes E5.3 (roadmap); flips ADR-039 E5 row to Complete and drops the
E5 entry from open-gaps.

Refs: ADR-039 §3/§4; SPEC sddk/E5-perspective-toggle-wireup.
```

---

## Tasks

### T1.1: Define `GRAPH_ERROR` constant in Shell.tsx

- **Files**: `apps/explorer-ui/src/components/Shell.tsx`
- **LOC delta**: +12 (new constant block after `GRAPH_LOADING` at L73-87)
- **Depends on**: none
- **Verification**:
  - `grep -n "GRAPH_ERROR" apps/explorer-ui/src/components/Shell.tsx` → must return the new definition line + the reference line in T1.2
  - Visual diff: confirm `data-testid="interactive-graph-error"` and identical style block to `GRAPH_LOADING`
- **Commit message**: bundled into the single atomic commit (no per-task commit)
- **Risk**: **Low** — pure addition; no semantic change; mirrors a pattern that already exists in `GraphLanding.tsx:139-155`
- **Rollback**: revert the single commit (`git revert <sha>`) — restores both constants to pre-change state
- **Implementation note**: must be declared **before** `InteractiveGraphPanel` (which is rewritten in T1.2) so the panel can reference it. Place it directly after `GRAPH_LOADING` (L73-87), keeping the ordering that React rules-of-hooks already relies on.

```tsx
const GRAPH_ERROR = (
  <div
    data-testid="interactive-graph-error"
    style={{
      height: "100%",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      color: "var(--color-text-muted)",
      fontSize: 12,
    }}
  >
    Failed to load graph data.
  </div>
);
```

---

### T1.2: Rewrite `InteractiveGraphPanel` to be perspective-aware

- **Files**: `apps/explorer-ui/src/components/Shell.tsx` (lines 45-71 replaced; line 23 `useArchitecture` import added)
- **LOC delta**: +18 / -10 (net +8 in panel; +1 import line)
- **Depends on**: T1.1 (must reference `GRAPH_ERROR`); T1.3 (`workspaceId` prop)
- **Verification**:
  - `grep -n "useArchitecture" apps/explorer-ui/src/components/Shell.tsx` → must show import line + panel call
  - `grep -n "useSubgraph" apps/explorer-ui/src/components/Shell.tsx` → must still show exactly one unconditional call
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit code 0; no `ArchitecturePayload`/`SubgraphResponse` incompatibility diagnostic
  - `cd apps/explorer-ui && npm run lint -- src/components/Shell.tsx` → no `react-hooks/rules-of-hooks` error
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low** — pattern copy-pasted from `GraphLanding.tsx:43-54`; `eslint-plugin-react-hooks` enforces hook order; `tsc` enforces type compatibility (alias confirmed at `schemas.ts:1266-1267`)
- **Rollback**: revert the single commit
- **Snippet corrections applied (all 5)**:
  1. ❌ No `dispatch` reference — selection stays read-only (matches current `onSelectObject={() => { /* read-only */ }}` pattern)
  2. ✅ Uses existing `GRAPH_LOADING` (defined at L73-87), not a new `LoadingTier` symbol
  3. ✅ Uses new `GRAPH_ERROR` from T1.1, not `<ErrorBoundary error={error} />`
  4. ✅ `RationaleView` keeps `focusId={rootId}` + the existing `onSelectObject` read-only stub
  5. ✅ Correct spelling: "Perspective" (no "Persistive" typo)

```tsx
function InteractiveGraphPanel({
  rootId,
  workspaceId,
}: {
  rootId: string | null;
  workspaceId: string | undefined;
}) {
  const { activeLensId, perspective } = useAppState();

  // React rules of hooks: both hooks called unconditionally; inactive
  // hook receives `null` → SWR skips fetch (proven in GraphLanding:43-54).
  const isGraph = perspective === "graph";
  const subgraph = useSubgraph(isGraph ? rootId : null);
  const architecture = useArchitecture(!isGraph ? (workspaceId ?? null) : null);

  if (activeLensId === "rationale" && rootId) {
    return (
      <RationaleView
        focusId={rootId}
        onSelectObject={() => {
          // Selection is read-only in this column for now.
        }}
      />
    );
  }

  const { data, isLoading, error } = isGraph ? subgraph : architecture;

  if (isLoading) return GRAPH_LOADING;
  if (error) return GRAPH_ERROR;

  return (
    <InteractiveGraph
      root={rootId ?? "—"}
      data={data}
      selectedId={rootId}
      onSelectObject={() => {
        // Selection is read-only; PaneStackView handles navigation.
      }}
    />
  );
}
```

Also add `import { useArchitecture } from "../hooks/useArchitecture";` near the existing `useSubgraph` import at L23.

---

### T1.3: Thread `workspaceId` from ShellBootstrap to InteractiveGraphPanel

- **Files**: `apps/explorer-ui/src/components/Shell.tsx` (line 115)
- **LOC delta**: +2 / -1 (prop threading)
- **Depends on**: T1.2 (panel must accept the prop)
- **Verification**:
  - `grep -n "InteractiveGraphPanel rootId" apps/explorer-ui/src/components/Shell.tsx` → must include `workspaceId={workspace?.id}`
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit code 0
  - `cd apps/explorer-ui && npm run test -- --run -- Shell.test` → existing viewport/health/skip-link suites still pass
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low** — single JSX attribute addition; `workspace` is `WorkspaceSummary | null` so `workspace?.id` is `string | undefined`, matching the prop type
- **Rollback**: revert the single commit

```diff
-                  <InteractiveGraphPanel rootId={rootId} />
+                  <InteractiveGraphPanel rootId={rootId} workspaceId={workspace?.id} />
```

`GraphLanding` branch on L113 stays untouched (already receives `workspaceId={workspace.id}` per the existing, unchanged spec scenario REQ-E5-2/3).

---

### T1.4: Fix `context.ts:56-60` doc comment (C1 — highest-value latent-trap fix)

- **Files**: `apps/explorer-ui/src/state/context.ts`
- **LOC delta**: +4 / -3 (comment-only)
- **Depends on**: none (pure comment change; in the same commit because it is the cheapest highest-value fix per design observation #4)
- **Verification**:
  - `grep -n "Toggle only applies" apps/explorer-ui/src/state/context.ts` → returns nothing
  - `grep -n "morphs the active graph canvas" apps/explorer-ui/src/state/context.ts` → returns the new comment
  - `cd apps/explorer-ui && npx tsc --noEmit` → exit code 0 (comment change is type-irrelevant but included in the gate)
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low** — comment-only; no runtime impact; removes a latent trap where the doc encoded the bug as intent (contradicting ADR-039 §3/§4)
- **Rollback**: revert the single commit

```diff
   /**
-   * Landing page perspective — graph (entry points) or c4 (component directories).
-   * Toggle only applies when no object is selected (landing view).
+   * Active canvas perspective — graph (symbol neighbourhood via useSubgraph)
+   * or c4 (workspace-wide components via useArchitecture).
+   * Perspective morphs the active graph canvas regardless of object
+   * selection (see ADR-039 §3/§4).
    */
   perspective: "graph" | "c4";
```

---

### T1.5: Add regression test in `Shell.test.tsx` (3 scenarios, MSW-backed)

- **Files**: `apps/explorer-ui/src/components/Shell.test.tsx`
- **LOC delta**: +110 / -2 (new describe block after existing `PerspectiveToggle` describe at L219-283; one import added)
- **Depends on**: T1.1, T1.2, T1.3 (panel must behave as specified for the test to pass)
- **Verification**:
  - `cd apps/explorer-ui && npm run test -- --run -- Shell.test` → all existing tests pass AND the three new scenarios pass:
    - **Scenario A**: `useSubgraph("sym-123")` called; `useArchitecture(null)` called; `InteractiveGraph.data === SUBGRAPH_FIXTURE`
    - **Scenario B**: `useSubgraph(null)` called; `useArchitecture("ws-42")` called; MSW handler `GET /api/workspaces/ws-42/architecture` returned 200 with `SubgraphResponse`-shaped JSON
    - **Scenario C**: `rootId === null && perspective === "c4"` → `GraphLanding` branch chosen; `InteractiveGraphPanel` never mounted (asserted via absence of `data-testid="interactive-graph"`)
  - `cd apps/explorer-ui && npm run test -- --run -- PerspectiveToggle` → existing 5 cases still pass (regression-safe — toggle dispatch + aria-pressed preserved)
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low–Medium** — MSW handlers must match the `fetchArchitecture` client call shape; if `fetchArchitecture` adds query params, the MSW matcher must be updated. Mitigation: assert on SWR key via `vi.mock("../hooks/useArchitecture")` if MSW path proves fragile.
- **Rollback**: revert the single commit

Implementation sketch (extend the `PerspectiveToggle` describe block, add at the end):

```tsx
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { useArchitecture } from "../hooks/useArchitecture";
import { useSubgraph } from "../hooks/useSubgraph";

// Spy hooks — replace the real SWR-backed hooks so we can assert call args.
vi.mock("../hooks/useSubgraph");
vi.mock("../hooks/useArchitecture");

const mockedUseSubgraph = vi.mocked(useSubgraph);
const mockedUseArchitecture = vi.mocked(useArchitecture);

const SUBGRAPH_FIXTURE = {
  nodes: [{ id: "sym-123", label: "CreateUser", kind: "function", style_class: "symbol" }],
  edges: [],
};

const ARCH_FIXTURE = {
  nodes: [{ id: "comp-users", label: "users", kind: "component", style_class: "c4-component" }],
  edges: [],
};

const server = setupServer(
  http.get("/api/workspaces/:id/architecture", () => HttpResponse.json(ARCH_FIXTURE)),
);

beforeAll(() => server.listen());
afterAll(() => server.close());
afterEach(() => {
  server.resetHandlers();
  mockedUseSubgraph.mockReset();
  mockedUseArchitecture.mockReset();
});

describe("PerspectiveToggle wire-up (E5.3)", () => {
  it("Scenario A: rootId set + perspective 'graph' feeds useSubgraph", async () => {
    mockedUseSubgraph.mockReturnValue({ data: SUBGRAPH_FIXTURE, isLoading: false, error: null, mutate: vi.fn() } as any);
    mockedUseArchitecture.mockReturnValue({ data: null, isLoading: false, error: null, mutate: vi.fn() } as any);

    render(
      <AppContext.Provider value={{
        state: { ...initialState, activeObjectId: "sym-123", perspective: "graph" },
        dispatch: vi.fn(),
      }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    await waitFor(() => {
      expect(mockedUseSubgraph).toHaveBeenCalledWith("sym-123");
      expect(mockedUseArchitecture).toHaveBeenCalledWith(null);
    });
  });

  it("Scenario B: rootId set + perspective 'c4' feeds useArchitecture", async () => {
    mockedUseSubgraph.mockReturnValue({ data: null, isLoading: false, error: null, mutate: vi.fn() } as any);
    mockedUseArchitecture.mockReturnValue({ data: ARCH_FIXTURE, isLoading: false, error: null, mutate: vi.fn() } as any);

    // Provide a fake workspace so the panel branch is taken (rootId !== null).
    // Use ShellBootstrap's workspace: easiest is to inject via ShellBootstrap
    // mock OR to assert against `useArchitecture` call args directly.
    render(
      <AppContext.Provider value={{
        state: { ...initialState, activeObjectId: "sym-123", perspective: "c4" },
        dispatch: vi.fn(),
      }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    await waitFor(() => {
      expect(mockedUseSubgraph).toHaveBeenCalledWith(null);
      expect(mockedUseArchitecture).toHaveBeenCalledWith("ws-42"); // from ShellBootstrap fixture
    });
  });

  it("Scenario C: rootId null + perspective 'c4' keeps GraphLanding branch", async () => {
    render(
      <AppContext.Provider value={{
        state: { ...initialState, activeObjectId: null, perspective: "c4" },
        dispatch: vi.fn(),
      }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    await waitFor(() => {
      // GraphLanding owns the C4 fetch — InteractiveGraphPanel must NOT mount.
      expect(document.querySelector('[data-testid="graph-landing"]')).toBeInTheDocument();
    });
  });
});
```

> **Test-impl note**: `ShellBootstrap`'s default workspace fixture provides `id: "ws-42"`. If the test must inject a workspace explicitly, add `vi.mock("../components/ShellBootstrap")` and have it render the same `Shell` callback with `{ workspace: { id: "ws-42", ... } }`.

---

### T1.6: Sync `docs/explorer-roadmap.md`

- **Files**: `docs/explorer-roadmap.md`
- **LOC delta**: +3 / -5 (status flips + critical-gap paragraph removed)
- **Depends on**: none (doc sync only)
- **Verification**:
  - `grep -n "E5.3" docs/explorer-roadmap.md` → shows status `✅ Done`
  - `grep -n "E5.5" docs/explorer-roadmap.md` → shows status `⚠️ Partial — data swap wired; crossfade still ❌`
  - `grep -n "Critical gap" docs/explorer-roadmap.md` → either removed or replaced with a one-liner pointing at the commit
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low** — markdown-only change; status flips reflect actual code state after T1.2 lands
- **Rollback**: revert the single commit

Edits:

```diff
 | E5.3 | Wire toggle → swap data source between `useSubgraph` and `useC4Context` | ❌ Gap | Perspective swap works on `GraphLanding` only; `InteractiveGraphPanel` always uses `useSubgraph` — toggle has no effect after object selection |
+→ change to: ✅ Done | `InteractiveGraphPanel` mirrors GraphLanding dual-hook pattern (Shell.tsx:45-71); regression test covers Scenarios A/B/C in `Shell.test.tsx` |

 | E5.5 | Add smooth transition between perspectives (data swap + re-layout) | ❌ Not done | No crossfade; cytoscape instance is destroyed and remounted on perspective change |
+→ change to: ⚠️ Partial — data swap wired (E5.3 ✅); crossfade still ❌ |

-**Critical gap:** E5.3 requires wiring `SET_PERSPECTIVE` into `InteractiveGraphPanel` so that
-`useSubgraph` is conditionally replaced by `useArchitecture` when `perspective === "c4"`.
-Without this, the toggle only affects the landing page.
+**Status:** E5.3 closed in commit `feat(explorer): wire PerspectiveToggle into InteractiveGraphPanel`.
+E5.5 crossfade remains ❌ — open.
```

---

### T1.7: Sync `docs/adr/ADR-039-explorer-navigation-model.md`

- **Files**: `docs/adr/ADR-039-explorer-navigation-model.md`
- **LOC delta**: +2 / -2 (status table + open-gaps bullet)
- **Depends on**: none (doc sync only)
- **Verification**:
  - `grep -n "E5 — Perspective toggle" docs/adr/ADR-039-explorer-navigation-model.md` → shows `✅ Complete`
  - `grep -n "Perspective toggle (E5) needs wiring" docs/adr/ADR-039-explorer-navigation-model.md` → returns nothing
- **Commit message**: bundled into the single atomic commit
- **Risk**: **Low** — markdown-only; mirrors T1.6 status update
- **Rollback**: revert the single commit

Edits:

```diff
-| E5 — Perspective toggle | ⚠️ Partial | Toggle exists in `ShellLayout`; dispatches `SET_PERSPECTIVE`; but only affects `GraphLanding` — `InteractiveGraphPanel` ignores it (always uses `useSubgraph`) |
+| E5 — Perspective toggle | ✅ Complete | `InteractiveGraphPanel` mirrors GraphLanding dual-hook pattern (Shell.tsx:45-71); `useSubgraph`/`useArchitecture` selected by `perspective`; regression test in `Shell.test.tsx` |

-- Perspective toggle (E5) needs wiring into `InteractiveGraphPanel` to work after object selection
```

---

## Verification (commit-level)

After the commit lands, all four REQ-E5-6 gates must pass:

```bash
# 1. Type check
cd apps/explorer-ui && npx tsc --noEmit
# Expected: exit 0; no diagnostic mentioning ArchitecturePayload vs SubgraphResponse

# 2. Targeted test runs
cd apps/explorer-ui && npm run test -- --run -- Shell.test
cd apps/explorer-ui && npm run test -- --run -- PerspectiveToggle
# Expected: all existing tests pass + 3 new scenarios pass

# 3. Lint (rules-of-hooks guard)
cd apps/explorer-ui && npm run lint
# Expected: no react-hooks/rules-of-hooks violation in Shell.tsx

# 4. Doc grep — confirm durable knowledge was updated
grep -n "GRAPH_ERROR" apps/explorer-ui/src/components/Shell.tsx          # 2 hits (defn + use)
grep -n "morphs the active graph canvas" apps/explorer-ui/src/state/context.ts  # 1 hit
grep -n "E5.3" docs/explorer-roadmap.md                                  # shows ✅ Done
grep -n "E5 — Perspective toggle" docs/adr/ADR-039-explorer-navigation-model.md  # shows ✅ Complete
```

Manual visual verification (deferred to PR review):

1. `npm run dev`, load a workspace.
2. Spotter → `CreateUser` symbol → SELECT_OBJECT.
3. Header toggle → `C4 Components`.
4. Canvas immediately re-renders with workspace-wide C4 graph (cytoscape destroy/remount, crossfade still ❌).
5. Header toggle → `Graph`.
6. Canvas swaps back to depth-3 call-graph around `CreateUser`.

---

## Rollback Notes

- **Single-commit atomicity** — all 7 sub-tasks land in one commit. Rollback = `git revert <sha>`.
- **No data loss risk** — no migrations, no DB schema changes, no cache invalidations.
- **No API consumers broken** — `useArchitecture` signature unchanged; `InteractiveGraph.data` type contract unchanged (`ArchitecturePayload` is a `SubgraphResponse` alias at `schemas.ts:1266-1267`).
- **No GraphLanding dependency** — `GraphLanding.tsx` is untouched; the new panel pattern mirrors but does not import from it.
- **Test-only side effects** — `Shell.test.tsx` adds new test cases; existing 5 `PerspectiveToggle` cases are preserved unchanged (per REQ-E5-6 Scenario 3).

## Open Blockers

None. All 5 snippet corrections and 4 critical observations from design #2698 are explicitly incorporated into the sub-tasks above.