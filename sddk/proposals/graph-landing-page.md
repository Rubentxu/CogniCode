# Kernel Proposal: Graph Landing Page (E4 / ADR-039)

## Intent
ADR-039 §2 mandates the Explorer's first screen be a graph overview (root nodes, hot paths, architecture structure), and §7 requires Graphify-style visual density. Today, when `activeObjectId === null`, `InteractiveGraph` renders "No graph data". E4 replaces that dead empty state with a `GraphLanding` showing workspace-level structure in the existing graph canvas, so the user sees the *shape* of the system before drilling in.

## Context Gate
| Knowledge Coverage | Quality | Taxonomy | Extra Effort |
|--------------------|---------|----------|--------------|
| sufficient | C2 | boundary-seam, api-contract, coupling | verify (resolved by explore) |

## Knowledge Alignment
- Roadmap / Backlog: ADR-039 evolution order (E4 follows E2 ELK, E3 column-cut)
- Work Items / Specs: `sddk/explorations/graph-landing-page.md` (full explore)
- ADR / Architecture Sources: ADR-039 §2, §7, §9
- Ownership Source: E4 = frontend-led + backend endpoint
- Prior Learnings: E2 ELK re-runnable without remount; E3 pane-stack-only nav

## Knowledge Decisions
- Stays memory-only: None
- Promote to durable knowledge: This proposal (review needed for LandingPayload shape)

**Scope correction:** The request framed E4 as "Explorer-only, no backend changes" but then recommended Option A. Option A *requires* a new Rust endpoint. The recommendation wins — E4 scope includes a thin `GET /api/workspaces/:id/landing` handler. This is the clean pattern, matching every other `/api/*` route in `api.rs`.

## Lens Routing
| Lens | Delegation | Status | Proposal Impact |
|------|------------|--------|-----------------|
| base-discipline | kernel | applied | Context gate + scope/invariant discipline |
| boundary-seam | custom heuristic | deepened | Resolved Option A (single HTTP base) over B (dual HTTP+MCP) or C (in-process proxy). Keeps Explorer backend as sole UI source of truth. |
| api-contract | custom heuristic | applied | `LandingPayload` DTO mirrors existing `HotPathDto`, `SymbolDto`, `graph_insights` shapes from cognicode-core. Inner `GraphNode`/`GraphEdge` must stay identical for `toCytoscapeElements` reuse. |
| coupling-entropy | custom heuristic | verified | SWR key `["/workspaces/:id/landing", wsId]` is new but follows `useSubgraph` pattern. |

## Scope
### In Scope
- `GET /api/workspaces/:id/landing` → `LandingPayload { stats, entry_points, hot_paths, god_nodes, communities, suggested_questions }`
- New `GraphLanding` component: renders entry-points + hot-paths + god-nodes as a single `InteractiveGraph` (no synthetic root) + suggestion strip
- `useLanding(workspaceId)` SWR hook (null key when no workspace)
- Shell wiring flip: `activeObjectId === null` → render `GraphLanding` instead of falling through to empty state
- Fix: ensure `state.workspace` is populated on startup (currently `null` until Spotter triggers `SET_WORKSPACE`)
- MSW fixture + handler for landing; Shell test assertions updated

### Out Of Scope
- C4 perspective toggle (ADR-039 evolution item 3 → separate E5)
- Visual clustering algorithm (§7 "progressive disclosure" — defer to E5; v1 shows ≤50 nodes)
- Option C in-process MCP proxy (cross-crate refactor, rejected)

## Invariants
- `useSubgraph` contract unchanged — landing uses a separate hook
- Pane-stack is the only navigation mode — click → `SELECT_OBJECT` → existing pane flow
- `GraphNode`/`GraphEdge` shapes identical across `SubgraphResponse` and `LandingPayload`

## Domain Language
- Resolved Terms: Landing, root nodes (= entry points), hot paths (= high fan-in symbols), suggested questions (static `config/suggestedQuestions.ts` + backend `graph_suggest_questions`)
- Unresolved Ambiguities: default node count (propose 50 for v1)

## Capabilities
### New Capabilities
- `workspace-landing-graph`: render workspace-level structure in the graph canvas when no object is active
- `landing-data-fetch`: single round-trip workspace landing payload

### Modified Capabilities
- `interactive-graph`: accepts landing-mode data (entry points + hot paths merged into one `SubgraphResponse`-shaped payload) without a root node

## Approach
Backend: new `landing_handler` in `api.rs` composes existing facades (`GraphService` for hot paths/god nodes, `SearchService` for entry points, `WorkspaceService` for stats) into one `LandingPayload`. Fan-out is server-side — one round trip.
Frontend: `GraphLanding` merges `entry_points` + `hot_paths` + `god_nodes` into a `SubgraphResponse`-shaped object, passes to reused `InteractiveGraph`. Suggestion strip below. Click → `dispatch SELECT_OBJECT`.

## Affected Areas
| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/api.rs` | add | `landing_handler` + route `/api/workspaces/:id/landing` |
| `crates/cognicode-explorer/src/dto.rs` | add | `LandingPayload` DTO |
| `apps/explorer-ui/src/components/GraphLanding/` | new | Component module |
| `apps/explorer-ui/src/hooks/useLanding.ts` | new | SWR hook |
| `apps/explorer-ui/src/components/Shell.tsx` | modify | Panel flip: null → GraphLanding |
| `apps/explorer-ui/src/state/context.ts` | modify | Populate workspace on startup |
| `apps/explorer-ui/src/mocks/` | add | Fixture + MSW handler |

## Entropy Budget
| Metric | Estimate | Status |
|--------|----------|--------|
| Existing change entropy | medium | OK — facade composition mirrors AskRouter |
| New connascence | low-medium | OK — one new DTO, one new SWR key |

## Risks
| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `state.workspace` is null at startup — landing can't fetch | high | Wire workspace bootstrap on Shell mount (blocked: needs startup-path review in design) |
| `graph_status` = missing/indexing — no graph to show | medium | Landing renders "Scan first" hint (reuse ScanBar), 503 from endpoint |
| Visual density overwhelms at >50 nodes | medium | Cap at 50 for v1; clustering deferred to E5 |

## Rollback Plan
Revert Shell flip — `activeObjectId === null` returns to `InteractiveGraph` empty state. Remove `landing` route + hook. No data migration, no schema change.

## Success Criteria
- [ ] On Explorer load with a workspace and graph ready, left zone shows entry points + hot paths in the graph canvas (not "No graph data")
- [ ] Single HTTP round trip to `/api/workspaces/:id/landing`
- [ ] Click a landing node → pane opens in PaneStackView (existing flow, no new nav path)
- [ ] Suggestion strip renders 3-5 prompts
- [ ] `graph_status` missing/indexing → landing shows ScanBar hint, no crash
- [ ] MSW handler lets Playwright suite run without live backend
