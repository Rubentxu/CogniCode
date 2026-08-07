# Design: E2E Coverage Audit — Moldable Parity via Playwright

## 1. Architecture overview

This is a **test-only cycle**. There is no production architecture change. The design is the **test architecture**: how 15 new Playwright specs compose into a coherent coverage matrix.

```
apps/explorer-ui/e2e/
├── smoke.spec.ts                         (existing — kept)
├── pane-stack.spec.ts                    (existing — extended)
├── spotter.spec.ts                       (existing — extended)
├── landing.spec.ts                       (existing — extended)
├── perspective-toggle.spec.ts            (existing — extended)
├── responsive.spec.ts                    (existing — extended)
├── error-states.spec.ts                  (existing — extended)
├── call-graph-rendering.spec.ts          (existing — extended)
├── exploration-snapshot.spec.ts          (existing — kept)
├── visual-regression.spec.ts             (existing — kept)
├── a11y.spec.ts                          (existing — extended)
├── bench-renderer.spec.ts                (existing — kept)
├── graph.spec.ts                         (existing — kept)
├── exploration.spec.ts                   (existing — extended)
├── utils/                                (new shared helpers)
│   ├── screenshot.ts                     (consistent screenshot opts)
│   ├── feature-flags.ts                  (query feature support)
│   └── msw-helpers.ts                    (deterministic state setup)
├── spotter-multifamily.spec.ts           (NEW)
├── view-tabs-coverage.spec.ts            (NEW — 15 sub-tests, one per executor)
├── pane-stack-drilldown.spec.ts          (NEW)
├── viewspec-wizard-full.spec.ts          (NEW)
├── landing-real-data.spec.ts             (NEW)
├── exploration-share.spec.ts             (NEW)
├── scan-progress.spec.ts                 (NEW)
├── lens-panel.spec.ts                    (NEW — debt-flagged)
├── visual-regression-baseline.spec.ts    (NEW)
├── msw-fixture-consistency.spec.ts       (NEW)
└── <existing>.spec.ts-snapshots/         (existing — extended)
```

```
scripts/                                  (new — outside e2e/)
├── coverage-matrix.ts                    (generates docs/inventory/e17-coverage-matrix.md)
├── inventory-check.ts                    (re-runs feature inventory)
└── visual-baseline.ts                    (manages PNG baselines)
```

```
docs/ (local-only, never pushed)
├── inventory/
│   ├── explorer-ui-feature-inventory.md  (existing)
│   ├── gtoolkit-parity.md                (existing — written this cycle)
│   ├── e17-coverage-matrix.md            (NEW — generated)
│   └── e17-deferred-bugs.md              (NEW — populated during apply)
└── ROADMAP.md                            (updated post-merge)
```

```
openspec/ (local-only, never pushed)
└── changes/e17-e2e-coverage-audit/
    ├── proposal.md                       (existing)
    └── specs/e2e-coverage/
        └── spec.md                       (existing)
```

## 2. Test architecture decisions

### 2.1 Spec granularity — one feature, one spec, multiple scenarios

Each spec maps to **one GToolkit capability**. Scenarios inside the spec exercise the variations. This makes failure messages actionable: "view-tabs-coverage: ownership-map FAILED" tells you exactly which executor broke.

**Trade-off**: more specs means more `beforeEach` overhead. We accept the trade because parallelism (`fullyParallel: true`) amortises setup time.

### 2.2 Screenshot capture — deterministic, parallel-safe

Every spec captures at least one screenshot. The pattern:

```ts
import { expect, test } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test("call-graph view renders interactive SVG", async ({ page }) => {
  // ... arrange ...
  await snapshot(page, "call-graph-interactive.png");
});
```

Where `utils/screenshot.ts` centralises:

```ts
export async function snapshot(page: Page, name: string) {
  const opts = {
    animations: "disabled" as const,
    fullPage: true,
    maxDiffPixels: 50,
  };
  await expect(page).toHaveScreenshot(name, opts);
}
```

Why `maxDiffPixels: 50`: tolerates minor anti-aliasing differences between CI runners without losing strictness for real regressions.

### 2.3 MSW fixture as API contract

`apps/explorer-ui/src/mocks/handlers.ts` declares every endpoint MSW intercepts. Each handler is owned by one team / one PR and **must** be exercised by at least one E2E test.

To enforce this, `msw-fixture-consistency.spec.ts`:

1. Imports the handler list (export from `handlers.ts`).
2. Subscribes to all intercepted requests during the run.
3. Asserts: for each handler, at least one matching request fired.

This is run **last** in the suite so it observes all earlier specs.

### 2.4 View-tabs coverage — parameterised test

`view-tabs-coverage.spec.ts` uses a parameterised approach:

```ts
const EXECUTORS = [
  { id: "overview", appliesTo: "Symbol", fixture: "symbol-overview" },
  { id: "call-graph", appliesTo: "Symbol", fixture: "symbol-callgraph" },
  // ... 13 more
];

for (const exec of EXECUTORS) {
  test(`${exec.id} renders for ${exec.appliesTo}`, async ({ page }) => {
    // Open MSW fixture for exec.appliesTo
    // Click view-tab-${exec.id}
    // Assert non-empty render + correct RendererKind
    // Screenshot
  });
}
```

Parameterised tests give 15 tests for ~150 LOC of code. Easy to extend when new executors land.

### 2.5 Flake mitigation

