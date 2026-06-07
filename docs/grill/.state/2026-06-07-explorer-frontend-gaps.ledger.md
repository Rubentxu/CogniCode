## Q001-P1 — TypeScript or JavaScript for the Explorer frontend?

**Pass:** 1
**Category:** tooling
**Status:** accepted
**Confidence:** high

### Question

TypeScript or JavaScript for the Explorer frontend?

### Evidence packet

- Codebase evidence: Leptos dashboard exists (18 pages, 18 components, 61 tests) in Rust WASM
- Prototype evidence: 478-line vanilla JS prototype at CodeExplorerPage
- ADR 0009: React 19 + Tailwind CSS already decided for web frontends
- Missing: No explicit Explorer technology decision recorded

### Proxy answer

TypeScript strict mode (strict: true, strictNullChecks, noImplicitAny), `.tsx` for components, `.ts` for utilities. Domain types generated from Rust DTOs via `ts-rs` into shared `@cognicode/types` package. Prototype rewritten, not migrated.

### Skeptic challenge

Six architectural challenges raised:
1. **ARCHITECTURAL FORK** (HIGH): Leptos dashboard exists — extend it instead of new React stack
2. **ts-rs fragility** (HIGH): Community crate, small team, complex Rust enums with serde could break codegen
3. **Build pipeline doubling** (HIGH): cargo build → ts-rs → npm → tsc → vite vs single cargo build
4. **Prototype already in Leptos** (HIGH): CodeExplorerPage proves model works in Rust
5. **TypeScript strict mode unnecessary** (MEDIUM): Rust's type system is already strict
6. **MCP boundary** (MEDIUM): TypeScript types benefit only web UI, not MCP consumers

Challenge intensity: HIGH

### Judge initial verdict

**REJECTED** — proposed extending Leptos dashboard with new explorer pages.

### User override

"no vamos a usar leptos. el dashboard es otra aplicacion independiente, no la relaciones"

### Judge final answer

**ACCEPTED.** React 19 + TypeScript strict mode for Explorer frontend. Clean rewrite from prototype. Dashboard (Leptos) is a separate, independent application — no code sharing, no technology coupling between them.

### Why this decision was made

User explicitly rejected extending the Leptos dashboard. Explorer frontend is a standalone React application as intended by ADR 0009. Dashboard and Explorer are separate products with different technology stacks.

### Rejection trace (when Status is rejected or modified)

(Not applicable — user override converted REJECTED to ACCEPTED)

### Impact

- CONTEXT.md: yes
- ADR: yes
- API: no
- Persistence: no
- Tests: yes
- Security: no
- Observability: no

### Follow-up questions created

- How should shared domain types flow from Rust DTOs to the TypeScript frontend without ts-rs fragility?
- Should Explorer share any infrastructure (auth, API client, deployment pipeline) with Dashboard?

### Validation required

Yes. User decision overrides architectural concern — should be documented to prevent future contributors from attempting to merge stacks.

### Proxy learning

Before answering technology-selection questions, ALWAYS search workspace for existing implementations. The answer was correct but the reasoning should have started by checking what exists (Leptos dashboard) and explaining why a separate React stack is intentional rather than accidental.

---

## Q002-P1 — Where does the Explorer frontend live and what build tool?

**Pass:** 2
**Category:** structure
**Status:** modified
**Confidence:** high

### Question

Where does the Explorer frontend live and what build tool?

### Evidence packet

- Codebase evidence: Root uses npm (package-lock.json present). No existing apps/ or packages/ directories.
- ADR 0009: React 19 + Tailwind CSS for web frontends
- Q001-P1 outcome: TypeScript strict mode, React 19, WASM layer permitted
- Prototype: CodeExplorerPage (446 lines) exists in Leptos dashboard but dashboard is independent application
- Missing: No explicit monorepo structure decision, no build-tool version pinned

### Proxy answer

`apps/explorer-ui/` with Vite 6 + React 19 + TypeScript + Tailwind, pnpm workspace, separate `packages/cognicode-types/` and `packages/cognicode-explorer-wasm/`.

### Skeptic challenge

