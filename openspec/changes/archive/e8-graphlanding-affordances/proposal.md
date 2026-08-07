# Proposal: e8-graphlanding-affordances

## Intent

The Explorer UI's landing page (`GraphLanding`) has accumulated three
independent drift bugs and one UX gap against its backend contract. The
working tree at `main` already contains the fixes (41 modified files), but
the change was never formalized through the SDD workflow. This proposal
captures the change so it can land as a single, reviewable PR with a clear
rollback path.

The user-facing outcome is:
1. The "Showing a truncated landing graph" banner becomes live (forward
   compatibility — activates when the backend lands the field).
2. The cytoscape canvas gets `role="application"` + `aria-label` +
   `tabIndex={0}`, plus a fallback list of buttons so screen-reader and
   keyboard-only users can navigate without the canvas.
3. The artifact generation flow stops 404-ing against the real backend.
4. Six e2e tests in `landing.spec.ts` and `error-states.spec.ts` start
   actually exercising their intended error/empty/large-graph scenarios
   instead of silently running against the default mock responses.

## Scope

### In Scope

- Add `truncated` + `truncated_reason` optional fields to
  `LandingPayload` zod schema (`apps/explorer-ui/src/api/schemas.ts`).
- Render truncation banner in `GraphLanding.tsx` when `truncated` is true.
- Add a11y affordances to `GraphLanding.tsx`: `role="application"`,
  `aria-label`, `tabIndex={0}` on canvas; fallback button list below.
- Memoize `selectObject` with `useCallback` so cytoscape doesn't
  re-mount on every dispatch.
- Fix artifact endpoint path in `useExplorations.ts` from
  `/explorations/...` to `/api/exploration-sessions/...`.
- Fix mock handler `*/api/exploration-sessions/:id/artifacts` accordingly.
- Add mock endpoint `/api/workspaces/:workspace_id/quality-summary` to
  MSW (dev-only; **no real backend endpoint exists**).
- Update `landingFixtures.ts` to include new optional fields.
- Update e2e tests to use `addInitScript` fetch override (MSW compat).
- Update `Shell.test.tsx` to include new `graph-landing*` testids.
- Regenerate 24 visual-regression snapshots.
- Exclude `apps/explorer-ui/artifacts/e7-renderer-bench/**` from the PR
  (runtime output, not source).

### Out of Scope

- Backend `LandingPayload` truncation fields — planned for a follow-up
  cycle (`e8b-landing-payload-truncation`) once the landing handler
  is implemented beyond stubs
  (`crates/cognicode-explorer/src/api.rs:670-671` TODO).
- Node-list virtualisation for very large workspaces (>500 nodes).
- Migration of `e7-renderer-bench` artifacts into a proper benchmark
  pipeline.
- Real `/api/workspaces/:workspace_id/quality-summary` backend
  implementation.

## Capabilities

### New Capabilities

- `graphlanding-affordances`: behavior of the landing page banner,
  canvas a11y, and node-list fallback when the cytoscape renderer is
  unavailable to the user.

### Modified Capabilities

- `contextual-views`: no requirement changes. The banner style is
  reused, not redefined.

(No spec-level requirement changes; the existing
`contextual-views.spec.md` Requirement 3 already defines a
`TruncationBanner` pattern that this proposal reuses.)

## Approach

Single frontend PR against `feat/e8-graphlanding-affordances` branch off
`main`. Cherry-pick and re-organize the working tree into three reviewable
commits:

1. `fix(landing): surface backend truncation flags and add a11y`
2. `fix(artifacts): align explorer-ui artifact path with /api/exploration-sessions/`
3. `test(e2e): switch MSW-incompatible page.route overrides to addInitScript fetch`

PATCH semver target (`v0.24.1`). The change does not introduce new public
API surface, does not change existing test contracts, and does not touch
the Rust codebase.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `apps/explorer-ui/src/api/schemas.ts` | Modified | +2 optional fields |
| `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` | Modified | Banner + a11y + useCallback |
| `apps/explorer-ui/src/hooks/useExplorations.ts` | Modified | 1-line path fix |
| `apps/explorer-ui/src/mocks/handlers.ts` | Modified | Mock alignment + new quality-summary mock |
| `apps/explorer-ui/src/mocks/landingFixtures.ts` | Modified | +2 fixture fields |
| `apps/explorer-ui/src/components/Shell.test.tsx` | Modified | testid coverage |
| `apps/explorer-ui/src/hooks/hooks.test.ts` | Modified | path fix |
| `apps/explorer-ui/e2e/{landing,error-states,pane-stack}.spec.ts` | Modified | MSW compat |
| `apps/explorer-ui/e2e/**/snapshots/**.png` | Regenerated | 24 snapshots |
| `apps/explorer-ui/src/tailwind.css` | Modified | 1-line comment fix |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Banner never renders in production (backend lacks field) | High (today) | Schema fields are `.optional()`. Banner code stays dormant. Follow-up cycle `e8b` ships backend field. |
| Snapshot drift masks real regressions | Medium | Land this PR alone; re-snapshot only after merge. Add a follow-up snapshot audit at v0.25. |
| MSW registration order changes break `addInitScript` overrides | Low | Document the override in test helper. If MSW migrates to module-level registration, refactor to use MSW handlers directly. |
| 24 snapshot updates hide unrelated drift | Low | Diff snapshots in PR review; the growth is consistent (all ~22025→~46046 bytes on c4 routes from new node-list region). |
| `quality-summary` mock promises a feature that has no backend | Low | Mock is MSW-only; explicitly marked dev-only. Real backend ships in a separate cycle. |

## Rollback Plan

Single PR, single branch, single squash merge. Rollback = `git revert` the
merge commit. The change is purely additive on the frontend side:

- Optional zod fields (`z.boolean().optional()`,
  `z.string().nullable().optional()`) parse successfully when absent.
- Banner element renders conditionally; absent state = no DOM.
- Path fix reverses to known-working state with `/explorations/` (which
  would 404 against the real backend but matches pre-change behavior).
- E2e tests revert to `page.route()` (which will become no-ops under MSW
  again — known pre-existing state).
- Snapshots revert to the previous baseline.

No data migration, no schema migration, no Rust changes. Revert is safe
to do at any commit within the PR.

## Dependencies

- None for the frontend PR.
- Future cycle `e8b-landing-payload-truncation` depends on
  `crates/cognicode-explorer/src/api.rs` landing handler being implemented
  beyond stubs.

## Success Criteria

- [ ] `just explorer-build` passes (TypeScript + Vite).
- [ ] `just explorer-test` passes (unit + integration).
- [ ] `just explorer-e2e` passes (24 regenerated snapshots accepted).
- [ ] No new warnings from MSW console in e2e runs.
- [ ] PR review notes the 3 atomic commits.
- [ ] `apps/explorer-ui/artifacts/e7-renderer-bench/**` is **not** in the PR.
- [ ] `docs/ROADMAP.md` is updated with `e8-graphlanding-affordances` under
      Completed once the PR merges.
