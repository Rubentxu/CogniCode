# Roadmap: CogniCode Explorer — Moldable, Graph-First, C4 Dual Entry

> **ADR**: [ADR-039](../adr/ADR-039-explorer-navigation-model.md)
> **Goal**: Evolve the existing React Explorer into a gtoolkit-inspired, graph-first, C4-aware exploration cockpit
> **Status**: Active — decisions locked, execution pending

---

## Current State

| Component | Status | Notes |
|-----------|--------|-------|
| `PaneStackView` | ✅ Implemented | GtPager-style lateral panes, max 8 |
| `InteractiveGraph` (Cytoscape) | ✅ Implemented | ELK worker integrated; supports `layered`, `force`, and `radial` layouts with progress, cancellation, and size guard |
| `layout.worker.ts` (ELK.js) | ✅ Implemented | Wired into `InteractiveGraph`; computes async layouts and falls back gracefully on failure |
| `rendererRegistry` | ⚠️ Partial | `graph` is wired to the real `InteractiveGraph`; remaining renderers still need consolidation |
| `Spotter` (cmdk) | ✅ Implemented | Server-side fuzzy search, kind filters |
| `ViewSpecWizard` | ✅ Implemented | 5-step authoring, localStorage drafts |
| `ContextualPanel` | ✅ Implemented | Focus + parent + children + neighbor minigraph |
| `RationaleView` | ✅ Implemented | Corroboration-scoped subgraph |
| `SvgGraph` | ✅ Exists | Manual pan/zoom SVG renderer (alternative) |
| `column` navigation | ⚠️ To remove | Hard cut per ADR-039 |
| `MillerColumns` | ⚠️ To remove | Hard cut per ADR-039 |
| C4 visual styles | ✅ Exists | `node-component`, `node-container`, `node-system` in stylesheet |
| C4 backend inference | ❌ Not implemented | `cognicode-diagram` crate planned in docs/planes/ |
| Perspective toggle | ❌ Not implemented | `Graph ↔ C4` canvas morph |
| Landing page | ❌ Not implemented | Graph overview with root nodes |
| WebGL / Sigma.js | ❌ Future | Registered as evolution path |

---

## Sprint E1 — Consolidate View Model

**Goal:** `rendererRegistry` becomes the authoritative render pipeline. No
ad-hoc rendering paths.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E1.1 | Wire `rendererRegistry["graph"]` to real `InteractiveGraph` (not placeholder) | `rendererRegistry.tsx`, `InteractiveGraph.tsx` | 3h |
| E1.2 | Wire `rendererRegistry["code"]` to a real code renderer (syntax highlight) | `rendererRegistry.tsx` | 2h |
| E1.3 | Wire `rendererRegistry["tree"]` to a real tree component | `rendererRegistry.tsx` | 2h |
| E1.4 | Make `ViewBlock` rendering go through `rendererRegistry` exclusively | `ViewBlock.tsx` | 3h |
| E1.5 | Remove parallel rendering paths (ad-hoc switches in components) | `PaneInspector.tsx`, `ObjectInspector/*` | 3h |

**Deliverable:** Every view block renders through the registry. New renderers
can be added without touching component code.

---

## Sprint E2 — Integrate ELK layout worker

**Goal:** `InteractiveGraph` uses dynamic layout (not `preset`).

**Status:** ✅ Implemented in the current codebase.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E2.1 | Wire `layout.worker.ts` (comlink) into `InteractiveGraph` mount cycle | `InteractiveGraph.tsx` | 4h |
| E2.2 | Add layout algorithm selector UI (`layered` / `force` / `radial`) | `InteractiveGraph.tsx` | 2h |
| E2.3 | Add animated layout transition (progress callbacks → tween) | `InteractiveGraph.tsx` | 3h |
| E2.4 | Add cancellation support (cancel in-flight layout on unmount/re-render) | `InteractiveGraph.tsx` | 1h |
| E2.5 | Add size guard fallback (graph > 500 nodes → drop animation) | `InteractiveGraph.tsx` | 1h |

**Deliverable:** Graph renders with dynamic layout, animated transitions, and
graceful degradation for large graphs.

**Implementation note:** The current implementation lives in
`apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` and
`apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts`.

---

## Sprint E3 — Hard Cut: Remove Column Navigation

**Goal:** `pane-stack` is the only navigation mode. `column` is eliminated.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E3.1 | Remove `NavigationModeToggle` from Shell | `Shell.tsx`, `Settings/NavigationModeToggle.tsx` | 1h |
| E3.2 | Remove `column` adapter from navigation state | `state/navigation/column.ts`, `state/navigation/index.ts` | 2h |
| E3.3 | Remove `MillerColumns` component and tests | `MillerColumns/*` | 1h |
| E3.4 | Simplify `Shell` layout: always pane-stack + graph | `Shell.tsx` | 3h |
| E3.5 | Remove `PUSH_COLUMN` / `POP_COLUMN` actions from types and reducer | `state/navigation/types.ts`, `state/context.ts` | 2h |
| E3.6 | Update tests: remove column-mode assertions | `*.test.tsx` | 3h |