Four concerns raised:
1. **Leptos dashboard exists** (LOW — already overruled by user in Q001-P1)
2. **pnpm switch unnecessary** (HIGH): Root uses npm. Switch adds migration cost with no stated benefit.
3. **packages/ directory premature** (HIGH): Three-directory layering (`apps/` + `packages/types` + `packages/wasm`) for an MVP with only one consumer
4. **ts-rs type staleness** (MEDIUM): CI-generate vs commit strategy not addressed

### Judge final answer

**MODIFIED.** `apps/explorer-ui/` with Vite 6 + React 19 + TypeScript strict + Tailwind CSS. Stay on npm (no pnpm switch). Delete `packages/` directory. Types generated to `apps/explorer-ui/src/types/generated/` (committed, `npm run gen-types`). WASM from Rust workspace crate consumed as local path dependency in package.json. No npm workspaces until second app proves sharing is real. Vite `server.proxy` for dev backend.

### Why this decision was made

Extraction must be earned. `packages/` directory and npm workspaces solve a problem that does not yet exist (only one consumer). Generated types and WASM live where consumed — simpler, zero indirection, zero configuration overhead. pnpm switch is a distraction with no stated benefit and real migration cost. Prove sharing is necessary before building sharing infrastructure.

### Rejection trace (only when Status is rejected or modified)

**Rejection reason:** Proxy proposed pnpm switch and multi-directory `packages/` extraction for a single-consumer MVP. Fails YAGNI principle — builds sharing infrastructure before a second consumer exists.

**Remedy proposed:** Keep npm. Delete `packages/`. Colocate artifacts where consumed. Remedy level: yagni_simplification.

**Alternative answer:** `apps/explorer-ui/` stays. Types in `src/types/generated/`. WASM as local path dep. No workspaces until needed.

**What Proxy missed:** Proposed tool-switch (npm→pnpm) without checking root tooling. Proposed `packages/` extraction before any second consumer exists. Did not evaluate "where consumed" as simpler alternative for generated artifacts.

**Proxy learning:** Before proposing directory structures or tool switches: (1) check what the existing monorepo uses; (2) ask "is the second consumer real or hypothetical?" Extract only when sharing is proven by an actual second application.

### Impact

- CONTEXT.md: yes
- ADR: no
- API: no
- Persistence: no
- Tests: yes
- Security: no
- Observability: no

### Follow-up questions created

- How should CI generate and commit TypeScript types from Rust DTOs?
- When is `packages/` extraction justified? (success criterion: second consumer exists)

### Validation required

Yes. Backend port for Vite dev proxy needs user confirmation. Branch strategy: start with `apps/explorer-ui/` flat, extract to packages/ only when second app proves sharing demand.

---

## Q003-P1 — What component library for the Explorer frontend?

**Pass:** 3
**Category:** tooling
**Status:** modified
**Confidence:** high

### Question

What component library for the Explorer frontend?

### Evidence packet

- Codebase evidence: ~8 Explorer components identified (MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb, Spotter)
- ADR 0009: React 19 + Tailwind CSS for web frontends
- Q001-P1 outcome: TypeScript strict mode, clean rewrite from prototype
- Q002-P1 outcome: `apps/explorer-ui/` with Vite 6, npm, no packages/
- WAI-ARIA Authoring Practices Guide: Tabs Pattern, Breadcrumb Pattern, Listbox Pattern — all ~60 lines each
- Missing: No per-component complexity analysis before library selection

### Proxy answer

Radix UI primitives + Tailwind CSS 4 + cmdk for Spotter. No shadcn/ui.

### Skeptic challenge

Five concerns raised:
1. Radix UI maintenance slowed post-WorkOS acquisition (2022). shadcn/ui already supports Base UI as alternative.
2. Miller Columns — the most complex component — has NO Radix primitive. Manual keyboard nav is the real complexity regardless.
3. ~8 components total — direct ARIA attributes + Tailwind may be simpler than a Radix dependency.
4. Spike needed for Miller Columns focus management before committing.
5. cmdk justified for Spotter (portal, focus trap, overlay, filtering — genuine complexity).

### Judge final answer

