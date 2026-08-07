# Archive Report: e8-graphlanding-affordances

**Change**: e8-graphlanding-affordances
**Tag**: `v0.24.1` (PATCH)
**PRs**: [#56](https://github.com/Rubentxu/CogniCode/pull/56), [#57](https://github.com/Rubentxu/CogniCode/pull/57), [#58](https://github.com/Rubentxu/CogniCode/pull/58), [snapshot re-baseline `78b12eb`](https://github.com/Rubentxu/CogniCode/commit/78b12eb)
**Verdict**: PASS WITH WARNINGS
**Closed**: 2026-06-25

## Summary

The Explorer UI's landing page (`GraphLanding`) accumulated three
independent drift bugs against its backend contract. The drift lived in
the working tree for an unknown period, with 41 files modified but no
formal SDD cycle. This change formalizes the drift, ships the fixes as
three chained PRs, and re-baselines visual-regression snapshots to match
the new UI.

## Merged Commits

| SHA | Title | PR |
|---|---|---|
| `94f70a1` | `fix(artifacts): align explorer-ui artifact path with /api/exploration-sessions/` | #57 |
| `586335b` | `feat(landing): add truncation banner, canvas a11y, and node list fallback` | #56 |
| `1dd5bba` | `test(e2e): switch MSW-incompatible page.route overrides to addInitScript fetch` | #58 |
| `78b12eb` | `chore(explorer-ui): re-baseline visual-regression snapshots (E8)` | (direct commit on main) |

## What Changed

### Bugs fixed

- **Artifact endpoint drift**: `useArtifact(id, format)` was calling
  `/explorations/{id}/artifacts/{format}` which had been renamed to
  `/api/exploration-sessions/{id}/...` by ADR-040 Wave 3. The hook was
  never migrated and silently 404'd against the real backend.
- **Pre-existing test failure**: `hooks.test.ts > generateArtifact` was
  red on `main` because MSW's mock handler URL didn't match what the hook
  was requesting. PR-2 closes this test by aligning both sides.
- **E2E test dead-letter**: Six e2e tests (P1.7, P1.8, P5.1, P5.3, P5.4)
  were silently running against MSW default responses instead of their
  intended error/empty/large-graph scenarios. Playwright's `page.route`
  cannot intercept requests that MSW has already hijacked inside the page
  context. Switching to `page.addInitScript` for `window.fetch` overrides
  fixed this.

### UX improvements

- **Truncation banner**: When the backend returns
  `LandingPayload.truncated === true`, a banner appears above the canvas
  explaining the truncation and suggesting refinement. Currently dormant
  (backend doesn't yet return the field) — see W-1 below.
- **Canvas a11y**: The cytoscape canvas now exposes `role="application"`,
  a perspective-aware `aria-label`, and `tabIndex={0}`.
- **Node-list fallback**: A row of `<button>` elements renders below the
  canvas, one per `LandingPayload.nodes` entry. Screen-reader and
  keyboard-only users get a flat, ordered alternative to the canvas.
- **`selectObject` memoization**: `useCallback([dispatch])` prevents
  cytoscape destroy/re-mount on unrelated dispatches.

### Test infra

- 24 visual-regression snapshots regenerated after PR-1 lands.
- `apps/explorer-ui/artifacts/` added to `.gitignore`.
- 2 pre-existing tracked benchmark files removed from tracking via
  `git rm --cached` (runtime output, not source).

## Verification

| Phase | Result |
|---|---|
| `just explorer-build` | exit 0 ✓ |
| `just explorer-test` | **671/671 pass** (was 670/671 on main pre-merge) |
| `just explorer-e2e --update-snapshots` | **67/67 pass** |

## Artifacts

```
openspec/specs/graphlanding-affordances/spec.md   ← promoted canonical spec
openspec/changes/archive/e8-graphlanding-affordances/
├── exploration.md
├── proposal.md
├── design.md
├── tasks.md
├── verify-report.md
└── archive-report.md  (this file)
```

## Open Follow-ups

These are tracked in the verify report (W-1, W-2) and surfaced for the
next planning cycle:

| Follow-up | Reason | Suggested cycle |
|---|---|---|
| Backend `LandingPayload.truncated` field | Activates the dormant banner; mirrors the existing pattern in `SubgraphResponse` | `e8b-landing-payload-truncation` (MINOR or PATCH) |
| Node-list virtualisation | Fallback renders one `<button>` per node; 500 nodes = 500 DOM elements | `e9-landing-perf` (PATCH) |
| MSW wildcard back-port | Other handlers may suffer the same SWR-key drift | repo-hygiene cycle |

## Lessons (jurisprudence)

### 1. `apiGet` adds `/api` prefix — paths in hooks MUST NOT include it

`apps/explorer-ui/src/api/client.ts:22` defines `DEFAULT_BASE = "/api"`.
`apiGet(key, ...)` and `makeSwrFetcher` both prepend this base to the
key. If a hook constructs its SWR key as `/api/some/path`, the resulting
URL becomes `/api/api/some/path` (404 against any real backend). Correct
pattern: hook uses the **path without** the `/api` prefix; the client
adds it.

**Diagnostic**: a hook test fails with `ERR_INVALID_URL` or
`ECONNREFUSED` on `localhost:8080`/`localhost:3000` after a frontend
"path migration" PR. Check whether the migrated path has `/api` in it
twice.

### 2. MSW + Playwright `page.route` is incompatible

MSW hijacks `window.fetch` inside the page context at module load.
Playwright's `page.route` operates on the browser context network stack,
which MSW has already bypassed. `page.route` becomes a silent no-op.

**Diagnostic**: tests using `page.route` pass (i.e. don't fail with
"route not matched") but the app receives the default MSW response, not
the route's intended response. The override is being ignored.

**Fix**: use `page.addInitScript(() => { window.fetch = ... })`. The
script runs in the page's main world before any other script (including
MSW's worker bootstrap), so the test's override wins.

### 3. Visual-regression snapshots are coupled to the new UI

If a UI change affects page height or layout (e.g. adding a region below
the canvas), snapshots in `e2e/**/snapshots/` must be regenerated from
the **post-merge** state, not from a feature branch alone — otherwise the
new testid/region may not exist when you try to regenerate.

**Fix**: split UI PRs from snapshot re-baseline PRs. Land the UI first,
then re-snapshot, then commit the snapshot diff.

## Final Verdict

PASS WITH WARNINGS — change shipped, follow-ups queued.
