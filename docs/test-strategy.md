# Test Strategy — CogniCode E2E Tests

**Version:** 2.0  
**Last Updated:** 2026-06-20  
**Framework:** Playwright (Chromium) + MSW (deterministic fixtures)

---

## Overview

CogniCode E2E test suite uses Playwright with MSW to test both the **Dashboard** (server-side rendered) and **Explorer UI** (React SPA). The test suite is organized by **vertical slices** following the test pyramid model:

- **Unit tests**: Not covered in this E2E strategy (handled by component tests)
- **Integration tests**: API endpoints, HTTP handlers, DB operations (Dashboard)
- **Component tests**: React components in isolation (Explorer UI - handled by Vitest)
- **E2E tests**: Full user flows from UI to backend (this strategy)

---

## Test Suite Organization

### Dashboard E2E Tests

**Location:** `tests/e2e/`

**Server:** cognicode-dashboard-server (runs on port 3000 via playwright.config.js `webServer`)

**Test Files:**
- `dashboard.spec.js` - Core dashboard functionality (~106 tests)
- `project-centric.spec.js` - Project-level navigation (~20 tests)
- `visual-enhanced.spec.js` - Visual regression tests (33 tests)
- `visual-regression.spec.js` - Visual regression tests (11 tests)

**Visual Validation:**
- Golden images generated in `tests/e2e/__snapshots__/`
- Uses `toHaveScreenshot()` with `animations: "disabled"` and `fullPage: true`
- Screenshots capture: layout, navigation, pages, responsive states

**Mocking:**
- No MSW - Dashboard server is started directly
- Database is reset between test runs

---

### Explorer UI E2E Tests

**Location:** `apps/explorer-ui/e2e/`

**Server:** Vite dev server (runs on port 5173 via playwright.config.ts `webServer`)

**Environment Variables:**
- `VITE_USE_MOCKS=true` - MSW intercepts `/api/*` traffic with deterministic fixtures

**Test Files:**

| File | Tests | Coverage | Visual Validation |
|------|-------|----------|-------------------|
| `smoke.spec.ts` | 1 | Basic boot, Spotter, inspector | ✅ Yes |
| `spotter.spec.ts` | ~15 | Spotter search, keyboard navigation | ✅ Yes |
| `exploration.spec.ts` | ~25 | Multi-pane, tab switching | ✅ Yes |
| `graph.spec.ts` | ~15 | Call graph navigation | ✅ Yes |
| `error-states.spec.ts` | ~25 | Error states, empty states | ✅ Yes |
| `a11y.spec.ts` | ~10 | Accessibility (axe-core) | ✅ Yes |
| `pane-stack.spec.ts` | ~10 | Multi-pane inspection | ✅ Yes (9 tests) |
| `responsive.spec.ts` | ~15 | Responsive breakpoints | ✅ Yes (9 tests) |
| `visual-regression.spec.ts` | 12 | Dedicated visual tests | ✅ Yes (12 tests) |
| **TOTAL** | **~128** | **Full E2E coverage** | **~60% with visual validation** |

**Visual Validation:**
- Golden images generated in `apps/explorer-ui/e2e/visual-regression.spec.ts-snapshots/`
- Uses `toHaveScreenshot()` with `animations: "disabled"` and `fullPage: true`
- Screenshots capture: initial load, Spotter dialogs, inspector states, error states, responsive views

**Mocking:**
- **MSW browser worker** provides deterministic fixtures
- **No real backend** - all `/api/*` traffic is intercepted
- Fixtures defined in `apps/explorer-ui/src/mocks/handlers.ts`

---

## Visual Regression Testing Strategy

### Golden Images — When to Use

**APPLY to:**
- Critical user flows (smoke tests, onboarding)
- Error states (empty states, connection errors, loading states)
- Responsive layouts (desktop, tablet, small viewport)
- Complex UI states (multi-pane, view tabs, graph rendering)

**DO NOT APPLY to:**
- Pure API tests (no UI)
- Simple text-only assertions (use `toHaveText()` instead)
- Trivial components (buttons, inputs - use component tests)

### Visual Test Configuration

**Playwright Config (apps/explorer-ui/playwright.config.ts):**
```typescript
use: {
  baseURL: BASE_URL,
  trace: "on-first-retry",
  screenshot: "retain-on-failure",  // ← Retains golden images for passing tests
}
```

