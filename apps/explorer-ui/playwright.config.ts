/**
 * Playwright config — E2E tests for the CogniCode Explorer.
 *
 * The dev server is started automatically before tests run with the
 * `VITE_USE_MOCKS=true` flag enabled, which makes `main.tsx` start the
 * MSW browser worker so all `/api/*` traffic is intercepted with
 * deterministic fixtures. No real axum backend is needed for the
 * E2E suite.
 *
 * If you want to run the E2E suite against a real backend, set
 * `VITE_USE_MOCKS=false` and ensure the Vite proxy target is up.
 */
import { defineConfig, devices } from "@playwright/test";

const PORT = 5173;
const BASE_URL = `http://127.0.0.1:${PORT}`;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env["CI"],
  retries: process.env["CI"] ? 2 : 0,
  workers: process.env["CI"] ? 1 : undefined,
  reporter: process.env["CI"] ? "github" : "list",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "retain-on-failure",
    // Env-var gate for screenshot capture (cycle e17).
    // Set PW_VISUAL=true to enable `toHaveScreenshot` assertions.
    // Default false locally so dev runs aren't blocked on baselines.
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run dev:mock",
    url: BASE_URL,
    reuseExistingServer: !process.env["CI"],
    timeout: 60_000,
    env: {
      VITE_USE_MOCKS: "true",
      ...(process.env["PW_VISUAL"] ? { PW_VISUAL: "true" } : {}),
    },
  },
  // Metadata for coverage matrix generation (cycle e17).
  // Read by scripts/coverage-matrix.ts.
  metadata: {
    testDir: "./e2e",
    coverageMatrix: "docs/inventory/e17-coverage-matrix.md",
    cycle: "e17-e2e-coverage-audit",
  },
});
