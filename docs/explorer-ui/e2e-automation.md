# Explorer E2E Automation

## Overview

Explorer E2E tests run against a mock-mode Vite dev server (`VITE_USE_MOCKS=true`).
MSW intercepts all `/api/*` traffic in the browser, so no real axum backend is required.
Tests live in `apps/explorer-ui/e2e/` and are driven by Playwright.

## Commands

### `just explorer-e2e`

Runs the full Explorer E2E suite once and prints results to stdout.

```bash
just explorer-e2e
```

- Uses the `list` reporter locally, `github` in CI.
- Auto-starts the mock dev server if one is not already running.
- Exit non-zero on any test failure.

---

### `just explorer-e2e-stability [N]`

Runs the E2E suite `N` times (default: 3) and produces a flakiness report.

```bash
just explorer-e2e-stability 3
```

**What it does:**
1. Creates an isolated run directory: `apps/explorer-ui/e2e-runs/<run-id>/`
2. For each repeat: runs `npx playwright test --reporter=json` → `run-N.json`
3. Builds `summary.json` with aggregate pass/fail counts and total duration
4. If `N > 1`: computes `stability.json` with per-test pass rates across repeats
5. Generates `report.html` from the campaign data

**Output files:**

| File | Contents |
|------|----------|
| `run-N.json` | Raw Playwright JSON reporter output per repeat |
| `summary.json` | Aggregate stats across all repeats |
| `stability.json` | Per-test pass rates (only when `N > 1`) |
| `report.html` | Self-contained HTML report with Tailwind styling |

**Exit behavior:** Non-zero if any repeat had failures.

---

### `just explorer-e2e-report [output-path]`

Generates an HTML report from the latest campaign (or a specific results directory).

```bash
# Default output
just explorer-e2e-report

# Custom path
just explorer-e2e-report /tmp/my-report.html
```

Report includes:
- KPI cards: pass rate, total tests, passed/failed, duration
- Per-file breakdown table
- Per-spec results with status badges
- Flaky tests section (when `stability.json` is present)

---

## Where results live

```
apps/explorer-ui/e2e-runs/
└── <run-id>/
    ├── run-1.json       # Playwright JSON — repeat 1
    ├── run-2.json       # Playwright JSON — repeat 2 (if N > 1)
    ├── summary.json     # Aggregate stats
    ├── stability.json   # Per-test flakiness (if N > 1)
    └── report.html      # HTML report
```

Run directories are timestamped: `YYYYMMDDTHHMMSS`.

---

## For CI / repeat runs

To run the suite N times on every PR check, chain the stability command:

```bash
just explorer-e2e-stability 3
```

Or run a single repeat for a fast gate:

```bash
just explorer-e2e-stability 1
```

---

## Architecture notes

- **Mock mode only**: `playwright.config.ts` starts `npm run dev:mock` automatically.
  Set `VITE_USE_MOCKS=false` and point `VITE_API_URL` at a live backend to test against
  the real stack.
- **No backend needed**: MSW handles all API mocking in the browser.
- **Reporter choice**: The campaign uses the `json` reporter (machine-readable, not XML)
  to align with the local convention of `result.json`-per-scenario files.
- **Isolated runs**: Each `just explorer-e2e-stability` invocation gets its own
  timestamped directory, so historical results are preserved.
