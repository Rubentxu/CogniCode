# Kernel Exploration: Graph Landing Page (E4 / ADR-039)

**Date:** 2026-06-19
**Triggered by:** E4 of ADR-039 — "Graph is the primary visual landing"
**Context level:** C2 (significant code has been read; some design choices still TBD)
**Recommendation:** Ready for proposal with one blocking question on the data source
(Explorer HTTP API vs. cognicode-mcp streamable HTTP).

---

## 1. Current State

### 1.1 What the user sees today (post-E3)

`Shell.tsx` (`apps/explorer-ui/src/components/Shell.tsx`) renders a 2-zone CSS grid:

```text
┌──────────────────────────────┬───────────────────────────┐
│ InteractiveGraph (left, 1.4fr)│ PaneStackView (right, 1fr) │
└──────────────────────────────┴───────────────────────────┘
```

The **left zone** is driven by `InteractiveGraphPanel({ rootId })` (line 39-65),
where `rootId = appState.activeObjectId`. When `activeObjectId === null`:
- `useSubgraph(null)` returns `{ data: null, ... }` (the SWR key is `null`,
  no fetch fires).
- `InteractiveGraph` enters its **empty-state branch** (lines 260-278):

  ```tsx
  if (!data || data.nodes.length === 0) {
    return (
      <div data-testid="interactive-graph-empty" ...>
        <span>No graph data — pick a symbol to see its neighbourhood.</span>
      </div>
    );
  }
  ```

The **right zone** (`PaneStackView`) is also empty by default:
- When `state.navigation.panes.length === 0` it shows the "No panes open"
  empty state (lines 56-76) with a pointer to Spotter (⌘K).

The top bar already carries: title, `HealthProbe`, `ScanBar` (scan button +
progress + symbol/edge counts), Spotter trigger (`⌘K`), `ShareExplorationButton`.

### 1.2 What is the "active object" right now?

`state.activeObjectId` is **derived from the navigation focus**, not stored
directly. `appReducer` (in `state/context.ts`) delegates `SELECT_OBJECT`,
`PUSH_PANE`, etc. to the navigation reducer, and then mirrors
`getActiveFocus(nav)` back onto the top-level `activeObjectId`,
`activeViewId`, `activeLensId` (lines 142-155). So:
- **On first load** → `activeObjectId === null` → graph shows "No graph data".
- **After Spotter pick** → `activeObjectId = spotterResult.id` → graph fetches
  the per-symbol subgraph via `GET /api/graph/:id/subgraph`.

### 1.3 How the graph renders today (when data is null vs. present)

`InteractiveGraph.tsx` does **not** have a "workspace overview" mode. Its
only branches are:
- `data === null || data.nodes.length === 0` → text empty state.
- otherwise → cytoscape mount with ELK layout (E2), fallback `<table>` for SR.

It accepts only a `data: SubgraphResponse | null` — there is no `mode` prop
or alternate data source. Reuse is **structural only** (cytoscape + ELK
worker + stylesheet are the same).

### 1.4 Mock data shape

`apps/explorer-ui/src/mocks/fixtures.ts` has:
- `workspaceSummaryFixture` (with `graph_status: "ready"`,
  `symbol_count: 1240`, `relation_count: 4312`, `indexed_at`).
- `spotterResultsFixture`, `contextualViewFixture`, `lensDescriptorsFixture`,
  `lensResultFixture`, `explorationPathFixture`, `decisionArtifactFixture`.

`apps/explorer-ui/src/mocks/handlers.ts` exposes 11 MSW handlers + the
subgraph/contextual/rationale handlers. **No landing / overview /
hot-paths / entry-points / god-nodes handlers exist** — they would need
to be added alongside any new HTTP endpoint.

---

## 2. Context Quality

