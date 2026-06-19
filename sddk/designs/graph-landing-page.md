# Kernel Design: Graph Landing Page (E4 / ADR-039)

## Context Reuse Check
| Input | Status | Notes |
|-------|--------|-------|
| Knowledge coverage | present | Reused ADR-039 §2/§7, exploration `sddk/explorations/graph-landing-page.md`, proposal `sddk/proposals/graph-landing-page.md`. |
| Exploration | present | C2 exploration fully reused; no re-exploration. |
| Proposal/spec alignment | ok (corrected) | Proposal Option A endorsed. **Two citations corrected**: `useWorkspaceList` lives in `useWorkspace.ts` (not its own file); the `workspaces?.[0]?.id` auto-select is `Spotter.tsx:117-118`, not `useSpotter.ts:192` (hook is 53 lines). Blocker diagnosis stands. |
| Code verification | ok | `Shell.tsx:99,185,230`, `context.ts:127,158`, `adapter.ts:15`, `useWorkspace.ts:32`, `api.rs:437,477,594`, `client.ts:382`, `schemas.ts:143`, `aix_handlers.rs:82` all read. |
| Context quality | C2 | Effort = verify; no deepen required. |
| Problem taxonomy | present | boundary-seam + api-contract + coupling-entropy reused from proposal. |
| Domain language | present | Landing, entry points, hot paths, god nodes, suggested questions resolved. `graph_status ∈ {missing,stale,ready,indexing}` confirmed. |
| Recommended effort | verify | Shaped a thin composition + one DTO + one hook; deferred clustering to E5. |

## Technical Approach
A single `GET /api/workspaces/:workspace_id/landing` handler composes the same three sources `SmartOverviewDto` (`aix_handlers.rs:82`) already composes — stats + entry points + hot paths — plus god nodes. One round trip; Explorer HTTP is the sole UI source of truth (rejects the MCP dual-base Option B). The frontend renders a dedicated `GraphLanding` (its own `InteractiveGraph` mount + suggestion strip) when `activeObjectId === null`, replacing today's "No graph data" empty state. The startup null-workspace blocker is fixed by a one-shot bootstrap effect, **not** by duplicating the workspace fetch.

