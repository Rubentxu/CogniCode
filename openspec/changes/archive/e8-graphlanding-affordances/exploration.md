# Exploration: e8-graphlanding-affordances

## Current State

The Explorer UI's `GraphLanding` component (the landing page shown when no object
is selected) is functional but **diverged from the backend contract** in three
independent ways. Working tree at `main` (8a0d6fe) contains 41 files modified
across `apps/explorer-ui/` that collectively fix this drift and add new UX. None
of the changes are committed, branched, or covered by an `openspec/changes/`
proposal.

The drift has three independent roots, each diagnosable to a specific
backend/frontend contract gap.

### Drift 1: Landing truncation not surfaced

The backend already truncates and reports `truncated` + `truncated_reason` for
two related payloads, but never for `LandingPayload`:

- `SubgraphResponse` (`crates/cognicode-explorer/src/facades/graph.rs:187-188`)
  returns `{ truncated, truncated_reason: Option<String> }`.
- `ContextualGraphResponse` (`crates/cognicode-explorer/src/facades/view.rs:268-284`)
  returns `{ truncated, truncation_reason: Option<String> }` (note: different
  field name).
- `LandingPayload` (`crates/cognicode-explorer/src/dto.rs:782-799` and built in
  `api.rs:669-688`) has **neither field**. The handler currently stubs an empty
  payload — `nodes: Vec::new()`, `entry_points: Vec::new()`, etc. (TODO comment
  at `api.rs:670-671`).

On the frontend, `LandingPayload` schema (`apps/explorer-ui/src/api/schemas.ts:1226-1231`)
now declares `truncated: z.boolean().optional()` and `truncated_reason:
z.string().nullable().optional()`. The mock handler
(`apps/explorer-ui/src/mocks/landingFixtures.ts:117-118`) emits `truncated: false,
truncated_reason: null`. `GraphLanding.tsx:216-231` renders a banner when
`truncated` is true. **But the real backend will never return these fields, so
the banner never shows in production.** Tests in
`ContextualPanel/ContextualPanel.test.tsx:181,210` and
`InteractiveGraph/InteractiveGraph.test.tsx:283,398-399,437-438` already exercise
truncation for the contextual and subgraph views, but not for the landing.

### Drift 2: Artifact endpoint path regression

`useExplorations.ts:181` historically called
`/explorations/${id}/artifacts/${format}`. ADR-040 Wave 3 renamed the backend
route to `/api/exploration-sessions/{id}/...`. The hook was never migrated:

- `useExplorations.ts:158,164,181` now calls
  `/api/exploration-sessions/${encodeURIComponent(explorationId)}/...` (fixed).
- Mock handler in `apps/explorer-ui/src/mocks/handlers.ts:272` is also fixed to
  `*/api/exploration-sessions/:exploration_id/artifacts` (with wildcard
  `*/` prefix to match both `/api/...` and the empty-prefix SWR key).
- Backend already serves these routes
  (`crates/cognicode-explorer/src/api_graph_tests.rs:1711,1732`).