**MODIFIED.** cmdk for Spotter (only third-party component dependency). Everything else — MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb — uses direct ARIA attributes + Tailwind CSS 4. No direct Radix dependencies. cmdk handles its own Radix Dialog internally. ViewTabs ~60 lines following WAI-ARIA Tabs Pattern (APG).

### Why this decision was made

Only Spotter has genuine complexity (portal, focus trap, overlay, filtering). Everything else is simple enough for direct ARIA. Miller Columns are custom work regardless — no library helps with cascading column keyboard navigation. Seven of eight components are standard patterns covered by WAI-ARIA APG at ~60 lines each.

### Rejection trace (only when Status is rejected or modified)

**Rejection reason:** Proxy added full Radix UI dependency for ~8 components without per-component complexity analysis. Seven of eight components are simple enough for direct ARIA. Only Spotter (cmdk) has genuine complexity warranting a third-party dependency.

**Remedy proposed:** Remove Radix UI. Keep cmdk only. Use direct ARIA + Tailwind for all other components. Remedy level: `minimize_dependencies`.

**Alternative answer:** cmdk for Spotter. Direct ARIA + Tailwind CSS 4 for everything else — MillerColumn, ViewTabs (WAI-ARIA Tabs Pattern), ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb.

**What Proxy missed:** Failed to evaluate per-component complexity. Applied "component library" as default answer without counting which components actually need library primitives. Overlooked that Miller Columns have no Radix equivalent anyway — the hardest component was going to be custom regardless.

**Proxy learning:** Before selecting a component library: (1) enumerate every component in the tree; (2) classify each as "standard ARIA pattern" or "genuinely complex"; (3) only add a dependency for the genuinely complex ones. WAI-ARIA Authoring Practices Guide covers Tabs, Breadcrumb, Listbox, Grid, Dialog patterns — each ~60 lines of direct TypeScript.

### Impact

- CONTEXT.md: yes — component architecture decision
- ADR: yes — component architecture pattern
- API: no
- Persistence: no
- Tests: yes — component tests per WAI-ARIA pattern
- Security: no
- Observability: no

### Follow-up questions created

- Should we spike Miller Columns keyboard navigation before committing to direct ARIA implementation?
- Does cmdk's internal Radix Dialog dependency create any version conflict risk with React 19?

### Validation required

Yes. Miller Columns keyboard navigation needs a spike before implementation. The cascading-column focus model (arrow keys across columns, tab between interactive elements) has no WAI-ARIA precedent — must be validated with real users.

---

## Q004-P1 — State management + data fetching for Explorer frontend?

**Pass:** 4
**Category:** state
**Status:** modified
**Confidence:** high

### Question

State management + data fetching for Explorer frontend?

### Evidence packet

- Q003-P1 outcome: minimize_dependencies pattern — cmdk only third-party component dep; direct ARIA + Tailwind for everything else
- Q002-P1 outcome: `apps/explorer-ui/` with Vite 6, React 19, TypeScript strict, npm
- Scale evidence: ~7 API endpoints (graph queries, search, provenance, etc.) and 8 Explorer components
- ADR 0009: React 19 + Tailwind CSS for web frontends
- Missing: No explicit state-management decision recorded

### Proxy answer

TanStack Query (server state) + Zustand (UI state) + typed fetch wrapper (~50 lines). No OpenAPI codegen.

### Skeptic challenge

Five concerns raised:
1. SWR (~4KB) lighter than TanStack Query (~12KB) for 7 endpoints — same minimize_dependencies pattern from Q003
2. Zustand columns[] array re-renders ALL columns on any mutation. Jotai atoms isolate per-column re-renders.
3. Typed fetch wrapper ~50 lines redundant — SWR's `fetcher` parameter IS the typed wrapper
4. Miller Columns UI state may not need Zustand — React Context + useReducer inside component tree sufficient
5. WASM integration path undefined

### Judge final answer

**MODIFIED.** SWR (~4KB) for server state — caching, dedup, refetch for 7 API endpoints. React Context + useReducer for Miller Columns array — the only shared UI state. Local useState per column for active view, active lens, playground query. cmdk-internal state for Spotter (already handled by cmdk). No typed fetch wrapper — SWR's fetcher parameter handles typing. No Zustand, no Jotai — Context + useReducer sufficient for a single shared state slice.

