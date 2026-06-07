# Working Summary — Explorer Frontend Gaps

**Session:** 2026-06-07
**Final status:** COMPLETE (Pass 2 finished)
**Original goal:** Identify and resolve technology and architecture gaps in the Explorer frontend.

## Final coverage

| | Pass 1 | Pass 2 | Total |
|---|---|---|---|
| Cycles | 6 | 6 | 12 |
| Accepted | 2 (+1 user override) | 6 | 9 |
| Modified | 4 | 0 | 4 |
| Rejected | 0 | 0 | 0 |
| Gaps covered | 6/14 (46%) | 6/8 remaining | 12/14 (86%) |
| Remaining gaps | 8 | 2 deferred LOW | Asset pipeline, i18n |

## Inferred goal model

- **Goal:** Ship a production-ready Explorer frontend
- **Non-goal:** Extend existing Leptos dashboard
- **Constraint:** React 19 + TypeScript (ADR 0009)
- **Constraint:** Dashboard (Leptos) is independent product — no code sharing
- **Constraint:** WASM layer permitted for performance-critical operations (graph traversal, MoldQL, search indexing)
- **Constraint:** Component library justified only where genuine complexity exists (not per-project default)
- **Constraint:** State management follows minimize_dependencies pattern — lightest tool that covers requirement
- **Constraint:** Dark-only MVP (light mode deferred until user demand)

## Accepted decisions (compressed)

### Q001-P1: TypeScript strict mode for Explorer frontend
- React 19 + TypeScript strict mode, `.tsx`/`.ts`, clean rewrite from prototype
- Dashboard (Leptos) is separate application — no technology coupling
- **Override:** User overrode Judge's Leptos-extension recommendation
- **Confidence:** high | **Status:** accepted

### Q005-P1: Routing strategy
- No router for MVP — single-page in-memory via Context/useReducer
- Deep linking via vanilla `history.pushState` + `popstate` when needed
- **Confidence:** medium | **Status:** accepted

### Q006-P1: Testing strategy and tools
- Vitest + React Testing Library + Playwright + MSW
- Test pyramid: unit → component → E2E; >80% domain logic coverage
- No visual regression initially; reuse existing monorepo Playwright CI
- **Confidence:** high | **Status:** accepted

### Q007-P2: Loading States
- Three tiers: (1) WASM init — full-page Suspense spinner, (2) first fetch — placeholder + Tailwind pulse, (3) cache hit — instant
- SWR isLoading, React 19 Suspense, no skeleton library
- **Confidence:** high | **Status:** accepted

### Q008-P2: Error States
- Error Boundary per Miller Column (isolation) + app-level (last resort)
- SWR error/retry for API; React 19 built-in ErrorBoundary
- Fallback: message + "Try again" button
- **Confidence:** high | **Status:** accepted

### Q009-P2: Visualization Library
- Custom React SVG fed by WASM Sugiyama layout engine (cognicode-diagram)
- Zero graph rendering library (~0KB vs React Flow ~200KB)
- Navigation is click-to-expand — no graph manipulation needed
- **Confidence:** high | **Status:** accepted

### Q010-P2: Accessibility Strategy
- Roving tabindex across Miller Columns (Tab/Shift+Tab between columns, Arrow keys within, Enter to expand, Escape to close)
- listbox/option ARIA roles; graph = complementary region
- axe-core via Playwright CI; WCAG 2.2 AA target
- No third-party accessibility library
- **Confidence:** high | **Status:** accepted

### Q011-P2: Design Tokens
- Tailwind CSS 4 `@theme` is single source of truth — ~20 semantic tokens
- No external token pipeline (Style Dictionary, Tokens Studio)
- SVG reads CSS custom properties directly (`var(--color-*)`)
- Dark-only MVP — light mode deferred
- **Confidence:** high | **Status:** accepted

### Q012-P2: Responsive Strategy
- 3 breakpoints: ≥1200px (3 cols + graph), 900–1199px (2 cols + graph toggle), <900px (1 col drill-down)
- Min 768px; horizontal scroll permitted for Miller Columns; Container queries for internals
- **Confidence:** high | **Status:** accepted

## Modified decisions

