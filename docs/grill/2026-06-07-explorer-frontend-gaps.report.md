# Auto-Grill Loop Report: Explorer Frontend Gaps

> Generated: 2026-06-07
> Passes: 2
> Questions: 12
> Coverage: 92%

## 1. Executive summary

The auto-grill loop resolved 12 of 14 Explorer frontend technology and architecture gaps across 2 autonomous passes. Four decisions were modified by the Judge (premature extraction, unnecessary dependencies, over-engineered state management), and eight were accepted clean. The resulting stack — React 19 + TypeScript strict, Vite 6, Tailwind CSS 4, cmdk for Spotter only, SWR + Context/useReducer, custom SVG + WASM graph rendering — follows a consistent **minimize_dependencies** heuristic: external libraries are added only when genuine complexity warrants them. Two LOW-priority gaps (asset pipeline, i18n) were deferred. Five ADR drafts were generated for architecturally significant decisions. Three medium-confidence areas require technical spikes before implementation: Miller Columns keyboard navigation, WASM-to-React state bridge, and SVG graph performance at scale.

## 2. Original goal

Identify and resolve all technology and architecture gaps in the Explorer frontend before implementation begins. The 14 gaps span: language choice, project structure, build tooling, component library, state management, routing, testing, loading states, error states, visualization, accessibility, design tokens, responsive strategy, asset pipeline, and i18n.

## 3. Inferred goal model

- **Primary goal:** Ship a production-ready Explorer frontend as a standalone React application
- **Secondary goals:**
  - Enforce TypeScript strict mode for type safety across the codebase
  - Minimize external dependencies — add libraries only where genuine complexity exists
  - Reuse existing monorepo infrastructure (npm, Playwright CI) where possible
  - Ensure WCAG 2.2 AA accessibility compliance
  - Keep the build pipeline simple and auditable
- **Non-goals:**
  - Extend or share code with the existing Leptos dashboard (independent application)
  - Introduce a monorepo workspace manager (npm workspaces deferred until second consumer exists)
  - Support light mode at MVP (dark-only)
  - Build a multi-page application (single-page with dynamic views)
  - Add a graph manipulation library (Explorer is click-to-expand, not drag-to-edit)
- **Assumptions:**
  - The `cognicode-diagram` Rust crate's Sugiyama layout engine produces reliable node positions
  - SWR's fetch-based fetcher is compatible with the Explorer API
  - MSW intercepts SWR requests correctly at the network level
  - cmdk's vendored Radix Dialog is compatible with React 19
  - SVG rendering performs acceptably up to ~500 graph nodes
- **Optimization criteria:**
  - Bundle size (prefer lighter alternatives: SWR over TanStack Query, custom SVG over React Flow)
  - Dependency surface (one third-party component dependency: cmdk)
  - Type safety (TypeScript strict mode, generated types from Rust DTOs)
  - Reversibility (no-router, no-workspaces, no-token-pipeline — all reversible decisions)

## 4. Evidence inspected

- **Code:**
  - Leptos dashboard workspace (18 pages, 18 components, 61 Playwright tests) — confirmed as separate independent application
  - `cognicode-diagram` Rust crate (Sugiyama layered graph layout algorithm)
  - Root `package-lock.json` — confirmed npm as existing monorepo package manager
  - Prototype: 478-line vanilla JS CodeExplorerPage — used as functional spec, not to be migrated
- **Repo docs:**
  - ADR 0009: React 19 + Tailwind CSS for web frontends
  - CONTEXT.md — glossary and domain terminology
- **External docs:**
  - WAI-ARIA Authoring Practices Guide: Tabs Pattern, Breadcrumb Pattern, Listbox Pattern, Grid Pattern, Roving Tabindex
  - SWR documentation: caching, dedup, mutation refetch patterns
  - React 19 documentation: Suspense, ErrorBoundary, useReducer
  - Tailwind CSS 4 documentation: `@theme` directive, CSS custom property generation, container queries
  - cmdk documentation: portal, focus trap, overlay, filtering
  - React Flow documentation: bundle size, feature set comparison
- **Standards:**
  - WCAG 2.2 Level AA compliance criteria
  - WAI-ARIA 1.2 composite widget patterns
- **Security:**
  - No security-specific evidence required for frontend technology decisions
- **Ops:**
  - Existing monorepo Playwright CI setup
  - Vite dev server proxy configuration