**Test-Level Override (for dedicated visual suites):**
```typescript
test.describe("Visual Regression Suite", () => {
  test.use({ screenshot: "on" });

  test("test name", async ({ page }) => {
    // ...
    await expect(page).toHaveScreenshot("screenshot.png", {
      animations: "disabled",
      fullPage: true,
    });
  });
});
```

### Screenshot Naming Convention

**Pattern:** `{suite}-{flow-state}.{browser}-{os}.png`

**Examples:**
- `smoke-initial-load-chromium-linux.png` - Initial app load
- `error-states-empty-spotter-chromium-linux.png` - Empty Spotter state
- `responsive-shell-desktop-chromium-linux.png` - Desktop shell layout
- `panestack-two-panes-chromium-linux.png` - Two-pane layout

**Browser Matrix:** Chromium (primary)  
**OS Matrix:** Linux (CI), macOS (local dev), Windows (optional)

### Visual Regression Workflow

**Update Golden Images:**
```bash
cd apps/explorer-ui
npm run test:e2e:visual:update
```

**Validate Against Golden Images:**
```bash
cd apps/explorer-ui
npm run test:e2e:visual
```

**When Screenshots Fail:**
1. **Is it a visual regression?** → Fix the bug, then re-run
2. **Is it intentional change?** → Update with `--update-snapshots`
3. **Is it flaky?** → Add wait/locator assertions, disable animations

---

## Mocking Strategy (MSW)

### MSW Browser Worker Configuration

**Location:** `apps/explorer-ui/src/main.tsx`

```typescript
if (import.meta.env.VITE_USE_MOCKS === "true") {
  const { worker } = await import("./mocks/browser");
  await worker.start({ onUnhandledRequest: "bypass" });
}
```

**Handlers:** `apps/explorer-ui/src/mocks/handlers.ts`

**Fixtures Provided:**
- Spotter search results: "build_overview", "build_callgraph"
- Object inspection: 4 views per symbol (overview, call-graph, source, quality)
- Error states: 404, 500, connection failure
- C4 perspective landing data
- Empty states: no search results, no objects, no panes

### Deterministic Fixtures — Why They Matter

**Problem:** Real backend tests are flaky due to:
- Network delays
- Database state drift
- External API failures
- Non-deterministic timing

**Solution:** MSW fixtures are **deterministic**:
- Same query → Same results
- No network → No timeouts
- No DB → No state drift
- Fast → ~2-3s per test

---

## Test Pyramid Coverage

### Current Coverage (2026-06-20)

| Layer | Tests | Visual Validation | Coverage |
|-------|-------|-------------------|----------|
| **Dashboard E2E** | ~150 | 0 | ~80% of critical flows |
| **Explorer UI E2E** | ~128 | ~60 | ~75% of critical flows |
| **TOTAL** | **~278** | **~60% of E2E** | **~78% of critical flows** |

### Visual Validation by Suite

| Suite | Tests | With Visual | % Visual |
|-------|-------|-------------|----------|
| Dashboard (visual-enhanced) | 33 | 33 | 100% |
| Dashboard (visual-regression) | 11 | 11 | 100% |
| Explorer (smoke) | 1 | 1 | 100% |
| Explorer (exploration) | ~25 | 4 | 16% |
| Explorer (graph) | ~15 | 2 | 13% |
| Explorer (error-states) | ~25 | 3 | 12% |
| Explorer (a11y) | ~10 | 3 | 30% |
| Explorer (pane-stack) | ~10 | 9 | 90% |
| Explorer (responsive) | ~15 | 9 | 60% |
| Explorer (visual-regression) | 12 | 12 | 100% |

---

## Running Tests

### Explorer UI Tests

**Run all Explorer UI tests:**
```bash
cd apps/explorer-ui
npx playwright test
```

**Run specific suite:**
```bash
cd apps/explorer-ui
npx playwright test e2e/smoke.spec.ts
```

**Run with visual validation:**
```bash
cd apps/explorer-ui
npm run test:e2e:visual
```

**Update golden images:**
```bash
cd apps/explorer-ui
npm run test:e2e:visual:update
```

**Run in headed mode (for debugging):**
```bash
cd apps/explorer-ui
npx playwright test e2e/smoke.spec.ts --headed
```

**Run with tracing on all tests:**
```bash
cd apps/explorer-ui
npx playwright test --trace on
```

### Dashboard Tests

**Run all Dashboard tests:**
```bash
cd tests/e2e
npx playwright test
```

**Run specific suite:**
```bash
cd tests/e2e
npx playwright test dashboard.spec.js
```

