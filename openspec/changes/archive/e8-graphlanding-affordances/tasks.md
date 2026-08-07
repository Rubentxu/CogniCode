# Tasks: e8-graphlanding-affordances

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~280 source-code LOC + 24 regenerated snapshots + 5 test files |
| 400-line budget risk | Medium (close to budget when including tests; trivial when excluding snapshots) |
| Chained PRs recommended | Yes (3 PRs) |
| Suggested split | PR-1: landing a11y + truncation banner · PR-2: artifact endpoint path · PR-3: e2e MSW-compat + snapshot re-baseline |
| Delivery strategy | ask-on-risk |
| Chain strategy | stacked-to-main |
| Decision needed before apply | Yes |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Landing a11y + truncation banner | PR-1 | Base: `main`. ~80 source LOC + 2 fixtures + GraphLanding + schemas. Independent of PR-2. |
| 2 | Artifact endpoint path alignment | PR-2 | Base: `main`. 1 hook line + 1 mock line + 1 test line. Independent of PR-1. |
| 3 | E2E MSW compat + snapshot re-baseline | PR-3 | Base: `main`. 3 e2e specs + 24 snapshots + Shell.test.tsx. Depends on PR-1+PR-2 being conceptually shipped (snapshots in this PR represent the post-PR-1 UI). |

Each PR is independently mergeable. The visual-regression snapshots in PR-3
must be regenerated AFTER PR-1+PR-2 land so the baseline reflects the new
UI.

---

## Phase 1: PR-1 — Landing a11y + Truncation Banner

- [x] 1.1 In `apps/explorer-ui/src/api/schemas.ts`, add `truncated: z.boolean().optional()` and `truncated_reason: z.string().nullable().optional()` to `landingPayloadSchema`
- [x] 1.2 In `apps/explorer-ui/src/mocks/landingFixtures.ts`, set `truncated: false, truncated_reason: null` on `landingFixture`
- [x] 1.3 In `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx`, lift `SELECT_OBJECT` dispatch into `selectObject = useCallback(..., [dispatch])`
- [x] 1.4 Same file, change the cytoscape mount `useEffect` deps to `[data, isGraph, landingData, godNodes, selectObject]` (remove `dispatch`)
- [x] 1.5 Same file, add `role="application"`, `aria-label` (perspective-aware), `tabIndex={0}` to the canvas div
- [x] 1.6 Same file, render truncation banner above the canvas when `data.truncated` is true, using `data-testid="graph-landing-warning"`
- [x] 1.7 Same file, render fallback `<div data-testid="graph-landing-node-list">` below the canvas with one `<button data-testid="graph-node-{id}">` per node
- [x] 1.8 Run `just explorer-test` and verify no regressions in existing GraphLanding tests

## Phase 2: PR-2 — Artifact Endpoint Path Alignment

- [x] 2.1 In `apps/explorer-ui/src/hooks/useExplorations.ts` line 181, change `/explorations/${id}/artifacts/${format}` to `/exploration-sessions/${id}/artifacts/${format}` (NO `/api` prefix — `apiGet` already adds it via `DEFAULT_BASE = "/api"` in `client.ts:22`)
- [x] 2.2 In `apps/explorer-ui/src/mocks/handlers.ts` line 272, change `/api/explorations/:exploration_id/artifacts` to `*/api/exploration-sessions/:exploration_id/artifacts`
- [x] 2.3 In `apps/explorer-ui/src/hooks/hooks.test.ts` line 334, mirror the path change in the `generateArtifact` test
- [x] 2.4 Run `just explorer-test` and confirm `generateArtifact` test passes

## Phase 3: PR-3 — E2E MSW-compat + Snapshot Re-baseline

- [x] 3.1 In `apps/explorer-ui/e2e/landing.spec.ts`, replace `page.route("**/api/workspaces/*/landing**", ...)` overrides in P1.7 and P1.8 with `page.addInitScript` fetch wrappers
- [x] 3.2 In `apps/explorer-ui/e2e/error-states.spec.ts`, replace `page.route(...)` overrides in P5.1, P5.3, P5.4 with `page.addInitScript` fetch wrappers
- [x] 3.3 In `apps/explorer-ui/e2e/pane-stack.spec.ts`, rename `openFirstSpotterResult(page)` to `openSpotterResult(page, resultIndex = 0)` and update the 2nd-pane test to pass index `1`
- [x] 3.4 In `apps/explorer-ui/src/components/Shell.test.tsx`, add `graph-landing` and `graph-landing-loading` testids to the empty-state assertion
- [x] 3.5 `just explorer-build` confirmed TypeScript and Vite pass on PR-1 branch
- [x] 3.6 `just explorer-test` confirmed 671/671 pass on combined branches post-merge
- [x] 3.7 Snapshot regeneration done post-merge as a separate commit (not in PR-3); 67/67 e2e tests pass after re-baseline
- [x] 3.8 Snapshot growth matches expected pattern (C4 routes ~22KB→46KB, full-flow ~75KB→114KB)
- [x] 3.9 `apps/explorer-ui/artifacts/` added to `.gitignore`; the 2 pre-existing tracked files removed from tracking via `git rm --cached`

## Phase 4: Verification & Cleanup

- [x] 4.1 `apps/explorer-ui/artifacts/e7-renderer-bench/**` is NOT in the PR diff
- [x] 4.2 `just explorer-build` passes (TypeScript + Vite)
- [x] 4.3 `docs/ROADMAP.md` updated with E8 in Completed section
- [x] 4.4 PRs #56, #57, #58 squash-merged to main; tag `v0.24.1` pushed
- [x] 4.5 Change folder moved from `openspec/changes/` to `openspec/changes/archive/`
- [x] 4.6 Spec promoted to `openspec/specs/graphlanding-affordances/spec.md`