### Why this decision was made

Follows Q003 pattern of minimize_dependencies. SWR covers all needed server state patterns (caching, dedup, mutation refetch) at 4KB — TanStack Query's extra 8KB adds nothing for this scale. The Miller Columns array is the ONLY shared UI state — Context is precisely the right tool for one shared slice, not a global store. Per-column state stays local to each column component. Zero unnecessary abstractions.

### Rejection trace (only when Status is rejected or modified)

**Rejection reason:** Proxy proposed TanStack Query (12KB) + Zustand + typed fetch wrapper when lighter alternatives cover the same requirements. Fails the minimize_dependencies pattern established by Q003-P1.

**Remedy proposed:** SWR replaces TanStack Query. React Context + useReducer replaces Zustand. Delete typed fetch wrapper. Remedy level: `minimize_dependencies`.

**Alternative answer:** SWR (~4KB) for server state. React Context + useReducer for Miller Columns (only shared UI state). Local useState per column. cmdk-internal for Spotter. No typed fetch wrapper, no Zustand, no Jotai.

**What Proxy missed:** Failed to apply the minimize_dependencies heuristic from Q003-P1. Proposed a 12KB library when a 4KB one covers identical patterns at this scale. Proposed Zustand as default global store when only one shared state slice exists. Proposed an unnecessary typed fetch wrapper when SWR's fetcher parameter already handles request typing.

**Proxy learning:** Before answering state management questions: (1) enumerate every state slice by category (server cache, shared UI, local UI, third-party internal); (2) apply the minimize_dependencies heuristic — start with the lightest tool that covers the requirement; (3) distinguish "one shared slice" (Context) from "many cross-cutting slices" (Zustand/Jotai).

### Impact

- CONTEXT.md: yes — state architecture decision
- ADR: no — standard React patterns, not architecturally significant
- API: no
- Persistence: no
- Tests: yes — state management tests per slice
- Security: no
- Observability: no

### Follow-up questions created

- Should SWR's cache be configured with a global SWRConfig provider or per-hook?
- How should the WASM layer expose its internal state (graph data) to React components?
- What revalidation strategy (stale-while-revalidate, polling, or mutation-triggered) fits Explorer's real-time requirements?

### Validation required

Yes. The WASM integration path for state management is undefined. Need to confirm whether WASM graph data flows through SWR's cache or via a separate bridge (postMessage / custom hook).

---

## Q005-P1 — Routing strategy for Explorer?

**Pass:** 5
**Category:** routing
**Status:** accepted
**Confidence:** medium

### Question

Routing strategy for Explorer?

### Evidence packet

- Q003-P1 outcome: cmdk for Spotter only; direct ARIA + Tailwind CSS 4 for everything else
- Q004-P1 outcome: SWR + Context/useReducer + local useState; minimize_dependencies pattern
- Scale evidence: Single page with dynamic views (tabs, columns, Spotter overlay). No multi-page navigation needed.
- Missing: No explicit routing decision recorded

### Proxy answer

No router for MVP. Single-page in-memory via Context/useReducer. Deep linking deferred.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** No router for MVP. Single-page navigation in-memory via Context/useReducer. Deep linking via vanilla `history.pushState` when needed.

### Why this decision was made

Explorer has only one page with dynamic views (tabs, columns, Spotter overlay). A full router (React Router, TanStack Router) solves navigation problems that don't exist at this scale. In-memory navigation via Context is sufficient for all internal state changes. Deep linking can be added incrementally with vanilla `history.pushState` and `popstate` event listener — no library required.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: no
- API: yes — vanilla history API for deep linking
- Persistence: no
- Tests: yes — deep linking behavior via popstate + Context state sync
- Security: no
- Observability: no

### Follow-up questions created

- When should a router be introduced? (success criterion: second page or URL-driven navigation required)

### Validation required

No. Decision is reversible — adding a router later is low-cost if deep-linking requirements grow.

### Proxy learning