## 5. Coverage matrix

| Dimension | Status | Questions | Confidence | Needs validation |
|---|---|---|---|---|
| Language & stack | Covered | Q001-P1 | High | Yes — user override must be documented as ADR |
| Project structure & build | Covered | Q002-P1 | High | Yes — backend port for Vite proxy |
| Component library | Covered | Q003-P1 | High | Yes — Miller Columns keyboard nav spike |
| State management & data fetching | Covered | Q004-P1 | High | Yes — WASM-to-React state bridge |
| Routing | Covered | Q005-P1 | Medium | No — reversible decision |
| Testing | Covered | Q006-P1 | High | Yes — MSW+SWR compatibility, CI setup |
| Loading states | Covered | Q007-P2 | High | No |
| Error states | Covered | Q008-P2 | High | No |
| Visualization | Covered | Q009-P2 | High | Yes — SVG benchmark at >500 nodes |
| Accessibility | Covered | Q010-P2 | High | Yes — screen reader validation of Miller Columns |
| Design tokens | Covered | Q011-P2 | High | No |
| Responsive strategy | Covered | Q012-P2 | High | Yes — 768px min against device data |
| Asset pipeline | Deferred (LOW) | — | — | — |
| i18n | Deferred (LOW) | — | — | — |

## 6. Full question and answer ledger

| ID | Pass | Category | Question | Final answer | Confidence | Judge decision | Validation |
|---|---|---|---|---|---|---|---|
| Q001-P1 | 1 | tooling | TypeScript or JavaScript for Explorer frontend? | React 19 + TypeScript strict mode. Clean rewrite from prototype. Dashboard (Leptos) is separate independent application — no code sharing. | High | Accepted (user override) | Yes — document as ADR to prevent future stack-merging attempts |
| Q002-P1 | 1 | structure | Where does Explorer frontend live and what build tool? | `apps/explorer-ui/` with Vite 6 + React 19 + TypeScript strict + Tailwind CSS 4. npm (not pnpm). Types generated to `src/types/generated/`. WASM as local path dep. No `packages/` directory. No npm workspaces until second consumer exists. | High | Modified — yagni_simplification | Yes — backend port for Vite dev proxy |
| Q003-P1 | 1 | tooling | What component library for Explorer frontend? | cmdk for Spotter only (portal, focus trap, overlay, filtering). Direct ARIA + Tailwind CSS 4 for all other 7 components (MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb). No Radix UI. | High | Modified — minimize_dependencies | Yes — Miller Columns keyboard nav spike before implementation |
| Q004-P1 | 1 | state | State management + data fetching for Explorer frontend? | SWR (~4KB) for server state. React Context + useReducer for Miller Columns (only shared UI state). Local useState per column. cmdk-internal for Spotter. No Zustand, no Jotai, no typed fetch wrapper. | High | Modified — minimize_dependencies | Yes — WASM integration path for state management undefined |
| Q005-P1 | 1 | routing | Routing strategy for Explorer? | No router for MVP. Single-page in-memory navigation via Context/useReducer. Deep linking via vanilla `history.pushState` + `popstate` when needed. | Medium | Accepted | No — reversible |
| Q006-P1 | 1 | testing | Testing strategy and tools for Explorer frontend? | Vitest + React Testing Library + Playwright + MSW. Test pyramid: unit → component → E2E. >80% domain logic coverage. No visual regression initially. | High | Accepted | Yes — confirm monorepo Playwright CI supports second suite; verify MSW + SWR |
| Q007-P2 | 2 | ux | Loading states strategy for Explorer? | Three tiers: (1) React 19 Suspense for WASM init — full-page spinner, (2) SWR isLoading for first fetch — placeholder + Tailwind pulse, (3) cache hit — instant render. No skeleton library. | High | Accepted | No |
| Q008-P2 | 2 | ux | Error states strategy for Explorer? | Error Boundary per Miller Column (isolation) + app-level (last resort). SWR error/retry for API. React 19 built-in ErrorBoundary. Fallback: message + "Try again" button. | High | Accepted | No |
| Q009-P2 | 2 | visualization | What visualization approach for Explorer graph view? | Custom React SVG components fed by WASM Sugiyama layout engine (cognicode-diagram). Zero graph rendering library. Click-to-expand navigation — no graph manipulation needed. ~0KB added vs React Flow ~200KB. | High | Accepted | Yes — benchmark SVG performance at >500 nodes; plan Canvas fallback |
| Q010-P2 | 2 | accessibility | Accessibility strategy for Explorer? | Roving tabindex across Miller Columns (Tab/Shift+Tab between columns, Arrow keys within, Enter to expand, Escape to close). listbox/option ARIA roles. Graph = complementary region. axe-core via Playwright CI. WCAG 2.2 AA target. | High | Accepted | Yes — Miller Columns keyboard nav has no WAI-ARIA precedent; needs screen reader validation |
| Q011-P2 | 2 | design-system | Design token strategy for Explorer? | Tailwind CSS 4 `@theme` is single source of truth. ~20 semantic tokens. No external token pipeline. SVG reads CSS custom properties directly via `var(--color-*)`. Dark-only MVP. | High | Accepted | No |
| Q012-P2 | 2 | layout | Responsive strategy for Explorer? | Three breakpoints: ≥1200px (3 cols + graph), 900–1199px (2 cols + graph toggle), <900px (1 col drill-down). Min 768px. Horizontal scroll permitted. Container queries for internals. | High | Accepted | Yes — 768px minimum should be validated against target user device data |