| Aspect | Status | Notes |
|---|---|---|
| ADR-039 read | ✅ | `docs/adr/ADR-039-explorer-navigation-model.md` |
| Current Shell rendering | ✅ | Lines 75-242 of `Shell.tsx` |
| Current InteractiveGraph empty state | ✅ | Lines 260-278 of `InteractiveGraph.tsx` |
| `useSubgraph` and the SWR key contract | ✅ | `hooks/useSubgraph.ts` |
| `useWorkspace` + `useWorkspaceList` + `useGraphStats` | ✅ | `useWorkspace.ts`, `useScanJob.ts` |
| `useAppState` shape | ✅ | `state/context.ts` lines 30-54 |
| Explorer API routes | ✅ | `api.rs::router` lines 469-515 |
| `get_entry_points` / `get_hot_paths` / `graph_god_nodes` / `graph_insights` / `graph_suggest_questions` | ✅ handlers exist in `cognicode-core/src/interface/mcp/handlers/graph_handlers.rs` and `aix_handlers.rs`, but **only over the cognicode-core MCP server**, not over the Explorer HTTP API |
| Mock fixtures / handlers | ✅ | `mocks/fixtures.ts`, `mocks/handlers.ts` |

**Missing context:**
- How many nodes the landing should show by default (ADR-039 says "500 max"
  via the existing `max_nodes` param; ADR-007 design language is
  "clustering + progressive disclosure" but the specific visual
  density rules for the landing are not specified).
- Whether the Explorer backend will host a *new* landing endpoint, or
  the frontend will call the cognicode-mcp streamable HTTP at port 9847
  (a different process).

**Recommended effort:** **verify** (the architecture is clear; the only
open question is data-source choice).

---

## 3. Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|---|---|---|---|
| Roadmap / Backlog | partial | E4 in ADR-039 evolution order; CONTEXT.md lists GraphLanding as a v1 view kind? | No — landing is a *screen*, not a view kind. It uses the existing `call_graph` / `dependency_graph` renderers. |
| Work Items | present | `sddk/M3-Sprint/apply-progress-pr-1..3` (M3 sprint), `sddk/explorations/file-watcher-integration.md` | Naming convention for sddk artifacts: `apply-progress-pr-N` (final), `sddk/explorations/<topic>.md` (pre-apply). |
| Architecture / ADRs | present | `docs/adr/ADR-039-explorer-navigation-model.md` (full text reviewed) | Section 7 (Graphify visual density) is the design constraint we have to honor. |
| Ownership | present | E2 owner = backend ELK wiring; E3 owner = column-mode removal (just shipped 2026-06-19); E4 owner = graph-landing. | E4 is a frontend-led work item, with backend additions (new endpoints or MCP tool proxies). |
| Learnings | present | E2 lesson: ELK layout is *re-runnable* on algorithm change without remounting cytoscape (`InteractiveGraph.tsx` lines 182-240). E3 lesson: navigation state is pane-stack only (`state/navigation/types.ts` line 23). | Both reusable for E4 — we get layout reuse for free, and the landing must respect the pane-stack mode (no column fallback). |

---

## 4. Problem Taxonomy

| Axis | Applies | Evidence |
|---|---|---|
| Domain modeling | **Yes** | ADR-039 §2 says the landing shows "root nodes, hot paths, suggested questions". The Explorer domain has `EntryPoint` (in `crates/cognicode-explorer/src/domain/entry_point.rs` — 14 variants), `HotPathDto`, `GodNode`, `GraphInsightsReport` (in `cognicode-core`). We must pick which to surface. |
| Boundary / seam | **Yes** | Two services: `cognicode-explorer` (HTTP `:8080`, what the UI talks to today) and `cognicode-mcp` (Streamable HTTP `:9847` or stdio, what OpenCode agents use). The landing data lives in `cognicode-core` and is currently exposed only via the MCP server — **not via the Explorer HTTP API**. |
| Coupling / connascence | **Yes** | Landing must respect `useSubgraph` key contract (SWR cache dedup at 5s). If we add a new data hook it must follow the same pattern or we will invalidate the cache. |
| API contract | **Yes** | `cognicode-core` already defines `HotPathDto` (workspace_session.rs), `SymbolDto` (entry points), and a JSON shape for `graph_insights` (`graph_handlers.rs` lines 514-555). We need to mirror these on the Explorer side. |
| Refactor / legacy | **No** | No legacy landing to retire. `MillerColumns` directory is empty (post-E3 hard cut) — clean slate. |
| Event / CQRS | **No** | Pure read-side page. |
| Testing | **Yes** | 11 handlers in `mocks/handlers.ts` today; the new landing must extend this so the E2E Playwright suite can run without a live backend. |
| Security / operations | **Partial** | Same CORS / bearer-token model as the rest of `/api/*` if we add the landing endpoint to `cognicode-explorer`. No new attack surface. |