Before selecting a routing library: (1) count the number of distinct pages/screens; (2) distinguish URL-driven navigation from in-memory view state; (3) if only one page with dynamic views, Context + vanilla history API may suffice.

---

## Q006-P1 — Testing strategy and tools for Explorer frontend?

**Pass:** 6
**Category:** testing
**Status:** accepted
**Confidence:** high

### Question

Testing strategy and tools for Explorer frontend?

### Evidence packet

- Stack context: React 19 + TypeScript + Vite 6 + SWR + Context/useReducer + cmdk + direct ARIA components
- Q003-P1 outcome: cmdk for Spotter; direct ARIA for 7 other components
- Q004-P1 outcome: SWR for server state, Context + useReducer for Miller Columns
- Q005-P1 outcome: No router; in-memory navigation via Context
- Monorepo evidence: Playwright already present in root for Dashboard E2E tests
- Missing: No explicit testing strategy for Explorer frontend recorded

### Proxy answer

- **Vitest** — Vite-native test runner, zero config
- **React Testing Library** — component tests (render, screen, userEvent)
- **Playwright** — E2E tests (already in monorepo root for dashboard)
- **MSW** (Mock Service Worker) — mock Explorer API in tests
- **No visual regression** initially — defer
- Test pyramid: unit (hooks/SWR fetchers) → component (MillerColumns, Spotter, ViewTabs) → E2E (full exploration flow)
- Coverage: aim for >80% on domain logic, E2E covers critical paths

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Vitest + React Testing Library + Playwright + MSW. No visual regression initially. Test pyramid: unit → component → E2E. Coverage: >80% on domain logic. Pattern: use what's already in the monorepo + ecosystem defaults.

### Why this decision was made

Vitest is the Vite ecosystem standard — zero-config, native ESM, same transform pipeline as Vite dev server. React Testing Library promotes testing based on accessibility roles, not implementation details — aligning with Q003-P1's ARIA-first component architecture. Playwright is already in the monorepo with CI setup — reuse, don't duplicate. MSW intercepts at the network level, catching realistic request/response behavior without mocking fetch. Visual regression deferred until component architecture stabilizes. Every choice is either an ecosystem default or an existing monorepo asset.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: no
- API: no
- Persistence: no
- Tests: yes — primary impact
- Security: no
- Observability: no

### Follow-up questions created

- When should visual regression testing be introduced? (success criterion: component architecture stabilizes)
- Should Playwright tests run in the same CI workflow as Dashboard E2E or a separate one?
- Should MSW handlers be shared between component tests and E2E tests?

### Validation required

Yes. Confirm that the existing monorepo Playwright CI setup supports a second test suite for Explorer. Verify MSW works with SWR's fetch-based fetcher (MSW intercepts `fetch` natively, so it should — but verify).

### Proxy learning

Before designing a testing strategy: (1) check what test tools already exist in the monorepo; (2) prefer ecosystem defaults (Vitest for Vite, RTL for React); (3) distinguish "needed now" (unit, component, E2E) from "defer to later" (visual regression).

---

## Q007-P2 — Loading States

**Pass:** 2
**Category:** ux
**Status:** accepted
**Confidence:** high

### Question

Loading states strategy for the Explorer frontend?

### Evidence packet

- Stack context: React 19 + SWR + Vite 6 + WASM layer for graph operations
- Q004-P1 outcome: SWR for server state (caching, dedup, mutation refetch)
- Missing: No explicit loading state strategy recorded

### Proxy answer

React 19 Suspense at top level for WASM init. SWR isLoading for data. Three tiers: WASM init (full-page spinner), first fetch (column placeholder + pulse), cache hit (instant). No skeleton library.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** React 19 Suspense at top level for WASM init. SWR isLoading for data. Three tiers: (1) WASM init — full-page spinner, (2) first fetch — column placeholder + pulse animation, (3) cache hit — instant render. No skeleton library.

### Why this decision was made

Three-tier loading matches the three real states of the application: WASM not yet ready, data not yet fetched, data cached. SWR's built-in isLoading avoids custom loading state management. Suspense at the top level isolates WASM initialization from data fetching concerns. No skeleton library needed — Tailwind pulse animation on placeholder divs covers the single loading variant.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: no
- API: no
- Persistence: no
- Tests: yes
- Security: no
- Observability: no