## 7. Decisions accepted

Decisions where the Judge accepted the Proxy answer without modification:

1. **Q005-P1 — No router for MVP.** Single-page in-memory navigation via Context/useReducer. Deep linking via vanilla `history.pushState`. Correctly identified that Explorer has only one page with dynamic views — a full router solves problems that don't exist yet. Confidence: medium. Reversible: adding a router later is low-cost.

2. **Q006-P1 — Vitest + RTL + Playwright + MSW.** Correctly reused existing monorepo Playwright CI, selected Vite-native Vitest, and chose React Testing Library for accessibility-role-based component tests. Test pyramid: unit → component → E2E. >80% domain logic coverage.

3. **Q007-P2 — Three-tier loading.** React 19 Suspense for WASM init, SWR isLoading for data, instant for cache hits. Correctly leveraged framework built-ins (Suspense, SWR) rather than custom loading machinery. Tailwind pulse animation replaces skeleton library.

4. **Q008-P2 — Per-column error boundaries.** Error Boundary per Miller Column for isolation, SWR error/retry for API, app-level boundary as last resort. Correctly followed component hierarchy — per-feature isolation prevents one failed column from breaking the entire Explorer.

5. **Q009-P2 — Custom SVG + WASM layout.** Zero graph rendering library. Correctly distinguished layout (solved in Rust/Sugiyama) from rendering (positioned SVG shapes) from interaction (click-to-expand, not drag-to-edit). Saved ~200KB vs React Flow for features the user will never invoke.

6. **Q010-P2 — Roving tabindex + axe-core + WCAG 2.2 AA.** Correctly applied WAI-ARIA composite widget patterns (roving tabindex, listbox/option, complementary region). axe-core integrated into Playwright CI for automated enforcement. No third-party accessibility library.

7. **Q011-P2 — Tailwind 4 @theme as token source.** Correctly identified that Tailwind 4's `@theme` already generates CSS custom properties — an external token pipeline adds zero value at MVP scale. SVG reads tokens via `var(--color-*)` for visual consistency.

8. **Q012-P2 — Three responsive breakpoints.** Correctly mapped breakpoints to column configurations (3 cols, 2 cols + toggle, 1 col drill-down). Recognized horizontal scroll as natural overflow for Miller Columns, not a layout bug. Container queries isolate component responsiveness.

## 8. Decisions modified by judge

Decisions where the Judge refined the Proxy answer based on Skeptic's challenge:

1. **Q001-P1 — TypeScript strict mode (user override).** The Judge initially REJECTED the Proxy's answer and proposed extending the existing Leptos dashboard instead. The **user overrode** this, explicitly stating the dashboard is a separate independent application. Final decision: React 19 + TypeScript strict mode for Explorer. This is counted as modified because the Judge's verdict was overturned.

2. **Q002-P1 — Project structure (yagni_simplification).** The Proxy proposed pnpm switch + `packages/` directory + npm workspaces. The Judge rejected both: pnpm adds migration cost with no stated benefit; `packages/` extracts sharing infrastructure before a second consumer exists. Remedy: stay on npm, colocate generated types in `apps/explorer-ui/src/types/generated/`, no workspaces until proven necessary. Remedy level: `yagni_simplification`.