---

## 5. Domain Language and Invariants

**Resolved terms** (per CONTEXT.md + ADR-039):
- **Landing** = the first screen after the Shell mounts, with no
  `activeObjectId` set. Currently the "No graph data" empty state.
- **Root nodes** = the top-level entry points of the workspace. The
  domain type is `EntryPoint` (14 variants: HttpRoute, CliCommand,
  Event, UseCase, Symbol, File, Scope, …) — but for the landing the
  narrowest useful interpretation is `Symbol` entry points, which is
  exactly what `AnalysisService::get_entry_points()` returns.
- **Hot paths** = symbols with the highest fan-in
  (`WorkspaceSession::get_hot_paths(limit, min_fan_in)` returns
  `Vec<HotPathDto>`). 5-10 max.
- **Suggested questions** = natural-language prompts. The Explorer
  already has `config/suggestedQuestions.ts` (typed map of
  `{ symbol, file, scope, module, workspace, … }` → 3-5 prompts each).
  The backend also has `graph_suggest_questions` which produces
  *different* prompts based on actual graph analysis (cycles, god
  nodes, surprising connections).
- **Workspace overview view** = the per-object `build_overview` in
  `domain/views.rs` — but this is **per-object**, not per-workspace.
  A *workspace-level* overview view does **not** exist.
- **Pane-stack** = the only navigation mode (per ADR-039 §1). The
  landing must be a graph surface + suggestion strip, not a column
  drill-down.
- **GraphLanding = `interactive-graph` with `data: SubgraphResponse |
  null` + suggestion strip + 0 panes open**. The simplest framing.

**Invariants** (asserted in code today):
- `useSubgraph` only fires when `rootId` is non-null (`hooks/useSubgraph.ts`
  line 15: `const key = rootId ? [...] : null`).
- `InteractiveGraph` accepts only `SubgraphResponse | null` — its
  data shape is **strictly** `{ root, nodes: GraphNode[], edges: GraphEdge[],
  truncated, truncated_reason, corroboration_scores }` (see
  `useSubgraph` and `api.rs::subgraph_handler`).
- `activeObjectId` is null on first load; setting it to a workspace
  id is **not** supported (the build_subgraph path calls
  `SymbolId::new(id)` and `symbol_repo.resolve` — a workspace id would
  404 with `SymbolNotFound`).
- `state.workspace` starts as `null` and is only populated by
  `SET_WORKSPACE` (which the Spotter can trigger via
  `useAsk → explorer_open_workspace → openWorkspace`). It is **not
  automatically set** when a workspace is opened at startup.
- The Explorer backend exposes `GraphStatus` as
  `"missing" | "stale" | "ready" | "indexing"` (snake_case in
  `dto.rs::GraphStatus` and `apps/.../api/schemas.ts::graphStatusSchema`).

**Unknowns to lock down before coding:**
- Visual density: how many root nodes show by default? (50? 200?)
- Are we using `cognicode-core` MCP tools (5 separate calls) or
  building a single `GET /api/workspaces/:id/landing` endpoint
  (1 round trip)?

---

## 6. Knowledge Gaps

1. **No workspace-level HTTP endpoint exists for entry points, hot
   paths, or insights.** The closest is
   `GET /api/workspaces/:id/graph/stats` returning
   `{ workspace_id, symbol_count, edge_count, last_scan_at }` — only
   counts, not the actual nodes. **Blocker for "show root nodes + hot
   paths in one go".**
2. **No frontend client functions for the landing data.** Searching
   `apps/explorer-ui/src` for `get_entry_points`, `get_hot_paths`,
   `graph_insights`, `graph_suggest_questions`, `graph_god_nodes` returns
   **zero matches**. The frontend never calls these MCP tools today.
