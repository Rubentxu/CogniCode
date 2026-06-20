# Plan: Explorer E2E Test Battery — Graph & C4 Navigation Flows

> **Status**: Active  
> **Date**: 2026-06-19  
> **ADR**: [ADR-039](adr/ADR-039-explorer-navigation-model.md)  
> **Goal**: Comprehensive E2E test suite covering all Explorer navigation flows

---

## Current State

### What already exists (13 tests across 5 files)

| File | Tests | Coverage |
|------|-------|----------|
| `smoke.spec.ts` | 1 | Spotter → inspect (core flow) |
| `exploration.spec.ts` | 4 | Toggle, Spotter, C4+Spotter |
| `graph.spec.ts` | 2 | Graph rendering (Mock-dependent) |
| `responsive.spec.ts` | 3 | Viewport layout verification |
| `a11y.spec.ts` | 3 | Accessibility audit |

### What's broken or missing

1. **`useBootstrapWorkspace` never wired** — `appState.workspace` stays null → `GraphLanding` never renders
2. **`graph-landing-canvas` never shown** — confirmed in debug E2E (only `interactive-graph-empty` found)
3. **C4 data can't be tested via graph** — only via Spotter today
4. **No landing page assertions** — GraphLanding is the face of the app and has 0 tests
5. **No error-state tests** — connection lost, workspace missing, fetch failures
6. **No keyboard navigation tests** — only mouse-based interactions
7. **No pane-stack interaction tests** — close, reorder, multiple panes

---

## Gaps

| User Journey | Current State | Target State |
|-------------|---------------|--------------|
| **First open → landing** | ❌ Broken (no workspace) | ✅ GraphLanding renders with root nodes |
| **Landing → click node** | ❌ Not tested | ✅ Pane-stack opens with inspector |
| **Landing → toggle C4** | ⚠️ Toggle works, graph empty | ✅ C4 components shown |
| **Graph → pan/zoom** | ❌ Not tested | ✅ Canvas interaction verified |
| **Pane-stack → multiple** | ❌ Not tested | ✅ 2+ panes, close, reorder |
| **Pane-stack → views** | ❌ Not tested | ✅ Switch views, content changes |
| **Spotter → filters** | ❌ Not tested | ✅ Kind filter, keyboard nav |
| **Error states** | ❌ Not tested | ✅ Connection lost, 404, missing data |
| **Responsive** | ⚠️ Basic checks | ✅ Bottom-sheet, desktop, tablet |
| **Keyboard** | ❌ Not tested | ✅ Tab order, Enter/Space |

---

## Prerequisites (must fix first)

### P1: Wire `useBootstrapWorkspace`
**Why**: Without workspace auto-detection, GraphLanding never renders.

**Fix**: Add useEffect in Shell or App that reads `useWorkspaceList()` and dispatches `SET_WORKSPACE`.

### P2: Ensure MSW covers all landing data
**Why**: E2E tests use `VITE_USE_MOCKS=true`. All /api/* traffic goes through MSW.

**Check**: Verify landing, architecture, subgraph, workspace handlers are registered.

---

## Test Battery — 6 Phases, 38 scenarios

### Phase 1: Landing Page (8 tests)

| # | Test |
|---|------|
| P1.1 | Graph landing renders after workspace bootstrap |
| P1.2 | Landing shows root nodes in cytoscape canvas |
| P1.3 | Landing shows suggested questions strip |
| P1.4 | Landing canvas is interactive (pan/zoom) |
| P1.5 | Click root node → pane-stack opens |
| P1.6 | Landing header: workspace name + symbol count |
| P1.7 | Landing error state when fetch fails |
| P1.8 | Landing loading state during fetch |

### Phase 2: Perspective Toggle (6 tests)

| # | Test |
|---|------|
| P2.1 | Toggle Graph → C4 perspective |
| P2.2 | C4 shows component architecture nodes |
| P2.3 | C4 shows correct node styles (component/container/system) |
| P2.4 | Toggle back C4 → Graph restores data |
| P2.5 | Repeated toggling doesn't duplicate nodes |
| P2.6 | Toggle keyboard accessible (Tab+Enter/Space) |

### Phase 3: Pane-Stack (8 tests)

| # | Test |
|---|------|
| P3.1 | First pane renders object inspector |
| P3.2 | Second pane creates new tab (max 8) |
| P3.3 | Click tab switches active pane |
| P3.4 | Close pane removes it |
| P3.5 | Close last pane shows empty state |
| P3.6 | Active pane shows object label |
| P3.7 | View tabs render for inspected object |
| P3.8 | Switch view updates inspector body |

### Phase 4: Spotter (5 tests)

| # | Test |
|---|------|
| P4.1 | Open via Cmd+K |
| P4.2 | Open via search button |
| P4.3 | Results grouped by kind |
| P4.4 | Close with Escape |
| P4.5 | Empty state when no results |

### Phase 5: Error & Edge Cases (6 tests)

| # | Test |
|---|------|
| P5.1 | Connection gate: backend unreachable |
| P5.2 | Error boundary catches crashes |
| P5.3 | Empty workspace: "open workspace" prompt |
| P5.4 | >500 nodes shows warning |
| P5.5 | Object not found → 404 message |
| P5.6 | >8 panes drops oldest (FIFO) |

### Phase 6: Responsive & Accessibility (5 tests)

| # | Test |
|---|------|
| P6.1 | Desktop: graph + pane-stack side-by-side |
| P6.2 | Tablet: lens overlay toggle |
| P6.3 | Small: bottom-sheet visible |
| P6.4 | Focus order: natural reading order |
| P6.5 | All elements reachable via keyboard |

---

## Implementation Order

| Sprint | What | Time |
|--------|------|------|
| **F1** | Fix prerequisites (bootstrap workspace) | 2-3h |
| **F2** | Phase 1 + 2 (Landing + Toggle) | 3-4h |
| **F3** | Phase 3 + 4 (Pane-Stack + Spotter) | 3-4h |
| **F4** | Phase 5 + 6 (Errors + Responsive) | 3-4h |

**Total**: 38 scenarios, ~12-15h, parallelizable (F2+F3 concurrently).

---

## Success Criteria

- [ ] `npx playwright test` runs all 38+ tests
- [ ] GraphLanding renders as the first screen
- [ ] C4 perspective toggle works end-to-end
- [ ] Pane-stack: multi-pane inspection verified
- [ ] All nav paths tested (graph→C4→pane→views→close)
- [ ] Error states handled gracefully
- [ ] Keyboard navigation verified
- [ ] Responsive layout at 3 breakpoints
- [ ] MSW mocks cover all endpoints
