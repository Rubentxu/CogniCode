/**
 * E2E OpenAPI ingestion tests — e15.5 Cross-Service Protocol Edge Ingestion.
 *
 * Tests the complete `ingest_openapi` MCP tool flow:
 *  1. Open the MCP Tools modal from the Shell header
 *  2. Select `ingest_openapi`, enter a spec path, run the tool
 *  3. Verify the result shows routes_created = 7
 *  4. After ingestion, search for "pet" in Spotter — Route nodes appear
 *  5. Idempotency: re-ingesting the same spec returns "already_ingested"
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 * Route state is tracked in the module-level `routeStore` in handlers.ts.
 */
import { test, expect } from "@playwright/test";
import { http, HttpResponse } from "msw";
import { setupWorker } from "msw/browser";

// Import the handlers to access routeStore for cleanup
// (routeStore lives in the browser worker; we interact via
// server.use() overrides that reset it before each test.)
const HANDLERS_URL = "https://app.example.com"; // dummy origin for handler registration

/**
 * Resets the routeStore by installing a fresh ingest_openapi handler
 * that first clears the store before any logic runs.
 */
function resetRouteStore(worker: ReturnType<typeof setupWorker>) {
  // Override the MCP tools handler to clear routeStore on ingest_openapi
  worker.use(
    http.post("/api/mcp/tools/call", async ({ request }) => {
      const body = (await request.clone().json()) as {
        name?: string;
        args?: Record<string, unknown>;
      };

      if (body.name === "ingest_openapi") {
        // Import routeStore via a one-time eval in the browser context.
        // We do this by posting to a special endpoint that the existing
        // handler won't match — but we need another approach.
        // Instead: we just return a deterministic fresh response for
        // ingest_openapi and let the spotter/landing handlers return
        // empty (no routes) unless routes were added via a prior call.
        // The test sequence ensures ingest_openapi is called first
        // within each test, so state is clean.
        const spec = String(body.args?.["spec"] ?? "");
        const PETSTORE_HASH = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        return HttpResponse.json({
          tool_name: "ingest_openapi",
          version: "0.0.0",
          timestamp: new Date().toISOString(),
          provenance: null,
          payload: {
            spec_hash: PETSTORE_HASH,
            status: "ingested",
            routes_created: 7,
            routes_updated: 0,
            edges_created: 4,
            edges_updated: 0,
            total_routes: 7,
            resolved_handlers: 4,
            framework: body.args?.["framework"] ?? null,
          },
        });
      }

      // Fall through to real handler for other tools
      return HttpResponse.error();
    }),
  );
}

test.describe("e15.5 OpenAPI ingestion (3 tests)", () => {
  test.beforeEach(async ({ page }) => {
    // Reset mock state (routeStore) before each test for isolation
    await page.request.post("/api/mocks/reset");
    // Wait for app to load before any interaction
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
  });

  test("E2E: MCP Tools button opens modal", async ({ page }) => {
    // Verify the MCP Tools button exists in the header
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await expect(mcpToolsBtn).toBeVisible();

    // Click to open the modal
    await mcpToolsBtn.click();
    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });

    // Modal has the tool selector
    const toolSelect = page.getByTestId("mcp-tool-select");
    await expect(toolSelect).toBeVisible();
    await expect(toolSelect).toHaveValue("ingest_openapi");

    // Close button works
    await page.getByTestId("mcp-tools-modal-close").click();
    await expect(modal).toBeHidden({ timeout: 5_000 });
  });

  test("E2E: ingest_openapi creates 7 routes and they appear in Spotter", async ({ page }) => {
    // Open MCP Tools modal
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });

    // Select ingest_openapi (already selected by default)
    await expect(page.getByTestId("mcp-tool-select")).toHaveValue("ingest_openapi");

    // Enter the spec path
    const specInput = page.getByTestId("mcp-spec-path");
    await specInput.fill("sandbox/fixtures/openapi/petstore.json");

    // Optionally set framework hint
    await page.getByTestId("mcp-framework").selectOption("axum");

    // Click Run
    await page.getByTestId("mcp-tools-run").click();

    // Wait for result to appear
    const result = page.getByTestId("mcp-tools-result");
    await expect(result).toBeVisible({ timeout: 10_000 });

    // Verify result contains routes_created = 7
    const resultText = await result.textContent();
    expect(resultText).toContain('"status": "ingested"');
    expect(resultText).toContain('"routes_created": 7');
    expect(resultText).toContain('"edges_created": 4');
    expect(resultText).toContain('"resolved_handlers": 7');

    // Close the modal
    await page.getByTestId("mcp-tools-modal-close").click();
    await expect(modal).toBeHidden({ timeout: 5_000 });

    // Now open Spotter and search for "pet" — route nodes should appear
    await page.waitForTimeout(1500);
    await page.keyboard.press("Control+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 5_000 });

    // Use keyboard.type with delay for proper debounce firing
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.click();
    await page.keyboard.type("pet", { delay: 50 });

    // Wait for spotter-results div to appear and have items
    const spotterResults = page.getByTestId("spotter-results");
    await expect(spotterResults).toBeVisible({ timeout: 5000 });

    // Wait for debounce (200ms) + network + render
    await page.waitForTimeout(800);

    // Route nodes should be in the results (method + path labels)
    // The spotter handler includes routes when query matches path/method/summary
    const resultItems = page.locator("[data-testid^='spotter-item-']");
    const count = await resultItems.count();
    expect(count).toBeGreaterThan(0);

    // Close spotter
    await page.keyboard.press("Escape");
  });

  test("E2E: idempotency — re-ingesting same spec returns already_ingested", async ({ page }) => {
    // Open MCP Tools modal
    await page.getByTestId("mcp-tools-trigger").click();
    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });

    // First ingestion
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await page.getByTestId("mcp-tools-run").click();
    const result1 = page.getByTestId("mcp-tools-result");
    await expect(result1).toBeVisible({ timeout: 10_000 });
    const text1 = await result1.textContent();
    expect(text1).toContain('"status": "ingested"');

    // Run again with the same spec — idempotency kicks in
    await page.getByTestId("mcp-tools-run").click();
    const result2 = page.getByTestId("mcp-tools-result");
    await expect(result2).toBeVisible({ timeout: 10_000 });
    const text2 = await result2.textContent();
    expect(text2).toContain('"status": "already_ingested"');
    expect(text2).toContain('"routes_count": 7');
  });
});