**Deliverable:** Single navigation model. Simpler state. No column artifacts.

---

## Sprint E4 — Graph Landing Page

**Goal:** The Explorer opens to a graph overview, not an empty inspector.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E4.1 | Create `GraphLanding` component (root nodes + hot paths overview) | `GraphLanding.tsx` | 4h |
| E4.2 | Hook `GraphLanding` to `GET /api/graph/roots` (entry points) | `hooks/useRootNodes.ts` | 2h |
| E4.3 | Add suggested questions strip (from `config/suggestedQuestions.ts`) | `GraphLanding.tsx` | 2h |
| E4.4 | Wire root-node click → open pane in pane-stack | `GraphLanding.tsx`, `state/context.ts` | 2h |
| E4.5 | Add recent explorations strip (from `useExplorations`) | `GraphLanding.tsx` | 2h |

**Deliverable:** Landing shows graph roots, suggested questions, and recent
explorations. Clicking a root opens the pane-stack workflow.

---

## Sprint E5 — Perspective Toggle (Graph ↔ C4)

**Goal:** The graph canvas morphs between call-graph and C4 perspectives.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E5.1 | Add perspective toggle UI (`[Context | Graph]`) in Shell header | `Shell.tsx` | 2h |
| E5.2 | Create `useC4Context` hook (calls backend C4 inference) | `hooks/useC4Context.ts` | 3h |
| E5.3 | Wire toggle → swap data source between `useSubgraph` and `useC4Context` | `InteractiveGraph.tsx` | 3h |
| E5.4 | Apply C4 stylesheet classes when in C4 perspective | `InteractiveGraph.tsx`, `stylesheet.ts` | 2h |
| E5.5 | Add smooth transition between perspectives (data swap + re-layout) | `InteractiveGraph.tsx` | 3h |

**Deliverable:** User can toggle between Graph and C4 perspectives on the same
canvas. C4 shows system/container/component nodes with proper styling.

---

## Sprint E6 — C4 Backend Inference (minimum viable)

**Goal:** Backend extracts C4 structure from code for the Explorer to consume.

| ID | Task | Files | Est |
|----|------|-------|-----|
| E6.1 | Create `cognicode-diagram` crate skeleton | `crates/cognicode-diagram/` | 2h |
| E6.2 | Implement container inference (Cargo.toml / package.json → containers) | `inference/container_inference.rs` | 4h |
| E6.3 | Implement component inference (directory structure → components) | `inference/component_inference.rs` | 4h |
| E6.4 | Add `GET /api/graph/c4` endpoint returning C4 nodes + edges | `api.rs`, `facades/` | 3h |
| E6.5 | Register C4 nodes in the graph with proper `NodeKind` / `EdgeKind` | `domain/` | 2h |

**Deliverable:** Explorer can show real C4 structure inferred from the
workspace's crates and modules.

---

## After E6: Future Evolution (tracked, not scheduled)

| Milestone | Description |
|-----------|-------------|
| E7 renderer scale evaluation | Benchmark Cytoscape canvas vs Cytoscape WebGL preview, and escalate to Sigma.js only if thresholds fail |
| WASM graph transforms | Rust layout/clustering compiled to WASM for client-side compute |
| C4 drift detection | Compare inferred C4 vs documented architecture (ADRs, CONTEXT.md) |
| Dynamic view authoring | ViewSpec wizard as primary view creation (not just runtime extras) |
| Exploration narratives | `composed_narrative` ViewKind — linkable story of objects + evidence |
| MCP ↔ Explorer sync | Real-time graph updates via PG NOTIFY / WebSocket when MCP tools run |

---

## ADRs Referenced

- [ADR-039](../adr/ADR-039-explorer-navigation-model.md) — This roadmap's ADR
- [ADR-041](../adr/ADR-041-explorer-renderer-scale-evaluation.md) — E7 renderer scale evaluation path
- [ADR-042](../adr/ADR-042-renderer-decision.md) — Renderer decision outcome placeholder
- [ADR-038](../adr/ADR-038-sandbox-hardening-and-coverage.md) — Sandbox hardening
- [ADR-034](../adr/ADR-034-mcp-production-readiness.md) — MCP production readiness

---

## Sprint F1-F4 — E2E Test Battery (38 scenarios)

**Goal:** Comprehensive E2E coverage for all Explorer navigation flows.

See [explorer-e2e-test-plan.md](explorer-e2e-test-plan.md) for the detailed plan.

| Sprint | Phase | Scenarios | Est |
|--------|-------|-----------|-----|
| F1 | Prerequisites: wire `useBootstrapWorkspace`, fix MSW coverage | — | 2-3h |
| F2 | Landing page + Perspective toggle | 14 | 3-4h |
| F3 | Pane-stack + Spotter interactions | 13 | 3-4h |
| F4 | Error handling + Responsive + Accessibility | 11 | 3-4h |

**Total:** 38 tests, 12-15h