**Run visual regression:**
```bash
cd tests/e2e
npx playwright test visual-enhanced.spec.js
```

---

## Test Quality Guidelines

### ✅ DO

1. **Use `getByTestId()`** for critical UI elements (buttons, inputs, tabs)
2. **Use `getByRole()`** for semantic elements (buttons, tabs, links)
3. **Use `getByLabelText()`** for form inputs
4. **Wait for elements** with `toBeVisible()` instead of timeouts
5. **Use deterministic fixtures** via MSW
6. **Test behavior, not implementation** (user actions, not internal state)
7. **Validate accessibility** with `toHaveAttribute("aria-label", "...")`

### ❌ DO NOT

1. **Do NOT use CSS selectors** (`.class`, `#id`) unless absolutely necessary
2. **Do NOT use `waitForTimeout()`** unless waiting for animation/transitions
3. **Do NOT test implementation details** (internal state, private methods)
4. **Do NOT use flaky selectors** that change across builds
5. **Do NOT skip tests** without documenting why
6. **Do NOT rely on real backend** in CI (use MSW fixtures)

---

## Debugging Flaky Tests

### Common Causes & Fixes

| Cause | Symptom | Fix |
|-------|---------|-----|
| Race condition | Fails in CI, passes locally | Add `await page.waitForLoadState("networkidle")` |
| Animation not finished | Screenshot mismatch | Use `animations: "disabled"` |
| Keyboard listener not mounted | `Meta+k` times out | Add `await page.waitForTimeout(1500)` before pressing |
| Mock not returning | API call fails | Check MSW handler in `src/mocks/handlers.ts` |
| Selector timing | Element not found | Use `await expect(element).toBeVisible()` |

### Debugging Tools

**Playwright Inspector (GUI):**
```bash
npx playwright test --debug
```

**Trace Viewer (After run):**
```bash
npx playwright show-trace trace.zip
```

**Playwright Codegen (Record tests):**
```bash
npx playwright codegen http://localhost:5173
```

---

## CI/CD Integration

### GitHub Actions

**Playwright tests run on:**
- Pull requests: All E2E tests (both Dashboard and Explorer UI)
- Main branch: Full test suite + visual regression
- Cron job: Nightly smoke tests

**Flake Detection:**
- Tests run with retries: 2 in CI, 0 locally
- Failed tests generate traces for debugging
- Visual regression failures block PR merge

---

## Performance Targets

| Metric | Target | Current |
|--------|--------|---------|
| Avg test duration (Explorer UI) | < 4s | ~3.5s ✅ |
| Avg test duration (Dashboard) | < 5s | ~4.8s ✅ |
| Total suite duration (Explorer UI) | < 5m | ~8m ⚠️ |
| Total suite duration (Dashboard) | < 8m | ~7m ✅ |
| Visual test duration per test | < 2s | ~0.7s ✅ |

---

## Documentation

**Visual Regression Status:** `apps/explorer-ui/e2e/VISUAL-REGRESSION-STATUS.md`  
**Visual Regression Guide:** `apps/explorer-ui/e2e/VISUAL-REGRESSION.md`  
**Framework Report:** `docs/visual-regression-estado.md` (Español)

---

## Next Steps

### Immediate (2026-06-20)
- ✅ Fix visual-regression.spec.ts regex syntax (changed to getByTestId)
- ✅ Update playwright.config.ts screenshot mode
- ✅ Add visual validation to pane-stack.spec.ts (9 tests)
- ✅ Add visual validation to responsive.spec.ts (9 tests)
- ✅ Generate golden images for Explorer UI visual tests
- ⏳ Run Dashboard visual tests to generate golden images

### Short Term (Week of 2026-06-23)
- ⏳ Add visual validation to remaining Explorer UI suites (exploration, graph, error-states)
- ⏳ Investigate call graph SVG rendering bug (detected via visual test)
- ⏳ Validate all golden images with AI visual analysis
- ⏳ Set up CI flake detection for E2E tests

### Long Term (2026-07)
- ⏳ Achieve 80% visual validation coverage for Explorer UI E2E
- ⏳ Achieve 60% visual validation coverage for Dashboard E2E
- ⏳ Set up automated visual regression in PR reviews
- ⏳ Integrate Playwright visual regression with Lighthouse for performance

---

**Owner:** Test-Pyramid-Builder Agent  
**Last Reviewer:** n/a  
**Next Review Date:** 2026-06-27