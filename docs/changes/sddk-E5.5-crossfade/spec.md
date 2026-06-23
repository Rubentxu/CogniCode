# Kernel Specs: sddk/E5.5-crossfade

## Router Context Used
- Knowledge Coverage: sufficient — affected files (`Shell.tsx`, `InteractiveGraph.tsx`, `GraphLanding.tsx`), test files (`Shell.test.tsx`, `InteractiveGraph.test.tsx`, `e2e/exploration.spec.ts`), doc anchors (`explorer-roadmap.md:127`, `ADR-039-explorer-navigation-model.md:120,132`) all read directly; HEAD confirmed `a096d7f`
- Context Quality: **C1** — file paths and line numbers verified by direct read, no architecture blind spots
- Taxonomy: UI persistence (stale-data hold) + CSS transition (opacity fade) — single capability, two layers, frontend-only
- Domain Language: **resolved** — "morph" (ADR-039 §3/§4, one instance data-swap), "crossfade" (visual overlap), "stale-data hold" (preserve last-good SWR payload); **unresolved**: whether `prefers-reduced-motion` is already wired elsewhere — flagged as verification note, not blocker
- Recommended Effort: **verify** — proceed to design with explicit invariants

## Knowledge Provenance
- Scope source: `explorer-roadmap.md:127` (E5.5 ⚠️ Partial — crossfade still ❌, cytoscape destroyed and remounted)
- Invariant source:
  - `Shell.tsx:73-75` — `isLoading` gate returns `GRAPH_LOADING` div → unmounts panel (root cause #2, dominant flash)
  - `InteractiveGraph.tsx:185-196` — cleanup calls `cy.destroy()`; effect dep `[root, data, layoutAlgorithm]` triggers full remount on data reference change (root cause #1)
  - `GraphLanding.tsx:114-119` — same remount smell on landing page (deferred — out of scope)
  - `ADR-039-explorer-navigation-model.md:120,132` — toggle must morph (one instance, data swapped), not remount
- Memory-only hints excluded from spec truth: **None** — explore session recommendations (Option C, frontend-only, single commit) adopted as user-stated constraints, not memory claims

## Capability: perspective-toggle-stability

Maintain a single, continuous graph visualization across perspective changes (Graph ↔ C4) and revalidating data fetches by holding the last-good payload in a ref and applying a CSS opacity transition during the swap.

### Requirement: REQ-E5.5-1 — Stale-data hold in InteractiveGraphPanel
The system SHALL preserve the last-good `data` payload across SWR revalidations so that the panel never unmounts while a previously rendered payload is still available, and SHALL fall through to the error UI on a hard fetch failure.

#### Scenario: warm-cache revalidation does not unmount the panel
- **Given** `InteractiveGraphPanel` (`Shell.tsx:73-87`) has rendered once with `data = G1` and `isLoading = false`
- **When** the user toggles perspective and the SWR hook transitions to `isLoading = true` with `data = G1` (stale) and `error = undefined`
- **Then** the panel continues to render `InteractiveGraph` with `data = G1`
- **And** the DOM does not contain `[data-testid="interactive-graph-loading"]` (`Shell.tsx:92`)
- **And** the `InteractiveGraph` fiber is not remounted (no new mount effect run)

#### Scenario: new data arrives and replaces stale data without flash
- **Given** the panel is rendering with stale `data = G1`
- **When** SWR resolves with fresh `data = G2` (`isLoading = false`)
- **Then** the panel re-renders with `data = G2`
- **And** the `InteractiveGraph` mount effect (`InteractiveGraph.tsx:185-196`) does NOT run again (no `cy.destroy()`)

#### Scenario: hard fetch error surfaces immediately
- **Given** the panel is rendering with stale `data = G1`
- **When** SWR resolves with `error = E` and `data = undefined`
- **Then** the panel renders `GRAPH_ERROR` (`Shell.tsx:76`, `[data-testid="interactive-graph-error"]`)
- **And** stale `data = G1` is NOT held indefinitely past a hard error

#### Scenario: cold-cache first paint still shows a loading state
- **Given** the panel has never rendered before (no `data` in `useRef`)
- **When** SWR returns `isLoading = true` with `data = undefined`
- **Then** the panel renders `GRAPH_LOADING` (`Shell.tsx:90-99`)
- **And** `[data-testid="interactive-graph-loading"]` is present in the DOM

### Requirement: REQ-E5.5-2 — Opacity transition on canvas container
The system SHALL apply a CSS `opacity` transition to the `InteractiveGraph` canvas container so that a `data` reference swap fades the prior render out and the new render in over a short, deterministic duration, and SHALL respect `prefers-reduced-motion: reduce`.

#### Scenario: data reference change triggers a 250ms fade
- **Given** the `InteractiveGraph` canvas container is rendered with `data = G1` (opacity 1)
- **When** `data` changes to `data = G2` (new object reference)
- **Then** the container's `style` (or className) drives an opacity transition from 0 → 1 over `250ms ease-in-out`
- **And** the transition timing is deterministic and applied via CSS (not JS-driven) so React batching does not interrupt it

#### Scenario: `prefers-reduced-motion: reduce` collapses the transition
- **Given** the user OS reports `prefers-reduced-motion: reduce`
- **When** `data` changes to a new reference
- **Then** the container's `transition-duration` resolves to `0ms`
- **And** the swap is instantaneous (no fade), but the no-flash guarantee from REQ-E5.5-1 still holds

#### Scenario: opacity transition does not block pointer events on the canvas
- **Given** the canvas container is mid-fade (opacity = 0.5)
- **When** the user clicks a node on the cytoscape canvas
- **Then** the tap handler (`InteractiveGraph.tsx:188` `cy.on("tap", "node", handler)`) still fires
- **And** selection state updates (no `pointer-events: none` regression)

### Requirement: REQ-E5.5-3 — GraphLanding parity (out of scope)
This change SHALL NOT modify `GraphLanding.tsx:114-119`. The landing page retains the same remount smell and is explicitly deferred to a follow-up.

#### Scenario: GraphLanding is unchanged
- **Given** the v1 E5.5 change is applied
- **When** a reviewer diffs `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx`
- **Then** lines 114-119 are unchanged
- **And** a follow-up note is recorded (see Out of scope)

### Requirement: REQ-E5.5-4 — Tests
The system SHALL add unit and E2E coverage proving the no-flash guarantee.

#### Scenario: InteractiveGraph unit test — no `cy.destroy()` on data swap
- **Given** `InteractiveGraph.test.tsx` renders the component with `data = G1`
- **When** the prop `data` changes to `data = G2` (new object reference, same `root`, same `layoutAlgorithm`)
- **Then** the mount effect (`InteractiveGraph.tsx:185-196`) does NOT re-run
- **And** `cy.destroy()` is NOT called as a result of the prop change
- **And** the test asserts on `expect(cy.destroy).not.toHaveBeenCalled()` via spy on `cytoscape` or by counting mount runs

#### Scenario: Shell unit test — no GRAPH_LOADING when stale data exists
- **Given** `Shell.test.tsx` mounts `InteractiveGraphPanel` with `data = G1`, `isLoading = false`
- **When** the mock SWR hook updates to `isLoading = true`, `data = G1` (SWR revalidation pattern)
- **Then** `[data-testid="interactive-graph-loading"]` is NOT in the DOM
- **And** `[data-testid="interactive-graph"]` IS in the DOM
- **And** the rendered `data` prop equals `G1`

#### Scenario: E2E test — canvas persists across cold toggle
- **Given** `exploration.spec.ts` navigates to `/` and drills into an object (Shell renders `InteractiveGraph`)
- **When** the test toggles perspective via `[data-testid="perspective-c4"]` then back to `[data-testid="perspective-graph"]`
- **Then** `[data-testid="interactive-graph"]` is present in the DOM throughout (asserted before click, between clicks, and after final click)
- **And** `[data-testid="interactive-graph-loading"]` is NEVER present during the toggle sequence
- **And** an existing visual regression (`exploration-graph-perspective.png`) still passes with `maxDiffPixels` adjusted if needed

### Requirement: REQ-E5.5-5 — Documentation sync
The system SHALL update project documentation to reflect E5.5 completion and explicitly note the GraphLanding deferral.

#### Scenario: roadmap row flipped from ⚠️ Partial to ✅ Done
- **Given** `docs/explorer-roadmap.md:127` reads `⚠️ Partial — Data swap now works; crossfade still ❌ — cytoscape instance is destroyed and remounted on perspective change`
- **When** the change is applied
- **Then** line 127 reads `✅ Done — stale-data hold + opacity fade; cytoscape instance preserved across perspective toggle`
- **And** Sprint E5 status (line 119) remains `✅ Complete`

#### Scenario: ADR-039 E5 row updated for E5.5 sub-item
- **Given** `docs/adr/ADR-039-explorer-navigation-model.md:132` reads `E5 — Perspective toggle | ✅ Complete`
- **When** the change is applied
- **Then** the row's Evidence column notes E5.5 morph-completion: "morph preserved on drilled-in canvas (`Shell.tsx:73-87` stale-data hold + `InteractiveGraph.tsx` opacity transition); GraphLanding parity deferred"
- **And** the parenthetical on line 120 (`partial; toggle works on landing only, not in InteractiveGraph`) is removed or marked resolved

#### Scenario: GraphLanding deferral is recorded
- **Given** the change is applied without touching `GraphLanding.tsx`
- **When** a follow-up note is written
- **Then** it appears in `docs/explorer-roadmap.md` (Sprint E5 footer or a Sprint E5.6 stub) recording that GraphLanding has the same remount smell and is deferred

### Requirement: REQ-E5.5-6 — Verification
The system SHALL pass all three local verification commands and a manual visual sanity check.

#### Scenario: TypeScript compiles cleanly
- **Given** the change is applied
- **When** `npx tsc --noEmit` runs from `apps/explorer-ui/`
- **Then** exit code is 0
- **And** no new `any` casts are introduced

#### Scenario: Shell unit tests pass
- **Given** the change is applied
- **When** `npm run test -- --run -- Shell.test` runs
- **Then** all existing Shell tests pass
- **And** the new stale-data-hold scenario from REQ-E5.5-4 passes

#### Scenario: InteractiveGraph unit tests pass
- **Given** the change is applied
- **When** `npm run test -- --run -- InteractiveGraph.test` runs
- **Then** all existing InteractiveGraph tests pass
- **And** the new no-`cy.destroy()` scenario from REQ-E5.5-4 passes

#### Scenario: manual visual sanity — no flash on drilled-in toggle
- **Given** the dev server runs with `VITE_USE_MOCKS=true`
- **When** a developer drills into an object and toggles perspective on the graph canvas
- **Then** the canvas remains visible across the toggle (no blank-then-redraw)
- **And** the transition completes in ≈250ms with a perceptible fade
- **And** the same toggle on `GraphLanding` still flashes (deferred — out of scope)

## Invariants Covered
- **Morph semantics (ADR-039 §3/§4)** — one cytoscape instance, data swapped — covered by REQ-E5.5-1 (Scenario: warm-cache) and REQ-E5.5-4 (InteractiveGraph unit test asserting no `cy.destroy()`)
- **SWR revalidation does not unmount** — covered by REQ-E5.5-1 (Scenario: warm-cache) and REQ-E5.5-4 (Shell unit test)
- **Hard errors still surface** — covered by REQ-E5.5-1 (Scenario: hard fetch error)
- **Accessibility — reduced motion** — covered by REQ-E5.5-2 (Scenario: `prefers-reduced-motion`)
- **Interactive parity during fade** — covered by REQ-E5.5-2 (Scenario: opacity does not block pointer events)
- **Doc/code drift prevention** — covered by REQ-E5.5-5 (all three scenarios)
- **Verification gate** — covered by REQ-E5.5-6 (all four scenarios)

## Out of scope
- **GraphLanding parity** (`apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx:114-119`) — same remount smell; deferred. Recorded as follow-up in `docs/explorer-roadmap.md`.
- **Advanced morph (FLIP / shared-element transitions)** — beyond v1 crossfade; not required by roadmap.
- **Backend changes** — explicitly frontend-only per adopted decisions; no Rust or SQL touched.
- **New dependencies** — no `npm install` additions; CSS transition + `useRef` are stdlib React.
- **Test infrastructure overhaul** — existing Vitest + Playwright stack is sufficient; no new runners.

## Open Questions
- **None blocking.** Two non-blocking verification notes:
  1. Confirm `prefers-reduced-motion` is not already handled by a parent CSS reset (`apps/explorer-ui/src/styles/*.css`) — if so, REQ-E5.5-2's `prefers-reduced-motion` branch may be a no-op. Verification, not spec truth.
  2. ADR-039 line 132 already reads `✅ Complete`; the spec mandates a more granular Evidence column note. Implementation should add the parenthetical about E5.5 sub-completion rather than flip the row.

## Verification commands

```bash
# From apps/explorer-ui/
npx tsc --noEmit
npm run test -- --run -- Shell.test
npm run test -- --run -- InteractiveGraph.test
npm run test:e2e -- exploration.spec
# Manual:
#   VITE_USE_MOCKS=true npm run dev → drill into object → toggle perspective → confirm no flash
```