### Q002-P1: Explorer frontend location and build tool
- **Location:** `apps/explorer-ui/` with Vite 6 + React 19 + TypeScript strict + Tailwind CSS 4
- **Package manager:** npm (not pnpm — stay consistent with root)
- **Types:** Generated to `apps/explorer-ui/src/types/generated/` (committed, `npm run gen-types`)
- **WASM:** Local path dep in package.json; no packages/ directory
- **No npm workspaces** until second app proves sharing is real
- **Confidence:** high | **Status:** modified (remedy: yagni_simplification)

### Q003-P1: Component library
- **cmdk** for Spotter (cmd+K overlay) — only third-party component dependency
- **Direct ARIA + Tailwind CSS 4** for all other 7 components (MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb)
- **No Radix UI.** cmdk vendors its own Radix Dialog internally.
- **Confidence:** high | **Status:** modified (remedy: minimize_dependencies)

### Q004-P1: State management + data fetching
- **Server state:** SWR (~4KB) for 7 API endpoints
- **Shared UI state:** React Context + useReducer for Miller Columns array (only shared slice)
- **Per-column state:** Local useState; **Spotter state:** cmdk-internal
- **No typed fetch wrapper, no Zustand, no Jotai**
- **Confidence:** high | **Status:** modified (remedy: minimize_dependencies)

## Rejected options

- Extending Leptos dashboard for Explorer pages (rejected by user override)
- JavaScript without TypeScript (rejected by proxy)
- pnpm switch (rejected by judge — unnecessary migration)
- `packages/` directory (rejected by judge — premature extraction)
- Radix UI primitives (rejected by judge — unnecessary for 7/8 components)
- shadcn/ui (rejected by proxy)
- TanStack Query (rejected by judge — 12KB vs 4KB SWR)
- Zustand, Jotai (rejected by judge — Context + useReducer sufficient)
- Typed fetch wrapper (rejected by judge — redundant)
- React Router / TanStack Router (rejected by proxy)
- React Flow / D3 / any graph rendering library (rejected — custom SVG sufficient)
- Skeleton libraries (rejected — Tailwind pulse animation sufficient)
- Style Dictionary / Tokens Studio (rejected — Tailwind 4 @theme sufficient)

## Open risks

1. **ts-rs fragility:** Community crate (5 contributors). Complex Rust enums may break TypeScript codegen.
2. **Build pipeline complexity:** cargo → ts-rs → tsc → vite. Acceptable if ts-rs stable.
3. **Two-frontend maintenance:** React Explorer + Leptos Dashboard. Clear separation helps, but shared concerns (auth, API) create coupling risk.
4. **Type staleness:** Generated types committed to repo. CI must regenerate on Rust DTO changes.
5. **Miller Columns keyboard nav:** No WAI-ARIA precedent for cascading-column focus model — must spike and validate with real users.
6. **cmdk Radix Dialog vendoring:** React 19 compatibility risk with vendored Radix Dialog.
7. **WASM state integration undefined:** How WASM graph data flows to React (SWR cache vs postMessage bridge).
8. **Deep linking deferred:** Vanilla history API may accumulate fragile URL management beyond MVP.
9. **MSW + SWR compatibility:** Theoretically sound, not yet verified.
10. **SVG performance at scale:** >500 node graphs may need Canvas fallback.
11. **768px minimum on tablets:** Single-column drill-down may be cramped on small tablets.

## Low-confidence areas

- ts-rs reliability for shared type generation
- Optimal build pipeline for Rust → TypeScript type flow
- CI strategy for type generation and commit
- Miller Columns focus management UX (needs spike)
- WASM-to-React state bridge pattern
- MSW + SWR compatibility
- Deep linking scalability beyond MVP
- SVG graph performance at 500+ nodes
- Tablet UX at 768px single-column mode

## Validation-required decisions

- **Q001-P1:** Document user override as ADR to prevent future stack-merging attempts
- **Q002-P1:** Backend port for Vite dev proxy needs user confirmation
- **Q003-P1/Q010-P2:** Miller Columns keyboard nav needs spike + screen reader validation
- **Q004-P1:** WASM integration path for state management needs definition
- **Q006-P1:** Confirm monorepo Playwright CI supports second test suite; verify MSW + SWR
- **Q009-P2:** Benchmark SVG performance at >500 nodes; plan Canvas fallback
- **Q012-P2:** Validate 768px minimum against target device data

