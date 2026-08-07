# Design: e8-graphlanding-affordances

## Technical Approach

A single frontend PR that ships the working tree as three atomic commits:
(1) landing a11y + truncation banner, (2) artifact endpoint path alignment,
(3) e2e MSW-compat test rewrites. Branch: `feat/e8-graphlanding-affordances`
off `main`. PATCH semver (`v0.24.1`).

The change is purely additive on the frontend. Schema additions are
optional. The banner is dormant until the backend ships `truncated`
(`e8b-landing-payload-truncation`, separate cycle).

## Architecture Decisions

### Decision: D-1 Reuse `TruncationBanner` pattern from `contextual-views`

**Choice**: Render the landing banner inline in `GraphLanding.tsx` with
the same visual language (`var(--color-warning)` background, `text-xs`)
that `ContextualPanel` uses for its `TruncationBanner`. Do **not** extract
a shared `LandingTruncationBanner` component.
**Alternatives considered**: Extract `<TruncationBanner kind="landing" />`
shared by `ContextualPanel` and `GraphLanding`.
**Rationale**: The two banners live in different layout containers (panel
column vs. graph top) and have different prop shapes (`truncationReason`
vs. `truncated_reason`). Extracting them costs an abstraction with one
caller (today) and a second (tomorrow) that would still differ. Inline
now; refactor when a third caller appears.

### Decision: D-2 Memoize `selectObject` with `useCallback([dispatch])`

**Choice**: Lift the `SELECT_OBJECT` dispatch into a `useCallback` with
`[dispatch]` deps. Cytoscape's mount effect depends on `selectObject`,
not on `dispatch`.
**Alternatives considered**: Keep the dispatch inline in the handler;
let the effect depend on `[data, dispatch, ...]`.
**Rationale**: `dispatch` from React-Redux `useDispatch` is referentially
stable, so the callback is stable across renders. This prevents
cytoscape destroy+remount on unrelated state changes (which would
re-trigger the `circle` layout and flicker the canvas).

### Decision: D-3 Canvas `role="application"` + node list fallback

**Choice**: Add `role="application"`, `aria-label`, `tabIndex={0}` to
the canvas div. Add a fallback `<div data-testid="graph-landing-node-list">`
below the canvas that renders one `<button>` per node.
**Alternatives considered**: Make the cytoscape canvas itself keyboard-
navigable; remove the canvas entirely.
**Rationale**: Cytoscape does not support keyboard navigation natively;
building a parallel keyboard model on top is high cost, low payoff.
A button list gives screen readers a flat, ordered alternative and gives
e2e tests a stable selector (`graph-node-{id}`) independent of canvas
coordinates.

### Decision: D-4 MSW override via `page.addInitScript`