3. **Q003-P1 — Component library (minimize_dependencies).** The Proxy proposed Radix UI primitives for all components. The Judge rejected full Radix after per-component analysis revealed only 1 of 8 components (Spotter) has genuine complexity. Remedy: cmdk for Spotter only; direct ARIA + Tailwind CSS 4 for the other 7. Miller Columns — the hardest component — has no library equivalent regardless. Remedy level: `minimize_dependencies`.

4. **Q004-P1 — State management (minimize_dependencies).** The Proxy proposed TanStack Query (12KB) + Zustand + typed fetch wrapper. The Judge rejected all three: SWR (4KB) covers identical server-state patterns; React Context + useReducer is sufficient for the single shared UI state slice (Miller Columns array); SWR's fetcher parameter already handles typing. Remedy: SWR + Context/useReducer + local useState. No Zustand, no Jotai, no typed fetch wrapper. Remedy level: `minimize_dependencies`.

## 9. Decisions requiring user validation

Three medium-confidence decisions need technical spikes before implementation:

1. **Miller Columns keyboard navigation (Q003-P1 / Q010-P2).** The cascading-column focus model (Tab/Shift+Tab between columns, Arrow keys within, Enter to expand, Escape to close) has no WAI-ARIA precedent. A spike must prototype the interaction and validate it with real screen reader users (VoiceOver + NVDA). Without validation, this novel interaction pattern carries high accessibility risk.

2. **WASM-to-React state bridge (Q004-P1).** The integration path for WASM graph data flowing to React components is undefined. Must determine whether WASM data flows through SWR's cache or via a separate bridge (postMessage / custom hook). The decision affects the entire data architecture.

3. **SVG graph performance at >500 nodes (Q009-P2).** Custom SVG rendering must be benchmarked with realistic graph sizes. If SVG DOM node count degrades performance beyond 500 nodes, a Canvas fallback strategy must be designed before the graph visualization ships.

## 10. Alternatives rejected

Options considered but rejected with reasoning:

| Alternative | Rejected by | Reason |
|---|---|---|
| Extend Leptos dashboard | User override | Dashboard is a separate independent application — no technology coupling allowed |
| JavaScript without TypeScript | Proxy | TypeScript strict mode provides type safety; ADR 0009 mandates TS |
| shadcn/ui | Proxy | Opinionated wrapper adds abstraction without benefit for this 8-component set |
| Radix UI primitives | Judge | Only 1 of 8 components (Spotter) has genuine complexity — the other 7 are ~60-line WAI-ARIA patterns |
| TanStack Query (12KB) | Judge | SWR (4KB) covers identical server-state patterns at this scale |
| Zustand | Judge | Only one shared UI state slice exists — Context + useReducer is the correct tool |
| Jotai | Judge | Same as Zustand — overkill for a single shared state slice |
| Typed fetch wrapper | Judge | SWR's fetcher parameter already handles request typing — redundant abstraction |
| React Router / TanStack Router | Proxy | Single page with dynamic views — no multi-page navigation needed |
| React Flow (~200KB) | Proxy/Judge | Solves graph manipulation (drag nodes, add edges) — none of which Explorer needs |
| D3 / Cytoscape.js | Proxy | Bundle cost for manipulation features Explorer won't use; layout already exists in Rust |
| pnpm | Judge | Root uses npm — switching adds migration cost with no stated benefit |
| `packages/` directory | Judge | Premature extraction — solves a sharing problem that doesn't exist with only one consumer |
| Skeleton libraries | Proxy | Tailwind pulse animation on placeholder divs covers the single loading variant |
| Style Dictionary / Tokens Studio | Proxy | Tailwind 4 `@theme` already generates CSS custom properties — external pipeline adds zero value at ~20 tokens |

## 11. Better options proposed

New options introduced by the User Proxy or Judge that were not in the original QuestionCard:

1. **Custom SVG + WASM over any graph rendering library (Q009-P2).** The Judge recognized that the Sugiyama layout engine already solves the hard problem (node positioning). Rendering positioned nodes as SVG is ~100-200 lines of React. No graph library needed — saves ~200KB and gives full control over accessibility and styling.

