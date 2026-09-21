# Tasks: E2E Coverage Audit — Moldable Parity via Playwright

## Phase 0 — Setup (4 tasks)

- [ ] **T0.1** Create `apps/explorer-ui/e2e/utils/` directory.
- [ ] **T0.2** Write `apps/explorer-ui/e2e/utils/screenshot.ts` — central `snapshot(page, name)` helper.
- [ ] **T0.3** Write `apps/explorer-ui/e2e/utils/feature-flags.ts` — `isFeatureSupported(name)` helper.
- [ ] **T0.4** Write `apps/explorer-ui/e2e/utils/msw-helpers.ts` — `freezeTime(date)`, `setupFixture(name)` helpers.

## Phase 1 — Shared utilities (3 tasks)

- [ ] **T1.1** Update `apps/explorer-ui/playwright.config.ts` — add `PW_VISUAL` env-var gate.
- [ ] **T1.2** Add `coverage:matrix` script to `apps/explorer-ui/package.json`.
- [ ] **T1.3** Write `apps/explorer-ui/scripts/coverage-matrix.ts` — generates `docs/inventory/e17-coverage-matrix.md`.

## Phase 2 — Spotter multi-family (3 tasks)

- [ ] **T2.1** Write `spotter-multifamily.spec.ts` — 7 scenarios (R1.1–R1.7) covering Symbol, File, ViewSpec, SavedExploration, QualityIssue, Rule, cross-family.
- [ ] **T2.2** Add `data-family` attribute to result rows in `apps/explorer-ui/src/components/Spotter.tsx` if not present.
- [ ] **T2.3** Capture 7 screenshots: `spotter-multifamily-{symbol,file,viewspec,saved,quality,rule,cross}.png`.

## Phase 3 — View executor coverage (5 tasks)

- [ ] **T3.1** Define `EXECUTORS` array in `view-tabs-coverage.spec.ts` — 15 entries mirroring `registry.rs:335-413`.
- [ ] **T3.2** Write parameterised test loop — 15 sub-tests, one per executor (R2.<i>).
- [ ] **T3.3** Capture 15 screenshots: `view-tabs-coverage-{executor}.png`.
- [ ] **T3.4** Add MSW fixtures for each `applies_to` × `view_id` pair.
- [ ] **T3.5** Verify `data-view-tab` and `data-renderer` attributes exist on view tabs.

## Phase 4 — Pane stack drill-down (4 tasks)

- [ ] **T4.1** Write `pane-stack-drilldown.spec.ts` — scenarios R3.1 (drill), R3.2 (3-level), R3.3 (close), R3.4 (dedup).
- [ ] **T4.2** Capture 4 screenshots: `pane-stack-drilldown-{drill,three-level,close,dedup}.png`.
- [ ] **T4.3** Verify pane count via `data-pane-count` attribute on `PaneStackView` root.
- [ ] **T4.4** Verify dedup activates via `data-pane-active` indicator.

## Phase 5 — ViewSpecWizard full flow (3 tasks)

- [ ] **T5.1** Write `viewspec-wizard-full.spec.ts` — R4.1 (save), R4.2 (live preview).
- [ ] **T5.2** Capture 2 screenshots: `viewspec-wizard-{save,preview}.png`.
- [ ] **T5.3** Verify MSW `POST /api/view-specs` is hit with the expected body.

## Phase 6 — Landing real-data + virtualization (3 tasks)

- [ ] **T6.1** Write `landing-real-data.spec.ts` — R5.1 (real data), R5.2 (virtualization), R5.3 (truncation banner).
- [ ] **T6.2** Capture 3 screenshots: `landing-real-data-{entries,virtualized,banner}.png`.
- [ ] **T6.3** Add MSW fixture with ≥200 nodes to test virtualization.

## Phase 7 — Exploration sharing (2 tasks)

- [ ] **T7.1** Write `exploration-share.spec.ts` — R6.1 (URL generation), R6.2 (restore on open).
- [ ] **T7.2** Capture 2 screenshots: `exploration-share-{generate,restore}.png`.

## Phase 8 — Scan progress (2 tasks)

