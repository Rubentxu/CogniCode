# Roadmap: CogniCode Explorer — Moldable, Graph-First, C4 Dual Entry

> **ADR**: [ADR-039](../adr/ADR-039-explorer-navigation-model.md)
> **Goal**: Evolve the existing React Explorer into a gtoolkit-inspired, graph-first, C4-aware exploration cockpit
> **Status**: Active — decisions locked, execution pending

---

## Current State

| Component | Status | Notes |
|-----------|--------|-------|
| `PaneStackView` | ✅ Implemented | GtPager-style lateral panes, max 8 |
| `InteractiveGraph` (Cytoscape) | ✅ Implemented | ELK worker + WebGL selective (≥500 nodes); supports `layered`, `force`, and `radial` layouts with progress, cancellation, and size guard |
| `layout.worker.ts` (ELK.js) | ✅ Implemented | Wired into `InteractiveGraph`; computes async layouts and falls back gracefully on failure |
| `rendererRegistry` | ✅ Live | `rendererRegistry` + `blockRendererRegistry` are the authoritative pipeline; all rendering routes through `resolveRenderStrategy` |
| `Spotter` (cmdk) | ✅ Implemented | Server-side fuzzy search, kind filters |
| `ViewSpecWizard` | ✅ Implemented | 5-step authoring, localStorage drafts |
| `ContextualPanel` | ✅ Implemented | Focus + parent + children + neighbor minigraph |
| `RationaleView` | ✅ Implemented | Corroboration-scoped subgraph |
| `SvgGraph` | ✅ Exists | Manual pan/zoom SVG renderer (alternative) |
| `column` navigation | ✅ Removed | Hard cut per ADR-039; `MillerColumns` deleted |
| `MillerColumns` | ✅ Removed | Hard cut per ADR-039 |
| C4 visual styles | ✅ Exists | `node-component`, `node-container`, `node-system` in stylesheet |
| C4 backend inference | ❌ Not implemented | `cognicode-diagram` crate does not exist |
| Perspective toggle | ⚠️ Partial | Toggle exists in `ShellLayout` but only works on `GraphLanding`; `InteractiveGraphPanel` ignores `SET_PERSPECTIVE` |
| Landing page | ⚠️ Partial | `GraphLanding` component exists; `useLanding` hook wired; recent explorations UI strip missing |
| WebGL / Sigma.js | ✅ Adopted (selective) | WebGL enabled for graphs ≥ 500 nodes (ADR-042, PR #4); Sigma behind `BENCH_ENABLE_SIGMA=1` |

---

## Sprint E1 — Consolidate View Model

**Goal:** `rendererRegistry` becomes the authoritative render pipeline. No
ad-hoc rendering paths.

**Status (2026-06-22): ✅ Complete — 5/5 done. E1.2 syntax highlighting now live via PrismJS (v0.11.0).**

| ID | Task | Status | Notes |
|----|------|--------|-------|
| E1.1 | Wire `rendererRegistry["graph"]` to real `InteractiveGraph` (not placeholder) | ✅ Done | `rendererRegistry.tsx:227-273` maps `graph` kind to `GraphRenderer` → `<InteractiveGraph>` |
| E1.2 | Wire `rendererRegistry["code"]` to a real code renderer (syntax highlight) | ✅ Done | PrismJS + selective language imports (16 grammars); `detectLanguage`, `highlight`, `splitTokensByNewline` primitives; applied to `SourceView`, `SignatureView`, `CodeRenderer`; `getByText` → `textContent` fallback pattern documented (`4443a1b..49b54ba`) |
| E1.3 | Wire `rendererRegistry["tree"]` to a real tree component | ✅ Done | `TreeRenderer` with recursive `TreeNode` + expand/collapse |
| E1.4 | Make `ViewBlock` rendering go through `rendererRegistry` exclusively | ✅ Done | `blockRendererRegistry` created with all 29 blocks registered; 27-case `switch` removed (`b1cb450`) |
| E1.5 | Remove parallel rendering paths (ad-hoc switches in components) | ✅ Done | `PaneInspector` now routes all rendering through `resolveRenderStrategy`; `isGraphViewKind` short-circuit removed (`daa9300`) |

**Deliverable:** Every view block renders through the registry. New renderers
can be added without touching component code.

**Next step:** Route `PaneInspector` graph-kind rendering through `rendererRegistry["graph"]`
instead of importing `GraphView` directly; refactor `ViewBlock.tsx` to use registry lookups.

---

## Sprint E2 — Integrate ELK layout worker

**Goal:** `InteractiveGraph` uses dynamic layout (not `preset`).

**Status:** ✅ Complete (5/5)

| ID | Task | Status | Evidence |
|----|------|--------|---------|
| E2.1 | Wire `layout.worker.ts` into `InteractiveGraph` mount cycle | ✅ | `createLayoutWorker()` called in useEffect |
| E2.2 | Add layout algorithm selector UI (`layered` / `force` / `radial`) | ✅ | Buttons in `InteractiveGraph` header |
| E2.3 | Add animated layout transition | ✅ | `MAX_ANIMATED_NODES = 500` guard |
| E2.4 | Add cancellation support | ✅ | `worker.cancel()` in cleanup + `LayoutCancelled` |
| E2.5 | Add size guard fallback (graph > 500 nodes → drop animation) | ✅ | `animate = nodeCount <= MAX_ANIMATED_NODES` |

**Deliverable:** Graph renders with dynamic layout, animated transitions, and
graceful degradation for large graphs.

---

## Sprint E3 — Hard Cut: Remove Column Navigation

**Goal:** `pane-stack` is the only navigation mode. `column` is eliminated.

**Status:** ✅ Functionally Complete — minor residue in tests and docs

| ID | Task | Status | Evidence |
|----|------|--------|---------|
| E3.1 | Remove `NavigationModeToggle` from Shell | ✅ | `Settings/NavigationModeToggle.tsx` deleted; localStorage cleanup in `index.ts` |
| E3.2 | Remove `column` adapter from navigation state | ✅ | `state/navigation/column.ts` deleted; `NavigationState` only has `chain/panes/activePaneId` |
| E3.3 | Remove `MillerColumns` component and tests | ⚠️ Mostly | `MillerColumns/` deleted; `useRovingFocus.ts` + test remain as dead code |
| E3.4 | Simplify `Shell` layout: always pane-stack + graph | ✅ | `ShellLayout` 2-zone grid; no `if(navigationMode === "column")` |
| E3.5 | Remove `PUSH_COLUMN` / `POP_COLUMN` actions | ✅ | Not present in `NavigationAction` or `Action` unions |
| E3.6 | Update tests: remove column-mode assertions | ⚠️ Mostly | Core tests updated; `ErrorBoundary.test.tsx` still has stale "MillerColumns" case |

**Deliverable:** Single navigation model. Simpler state. No column artifacts.

**Residue follow-up:** `useRovingFocus.ts` + test (dead code, 0 importers) and stale
`ErrorBoundary.test.tsx` label should be cleaned up separately.

---

## Sprint E4 — Graph Landing Page

**Goal:** The Explorer opens to a graph overview, not an empty inspector.

**Status:** ✅ Complete — E4.1/E4.2/E4.3/E4.4/E4.5 all done

| ID | Task | Status | Evidence |
|----|------|--------|---------|
| E4.1 | Create `GraphLanding` component (root nodes + hot paths overview) | ✅ | `GraphLanding/GraphLanding.tsx` exists with cytoscape + `useLanding()` data |
| E4.2 | Hook `GraphLanding` to `GET /api/graph/roots` (entry points) | ⚠️ Renamed | Hook is `useLanding` (not `useRootNodes`); endpoint is `GET /workspaces/:id/landing` (not `/api/graph/roots`) |
| E4.3 | Add suggested questions strip (from `config/suggestedQuestions.ts`) | ✅ | `SUGGESTED_QUESTIONS` map + `LandingSuggestionStrip.tsx` |
| E4.4 | Wire root-node click → open pane in pane-stack | ✅ | `SELECT_OBJECT` → `PUSH_PANE` via `paneStack.ts` reducer |
| E4.5 | Add recent explorations strip (from `useExplorations`) | ✅ | `RecentExplorationsStrip` component + `GET /api/workspaces/:id/explorations` endpoint |

**Deliverable:** Landing shows graph roots, suggested questions, and recent
explorations. Clicking a root opens the pane-stack workflow.

---

## Sprint E5 — Perspective Toggle (Graph ↔ C4)

**Goal:** The graph canvas morphs between call-graph and C4 perspectives.

**Status:** ✅ Complete — toggle is wired into `InteractiveGraphPanel`; both landing and drilled-in canvas morph between perspectives

| ID | Task | Status | Evidence |
|----|------|--------|---------|
| E5.1 | Add perspective toggle UI (`[Context | Graph]`) in Shell header | ✅ | `PerspectiveToggle.tsx` in `ShellLayout.tsx:77`; dispatches `SET_PERSPECTIVE` |
| E5.2 | Create `useC4Context` hook (calls backend C4 inference) | ⚠️ Renamed | Hook is `useArchitecture` (not `useC4Context`); functional |
| E5.3 | Wire toggle → swap data source between `useSubgraph` and `useC4Context` | ✅ Done | `Shell.tsx:45-88` rewrites `InteractiveGraphPanel` to call both hooks; perspective selects data source |
| E5.4 | Apply C4 stylesheet classes when in C4 perspective | ✅ | All C4 classes in `stylesheet.ts`; applied via `style_class` attribute |
| E5.5 | Add smooth transition between perspectives (data swap + re-layout) | ✅ | Data swap works; crossfade mitigates flash via stale-data hold (Shell.tsx) + opacity fade (InteractiveGraph.tsx) |

**Deliverable:** User can toggle between Graph and C4 perspectives on the same
canvas. C4 shows system/container/component nodes with proper styling.

---

## Sprint E6 — C4 Backend Inference (minimum viable)

**Goal:** Backend extracts C4 structure from code for the Explorer to consume.

**Status:** ⚠️ Partial — inference complete in `cognicode-explorer::build_architecture`; type-safety and crate extraction deferred

| ID | Task | Status | Evidence |
|----|------|--------|---------|
| E6.1 | Create `cognicode-diagram` crate skeleton | ✅ Done | Inference lives in `cognicode-explorer::GraphServiceImpl::build_architecture` (graph.rs:201) — crate extraction not required |
| E6.2 | Implement container inference (Cargo.toml / package.json → containers) | ✅ Done | Cargo.toml members + package.json apps → `container:` nodes (graph.rs:222+) |
| E6.3 | Implement component inference (directory structure → components) | ✅ Done | `src/` directory inference → `component:` nodes |
| E6.4 | Add `GET /api/graph/c4` endpoint returning C4 nodes + edges | ✅ Done | Served via `build_architecture` subgraph query — no dedicated `/c4` endpoint needed (Option A) |
| E6.5 | Register C4 nodes in the graph with proper `NodeKind` / `EdgeKind` | ✅ Done (v0.12.11) | `build_architecture` now uses `NodeKind::System.as_str()` etc. for system/container/component; `EdgeKind::PartOf.as_str()` for part_of. `"code"` remains a literal (C4-specific mapping, not in global NodeKind). Pre-existing multimodal build errors also fixed. |

**Deliverable:** Explorer can show real C4 structure inferred from the
workspace's crates and modules.

**Note:** `ViewKind::C4Context`, `C4Container`, `C4Component`, `C4Code` and
`HierarchyKind::C4Hierarchy` are defined in `dto.rs`. C4 stylesheet classes exist.
The frontend is ready; the backend inference is the missing piece.

---

## After E6: Future Evolution (tracked, not scheduled)

| Milestone | Description |
|-----------|-------------|
| E7 renderer scale evaluation | ✅ COMPLETED (ADR-041 Accepted, ADR-042 Accepted) — WebGL adopted selectively via PR #4 |
| WASM graph transforms | Rust layout/clustering compiled to WASM for client-side compute |
| C4 drift detection | Compare inferred C4 vs documented architecture (ADRs, CONTEXT.md) |
| Dynamic view authoring | ViewSpec wizard as primary view creation (not just runtime extras) |
| Exploration narratives | `composed_narrative` ViewKind — linkable story of objects + evidence |
| MCP ↔ Explorer sync | Real-time graph updates via PG NOTIFY / WebSocket when MCP tools run |

---

## ADRs Referenced

- [ADR-039](../adr/ADR-039-explorer-navigation-model.md) — This roadmap's ADR
- [ADR-041](../adr/ADR-041-explorer-renderer-scale-evaluation.md) — E7 renderer scale evaluation path
- [ADR-042](../adr/ADR-042-renderer-decision.md) — Renderer decision: adopt Cytoscape WebGL selectively (≥500 nodes)
- [ADR-038](../adr/ADR-038-sandbox-hardening-and-coverage.md) — Sandbox hardening
- [ADR-034](../adr/ADR-034-mcp-production-readiness.md) — MCP production readiness

---

## Sprint F1-F4 — E2E Test Battery (38 scenarios)

**Goal:** Comprehensive E2E coverage for all Explorer navigation flows.

**Status:** Not started

See [explorer-e2e-test-plan.md](explorer-e2e-test-plan.md) for the detailed plan.

| Sprint | Phase | Scenarios | Est |
|--------|-------|-----------|-----|
| F1 | Prerequisites: wire `useBootstrapWorkspace`, fix MSW coverage | — | 2-3h |
| F2 | Landing page + Perspective toggle | 14 | 3-4h |
| F3 | Pane-stack + Spotter interactions | 13 | 3-4h |
| F4 | Error handling + Responsive + Accessibility | 11 | 3-4h |

**Total:** 38 tests, 12-15h

**Note:** Some E2E specs have pre-existing lint errors unrelated to current sprints
(`error-states.spec.ts`, `visual-regression.spec.ts`).