| Source of flake | Mitigation |
|---|---|
| Animation timing | `animations: "disabled"` in all screenshots |
| Async data load | `expect(...).toBeVisible({ timeout: 5000 })` instead of `waitForTimeout` |
| Time-dependent UI | MSW freezes `Date.now()` to `2026-06-27T10:00:00Z` |
| Locale-dependent rendering | `TZ=UTC`, `LANG=C` in CI |
| Font availability | System fonts pinned via `font-family` CSS var |
| WebGL/WebGPU | Specs tagged `@no-webgl` skip WebGL path; default path uses SVG |
| Test parallelism race | Each spec sets its own MSW state via `test.beforeEach` |

### 2.6 Coverage matrix generation

`scripts/coverage-matrix.ts`:

1. Reads `apps/explorer-ui/e2e/**/*.spec.ts` and extracts `test(...)` descriptions.
2. Reads `apps/explorer-ui/src/` and extracts component names + exported view ids.
3. Cross-references with the 65-feature inventory.
4. Outputs `docs/inventory/e17-coverage-matrix.md` as a markdown table.

Run via `npm run coverage:matrix`. CI runs this script on PR and fails if coverage drops below threshold.

### 2.7 Skip-not-delete for missing features

Features that exist in the catalog but have no implementation (e.g. `composed_narrative`, `project_diary`) get a `test.skip()` with a comment explaining the debt:

```ts
test.skip("composed_narrative view renders (KNOWN DEBT: catalog-only)", async ({ page }) => {
  // ADR-002 §Phase 3 — narrative runtime not yet implemented
  // Re-enable when e14-narrative-runtime lands
});
```

The skip is visible in the test report and the coverage matrix marks it as "SKIPPED (debt)" — debt is tracked, not hidden.

## 3. CI integration

`.github/workflows/e2e-coverage.yml` (new):

```yaml
name: e2e-coverage
on:
  pull_request:
    paths:
      - "apps/explorer-ui/**"
      - "apps/explorer-ui/e2e/**"
  schedule:
    - cron: "0 3 * * 1"  # Weekly Monday 03:00 UTC
jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
      - run: npm run test:e2e -- --grep @coverage
      - if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-failures
          path: apps/explorer-ui/test-results/
      - if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-snapshots
          path: apps/explorer-ui/e2e/**/spec.ts-snapshots/*-actual.png
```

PR runs: chromium only, baseline snapshots.
Weekly runs: full matrix (chromium + firefox + webkit), uploads failure artifacts.

## 4. Documentation

### 4.1 Coverage matrix (the source of truth)

`docs/inventory/e17-coverage-matrix.md`:

```markdown
# E2E Coverage Matrix — Cycle e17

Last generated: 2026-06-27T10:00:00Z

## Summary

| Category | Total features | E2E covered | Skipped (debt) | Missing |
|---|---|---|---|---|
| Navigation | 8 | 8 | 0 | 0 |
| Views (15 executors) | 15 | 15 | 0 | 0 |
| Views (catalog-only) | 23 | 0 | 23 | 0 |
| Landing | 5 | 5 | 0 | 0 |
| Inspectable objects | 9 | 9 | 0 | 0 |
| Authoring | 3 | 3 | 0 | 0 |
| Settings | 3 | 3 | 0 | 0 |
| Error states | 4 | 4 | 0 | 0 |
| Accessibility | 2 | 2 | 0 | 0 |

## Detailed matrix

| Feature | Spec | Test name | Screenshot | GToolkit equivalent |
|---|---|---|---|---|
| `Spotter` (Cmd+K) | `spotter-multifamily.spec.ts` | `Symbol family` | `spotter-multifamily-symbol.png` | Spotter (universal search) |
...
```

### 4.2 ROADMAP and ADR updates

- **ROADMAP.md** — append `e17-e2e-coverage-audit` to Completed with tag `v0.30.0`, PR link, coverage delta.
- **ADR-002** — append "## E2E Verification (e17)" section with coverage metrics + screenshot evidence.

## 5. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Specs depend on each other | Each spec sets up its own MSW state via `beforeEach`. No shared `beforeAll`. |
| MSW fixture drift | `msw-fixture-consistency.spec.ts` flags unused handlers as debt. |
| CI > 10min | Parallel workers, chromium-only on PR, weekly full matrix. |
| Visual baseline churn | `maxDiffPixels: 50` tolerance. Manual review for any baseline update. |
| Tests silently pass for missing features | `test.skip` with explicit debt comment + matrix tracking. |

## 6. Verification gate

Pre-merge:

1. `cd apps/explorer-ui && npm run test:e2e` — all specs green locally.
2. `npm run coverage:matrix` — coverage matrix generated, ≥60/65 features covered.
3. `cargo test --workspace --lib` — Rust tests still green (no production changes).
4. Manual review: open at least one screenshot per spec, confirm it shows the documented feature.

Post-merge:

1. CI green on PR.
2. Coverage matrix committed in `docs/inventory/`.
3. ROADMAP + ADR updated.

## 7. Out-of-cycle work

Any production bug surfaced during apply goes to `docs/inventory/e17-deferred-bugs.md` as:

```markdown
## Bug #1: <short title>

- **Severity**: Trivial | Moderate | Systemic
- **Found in**: <spec-name>
- **What**: <one-line description>
- **Why**: <root cause>
- **Proposed fix**: <one-line>
- **Follow-up cycle**: <e17.x or other>
```

Follow-up cycles proposed separately, not fixed in this PR.