### Follow-up questions created

None.

### Validation required

No. Pattern is standard React 19 + SWR — well-established in the ecosystem.

### Proxy learning

Before designing loading states: (1) enumerate the distinct states (WASM init, first fetch, cache hit); (2) leverage framework built-ins (Suspense, SWR isLoading) rather than custom loading machinery; (3) Tailwind animations cover skeleton-like patterns without a skeleton library.

---

## Q008-P2 — Error States

**Pass:** 2
**Category:** ux
**Status:** accepted
**Confidence:** high

### Question

Error states strategy for the Explorer frontend?

### Evidence packet

- Stack context: React 19 (built-in ErrorBoundary) + SWR (error/retry) + direct ARIA components
- Q006-P1 outcome: Vitest + RTL + Playwright + MSW for testing
- Missing: No explicit error state strategy recorded

### Proxy answer

Error Boundary per Miller Column. SWR error/retry for API. App-level boundary as last resort. React 19 built-in ErrorBoundary. Fallback: message + "Try again" button.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Error Boundary per Miller Column for component-level isolation. SWR error/retry for API calls (SWR exposes error + mutate for retry). App-level ErrorBoundary as last resort for unhandled exceptions. React 19 built-in ErrorBoundary component. Fallback UI: error message + "Try again" button triggering SWR retry or reload.

### Why this decision was made

Per-column error boundaries prevent one failed column from breaking the entire Explorer. SWR's built-in error and retry handling eliminates custom error state management for data fetching. React 19's built-in ErrorBoundary avoids a third-party dependency. The app-level boundary catches everything else — a standard defensive pattern.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: no
- API: no
- Persistence: no
- Tests: yes — error boundary behavior tests
- Security: no
- Observability: yes — error boundaries are instrumentation points

### Follow-up questions created

- Should errors be reported to an external service (Sentry, Datadog) from the error boundaries?

### Validation required

No. Pattern is standard React 19 + SWR — well-established.

### Proxy learning

Error boundaries should follow component hierarchy: per-feature (Miller Column) for isolation, app-level for last resort. Avoid global-only error handling — it kills the entire UI for local failures.

---

## Q009-P2 — Visualization Library

**Pass:** 2
**Category:** visualization
**Status:** accepted
**Confidence:** high

### Question

What visualization approach for the Explorer graph view?

### Evidence packet

- Codebase evidence: cognicode-diagram Rust crate implementing Sugiyama layered graph layout algorithm
- Q004-P1 outcome: WASM layer for performance-critical operations
- Q003-P1 outcome: minimize_dependencies pattern — cmdk only third-party component
- Scale evidence: Graph visualization is the central Explorer feature
- Missing: No explicit graph rendering decision recorded

### Proxy answer

Custom React SVG components fed by WASM layout from cognicode-diagram Sugiyama engine. Zero graph rendering library. Bundle: ~0KB vs React Flow ~200KB. Navigation clicks, not graph manipulation.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Custom React SVG components fed by WASM layout engine (cognicode-diagram Sugiyama). Zero graph rendering library. Layout computed in Rust/WASM; rendering in React SVG with full control over styling, interactivity, and accessibility. Navigation is click-to-expand (not graph manipulation) — no D3 force simulation, no React Flow dragging needed.

### Why this decision was made

The Sugiyama layout engine already exists in Rust — it does the hard part (node positioning). Rendering positioned nodes and edges as SVG is straightforward (~100-200 lines of React). React Flow (~200KB) solves graph manipulation (dragging nodes, adding edges) — none of which Explorer needs. Explorer navigation is: click a node → WASM computes new layout → re-render SVG. A full graph library adds 200KB for features the user will never invoke. Follows the minimize_dependencies pattern from Q003-P1 applied to visualization.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: yes — visualization architecture is architecturally significant
- API: yes — SVG component API for graph nodes, edges, and interaction handlers
- Persistence: no
- Tests: yes — layout output → SVG render snapshot tests
- Security: no
- Observability: no

### Follow-up questions created