## CONTEXT.md patch candidates

- Clarify Dashboard (Leptos) and Explorer (React) are separate independent applications
- Document `apps/explorer-ui/` as the canonical Explorer frontend location
- Document npm as the monorepo package manager (no pnpm)
- Document rule: `packages/` extraction requires ≥2 consumers
- Document component architecture: direct ARIA + Tailwind by default; external library only for genuinely complex components
- Document cmdk as the sole third-party component dependency (Spotter)
- Document state architecture: SWR for server state, Context + useReducer for shared UI, local useState per component
- Document minimize_dependencies as a cross-cutting heuristic
- Document routing: in-memory navigation via Context; no router for MVP; vanilla history API for deep linking
- Document testing: Vitest + RTL + Playwright + MSW; test pyramid; >80% domain logic; visual regression deferred
- Document three-tier loading: Suspense (WASM init) → SWR isLoading (first fetch + pulse) → instant (cache hit)
- Document error strategy: per-column ErrorBoundary + SWR retry + app-level fallback
- Document visualization: custom React SVG fed by WASM Sugiyama layout; zero graph library
- Document accessibility: roving tabindex + listbox/option + complementary region + axe-core CI + WCAG 2.2 AA
- Document design tokens: Tailwind 4 @theme as single source; SVG reads CSS custom properties directly
- Document responsive breakpoints: 1200px / 900px / 768px min; horizontal scroll permitted; container queries for internals

## ADR candidates

- **DRAFT-explorer-frontend-technology:** React 19 + TypeScript strict for Explorer; Leptos for Dashboard; independent stacks (from Q001-P1)
- **DRAFT-explorer-component-architecture:** cmdk for Spotter only; direct ARIA + Tailwind CSS 4 for all other components; no Radix UI (from Q003-P1)
- **DRAFT-explorer-visualization:** Custom React SVG + WASM Sugiyama layout; zero graph rendering library (from Q009-P2)
- **DRAFT-explorer-accessibility:** Roving tabindex + listbox/option + axe-core CI + WCAG 2.2 AA (from Q010-P2)
- **DRAFT-explorer-design-tokens:** Tailwind 4 @theme as single source; SVG reads CSS custom properties; dark-only MVP (from Q011-P2)

## Rejection patterns (cumulative — Pass 1 + Pass 2)

### Pattern 1: Premature extraction (Q002-P1)
- **Symptom:** Proxy proposed `packages/` directory + npm workspaces when only one consumer exists
- **Root cause:** Assuming future consumers justifies extraction infrastructure today
- **Heuristic:** Extract only when ≥2 real consumers exist, interface is stable, and versioning needs understood

### Pattern 2: Unjustified tool migration (Q002-P1)
- **Symptom:** Proxy proposed npm→pnpm switch without stated benefit
- **Root cause:** Did not check root tooling before proposing switch
- **Heuristic:** Read root config files first; articulate specific benefit; estimate migration cost

### Pattern 3: Default library without per-component analysis (Q003-P1)
- **Symptom:** Proxy proposed Radix UI for entire project without enumerating components by complexity
- **Root cause:** "Component library" treated as project-level default, not per-component decision
- **Heuristic:** Enumerate every component → classify by complexity → add deps only for genuinely complex ones. WAI-ARIA APG patterns are ~60 lines each.

### Pattern 4: Over-engineering state management (Q004-P1)
- **Symptom:** Proxy proposed TanStack Query (12KB) + Zustand when SWR (4KB) + Context covers all requirements
- **Root cause:** Defaulting to popular libraries; treating all UI state as needing a global store
- **Heuristic:** Enumerate state slices by category → start with lightest tool → distinguish "one shared slice" (Context) from "many cross-cutting" (Zustand/Jotai)

### Pattern 5: No Pass 2 rejection patterns — all 6 accepted clean

## Proxy learning points (cumulative)

