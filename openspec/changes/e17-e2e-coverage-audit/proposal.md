# e17: E2E Coverage Audit — Moldable Parity via Playwright

## Why

CogniCode's ROADMAP and ADR-002 promise a path to **functional parity with GToolkit's moldable development model** — every object inspectable as a first-class entity, every object having multiple contextual views, lateral navigation that preserves the exploration narrative, universal discovery (Spotter), and durable narratives.

Today, the gap between vocabulary and verification is invisible to everyone except the test suite authors. We have:

- **15 real `ViewExecutor`s** wired in `crates/cognicode-explorer/src/registry.rs:335-413`
- **6 `SpotterSearchResult` families** wired in `crates/cognicode-explorer/src/dto.rs`
- **65 user-facing features** cataloged in `docs/inventory/explorer-ui-feature-inventory.md`
- **14 Playwright E2E specs** covering approximately 42 features by integration
- **23 features without E2E coverage** including: `ShareExplorationButton`, `ScanBar`, `LensSidebarToggle`, `ViewSpecWizardTrigger`, `HotspotTreemap`, `DeadCodeSunburst`, `LensPanel`, full 5-step ViewSpecWizard, `RationaleView`, SVG zoom controls, **38 declared ViewKinds with only 6 wired** (others fall back to `UnknownBlockView`)

The risk: the moldable exploration features work in the **wiring layer** (unit tests, fixtures, registry) but break in the **rendering layer** (no test ever visits them with a real browser). When e13's multi-family Spotter shipped without an E2E test, nothing caught the cross-family dedup / per-family cap / hardcoded workspace_id / double `issues_for_workspace` warnings until verify pass — and even then, no test reproduces the user-visible flow.

We need an E2E suite that:

1. **Asserts user-facing behavior**, not internal wiring.
2. **Captures screenshots as evidence** — every feature gets at least one golden image so we can spot regressions visually.
3. **Proves moldable parity** — pane-stack drill-down must preserve narrative; view-tabs must render the right renderer; Spotter must return 6 families; ViewSpecWizard must complete end-to-end.
4. **Surfaces the gap to GToolkit honestly** — tests should fail or be marked `.skip()` for missing features, not silently pass.

## What changes

This is a **test-only / docs-only cycle**. No production code changes. No new `ViewExecutor`s. No new Spotter families. The deliverable is:

### A. Test coverage gap closure (15 new specs, ~50 new tests)

- **`spotter-multifamily.spec.ts`** — exercises all 6 Spotter families (Symbol, File, ViewSpec, SavedExploration, QualityIssue, Rule) from the UI. Screenshot per family.
- **`view-tabs-coverage.spec.ts`** — for each of the 15 wired `ViewExecutor`s, open an inspectable object whose `applies_to` matches, click the corresponding view tab, assert the renderer produces a non-empty DOM, screenshot.
- **`pane-stack-drilldown.spec.ts`** — drilling into a callee / caller / related symbol opens a new pane (not replaces), history is preserved across 3+ drill levels, `✕` closes the active pane, dedup activates when the same object is selected twice. Screenshots at each level.
- **`viewspec-wizard-full.spec.ts`** — exercise the full 5-step wizard: pick `ViewKind`, pick `RendererKind`, pick data source, edit JSONata, save → assert ViewSpec persisted, then re-open via Spotter / pane.
- **`landing-real-data.spec.ts`** — assert landing payload surfaces `entry_points`, `hot_paths`, `god_nodes` with real data from MSW fixtures; banner activates when landing is truncated; virtualization kicks in when node count > 200. Screenshot.
- **`exploration-share.spec.ts`** — `ShareExplorationButton` produces a URL with `?exploration=<id>`, opening that URL restores the pane stack state.
- **`scan-progress.spec.ts`** — `ScanBar` shows scanning progress, completes, triggers ingest notification.
- **`lens-panel.spec.ts`** — `LensPanel` opens, surfaces mock fixtures (treemap, sunburst) as documented in `LensResultView.tsx` (flag the mock fixture as known-debt if not yet wired to real data). Screenshot.
- **`perspective-toggle-full.spec.ts`** — toggling perspective (Default / C4 / Quality) reloads the explorer layout with the right surface, restores on toggle back. Screenshot per perspective.
- **`responsive-full.spec.ts`** — 320/768/1280/1920 viewports render without horizontal scroll, no broken layouts. Screenshot per viewport.
- **`error-states-coverage.spec.ts`** — every error boundary in the app (network failure, parse error, panics) renders an actionable fallback, not a blank page. Screenshot per scenario.
- **`a11y-coverage.spec.ts`** — every interactive view passes `axe-core` (extended to all view-tabs, not just landing). Screenshot the violations panel if any.
- **`call-graph-rendering-extended.spec.ts`** — call-graph view renders interactive SVG with pan/zoom controls, node click opens callee pane. Screenshot of multi-level call graph.
- **`visual-regression-baseline.spec.ts`** — for every new spec, capture the canonical "happy path" screenshot and commit as baseline. Subsequent runs use `toHaveScreenshot()` with `--update-snapshots` for intentional changes.
- **`msw-fixture-consistency.spec.ts`** — every MSW mock handler declared in `apps/explorer-ui/src/mocks/handlers.ts` is hit by at least one E2E test. Fail otherwise.

### B. Documentation (docs/ and ADR updates — local-only, not pushed)

- `docs/inventory/explorer-ui-feature-inventory.md` — **canonical feature inventory** with E2E coverage column. Already exists from explore phase.
- `docs/inventory/gtoolkit-parity.md` — **GToolkit feature mapping + visual reference URLs**. Already exists.
- `docs/inventory/e17-coverage-matrix.md` — **new** — explicit matrix: feature → spec → test name → screenshot → gtoolkit equivalent. Updated as tests land.
- `docs/adr/ADR-002-moldable-exploration-parity-program.md` — append a "Verification (E2E)" section with the new coverage metrics.
- `docs/ROADMAP.md` — add `e17-e2e-coverage-audit` entry to Completed with PR link and coverage delta.