- What zoom/pan strategy for the SVG graph viewport? (CSS transform, viewBox manipulation, or library like d3-zoom?)
- How should edge routing (curves, orthogonal) be handled — in Rust layout or React SVG?

### Validation required

Yes. SVG performance with large graphs (>500 nodes) should be benchmarked. Canvas fallback may be needed for very large graphs.

### Proxy learning

Before selecting a visualization library: (1) distinguish "layout" (math) from "rendering" (SVG/Canvas) from "interaction" (drag, zoom, edit); (2) if layout already exists in Rust, rendering is just positioned shapes — ~100 lines of React SVG; (3) graph libraries charge their bundle cost for manipulation features the user may never need.

---

## Q010-P2 — Accessibility Strategy

**Pass:** 2
**Category:** accessibility
**Status:** accepted
**Confidence:** high

### Question

Accessibility strategy for the Explorer frontend?

### Evidence packet

- Q003-P1 outcome: Direct ARIA + Tailwind CSS 4 for all components except Spotter (cmdk)
- Q006-P1 outcome: React Testing Library (accessibility-role queries) + Playwright for E2E
- Q005-P1 outcome: No router; single-page in-memory navigation
- Miller Columns: NO WAI-ARIA precedent for cascading-column keyboard navigation
- Missing: No explicit accessibility strategy recorded

### Proxy answer

Roving tabindex across Miller Columns. listbox/option ARIA roles. Graph = complementary region. Tab/Shift+Tab between columns. Arrow keys within column. Enter to expand. Escape to close. axe-core via Playwright CI. WCAG 2.2 AA.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Roving tabindex across Miller Columns (Tab/Shift+Tab between columns, Arrow Up/Down within column, Enter to expand, Escape to close parent). listbox/option ARIA roles for column items. Graph view as complementary region (aria-label for screen readers). axe-core integrated into Playwright CI for automated audit. WCAG 2.2 AA target. No third-party accessibility library.

### Why this decision was made

Roving tabindex is the WAI-ARIA standard pattern for composite widgets with single active item — Miller Columns fit this model perfectly. listbox/option roles give screen readers the right semantics for selectable lists. Graph as complementary region is pragmatic — SVG graphs are inherently visual; providing alt text and keyboard navigation to raw data is the realistic AA path. axe-core + Playwright provides automated enforcement in CI. WCAG 2.2 AA is the industry standard compliance target. No library needed — these are all direct ARIA attribute patterns.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: yes — accessibility architecture is architecturally significant
- API: no
- Persistence: no
- Tests: yes — axe-core CI integration, keyboard navigation Playwright tests
- Security: no
- Observability: no

### Follow-up questions created

- How should the SVG graph be made accessible to screen reader users? (aria-label on graph region + off-screen data table?)
- Should keyboard shortcuts (e.g., `/` for Spotter) be documented in-app?

### Validation required

Yes. Miller Columns keyboard navigation has no WAI-ARIA precedent — must be validated with real screen reader users. Roving tabindex across cascading columns is novel interaction pattern.

### Proxy learning

Before designing accessibility: (1) Map the component tree to WAI-ARIA composite widget patterns (roving tabindex, listbox, tabs, breadcrumb); (2) Distinguish automated enforcement (axe-core CI) from manual validation (screen reader testing); (3) SVG graphs need pragmatic alt-text + keyboard-accessible data views — don't over-engineer inaccessible visualizations.

---

## Q011-P2 — Design Tokens

**Pass:** 2
**Category:** design-system
**Status:** accepted
**Confidence:** high

### Question

Design token strategy for the Explorer frontend?

### Evidence packet

- Q002-P1 outcome: Tailwind CSS v4 for styling
- Q012-P2 context: Dark-only MVP (no light mode initially)
- Missing: No explicit design token strategy recorded

### Proxy answer

Tailwind 4 @theme IS the token source. ~20 semantic tokens. No pipeline tool. SVG reads CSS custom properties directly. Dark-only MVP.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Tailwind CSS 4 `@theme` directive is the single source of truth for design tokens. ~20 semantic tokens (colors, spacing, radii, typography scale). No external token pipeline (Style Dictionary, Tokens Studio). SVG components read CSS custom properties directly via `var(--color-*)` for graph styling consistency. Dark-only MVP — light mode deferred.

