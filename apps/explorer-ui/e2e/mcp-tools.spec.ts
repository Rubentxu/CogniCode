/**
 * E2E tests for McpToolsModal component (G6).
 *
 * Covers:
 * - Modal opens via header trigger button
 * - Tool selector switches between ingest_openapi and trace_route forms
 * - ingest_openapi form validation (spec path required)
 * - trace_route executes and shows result (after OpenAPI ingest)
 * - Modal closes via close button and overlay click
 * - Error state when trace_route called before ingest (route not found)
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 */
import { test, expect } from "@playwright/test";

test.describe("MCP Tools Modal (G6)", () => {
  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  /** Open the modal via the header trigger button. */
  async function openModal(page: any) {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();
    await page.waitForTimeout(1500);

    const trigger = page.getByTestId("mcp-tools-trigger");
    await expect(trigger).toBeVisible();
    await trigger.click();

    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });
    return modal;
  }

  /** Select a tool by value. */
  async function selectTool(page: any, tool: "ingest_openapi" | "trace_route") {
    const select = page.getByTestId("mcp-tool-select");
    await select.selectOption(tool);
    // Wait for form to re-render
    await page.waitForTimeout(100);
  }

  /** Click Run and wait for result or error. */
  async function runTool(page: any) {
    const runBtn = page.getByTestId("mcp-tools-run");
    await runBtn.click();
    // Wait for loading to finish (button text changes back)
    await expect(runBtn).toHaveText("Run", { timeout: 10_000 });
  }

  // ---------------------------------------------------------------------------
  // G6.1: Modal open / close
  // ---------------------------------------------------------------------------

  test("modal opens via trigger button and closes via ✕ button", async ({ page }) => {
    const modal = await openModal(page);

    // Modal panel is visible
    await expect(page.getByTestId("mcp-tools-modal-panel")).toBeVisible();

    // Close via ✕ button
    await page.getByTestId("mcp-tools-modal-close").click();
    await expect(modal).not.toBeVisible();
  });

  test("modal closes when clicking the overlay backdrop", async ({ page }) => {
    const modal = await openModal(page);

    // Click the overlay (outside the panel)
    await modal.click({ position: { x: 10, y: 10 } });
    await expect(modal).not.toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G6.2: Tool selector — form fields visible per tool
  // ---------------------------------------------------------------------------

  test("ingest_openapi form shows spec path and framework fields", async ({ page }) => {
    await openModal(page);

    await selectTool(page, "ingest_openapi");

    // Fields present
    await expect(page.getByTestId("mcp-spec-path")).toBeVisible();
    await expect(page.getByTestId("mcp-framework")).toBeVisible();

    // trace_route fields hidden
    await expect(page.getByTestId("mcp-trace-method")).not.toBeVisible();
    await expect(page.getByTestId("mcp-trace-path")).not.toBeVisible();
  });

  test("trace_route form shows method and path fields", async ({ page }) => {
    await openModal(page);

    await selectTool(page, "trace_route");

    // Fields present
    await expect(page.getByTestId("mcp-trace-method")).toBeVisible();
    await expect(page.getByTestId("mcp-trace-path")).toBeVisible();

    // ingest_openapi fields hidden
    await expect(page.getByTestId("mcp-spec-path")).not.toBeVisible();
    await expect(page.getByTestId("mcp-framework")).not.toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G6.3: Run button enabled/disabled by form validity
  // ---------------------------------------------------------------------------

  test("Run button is disabled when spec path is empty (ingest_openapi)", async ({ page }) => {
    await openModal(page);
    await selectTool(page, "ingest_openapi");

    const runBtn = page.getByTestId("mcp-tools-run");
    await expect(runBtn).toBeDisabled();

    // Fill spec path — Run should enable
    await page.getByTestId("mcp-spec-path").fill("/tmp/petstore.json");
    await expect(runBtn).toBeEnabled();
  });

  test("Run button is disabled when path is empty (trace_route)", async ({ page }) => {
    await openModal(page);
    await selectTool(page, "trace_route");

    const runBtn = page.getByTestId("mcp-tools-run");
    await expect(runBtn).toBeDisabled();

    // Fill path — Run should enable
    await page.getByTestId("mcp-trace-path").fill("/pets");
    await expect(runBtn).toBeEnabled();
  });

  // ---------------------------------------------------------------------------
  // G6.4: ingest_openapi — idempotent ingest (already covered in openapi-ingestion.spec.ts)
  //        This test verifies the modal flow specifically.
  // ---------------------------------------------------------------------------

  test("ingest_openapi executes and shows result with routes", async ({ page }) => {
    await openModal(page);
    await selectTool(page, "ingest_openapi");

    // The MSW handler returns a success result with petstore routes
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await runTool(page);

    // Result displayed
    const result = page.getByTestId("mcp-tools-result");
    await expect(result).toBeVisible({ timeout: 10_000 });
    // Result should contain route information (payload.routes array or similar)
    const resultText = await result.textContent();
    expect(resultText).toContain("routes");
  });

  // ---------------------------------------------------------------------------
  // G6.5: trace_route — 404 before ingest, success after ingest
  //        This mirrors the trace-route.spec.ts scenarios but through the modal UI.
  // ---------------------------------------------------------------------------

  test("trace_route shows error for unknown route before ingest", async ({ page }) => {
    await openModal(page);
    await selectTool(page, "trace_route");

    await page.getByTestId("mcp-trace-method").selectOption("GET");
    await page.getByTestId("mcp-trace-path").fill("/unknown/route");
    await runTool(page);

    // Error displayed — route not found
    const error = page.getByTestId("mcp-tools-error");
    await expect(error).toBeVisible({ timeout: 10_000 });
    const errorText = await error.textContent();
    expect(errorText.toLowerCase()).toMatch(/not found|404|error/i);
  });

  test("trace_route succeeds after OpenAPI ingest (within same modal session)", async ({ page }) => {
    // Open modal once
    await openModal(page);

    // First: ingest the spec (leaves routes in routeStore)
    await selectTool(page, "ingest_openapi");
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await runTool(page);
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 10_000 });

    // Switch to trace_route tool — form updates, modal stays open
    await selectTool(page, "trace_route");
    await page.getByTestId("mcp-trace-method").selectOption("GET");
    await page.getByTestId("mcp-trace-path").fill("/pets");
    await runTool(page);

    // Success result (not 404)
    const result = page.getByTestId("mcp-tools-result");
    await expect(result).toBeVisible({ timeout: 10_000 });
    const resultText = await result.textContent();
    expect(resultText.toLowerCase()).not.toMatch(/not found|404|error/i);
  });
});