3. **No fixture for landing data.** `mocks/fixtures.ts` has
   `workspaceSummaryFixture` (counts only) but no `entryPointsFixture`,
   `hotPathsFixture`, `insightsFixture`. The MSW handlers don't mock
   them either. Without mocks, tests would have to spin up the real
   axum service.
4. **No "landing view" in the registry.** `domain/views.rs::OVERVIEW_EXECUTOR`
   is per-object (symbol/file/scope). There is no
   `WORKSPACE_LANDING_EXECUTOR`.
5. **C4 perspective is not yet built.** ADR-039 §4 (perspective
   toggle `Context ↔ Graph`) is an evolution item that **comes after**
   E4 — but the landing should be wired so the C4 toggle can drop
   in later.

---

## 7. Affected Areas

| File / crate | Why it changes for E4 |
|---|---|
| `apps/explorer-ui/src/components/Shell.tsx` | The `InteractiveGraphPanel` decision flips: when `activeObjectId === null` we render the new `GraphLanding` (which contains its own graph + suggestion strip) instead of letting `InteractiveGraph` fall through to "No graph data". |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` | No structural change. Reuse as-is. Optionally we add a `mode: "subgraph" \| "landing"` prop or a `landingData: LandingPayload` prop so it can show a `max_nodes=200` overview without a `root`. |
| `apps/explorer-ui/src/api/client.ts` + `apps/explorer-ui/src/api/schemas.ts` | Add `fetchLanding(workspaceId)` (or three calls) and corresponding zod schemas. |
| `apps/explorer-ui/src/hooks/useLanding.ts` (new) | SWR wrapper around `fetchLanding`. Keyed by `workspaceId`. |
| `apps/explorer-ui/src/components/GraphLanding/*` (new) | New module: `GraphLanding.tsx`, `LandingSuggestionStrip.tsx`, `LandingHeader.tsx`. |
| `apps/explorer-ui/src/config/suggestedQuestions.ts` | Already covers `workspace` type. No change required, but the landing strip may want a **subset** (e.g. only 3 of the 5 workspace prompts). |
| `apps/explorer-ui/src/mocks/fixtures.ts` + `apps/explorer-ui/src/mocks/handlers.ts` | Add landing fixtures + MSW handlers so tests don't need a live backend. |
| `crates/cognicode-explorer/src/api.rs` | Add `GET /api/workspaces/:id/landing` (preferred path — see §9). |
| `crates/cognicode-explorer/src/facades/*.rs` | Possibly add a `LandingService` facade or wire the existing facades (`GraphService` for hot paths, `SearchService` for entry points, `WorkspaceService` for stats). |
| `crates/cognicode-explorer/src/dto.rs` | Add `LandingPayload` DTO. |
| `apps/explorer-ui/src/components/Shell.test.tsx` + new `GraphLanding.test.tsx` | Update `desktop/tablet/ultrawide` assertions: they currently expect `interactive-graph-empty` or `interactive-graph` test ids. After E4, they should expect `graph-landing` (which contains the graph + strip). |

---

## 8. Options

| # | Option | Pros | Cons | Effort |
|---|---|---|---|---|
| A | **New `GET /api/workspaces/:id/landing`** in the Explorer backend that fans out to the existing facades (stats, hot paths from `get_hot_paths` semantic, entry points, insights). | Single round trip; reuses zod-at-boundary pattern; matches the rest of `/api/*`; tests can mock with one MSW handler. | Touches Rust + TS; needs a new `LandingService` facade or three service calls composed in the handler. | M (1-2 days backend + 1 day TS wiring + tests) |
| B | **Three MCP calls** from the frontend: `get_entry_points`, `get_hot_paths`, `graph_suggest_questions` against the cognicode-mcp streamable HTTP server at `:9847/mcp`. | Zero backend work; reuses already-shipped MCP tools. | Two different HTTP bases (`/api` for Explorer, `:9847/mcp` for MCP). Requires CORS plumbing on the MCP side or a proxy. The Explorer backend already has a CORS-permissive layer; the MCP one requires a bearer token (or local-dev no-auth). Inconsistent and surprising. | S (frontend-only) but risk: **high** |
| C | **One MCP call** to `graph_insights` (returns summary + god_nodes + suggested_questions + communities all in one envelope). Add a small proxy in the Explorer backend that calls the local MCP `CogniCodeHandler` in-process. | Single round trip; one wire shape; in-process call (no CORS). | Requires wiring the in-process MCP handler from the Explorer runtime (currently they live in different binaries). High initial coupling. | M-L (cross-crate refactor) |

**Recommendation:** **Option A** — it follows the existing pattern
(`/api/workspaces/:id/...`), keeps the Explorer backend as the
single source of truth for the UI, and reuses the same zod-at-
boundary + SWR plumbing.

---

## 9. Entropy Envelope

**Method:** heuristic.

- **Coupling risk:** **medium**.
  - The new `LandingService` (or composition in the handler) will
    reach into 2-3 existing facades. Reasonable; matches how
    `AskRouter` already composes multiple services.
  - The new frontend `GraphLanding` is **loosely coupled** to
    `InteractiveGraph` (it just passes data through).
- **OCP risk:** **low**. We add a new prop / new component; we do
  not change the contract of `useSubgraph` or `InteractiveGraph`.
- **Connascence risk:** **medium**.
  - **Connascence of meaning:** both the existing `SubgraphResponse`
    shape and the new `LandingPayload` carry `nodes`/`edges` for
    cytoscape — we must keep the inner `GraphNode` / `GraphEdge`
    shapes identical so the same `toCytoscapeElements` adapter works.
  - **Connascence of position:** SWR key shape changes from
    `["/graph/:id/subgraph", rootId, params]` to
    `["/workspaces/:id/landing", workspaceId, params]`. Documented in
    the new hook.

---

## 10. Recommendation

**Build Option A — `GET /api/workspaces/:id/landing` + new `GraphLanding` component.**

Concrete shape:

```text
GET /api/workspaces/:id/landing
→ 200 LandingPayload {
    workspace: WorkspaceSummary,
    stats: GraphStats,                   // existing DTO
    entry_points: InspectableObjectSummary[],   // 5-10 symbols
    hot_paths: InspectableObjectSummary[],       // 5-10 symbols
    god_nodes: Array<{ id: string, score: number }>,  // top 5
    communities: Array<{ id, label, size, cohesion }>,  // top 3
    suggested_questions: string[],              // 3-5 strings
  }
→ 404 if workspace not found
→ 503 if graph_status in { "missing", "indexing" } — caller
  should still render the Shell with a "scan first" hint.
```

The frontend `GraphLanding` composes the response as:

```text
┌──────────────────────────────────────────────┐
│ LandingHeader: workspace path + Scan button  │  ← existing ScanBar
├──────────────────────────────────────────────┤
│ InteractiveGraph (root=workspace)            │  ← cytoscape of
│   data={ nodes: landing, edges: landing }    │    entry points +
│                                              │    hot paths + god
│                                              │    nodes (no synthetic
│                                              │    root node)
├──────────────────────────────────────────────┤
│ LandingSuggestionStrip:                      │  ← static prompts
│   - "Where do I start?" → cognicode_ask      │    from workspace
│   - "What is the shape?" → cognicode_ask     │    branch of
│   - suggested_questions from backend         │    suggestedQuestions.ts
└──────────────────────────────────────────────┘
```

Click on a node → `dispatch SELECT_OBJECT { objectId: nodeId, viewId: "overview" }`
→ existing PaneStackView flow.

---

## 11. Ready For Proposal

**Partially — one design question is blocking the proposal.**

The only open question is the **C4 perspective toggle** (ADR-039 §4):

> "Single canvas with perspective toggle (`Context ↔ Graph`)"
> Evolution item 3: "Ship perspective toggle (`Graph ↔ C4`) on the graph canvas"

Should E4 ship the toggle now (graph + c4 modes from the start) or
graph-only with a follow-up E5 for the c4 mode? ADR-039 evolution
order lists the toggle as item 3 (after view-model consolidation +
ELK integration, both already done) and before column-removal (item 4,
done in E3). So **the toggle is next, not part of E4**.

**Question to user before proposal:** Confirm that E4 = "graph-only
landing" and the C4 perspective is a separate E5.

Everything else (data shape, component decomposition, risk) is
ready.