### C. Hygiene / process changes (in `apps/explorer-ui/`)

- **`playwright.config.ts`** — add `PW_VISUAL=true` env-var gate so screenshots are taken in CI but not locally by default. Keep `screenshot: "retain-on-failure"` for debugging.
- **`apps/explorer-ui/e2e/utils/screenshot.ts`** — shared helper for `toHaveScreenshot` with consistent viewport, `animations: "disabled"`, `fullPage: true`. Used by all new specs.
- **`apps/explorer-ui/e2e/utils/feature-flags.ts`** — helpers to query feature support (e.g. "is moldable-view-call-graph active"). Lets specs `.skip()` cleanly for unimplemented features instead of failing.
- **CI workflow (`.github/workflows/e2e-coverage.yml` — new)** — runs `npm run test:e2e -- --grep @coverage` matrix: chromium only by default; weekly schedule adds firefox/webkit; uploads screenshots as artifacts on failure.

## Scope

### In scope

- Frontend E2E coverage only.
- Playwright only (no new test runner; no new unit-test framework).
- MSW fixtures only (no real Postgres / real backend in CI).
- Moldable-parity tests only (no perf, no security, no a11y-perf).
- Local-only documentation in `docs/`, `plans/`, `openspec/`.

### Out of scope (deferred to future cycles)

- **Real-backend E2E** (`VITE_USE_MOCKS=false`). Defer until MCP integration tests are stable.
- **Cross-browser matrix** beyond weekly schedule. CI runs chromium-only.
- **Mobile / touch gestures**. The PWA is desktop-first.
- **Component-level tests in isolation** (covered by Vitest + RTL).
- **New `ViewExecutor`s or `Spotter` families** — separate cycles (`e12g`, `e12h`, `e13-wave-2`).
- **GToolkit feature implementation** — separate cycles (`e14`, `e15`).

## Success criteria

1. **Coverage metrics** in `docs/inventory/e17-coverage-matrix.md` show:
   - ≥ 60 of 65 features have at least one E2E test asserting user-visible behavior.
   - Each of the 15 wired `ViewExecutor`s has a `view-tabs-coverage` assertion.
   - Each of the 6 wired `Spotter` families has a `spotter-multifamily` assertion.
   - Every MSW mock handler is hit by at least one test.
2. **All E2E specs pass** locally with `npm run test:e2e` (chromium, no retries).
3. **CI green** on PRs that touch UI components.
4. **At least 30 new screenshots** committed as visual baselines under `apps/explorer-ui/e2e/<spec>.spec.ts-snapshots/`.
5. **Visual regression** test runs in `<5min` total (current baseline: ~3min, target: <5min).
6. **Flake rate** `<2%` measured over 100 CI runs.
7. **No production code changes**. The cycle is purely verification + documentation.

## Key decisions

- **Tests are behavior-focused, not implementation-focused.** No `data-testid` count assertions beyond what proves the feature works for the user.
- **Screenshots are evidence, not decoration.** Every test captures at least one screenshot; the spec file is invalid without one.
- **Missing features get `.skip()` not deletion.** `example_object`, `composed_narrative`, `project_diary` ViewKinds are skip-tested (Spotter returns them; renderer falls back to `UnknownBlockView`) — visible as debt, not removed.
- **Local-only docs.** All coverage matrices, gap reports, and ADRs live in `docs/`, `plans/`, `openspec/` — never pushed to remote per user policy (2026-06-24).
- **MSW is the contract.** The MSW handler list is the API contract; tests assert against the contract, not against the real backend.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Flaky CI from `waitForTimeout` / animation drift | High | Use `getByRole`, `expect(...).toBeVisible()` exclusively. `animations: "disabled"` in screenshots. Per-spec retry budget (max 2). |
| Screenshots drift across locales / timezones | Medium | Pin `TZ=UTC`, `LANG=C`, use system fonts, freeze `Date.now()` via MSW. |
| MSW fixture drift breaks unrelated tests | Medium | Treat `apps/explorer-ui/src/mocks/handlers.ts` as the contract. One owner per handler. PR review enforces handler-test pairing. |
| Tests slow CI to >10min | Medium | Parallelize via Playwright workers (`fullyParallel: true`, currently). Split visual-regression to a separate weekly job. |
| Specs depend on each other and break in parallel | Medium | Each spec sets up its own MSW state via `test.beforeEach`. No shared `beforeAll`. |
| GToolkit parity screenshots prove nothing measurable | Medium | Pair each screenshot with at least one text assertion (text/role/attribute). |
| Feature inventory goes stale | Low | Re-run inventory script in CI as a sanity check (fail if feature count drops without ADR). |

## Verification gate

- **Pre-merge**: `npm run test:e2e` passes locally + CI.
- **Visual diff**: `npm run test:e2e -- --update-snapshots` then commit updated snapshots.
- **Coverage report**: generate `docs/inventory/e17-coverage-matrix.md` from a script (`apps/explorer-ui/scripts/coverage-matrix.ts`) — the script is part of the cycle.
- **ADR update**: append "Verification (E2E)" section to ADR-002.

## Out-of-cycle work discovered during apply

Any production bug or missing feature surfaced during testing becomes its own cycle (per the test-pyramid-builder protocol: "Bug Fixing During Testing"). We do not fix bugs in this cycle. We document them in `docs/inventory/e17-deferred-bugs.md` and propose follow-up cycles.