### Why this decision was made

Tailwind CSS 4's `@theme` generates CSS custom properties natively — they are the tokens. Adding an external pipeline (Style Dictionary, Tokens Studio, Figma Tokens) adds tooling maintenance for zero benefit at MVP scale. ~20 tokens is small enough to manage as a single theme block. SVG components reading `var(--color-*)` directly ensures graph styling stays synchronized with the rest of the UI without a bridge layer. Dark-only defers the light mode design system until there's user demand — avoids building token infrastructure for a mode that doesn't exist yet.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: possible — design token strategy influences component authoring patterns
- API: no
- Persistence: no
- Tests: no
- Security: no
- Observability: no

### Follow-up questions created

- When should a token pipeline (Style Dictionary) be introduced? (success criterion: second platform or light mode ships)
- Should the ~20 tokens be documented as a design token catalog page?

### Validation required

No. Tailwind 4 @theme is the framework's built-in mechanism — zero additional infrastructure to validate.

### Proxy learning

Before proposing a token pipeline: (1) check if the CSS framework already generates tokens (Tailwind 4 @theme → CSS custom properties); (2) count the tokens — ~20 is manageable inline, 100+ justifies a pipeline; (3) SVG-to-CSS token bridge is solved by CSS custom properties — no intermediate format needed.

---

## Q012-P2 — Responsive Strategy

**Pass:** 2
**Category:** layout
**Status:** accepted
**Confidence:** high

### Question

Responsive strategy for the Explorer frontend?

### Evidence packet

- Q003-P1 outcome: Direct ARIA + Tailwind CSS 4 components
- Q005-P1 outcome: Single-page, in-memory navigation via Context
- Q009-P2 outcome: Custom SVG graph (not React Flow — full control over rendering)
- Missing: No explicit responsive strategy recorded

### Proxy answer

3 breakpoints: ≥1200px (3 cols + graph), 900-1200px (2 cols + graph toggle), <900px (single column drill-down). Horizontal scroll allowed. Min 768px. Container queries for column internals.

### Skeptic challenge

No challenge raised.

### Judge final answer

**ACCEPTED.** Three breakpoints: (1) ≥1200px — 3 Miller Columns + visible graph panel, (2) 900–1199px — 2 Miller Columns + graph toggle, (3) <900px — single column drill-down with graph accessible via toggle or overlay. Minimum supported width: 768px. Horizontal scroll permitted within columns for wide content (long paths, wide tables). Container queries for column-internal responsive behavior (card grid columns, list density). Tailwind 4 breakpoint utilities drive all breakpoints.

### Why this decision was made

Miller Columns are inherently horizontal — horizontal scroll is the natural overflow behavior, not a layout bug. Three breakpoints map to the three meaningful column configurations: full exploration (3 cols + graph), focused exploration (2 cols), and mobile drill-down (1 col). Container queries handle internal component adaptability without coupling to viewport width — a card grid inside a narrow column adjusts independently of the page layout. Min 768px is realistic for developer tools — the Explorer is a power-user interface, not a consumer landing page. Tailwind 4 breakpoints avoid custom media query management.

### Rejection trace (only when Status is rejected or modified)

(Not applicable — accepted without modification)

### Impact

- CONTEXT.md: yes
- ADR: no — breakpoint values are implementation detail
- API: no
- Persistence: no
- Tests: yes — responsive Playwright viewport tests
- Security: no
- Observability: no

### Follow-up questions created

- Should graph view be a toggle overlay or a resizable panel at 900–1199px?
- Should the <768px cut-off be a "best effort" view or a hard "unsupported screen size" message?

### Validation required

Yes. 768px minimum should be validated against target user device data. If significant tablet usage exists, single-column drill-down at 768px may be cramped.

### Proxy learning

Before designing responsive breakpoints: (1) map breakpoints to component configurations, not arbitrary pixel values; (2) horizontal scroll is NOT a bug for horizontally-oriented layouts (Miller Columns, wide tables); (3) Container queries isolate component responsiveness from page-level breakpoints.