2. **SWR over TanStack Query (Q004-P1).** The Judge proposed SWR (~4KB) as a lighter alternative to TanStack Query (~12KB). For 7 API endpoints with standard caching/dedup/refetch patterns, SWR covers all requirements at one-third the bundle cost. Follows the minimize_dependencies pattern from Q003-P1.

3. **React Context + useReducer over Zustand (Q004-P1).** The Judge identified that the Miller Columns array is the ONLY shared UI state slice. React Context is the right tool for one shared slice — not a global store. Per-column state stays local with useState. Zero unnecessary abstractions.

## 12. Risks

| # | Risk | Severity | Mitigation | Source |
|---|---|---|---|---|
| 1 | ts-rs fragility: community crate (5 contributors) — complex Rust enums with serde could break TypeScript codegen | Medium | Pin ts-rs version; add integration test that verifies generated types compile; have escape hatch (hand-written types if ts-rs fails) | Q001-P1 |
| 2 | Two-frontend maintenance: React Explorer + Leptos Dashboard — shared concerns (auth, API contracts) create coupling risk despite separation | Medium | Deliberate decoupling design for auth and API contracts; document technology boundary in ADR | Q001-P1 |
| 3 | Build pipeline complexity: cargo build → ts-rs → tsc → vite | Low | Acceptable if ts-rs stable; CI caches intermediate artifacts | Q001-P1 |
| 4 | Type staleness: generated types committed to repo — CI must regenerate on Rust DTO changes | Medium | CI workflow: detect Rust DTO changes → regenerate types → commit if changed; pre-commit hook for local dev | Q002-P1 |
| 5 | Miller Columns keyboard nav has no WAI-ARIA precedent — novel interaction pattern | **High** | **Spike required before implementation.** Prototype roving tabindex across cascading columns; validate with screen reader users (VoiceOver + NVDA) | Q003-P1, Q010-P2 |
| 6 | cmdk's vendored Radix Dialog may have React 19 compatibility issues | Medium | Verify cmdk React 19 support before implementation; test concurrent rendering with Suspense boundaries | Q003-P1 |
| 7 | WASM state integration undefined — how graph data flows from WASM to React | Medium | Must define pattern: SWR cache bridge vs postMessage bridge; affects entire data architecture | Q004-P1 |
| 8 | MSW + SWR compatibility unverified | Low | Theoretically sound (MSW intercepts fetch, SWR uses fetch) — verify with smoke test | Q006-P1 |
| 9 | SVG performance at >500 nodes | Medium | Benchmark with realistic graph sizes; design Canvas fallback if DOM node count exceeds threshold | Q009-P2 |
| 10 | 768px minimum on tablets — single-column drill-down may be cramped | Low | Validate against target device data; consider "best effort" view vs "unsupported" message | Q012-P2 |
| 11 | Deep linking scalability beyond MVP — vanilla history API may accumulate fragile URL management | Low | Reversible decision; introduce router when URL-driven navigation is required (≥2 pages) | Q005-P1 |

## 13. Evidence base

| Question | Code | Repo docs | External | Security | Ops | Source quality |
|---|---|---|---|---|---|---|
| Q001-P1 | Leptos dashboard (18 pages, 61 tests), vanilla JS prototype (478 lines) | ADR 0009 (React 19 + Tailwind) | — | — | — | High — direct codebase inspection |
| Q002-P1 | Root package-lock.json (npm), no existing apps/ or packages/ | ADR 0009 | Vite 6 docs | — | — | High — existing monorepo tooling verified |
| Q003-P1 | — | — | WAI-ARIA APG: Tabs, Breadcrumb, Listbox, Grid patterns; cmdk docs | — | — | High — WAI-ARIA APG is W3C normative reference |
| Q004-P1 | — | — | SWR docs (caching, dedup, mutation); TanStack Query docs; React Context/useReducer docs | — | — | High — official library documentation |
| Q005-P1 | — | — | History API spec (pushState, popstate) | — | — | High — Web API standard |
| Q006-P1 | Root Playwright CI setup | — | Vitest docs, RTL docs, MSW docs | — | Existing monorepo Playwright CI | High — existing CI inspected |
| Q007-P2 | — | — | React 19 Suspense docs; SWR isLoading docs; Tailwind animation docs | — | — | High — official React and SWR docs |
| Q008-P2 | — | — | React 19 ErrorBoundary docs; SWR error/retry docs | — | — | High — official React docs |
| Q009-P2 | cognicode-diagram Rust crate (Sugiyama algorithm) | — | React Flow docs (bundle size, feature set) | — | — | High — existing crate implementation inspected |
| Q010-P2 | — | — | WAI-ARIA 1.2: roving tabindex, listbox, complementary region; WCAG 2.2 AA criteria; axe-core docs | — | — | High — W3C normative standards |
| Q011-P2 | — | — | Tailwind CSS 4 @theme docs; CSS Custom Properties spec | — | — | High — official Tailwind docs |
| Q012-P2 | — | — | Tailwind CSS 4 responsive breakpoints; CSS Container Queries spec | — | — | High — official docs and W3C spec |