- [ ] **T8.1** Write `scan-progress.spec.ts` — R7.1 (ScanBar progress).
- [ ] **T8.2** Capture 1 screenshot: `scan-progress.png`.

## Phase 9 — Lens panel (2 tasks, debt-flagged)

- [ ] **T9.1** Write `lens-panel.spec.ts` — R8.1 (LensPanel opens), R8.2 (debt-flag for mock fixture).
- [ ] **T9.2** Capture 1 screenshot: `lens-panel.png` + add comment noting debt.

## Phase 10 — Perspective toggle (2 tasks)

- [ ] **T10.1** Write `perspective-toggle-full.spec.ts` — R9.1 (toggle to C4), R9.2 (toggle back).
- [ ] **T10.2** Capture 2 screenshots: `perspective-toggle-{c4,back}.png`.

## Phase 11 — Responsive layout (4 tasks)

- [ ] **T11.1** Write `responsive-full.spec.ts` — 4 viewports × scenarios R10.<viewport>.
- [ ] **T11.2** Capture 4 screenshots: `responsive-full-{320,768,1280,1920}.png`.
- [ ] **T11.3** Assert no horizontal scroll at any viewport.
- [ ] **T11.4** Assert no overlapping interactive elements.

## Phase 12 — Error states (2 tasks)

- [ ] **T12.1** Write `error-states-coverage.spec.ts` — R11.1 (network), R11.2 (unknown view).
- [ ] **T12.2** Capture 2 screenshots: `error-states-{network,unknown-view}.png`.

## Phase 13 — A11y coverage (2 tasks)

- [ ] **T13.1** Extend `a11y.spec.ts` — R12.1 (all view tabs pass axe-core).
- [ ] **T13.2** Capture violations panel screenshot if any test fails.

## Phase 14 — Call-graph interaction (2 tasks)

- [ ] **T14.1** Write `call-graph-rendering-extended.spec.ts` — R13.1 (pan/zoom), R13.2 (node click).
- [ ] **T14.2** Capture 2 screenshots: `call-graph-extended-{panzoom,nodedrill}.png`.

## Phase 15 — Visual regression baselines (1 task)

- [ ] **T15.1** Run `npm run test:e2e -- --update-snapshots` once to commit initial baselines.

## Phase 16 — MSW fixture consistency (1 task)

- [ ] **T16.1** Write `msw-fixture-consistency.spec.ts` — R15.1 (every handler hit).
- [ ] **T16.2** Export handler list from `apps/explorer-ui/src/mocks/handlers.ts`.

## Phase 17 — Documentation (5 tasks)

- [ ] **T17.1** Generate `docs/inventory/e17-coverage-matrix.md` via script.
- [ ] **T17.2** Update `docs/adr/ADR-002-moldable-exploration-parity-program.md` — append "## E2E Verification (e17)" section.
- [ ] **T17.3** Update `docs/ROADMAP.md` — add e17 entry to Completed.
- [ ] **T17.4** Populate `docs/inventory/e17-deferred-bugs.md` with any bugs found during apply.
- [ ] **T17.5** Update `openspec/changes/team-setup.md` if any convention changes.

## Phase 18 — CI integration (2 tasks)

- [ ] **T18.1** Write `.github/workflows/e2e-coverage.yml` — PR + weekly schedule.
- [ ] **T18.2** Verify CI workflow syntax (yaml validation).

## Phase 19 — Verification (5 tasks)

- [ ] **T19.1** Run `cargo test --workspace --lib` — confirm Rust tests still green (no production changes).
- [ ] **T19.2** Run `npm run test:e2e` — all specs pass locally.
- [ ] **T19.3** Run `npm run coverage:matrix` — matrix generated, ≥60/65 features covered.
- [ ] **T19.4** Manual review: open 30+ screenshots, confirm each shows the documented feature.
- [ ] **T19.5** Run flake detector: re-run suite 3 times, all green.

## Phase 20 — Closing (3 tasks)

- [ ] **T20.1** Update ROADMAP with PR link, coverage delta.
- [ ] **T20.2** Commit, push, open PR for the cycle.
- [ ] **T20.3** Archive openspec change to `openspec/changes/archive/`.