**Choice**: Replace `page.route(...)` with `page.addInitScript(() => {
window.fetch = ... })`. The script runs in the page's main world before
any other script (including MSW's worker bootstrap) and short-circuits
matching URLs.
**Alternatives considered**: Use `page.route` from inside the SW context;
disable MSW per-test via MSW's `server.resetHandlers(...)`.
**Rationale**: `page.route` operates on the browser context's network
stack, which MSW bypasses by intercepting at the `fetch` level inside the
page. `server.resetHandlers(...)` would require test code to import MSW
into Playwright tests, breaking the test/app boundary. `addInitScript`
runs before everything else in the page and is the simplest fix.

### Decision: D-5 MSW wildcard `*/api/exploration-sessions/...`

**Choice**: The artifact handler is registered as
`*/api/exploration-sessions/:exploration_id/artifacts` (wildcard `*/`
prefix).
**Alternatives considered**: Register two handlers — one with `/api`
prefix and one without.
**Rationale**: SWR constructs the cache key without the `/api` prefix
(`/exploration-sessions/{id}/artifacts/{format}` historically); the fetch
URL goes through a client wrapper that adds `/api`. The wildcard matches
both URL forms with one handler. msw 1.x supports `*/` glob prefix
natively.

### Decision: D-6 Quality summary mock is dev-only, no contract

**Choice**: Add the `/api/workspaces/:workspace_id/quality-summary` mock
handler with a fixed fixture. Do NOT add a corresponding zod schema or
hook in this change.
**Alternatives considered**: Add a `useQualitySummary` hook + zod schema
matching the mock.
**Rationale**: Without a backend, the hook would always be a lie. Adding
the hook now risks a schema/contract mismatch when the backend lands. The
mock is sufficient for landing-page visual development; the real wiring
belongs in the cycle that ships the backend endpoint.

## Data Flow

### Truncation Banner

```
Backend LandingPayload
  { truncated: bool, truncated_reason: Option<String>, ... }
        │
        ▼ fetchLanding(workspaceId)            (apps/explorer-ui/src/api/client.ts)
        │
        ▼ landingPayloadSchema.parse(body)
        │
        ▼ useLanding(workspaceId)               (SWR-cached, dedupingInterval=10s)
        │
        ▼ GraphLanding({ workspaceId })
        │
        ▼ {data.truncated ? <Banner /> : null}  (line 216)
        │
        ▼ DOM <div data-testid="graph-landing-warning">
```

### Canvas Click → Pane Stack (no change, refactored)

```
cy.on("tap", "node", handler)
  │
  ▼ selectObject(id)        (useCallback, [dispatch])
  │
  ▼ dispatch({ type: "SELECT_OBJECT", payload: { objectId, viewId: "overview" }})
  │
  ▼ Shell reducer           (apps/explorer-ui/src/state/context.tsx)
  │
  ▼ PaneStackView renders new pane (already implemented)
```

### Node List Fallback (new)

```
<button data-testid="graph-node-{id}" onClick={() => selectObject(id)}>
  │
  ▼ same SELECT_OBJECT path as canvas
```

### Artifact Fetch (fixed)

```
useArtifact(explorationId, format)
  │
  ▼ SWR key: /api/exploration-sessions/{id}/artifacts/{format}
  │
  ▼ artifactFetcher(key) → fetch(`/api/exploration-sessions/{id}/artifacts/{format}`)
  │
  ▼ MSW mock: */api/exploration-sessions/:exploration_id/artifacts
  │
  ▼ DecisionArtifactSummary
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/api/schemas.ts` | Modify | +2 optional fields on `landingPayloadSchema` |
| `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` | Modify | +68/-13 LOC: banner, a11y attrs, fallback list, useCallback |
| `apps/explorer-ui/src/hooks/useExplorations.ts` | Modify | 1-line path fix at line 181 |
| `apps/explorer-ui/src/mocks/handlers.ts` | Modify | +35/-1 LOC: wildcard artifact handler, new quality-summary mock |
| `apps/explorer-ui/src/mocks/landingFixtures.ts` | Modify | +2 fixture fields |
| `apps/explorer-ui/src/components/Shell.test.tsx` | Modify | testid coverage for graph-landing |
| `apps/explorer-ui/src/hooks/hooks.test.ts` | Modify | 1-line artifact path fix in test |
| `apps/explorer-ui/e2e/landing.spec.ts` | Modify | P1.7, P1.8 use addInitScript |
| `apps/explorer-ui/e2e/error-states.spec.ts` | Modify | P5.1, P5.3, P5.4 use addInitScript |
| `apps/explorer-ui/e2e/pane-stack.spec.ts` | Modify | `openSpotterResult` helper supports index |
| `apps/explorer-ui/e2e/**/snapshots/*.png` | Regenerate | 24 PNGs |
| `apps/explorer-ui/src/tailwind.css` | Modify | 1-line comment fix |
| `apps/explorer-ui/artifacts/e7-renderer-bench/**` | Exclude | Add to `.gitignore` (or PR-excluded) |

## Interfaces / Contracts

```ts
// apps/explorer-ui/src/api/schemas.ts
export const landingPayloadSchema = z.object({
  workspace: workspaceSummarySchema,
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
  entry_points: z.array(inspectableObjectSummarySchema),
  hot_paths: z.array(inspectableObjectSummarySchema),
  god_nodes: z.array(godNodeEntrySchema),
  suggested_questions: z.array(z.string()),
  graph_status: graphStatusSchema,
  truncated: z.boolean().optional(),
  truncated_reason: z.string().nullable().optional(),
});

// MSW handler (apps/explorer-ui/src/mocks/handlers.ts)
http.post(
  "*/api/exploration-sessions/:exploration_id/artifacts",
  async ({ request }) => { /* ... */ }
);

http.get(
  "/api/workspaces/:workspace_id/quality-summary",
  async () => HttpResponse.json({
    summary: { /* fixed fixture */ },
    issues: [ /* fixed fixture */ ],
  })
);

// a11y attrs on canvas div
<div
  ref={containerRef}
  data-testid="graph-landing-canvas"
  role="application"
  aria-label={`${perspective === "c4" ? "Architecture" : "Workspace"} landing graph`}
  tabIndex={0}
  style={{ flex: "1 1 auto", minHeight: 0 }}
/>
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Schema accepts `truncated` optional | `subgraph_schemas.test.ts` already covers missing `truncated_reason`; add a case for `truncated: true` |
| Unit | `Shell.test.tsx` includes new testids | Already added; verify in PR diff |
| Integration | Hooks/handlers test path fix | `hooks.test.ts` line 334 already updated; verify the test passes |
| E2E | Banner renders when fixture truncated | `landing.spec.ts` — extend P1.7 to assert `graph-landing-warning` is absent in error case |
| E2E | Override survives MSW | `error-states.spec.ts` P5.1, P5.3, P5.4 |
| E2E | Second pane via `openSpotterResult(page, 1)` | `pane-stack.spec.ts` |
| Visual regression | 24 snapshots | Re-baseline after merge; verify growth is uniform across snapshots |
| Build | `just explorer-build` | TypeScript + Vite; no new types required |
| Lint | `just explorer-lint` | Tailwind class consistency on the new banner div |

## Migration / Rollout

No migration required. Pure frontend changes; no DB, no Rust binary.

Rollout: PR lands → squash-merge to main → `v0.24.1` tag. No feature
flag; the banner is dormant (`truncated` defaults to absent) until the
backend ships.

## Open Questions

- [ ] Should the node-list fallback also receive a virtualization
      treatment for workspaces >500 nodes? Deferred — flag in
      `e8-graphlanding-affordances` archive-report and punt to
      `e9-landing-perf` if needed.
- [ ] Should the MSW `*/api/exploration-sessions/...` wildcard be
      back-ported to other handlers that may suffer the same SWR-key
      drift? Out of scope for this PR; tracked separately.
- [ ] Should `e7-renderer-bench/` artifacts move to a `benchmarks/`
      directory at the repo root instead of being gitignored? Decision
      deferred to a repo-hygiene cycle.