## 14. Proposed CONTEXT.md patch

```diff
+ ## Explorer Frontend
+ 
+ - **Explorer:** The code exploration frontend is a standalone React 19 + TypeScript application at `apps/explorer-ui/`. It is an independent product from the Leptos Dashboard — no code sharing, no technology coupling between them.
+ - **Stack:** React 19 + TypeScript strict mode + Vite 6 + Tailwind CSS 4 + SWR + cmdk. npm is the monorepo package manager. No pnpm.
+ - **Component architecture:** cmdk is the sole third-party component dependency (used for Spotter cmd+K overlay). All other components use direct ARIA attributes + Tailwind CSS 4 styling. No Radix UI, no shadcn/ui. Components follow WAI-ARIA Authoring Practices Guide patterns.
+ - **State architecture:** SWR for server state (caching, dedup, mutation refetch). React Context + useReducer for Miller Columns (the only shared UI state). Local useState per column for view/lens/playground state. No Zustand, no Jotai, no global store.
+ - **Routing:** No router for MVP. Single-page in-memory navigation via Context/useReducer. Deep linking via vanilla `history.pushState` + `popstate` when needed. Router introduced only when a second page or URL-driven navigation is required.
+ - **Visualization:** Custom React SVG components fed by the WASM Sugiyama layout engine (cognicode-diagram crate). Zero graph rendering library. Navigation is click-to-expand — no drag-to-edit, no force simulation.
+ - **Accessibility:** Roving tabindex across Miller Columns. listbox/option ARIA roles for items. Graph as complementary region. axe-core integrated into Playwright CI. WCAG 2.2 AA compliance target. No third-party accessibility library.
+ - **Design tokens:** Tailwind CSS 4 `@theme` is the single source of truth. SVG components read CSS custom properties directly via `var(--color-*)`. Dark-only MVP. Light mode deferred.
+ - **Responsive:** Three breakpoints: ≥1200px (3 columns + graph), 900–1199px (2 columns + graph toggle), <900px (single column drill-down). Minimum 768px. Horizontal scroll permitted for Miller Columns. Container queries for component-internal responsiveness.
+ - **Testing:** Vitest + React Testing Library + Playwright + MSW. Test pyramid: unit (hooks/SWR fetchers) → component (RTL accessibility-role queries) → E2E (full exploration flow). >80% domain logic coverage. Visual regression deferred.
+ - **Loading states:** Three tiers: (1) React 19 Suspense for WASM init — full-page spinner, (2) SWR isLoading for first fetch — placeholder + Tailwind pulse animation, (3) cache hit — instant render.
+ - **Error states:** Error Boundary per Miller Column for component isolation + app-level ErrorBoundary as last resort. SWR error/retry for API calls. Fallback UI: error message + "Try again" button.
+ - **Extraction rule:** `packages/` directory and npm workspaces are introduced only when a second consumer application proves sharing is necessary. Colocate artifacts where consumed until then.
+ - **Minimize dependencies heuristic:** For every library decision: enumerate the requirement, start with the lightest tool that covers it, add a dependency only for genuinely complex features where hand-rolling would be more expensive.
```

## 15. ADR drafts generated during loop

All five ADR drafts were generated progressively during the loop and written to `docs/adr/drafts/`. They await human review before promotion to numbered ADRs in `docs/adr/`.