- `ShareExplorationButton.tsx:7` and `useRestoreExploration.ts:18` were
  already correct (they didn't have the regression).

Result: before this fix, calling `useArtifact(id, "json")` from the Explorer UI
would 404 against the real backend and silently fail.

### Drift 3: Playwright route interception broke under MSW

`e2e/error-states.spec.ts`, `e2e/landing.spec.ts`, and `e2e/pane-stack.spec.ts`
were using `page.route("**/api/...", ...)` to override HTTP responses in tests.
After MSW was wired into the Explorer UI's test harness, MSW intercepts
requests inside the page context **before** Playwright's network layer sees
them. The route overrides became no-ops, so tests P1.7, P1.8, P5.1, P5.3, P5.4
were silently running against the default mock responses instead of the
intended error/empty/large-graph scenarios.

The fix swaps `page.route()` for `page.addInitScript()` that overrides
`window.fetch` directly — this runs before MSW bootstraps and lets the test
short-circuit specific URLs.

## What the working tree changes

The working tree is a single coherent unit. It is **not** three independent
changes; the three drifts share the same root cause (UI work that landed
without a formal SDD cycle), and a11y improvements piggy-back on the truncation
banner to make the landing usable without a working canvas.

### Code changes (10 source files, +278/-180 LOC)

| File | Change | Why |
|---|---|---|
| `apps/explorer-ui/src/api/schemas.ts` | +2 lines (`truncated`, `truncated_reason` optional) | Schema accepts new fields from backend (when backend ships them) |
| `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` | +68/-13 lines (banner, role/aria-label/tabIndex, node list fallback, useCallback) | Render truncation banner + a11y + memoize selectObject |
| `apps/explorer-ui/src/hooks/useExplorations.ts` | 1 line path fix | Drift 2 fix |
| `apps/explorer-ui/src/mocks/handlers.ts` | +35/-1 lines (new quality-summary mock + artifact path fix) | Drift 2 fix + new mock |
| `apps/explorer-ui/src/mocks/landingFixtures.ts` | +2 lines | Schema fixture includes new fields |
| `apps/explorer-ui/src/tailwind.css` | 1-line comment fix | Incidental cleanup |

### Test changes (5 files)

| File | Change |
|---|---|
| `e2e/landing.spec.ts` | P1.7, P1.8 — `page.route` → `addInitScript fetch` override |
| `e2e/error-states.spec.ts` | P5.1, P5.3, P5.4 — same swap |
| `e2e/pane-stack.spec.ts` | Refactor `openFirstSpotterResult` → `openSpotterResult(page, index)` for second-tab tests |
| `src/components/Shell.test.tsx` | Adds `graph-landing` + `graph-landing-loading` testids to the empty-state check |
| `src/hooks/hooks.test.ts` | 1-line artifact path fix in the generateArtifact test |

### Snapshot updates (24 PNGs)

Every visual-regression snapshot in `e2e/**/snapshots/` is regenerated. The
dominant cause is the **fallback node list** (button row) appearing below the
cytoscape canvas, which extends the page height and shifts subsequent regions.
Snapshots that grew most (~22KB → ~46KB, ~75KB → ~114KB) are the C4 perspective
flows because the new node list is dense and visually distinctive against the
c4 palette.

### Benchmark artifacts (`apps/explorer-ui/artifacts/e7-renderer-bench/`)

`report.md` and `results.json` were updated by an E7 benchmark run that
happened on this branch. **These are not part of the proposed change** —
they are runtime artifacts that should not be committed. They need to be
excluded from the PR (gitignored or moved).

## Affected areas

```
apps/explorer-ui/src/
├── api/schemas.ts                                  ← Drift 1
├── components/GraphLanding/GraphLanding.tsx        ← Drift 1 + a11y + refactor
├── components/Shell.test.tsx                       ← testid coverage
├── hooks/useExplorations.ts                        ← Drift 2
├── hooks/hooks.test.ts                             ← Drift 2 test fix
├── mocks/handlers.ts                               ← Drift 2 mock + quality-summary
├── mocks/landingFixtures.ts                        ← Drift 1 fixture
└── tailwind.css                                    ← cosmetic

apps/explorer-ui/e2e/
├── landing.spec.ts                                 ← Drift 3
├── error-states.spec.ts                            ← Drift 3
├── pane-stack.spec.ts                              ← helper refactor
└── **/snapshots/                                   ← 24 PNGs (visual drift)

apps/explorer-ui/artifacts/e7-renderer-bench/       ← runtime artifacts, exclude
```

Backend (NOT in working tree but contractually affected):

```
crates/cognicode-explorer/src/dto.rs:779-799        ← LandingPayload needs truncation fields
crates/cognicode-explorer/src/api.rs:639-691        ← handler currently returns empty stubs
```

## Approaches

### Approach A: Frontend-only change + land first, backend in a follow-up

The frontend changes are self-contained and shippable on their own. The
truncation banner stays dormant until the backend ships the fields. The
artifact path fix and Playwright swap are independent wins.

- **Pros**: Smallest PR; doesn't touch Rust; fast to merge; a11y improvements
  ship now.
- **Cons**: The banner is dead UI until a follow-up cycle ships backend
  truncation. The mock `truncated: false` is misleading because no real
  payload has these fields.
- **Effort**: Low.

### Approach B: Frontend + backend in a chained pair (recommended)

Frontend PR (this change) + a follow-up backend PR that adds
`truncated: bool` and `truncated_reason: Option<String>` to `LandingPayload`
and sets them in `landing_handler` based on actual node count vs the backend's
landing cap. Mirrors how `SubgraphResponse` handles it. This is what the
schema already declares.

- **Pros**: Coherent. The banner is live the moment it ships. Follow-up can be
  a small backend-only PR with its own review focus.
- **Cons**: Two PRs instead of one. Backend PR needs analysis service
  integration that may not be ready.
- **Effort**: Medium.

### Approach C: Drop the truncation banner entirely

Realize the backend doesn't return truncation for the landing (because the
landing handler is still a TODO), and revert the banner / schema additions.
Ship only the path fix + Playwright swap + a11y.