## Knowledge Impact
- Durable artifacts reused: ADR-039, E2/E3 learnings (ELK re-runnable w/o remount; pane-stack-only nav), `SmartOverviewDto` aggregation pattern, `toCytoscapeElements` adapter, `makeSwrFetcher`+zod-at-boundary.
- Artifacts that may become stale: none (no existing endpoint changes shape). E5 clustering will *extend* `LandingPayload`, not supersede it.
- Memory-only learnings consulted: E4 exploration findings (#2790-equivalent in `sddk/explorations/`).

## Applied Lenses
| Lens | Delegation | Status | Why Applied | Design Impact |
|------|------------|--------|-------------|---------------|
| base-discipline | kernel | applied | Always active | Enforced zod-at-boundary, invariant discipline, 50-node v1 cap. |
| boundary-seam | custom heuristic | deepened | Explorer HTTP vs MCP `:9847` dual-base | Single `ApiState`-injected handler; frontend keeps one `/api` base. |
| api-contract | custom heuristic | deepened | New DTO mirrors `HotPathDto`/`SymbolDto` | Inner `GraphNode`/`GraphEdge` kept identical to `SubgraphResponse` so `toCytoscapeElements` reuses verbatim. **Added `edges` to payload** (user's shape omitted it — adapter needs them). |
| coupling-entropy | custom heuristic | verified | New SWR key + connascence of meaning | SWR key `/workspaces/:id/landing`; GraphNode meaning pinned identically across both responses. |

## Invariants And Constraints
| Invariant / Constraint | Enforcement Point | Verification |
|------------------------|-------------------|--------------|
| `useSubgraph` contract unchanged | Separate `useLanding` hook, distinct SWR key | Hook test |
| Pane-stack is the only nav mode | Click → `SELECT_OBJECT` (no new action) | Shell test |
| `GraphNode`/`GraphEdge` identical across responses | Shared `graphNodeSchema`/`graphEdgeSchema` | Schema parity test |
| zod validates at the boundary | `makeSwrFetcher(landingPayloadSchema)` | Boundary test |
| ≤50 nodes v1 | Server-side cap + `truncated` flag | Handler test |
| `state.workspace` populated before fetch | Bootstrap effect guarded by `!workspace` | Shell mount test |

## Architecture Decisions
| Decision | Choice | Alternatives Considered | Rationale |
|----------|--------|-------------------------|-----------|
| Data source | New HTTP endpoint (Option A) | B (MCP `:9847`), C (in-process proxy) | Matches every `/api/workspaces/:id/*` route; single base; reuses zod+SWR. |
| Workspace bootstrap | One-shot `useBootstrapWorkspace()` effect in Shell | Auto-init from localStorage; duplicate fetch | Reuses SWR-deduped `useWorkspaceList`; no new persistence; mirrors `Spotter.tsx:118` selection. |
| Missing/indexing graph | `200` with `graph_status` + empty arrays | `503` | Degraded render (ScanBar hint) cleaner than error-handling 503; one happy path. |
| Edges in payload | Include `edges: GraphEdge[]` | Nodes-only | `toCytoscapeElements(nodes, edges)` requires both. |

## Data Flow
```text
Shell mount ── useBootstrapWorkspace() ── dispatch SET_WORKSPACE(list[0])
   │
activeObjectId === null?
   ├─ yes → GraphLanding
   │          ├─ useWorkspaceList() (SWR-deduped) → wsId
   │          ├─ useLanding(wsId) ──GET /api/workspaces/:id/landing──► LandingPayload
   │          ├─ toCytoscapeElements(nodes, edges) + style_class color buckets
   │          └─ LandingSuggestionStrip (static workspace prompts)
   └─ no  → InteractiveGraphPanel (unchanged)

node click → dispatch SELECT_OBJECT {objectId, viewId:"overview"} → PaneStackView
```

## File Changes
| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/api.rs` | modify | `landing_handler(State, Path<ws_id>)` + route in **both** `router` and `router_with_state` (lines ~437,477). |
| `crates/cognicode-explorer/src/dto.rs` | add | `LandingPayload` (mirrors `SmartOverviewDto` field set + `edges` + `graph_status`). |
| `crates/cognicode-explorer/src/facades/` | add/extend | Compose `WorkspaceService`(stats) + `SearchService`(entry points) + hot-paths/god-nodes; reuse `get_hot_paths_from_graph` semantic. |
| `apps/explorer-ui/src/api/schemas.ts` | add | `landingPayloadSchema` reusing `graphNodeSchema`/`graphEdgeSchema`/`inspectableObjectSummarySchema`. |
| `apps/explorer-ui/src/hooks/useLanding.ts` | new | `useLanding(workspaceId)` — null key when null. |
| `apps/explorer-ui/src/components/GraphLanding/` | new | `GraphLanding.tsx`, `LandingHeader.tsx`, `LandingSuggestionStrip.tsx`. |
| `apps/explorer-ui/src/components/Shell.tsx` | modify | Panel flip (line ~185/230) + `useBootstrapWorkspace()` mount effect. |
| `apps/explorer-ui/src/mocks/{fixtures,handlers}.ts` | add | `landingFixture` + MSW handler. |

## Interfaces / Contracts
```rust
// dto.rs — wire shape (snake_case over the wire)
pub struct LandingPayload {
    pub workspace: WorkspaceSummary,
    pub graph_status: GraphStatus,          // missing|stale|ready|indexing
    pub stats: GraphStats,
    pub entry_points: Vec<InspectableObjectSummary>, // ≤10
    pub hot_paths: Vec<InspectableObjectSummary>,    // ≤10
    pub god_nodes: Vec<GodNodeDto>,                  // ≤5 {id, score}
    pub communities: Vec<CommunityDto>,              // ≤3 {id, label, size, cohesion}
    pub nodes: Vec<GraphNode>,            // ← identical to SubgraphResponse.nodes
    pub edges: Vec<GraphEdge>,            // ← identical to SubgraphResponse.edges
    pub truncated: bool,
    pub suggested_questions: Vec<String>,            // 3-5
}
```
```ts
// useLanding.ts
export function useLanding(workspaceId: string | null, opts?: SWRConfiguration)
  : SWRResponse<LandingPayload, ApiError>;   // key null when workspaceId null
```
```tsx
// Signature parity with existing InteractiveGraph mount
function GraphLanding(): JSX.Element            // reads workspaceId from state
function LandingHeader({stats, graphStatus, onScan}): JSX.Element
function LandingSuggestionStrip({questions, onAsk}): JSX.Element
```
Node color buckets via existing `style_class`: `entry_point`→green, `hot`→amber, `god`→purple (backend tags `style_class`; adapter passes it through unchanged).

## Entropy Constraints
| Interface/Module | Risk | Constraint |
|------------------|------|------------|
| `GraphNode`/`GraphEdge` meaning | drift between responses | Pin to shared schema; parity test. |
| Landing SWR key | cache collision | Distinct `/workspaces/:id/landing`; never reuse subgraph key. |
| Bootstrap effect | double-dispatch on StrictMode | Guard `if (!state.workspace && workspaces?.[0])` once. |
| Node density | O(n²) layout at >50 | Server cap + `truncated`; clustering deferred to E5. |

## Testing Strategy
| Layer | What To Test | Approach |
|-------|--------------|----------|
| Rust unit | `landing_handler` composition + 50-cap + empty-graph | mock `ApiState` facades (existing test pattern api_graph_tests.rs) |
| Rust contract | `404` unknown ws; `200`+`graph_status` on missing graph | axum one-shot test |
| TS schema | `landingPayloadSchema` parity with `subgraphResponseSchema` node/edge shapes | zod parse test |
| TS hook | `useLanding(null)` → no fetch; resolves payload | MSW + SWR test (hooks.test.ts pattern) |
| Component | render graph + strip; click → `SELECT_OBJECT` dispatched; missing-graph → ScanBar hint | RTL + dispatch spy |
| Shell | null `activeObjectId` renders `graph-landing`, non-null renders `interactive-graph` | existing Shell.test.tsx updated |

## Migration / Rollout
No data migration, no schema change. Rollback = revert Shell flip + remove route/hook (proposal §Rollback Plan). MSW fixture enables the Playwright suite without a live backend.

## Open Questions
- None blocking. E5 will extend `LandingPayload` with clustering + C4 perspective (deferred per ADR-039 evolution order).