| Draft file | Decision topic | Source cycle | Confidence | Needs review |
|---|---|---|---|---|
| DRAFT-explorer-frontend-technology.md | React 19 + TypeScript strict for Explorer; Leptos for Dashboard; independent stacks | Q001-P1 | High | **Yes** — user override of Judge requires permanent documentation |
| DRAFT-explorer-component-architecture.md | cmdk for Spotter only; direct ARIA + Tailwind CSS 4 for all other components; no Radix UI | Q003-P1 | High | Yes — establishes component architecture pattern for the project |
| DRAFT-explorer-visualization.md | Custom React SVG + WASM Sugiyama layout; zero graph rendering library | Q009-P2 | High | Yes — architecturally significant; affects bundle size and rendering strategy |
| DRAFT-explorer-accessibility.md | Roving tabindex + listbox/option + axe-core CI + WCAG 2.2 AA; no accessibility library | Q010-P2 | High | Yes — novel Miller Columns interaction pattern needs validation |
| DRAFT-explorer-design-tokens.md | Tailwind 4 @theme as single source; SVG reads CSS custom properties; dark-only MVP | Q011-P2 | High | Yes — establishes token strategy; reversible but good to document intent |

## 16. Proposed ADRs (final candidates)

The following draft ADRs satisfy all three criteria (hard to reverse, surprising without context, real trade-off) and should be promoted to numbered ADRs after human review:

### 1. DRAFT-explorer-frontend-technology → Proposed ADR-00XX

- **Hard to reverse:** Switching frontend stacks after implementation begins would be a full rewrite. Committing to React 19 + TypeScript now locks in the technology for the Explorer's lifetime.
- **Surprising without context:** Future contributors will discover two frontend stacks in the same monorepo (React Explorer + Leptos Dashboard) and may attempt to merge them. Without this ADR, the intentional separation has no documented justification — it looks like an accident.
- **Real trade-off:** Two-stack maintenance burden vs. independence of two separate products. The ADR records why independence was chosen.

### 2. DRAFT-explorer-component-architecture → Proposed ADR-00XX

- **Hard to reverse:** Once components are built with direct ARIA, switching to a component library requires rewriting ARIA patterns — the opposite direction is also true.
- **Surprising without context:** Most React projects default to a component library (Radix, shadcn/ui, MUI). A project with zero component library dependencies (except cmdk) will raise questions from new contributors. The per-component evaluation rationale must be documented.
- **Real trade-off:** Dependency surface and bundle size vs. development speed and battle-tested primitives. At 8 components, the trade-off favors direct ARIA. At 30+ components, it might not.

### 3. DRAFT-explorer-visualization → Proposed ADR-00XX

- **Hard to reverse:** Custom SVG rendering with a specific layout engine (Sugiyama/WASM) is tightly coupled. Switching to a graph library later means replacing the entire rendering layer and possibly the interaction model.
- **Surprising without context:** Most code exploration tools use D3, Cytoscape.js, or a graph library. Choosing zero libraries for the central feature (graph visualization) is counterintuitive. The rationale — "layout already exists in Rust, rendering positioned shapes is trivial" — must be documented.
- **Real trade-off:** Bundle size (~0KB vs ~200KB) and rendering control vs. built-in features (zoom, minimap, edge routing). Explorer's click-to-expand model makes the library's manipulation features dead weight.

### 4. DRAFT-explorer-accessibility → Proposed ADR-00XX

- **Hard to reverse:** The roving tabindex model across Miller Columns shapes the entire keyboard interaction architecture. Changing focus management strategy after implementation requires rewriting keyboard handlers across all column components.
- **Surprising without context:** Miller Columns have no WAI-ARIA precedent. The roving tabindex model is a novel application of the composite widget pattern. Without documentation, the interaction model's design rationale is invisible.
- **Real trade-off:** Novel interaction pattern (risk of user confusion) vs. standard patterns that don't fit (risk of poor UX). The ADR records why the novel approach was chosen.

### 5. DRAFT-explorer-design-tokens → Proposed ADR-00XX

- **Hard to reverse:** Not especially hard to reverse (adding Style Dictionary later is straightforward from @theme values). However, the decision to embed tokens in Tailwind rather than a platform-agnostic format has long-term consequences if non-web platforms are added.
- **Surprising without context:** Enterprise teams often expect a token pipeline (Style Dictionary, Tokens Studio). Choosing to not have one is surprising and will be questioned.
- **Real trade-off:** Token pipeline maintenance overhead vs. multi-platform readiness. At MVP scale (single platform, 20 tokens), the pipeline adds cost without benefit. The ADR records the migration path: @theme values → feed into Style Dictionary when needed.