- **Pros**: No half-feature in the UI.
- **Cons**: Loses the a11y improvements (canvas role + fallback node list) which
  are valuable on their own. The banner code is small and forward-compatible.
- **Effort**: Lowest.

## Recommendation

**Approach B with the frontend PR as `e8-graphlanding-affordances` (PATCH
v0.24.1), and the backend truncation as a separate cycle
`e8b-landing-payload-truncation` once the landing handler is implemented
beyond stubs.**

Rationale:
- The frontend PR is independently shippable today: the path fix and Playwright
  swap are real bug fixes; the a11y work is real UX improvement; the banner is
  forward-compatible code that activates when the backend lands.
- The banner code stays in the bundle (≤30 LOC) and is correctly wired — no
  half-state to clean up later.
- The backend cycle is a follow-up that follows naturally from
  `crates/cognicode-explorer/src/api.rs:670-671` ("TODO: Wire get_entry_points,
  get_hot_paths, graph_insights...").
- It matches the pattern already established for `SubgraphResponse` and
  `ContextualGraphResponse`.

## Risks

- **Snapshot drift cascade**: 24 PNGs are regenerated. Any future visual
  regression in unrelated regions will be harder to spot against a
  just-regenerated baseline. The recommendation is to land this PR alone and
  do a clean re-snapshot only after this lands.
- **MSW + Playwright interaction**: The init-script fetch override runs before
  MSW. If MSW later changes its registration order (e.g., moves from
  init-script to module-level), the override may stop intercepting. Document
  this in the test helper.
- **Benchmark artifacts**: `e7-renderer-bench/{report.md,results.json}` should
  not be committed. They are output, not source. Verify `.gitignore` covers
  `apps/explorer-ui/artifacts/**` or explicitly exclude in the PR.
- **Mock-only quality summary endpoint**: The new mock at
  `/api/workspaces/:workspace_id/quality-summary` has no backend. The proposal
  must NOT pretend it is wired — it is dev-only scaffolding for upcoming work.
  Mark it explicitly.
- **Node list fallback perf**: For very large workspaces (the spec mentions
  `>500` nodes triggering the warning), the fallback list renders **all** nodes
  as buttons. With 500 nodes, that's 500 DOM buttons. Need to verify
  `data-testid="graph-landing-node-list"` does not explode DOM size in
  extreme cases. Probably fine for v0.24.x — flag for follow-up.

## Ready for Proposal

**Yes.** The three drifts are well-scoped, the backend contract is clear, the
approach (B) is recommendable, and the change can be cut as a single
frontend-focused PR with PATCH semver.