- **Before technology-selection questions:** Search workspace for existing implementations first. Explain why separate stacks are intentional. _(Q001-P1)_
- **Before proposing directory structures:** Check existing monorepo tooling; ask "is the second consumer real or hypothetical?" _(Q002-P1)_
- **Before suggesting tool switches:** Read root config files first; articulate benefit and migration cost. _(Q002-P1)_
- **Before selecting component libraries:** Enumerate components → classify by complexity → add deps only for genuinely complex ones. WAI-ARIA APG covers standard patterns at ~60 lines each. _(Q003-P1)_
- **Before answering state management:** Enumerate state slices by category → start with lightest tool → Context for one shared slice, Zustand/Jotai for many cross-cutting. _(Q004-P1)_
- **Before selecting routing:** Count distinct pages/screens → distinguish URL-driven from in-memory view state → one page with dynamic views = no router needed. _(Q005-P1)_
- **Before designing testing strategy:** Check existing monorepo test tools → prefer ecosystem defaults → distinguish "needed now" from "defer to later." _(Q006-P1)_
- **Before designing loading states:** Enumerate distinct states → leverage framework built-ins (Suspense, SWR isLoading) → Tailwind pulse covers skeleton patterns. _(Q007-P2)_
- **Before designing error handling:** Error boundaries follow component hierarchy — per-feature for isolation, app-level for last resort. _(Q008-P2)_
- **Before selecting visualization:** Distinguish layout (math) from rendering (SVG) from interaction (drag/zoom) — if layout exists, rendering is ~100 lines of SVG. _(Q009-P2)_
- **Before designing accessibility:** Map components to WAI-ARIA patterns → distinguish automated (axe-core) from manual (screen reader) validation. _(Q010-P2)_
- **Before proposing token pipelines:** Check if CSS framework already generates tokens; ~20 tokens manageable inline; SVG-to-CSS bridge is CSS custom properties. _(Q011-P2)_
- **Before designing responsive breakpoints:** Map breakpoints to component configurations; horizontal scroll is NOT a bug for horizontal layouts; container queries isolate component responsiveness. _(Q012-P2)_

## Judge remedy patterns

| Remedy level | Count | Cycles |
|---|---|---|
| yagni_simplification | 1 | Q002-P1 |
| minimize_dependencies | 2 | Q003-P1, Q004-P1 |

## Follow-up question backlog

- Q?: How should shared domain types flow from Rust DTOs → TypeScript without ts-rs fragility?
- Q?: Should Explorer share infrastructure (auth, API client, deployment) with Dashboard?
- Q?: How should CI generate and commit TypeScript types from Rust DTOs?
- Q?: When is `packages/` extraction justified? (≥2 consumers)
- Q?: Should we spike Miller Columns keyboard navigation before committing?
- Q?: Does cmdk's internal Radix Dialog create React 19 version conflict risk?
- Q?: Global SWRConfig provider or per-hook configuration?
- Q?: How should WASM layer expose graph data to React? (SWR cache vs postMessage bridge)
- Q?: What revalidation strategy fits Explorer's real-time requirements?
- Q?: When should a router be introduced? (≥2 pages or URL-driven navigation)
- Q?: When should visual regression testing be introduced? (component architecture stabilizes)
- Q?: Should Playwright tests share CI workflow with Dashboard E2E?
- Q?: Should MSW handlers be shared between component and E2E tests?
- Q?: Should errors be reported to Sentry/Datadog from error boundaries?
- Q?: What zoom/pan strategy for SVG graph viewport?
- Q?: How should edge routing (curves, orthogonal) be handled? (Rust layout or React SVG)
- Q?: How to make SVG graph accessible to screen readers? (aria-label + off-screen data table?)
- Q?: Should keyboard shortcuts be documented in-app?
- Q?: When should a token pipeline be introduced? (second platform or light mode)
- Q?: Should ~20 tokens be documented as a design token catalog page?
- Q?: Graph toggle overlay or resizable panel at 900–1199px?
- Q?: <768px: "best effort" view or hard "unsupported" message?
- Q?: Asset pipeline strategy for Explorer frontend? (DEFERRED LOW)
- Q?: i18n / internationalization strategy? (DEFERRED LOW)

## Coverage status

- **Passes completed:** 2
- **Decisions recorded:** 12 (8 accepted + 4 modified)
- **ADR drafts:** 5
- **Follow-up pending:** 24
- **Remaining gaps:** 2 (asset pipeline, i18n — deferred LOW priority)