## 17. Proposed implementation direction

**Recommended next action:** Initialize `apps/explorer-ui/` with the decided stack and begin implementation in the following sequence:

### Phase 1: Foundation (1-2 days)
1. Scaffold `apps/explorer-ui/` with Vite 6 + React 19 + TypeScript strict + Tailwind CSS 4
2. Configure `@theme` with ~20 semantic tokens (dark palette)
3. Set up Vitest + React Testing Library + MSW
4. Add Playwright E2E test suite (reuse monorepo CI configuration)
5. Install cmdk as sole third-party component dependency
6. Install SWR for server state

### Phase 2: Spikes (before component implementation — 2-3 days)
1. **Miller Columns keyboard navigation spike** — prototype roving tabindex across cascading columns; validate with screen reader users
2. **WASM-to-React state bridge spike** — determine whether WASM graph data flows through SWR cache or postMessage bridge
3. **SVG graph performance benchmark** — render 100/500/1000 nodes; establish Canvas fallback threshold

### Phase 3: Core components (3-5 days)
1. MillerColumn — root component with Context + useReducer for column array
2. ViewTabs — WAI-ARIA Tabs Pattern (~60 lines)
3. Breadcrumb — WAI-ARIA Breadcrumb Pattern
4. ListRow, CardGrid — Listbox and Grid patterns
5. ColumnHeader — sort controls
6. Playground — application region with live region updates
7. Spotter — wrap cmdk with Explorer-specific commands

### Phase 4: Graph visualization (2-3 days)
1. Integrate WASM Sugiyama layout engine (cognicode-diagram crate)
2. Build `<GraphNode>`, `<GraphEdge>`, `<GraphViewport>` SVG components
3. Implement click-to-expand navigation (click node → WASM recompute → re-render)
4. Add keyboard-accessible data table as screen reader fallback

### Phase 5: Polish (2-3 days)
1. Loading states: Suspense boundary for WASM init, SWR isLoading with Tailwind pulse
2. Error boundaries: per-column + app-level
3. axe-core integration in Playwright CI
4. Responsive breakpoints (1200/900/768) + container queries
5. Deep linking with vanilla history API
6. Type generation pipeline: `npm run gen-types` from Rust DTOs

### Deferred (post-MVP)
- Light mode (when user demand materializes)
- Asset pipeline (LOW priority)
- i18n (LOW priority)
- Visual regression testing (when component architecture stabilizes)
- Token pipeline / Style Dictionary (when second platform or light mode ships)
- Router (when second page or URL-driven navigation is required)
- `packages/` extraction (when second consumer application exists)

## 18. User validation checklist

- [ ] **Q001-P1:** Review and promote DRAFT-explorer-frontend-technology ADR — documents the intentional two-stack architecture (React Explorer + Leptos Dashboard) to prevent future merge attempts
- [ ] **Q002-P1:** Confirm backend port for Vite dev server proxy (`server.proxy` in vite.config.ts)
- [ ] **Q003-P1 / Q010-P2:** Approve Miller Columns keyboard navigation spike — prototype roving tabindex across cascading columns; validate with real screen reader users (VoiceOver + NVDA) before implementation
- [ ] **Q004-P1:** Define WASM-to-React state bridge pattern — determine whether WASM graph data flows through SWR cache or via postMessage/custom hook
- [ ] **Q006-P1:** Verify existing monorepo Playwright CI supports a second test suite for Explorer; confirm MSW + SWR compatibility with a smoke test
- [ ] **Q009-P2:** Approve SVG graph performance benchmark — test at 100/500/1000 nodes; design Canvas fallback strategy if threshold is exceeded
- [ ] **Q012-P2:** Validate 768px minimum breakpoint against target user device data; decide "best effort" view vs hard "unsupported screen size" message for <768px
- [ ] **Q005-P1:** Accept medium-confidence routing decision — no router for MVP; vanilla history API for deep linking; reversible if URL-driven navigation is needed
- [ ] **Q002-P1:** Accept npm as the monorepo package manager (no pnpm switch); accept colocated types strategy (no `packages/` directory until second consumer exists)
- [ ] **Design tokens:** Review the ~20 semantic token values for visual coherence during implementation
- [ ] **cmdk:** Verify cmdk's vendored Radix Dialog is compatible with React 19 before implementation
