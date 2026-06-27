/**
 * E2E trace_route tests — e15.5 Cross-Service Protocol Edge Ingestion.
 *
 * Tests the `trace_route` MCP tool:
 *  1. Before ingestion: trace_route for /pets GET → not found
 *  2. Ingest the spec
 *  3. After ingestion: trace_route /pets GET → finds the route with handler_symbol
 *  4. Trace a non-existent route → not found error
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 */
import { test, expect } from "@playwright/test";

test.describe("e15.5 trace_route (3 tests)", () => {
  test.beforeEach(async ({ page }) => {
    // Reset mock state (routeStore) before each test for isolation
    await page.request.post("/api/mocks/reset");
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
  });

  /**
   * Helper: open MCP Tools modal and run a tool with given args.
   * Returns when the result panel is visible.
   */
  async function runMcpTool(
    page: import("@playwright/test").Page,
    tool: "ingest_openapi" | "trace_route",
    args: Record<string, string>,
  ) {
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });

    // Select tool
    await page.getByTestId("mcp-tool-select").selectOption(tool);

    if (tool === "ingest_openapi") {
      await page.getByTestId("mcp-spec-path").fill(args["spec"] ?? "");
      if (args["framework"]) {
        await page.getByTestId("mcp-framework").selectOption(args["framework"]);
      }
    } else {
      await page.getByTestId("mcp-trace-method").selectOption(args["method"] ?? "GET");
      await page.getByTestId("mcp-trace-path").fill(args["path"] ?? "");
    }

    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 15_000 });
  }

  test("E2E: trace_route returns 404 before ingestion", async ({ page }) => {
    // Open modal directly — handle error case separately from helper
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    await expect(page.getByTestId("mcp-tools-modal")).toBeVisible({ timeout: 5_000 });

    // Select trace_route tool
    await page.getByTestId("mcp-tool-select").selectOption("trace_route");
    await page.getByTestId("mcp-trace-method").selectOption("GET");
    await page.getByTestId("mcp-trace-path").fill("/pets");

    // Click Run — API should return 404 for unknown route
    await page.getByTestId("mcp-tools-run").click();

    // Wait for EITHER error or result div to appear (diagnostic)
    const errorDiv = page.getByTestId("mcp-tools-error");
    const resultDiv = page.getByTestId("mcp-tools-result");
    const appeared = await Promise.race([
      errorDiv.waitFor({ state: "visible", timeout: 15_000 }),
      resultDiv.waitFor({ state: "visible", timeout: 15_000 }),
    ]).then(() => "visible").catch(() => "timeout");

    // Diagnostic: report what appeared
    if (appeared === "timeout") {
      // Check if loading spinner is stuck
      const loading = await page.getByTestId("mcp-tools-run").getAttribute("disabled");
      throw new Error(`Both error and result divs timed out after 15s. Button disabled: ${loading}`);
    }

    // Should show error div for unknown route
    const errorVisible = await errorDiv.isVisible().catch(() => false);
    if (errorVisible) {
      const text = await errorDiv.textContent();
      expect(text).toMatch(/not found|no route/i);
    } else {
      // Result div appeared — route was unexpectedly found
      const resultText = await resultDiv.textContent();
      throw new Error(`Expected 404 error but got result: ${resultText?.slice(0, 200)}`);
    }
  });

  test("E2E: trace_route resolves handler after ingestion", async ({ page }) => {
    // Open modal and ingest the spec
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    await expect(page.getByTestId("mcp-tools-modal")).toBeVisible({ timeout: 5_000 });

    // Ingest spec
    await page.getByTestId("mcp-tool-select").selectOption("ingest_openapi");
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await page.getByTestId("mcp-framework").selectOption("axum");
    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 10_000 });
    const ingestText = await page.getByTestId("mcp-tools-result").textContent();
    expect(ingestText).toContain('"status": "ingested"');

    // Switch to trace_route tool and trace /pets GET
    await page.getByTestId("mcp-tool-select").selectOption("trace_route");
    await page.getByTestId("mcp-trace-method").selectOption("GET");
    await page.getByTestId("mcp-trace-path").fill("/pets");
    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 10_000 });

    const result = page.getByTestId("mcp-tools-result");
    const text = await result.textContent();
    expect(text).toContain('"method": "GET"');
    expect(text).toContain('"/pets"');
    expect(text).toContain('"protocol": "http"');
    expect(text).toContain('"handler_symbol": "list_pets"');
    expect(text).toContain('"confidence": 0.85');
  });

  test("E2E: trace_route with path params resolves correctly", async ({ page }) => {
    // Open modal and ingest
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    await expect(page.getByTestId("mcp-tools-modal")).toBeVisible({ timeout: 5_000 });

    await page.getByTestId("mcp-tool-select").selectOption("ingest_openapi");
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 10_000 });

    // Trace /pets/{petId} GET — should match the wildcard route
    await page.getByTestId("mcp-tool-select").selectOption("trace_route");
    await page.getByTestId("mcp-trace-method").selectOption("GET");
    await page.getByTestId("mcp-trace-path").fill("/pets/42");
    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 10_000 });

    const result = page.getByTestId("mcp-tools-result");
    const text = await result.textContent();
    expect(text).toContain('"method": "GET"');
    expect(text).toContain('"/pets/{petId}"');
    expect(text).toContain('"handler_symbol": "get_pet_by_id"');
  });
});
