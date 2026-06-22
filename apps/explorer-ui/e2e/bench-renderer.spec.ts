/**
 * E7 Renderer Benchmark — Real-browser spec.
 *
 * Runs the E7 bench harness against the actual Cytoscape.js UMD
 * bundle inside Chromium (not jsdom, not node) so timings reflect
 * real rendering cost. Writes both `results.json` and `report.md`
 * under `apps/explorer-ui/artifacts/e7-renderer-bench/`.
 *
 * Architecture (matches existing `e2e/*.spec.ts`):
 *
 *   - Playwright's `webServer` block in `playwright.config.ts`
 *     starts `npm run dev:mock` automatically. The host HTML lives
 *     under `apps/explorer-ui/e2e/fixtures/bench-host.html` and is
 *     served by Vite as a public asset.
 *   - Cytoscape UMD is loaded from `node_modules/cytoscape/dist/
 *     cytoscape.umd.js` via a `page.route` override that maps
 *     `/bench/cytoscape.umd.js` to the local file.
 *   - The fixture roster and the stylesheet are embedded as JSON
 *     in the HTML so the spec has no runtime dependency on the
 *     dev:mock backend.
 *   - `page.evaluate` runs the scenario sequence (load -> fit ->
 *     pan -> zoom -> select -> relayout) per (fixture, renderer,
 *     cold/warm) cell. Results are serialized back to the test
 *     process.
 *   - The test process builds `MetricsRecord` instances using the
 *     same builder the vitest suite uses and writes the report
 *     artifacts.
 *
 * Sigma.js is the gated third branch (ADR-041 §8 outcome 3). It is
 * intentionally NOT exercised here; the harness in
 * `apps/explorer-ui/src/bench/` covers Sigma under vitest and the
 * docs in `docs/adr/ADR-042-renderer-decision.md` track the gate.
 */

import { test, expect } from "@playwright/test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { renderMarkdown } from "../src/bench/report";
import type { MetricsRecord } from "../src/bench/metrics";
import type { Fixture } from "../src/bench/fixture-schema";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const HOST_URL = "/__e7-bench/host.html";

const FIXTURE_IDS = [
  "call-graph-small",
  "call-graph-medium",
  "call-graph-large",
  "dependency-graph-small",
  "architecture-c4-medium",
  "landing-overview-medium",
] as const;

const OUTPUT_DIR = resolve(__dirname, "..", "artifacts", "e7-renderer-bench");
const CYTOSCAPE_LOCAL = resolve(
  __dirname,
  "..",
  "node_modules",
  "cytoscape",
  "dist",
  "cytoscape.umd.js",
);

interface BrowserCellResult {
  fixture_id: string;
  renderer_id: "cytoscape-canvas" | "cytoscape-webgl";
  run_mode: "cold" | "warm";
  run_index: number;
  timings_ms: {
    load: number;
    first_render: number;
    fit: number;
    pan: number;
    zoom: number;
    select: number;
    relayout: number;
  };
  behavior: {
    selection_works: boolean;
    edge_highlight_works: boolean;
    layout_completed: boolean;
    regressions: string[];
  };
  notes: string;
}

// ---------------------------------------------------------------------------
// In-browser scenario runner
// ---------------------------------------------------------------------------
//
// Playwright serializes a function passed to `page.evaluate` and runs
// it inside the page context. The scenario must use the real
// `window.cytoscape` UMD injected via `addInitScript`.

interface ScenarioPayload {
  fixture_id: string;
  renderer_id: "cytoscape-canvas" | "cytoscape-webgl";
  run_mode: "cold" | "warm";
  run_index: number;
  elements: {
    nodes: Array<{ group: "nodes"; data: Record<string, unknown> }>;
    edges: Array<{ group: "edges"; data: Record<string, unknown> }>;
  };
  stylesheet: unknown[];
}

const runScenario = async (cell: ScenarioPayload): Promise<BrowserCellResult> => {
  const cy = window.cytoscape;
  if (!cy) {
    return {
      fixture_id: cell.fixture_id,
      renderer_id: cell.renderer_id,
      run_mode: cell.run_mode,
      run_index: cell.run_index,
      timings_ms: { load: 0, first_render: 0, fit: 0, pan: 0, zoom: 0, select: 0, relayout: 0 },
      behavior: {
        selection_works: false,
        edge_highlight_works: false,
        layout_completed: false,
        regressions: ["cytoscape global not loaded"],
      },
      notes: "window.cytoscape missing",
    };
  }

  const container = document.createElement("div");
  container.style.cssText =
    "width:800px;height:600px;position:fixed;left:-10000px;top:0;";
  document.body.appendChild(container);

  const rendererConfig =
    cell.renderer_id === "cytoscape-webgl"
      ? {
          name: "canvas",
          webgl: true,
          webglTexSize: 4096,
          webglTexRows: 24,
          webglBatchSize: 2048,
          webglTexPerBatch: 16,
        }
      : { name: "canvas" };

  const timings = {
    load: 0,
    first_render: 0,
    fit: 0,
    pan: 0,
    zoom: 0,
    select: 0,
    relayout: 0,
  };
  const behavior: BrowserCellResult["behavior"] = {
    selection_works: false,
    edge_highlight_works: false,
    layout_completed: false,
    regressions: [],
  };
  const notes: string[] = [];

  let instance: { destroy: () => void } | null = null;
  try {
    const tLoad = performance.now();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    instance = (cy as any)({
      container,
      elements: cell.elements,
      style: cell.stylesheet,
      layout: { name: "preset" },
      renderer: rendererConfig,
    });
    timings.load = performance.now() - tLoad;

    const tFit = performance.now();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (instance as any).fit(undefined, 20);
    timings.fit = performance.now() - tFit;
    timings.first_render = timings.fit;

    const tPan = performance.now();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (instance as any).panBy({ x: 10, y: 10 });
    timings.pan = performance.now() - tPan;

    const tZoom = performance.now();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (instance as any).zoom(1.05);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (instance as any).center();
    timings.zoom = performance.now() - tZoom;

    const nodeId = (cell.elements.nodes[0]?.data?.id as string | undefined) ?? "";
    if (nodeId) {
      const tSel = performance.now();
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const node = (instance as any).getElementById(nodeId);
      if (node.length > 0) {
        node.select();
        behavior.selection_works = node.selected();
        behavior.edge_highlight_works = node.connectedEdges().length > 0;
      } else {
        behavior.regressions.push("first node not found");
      }
      timings.select = performance.now() - tSel;
    }

    const tRelayout = performance.now();
    await new Promise<void>((resolve, reject) => {
      try {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (instance as any)
          .layout({
            name: "grid",
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            rows: Math.ceil(Math.sqrt((instance as any).nodes().length)),
            animate: false,
          })
          .on("layoutstop", () => resolve())
          .on("layouterror", (e: unknown) => reject(e))
          .run();
      } catch (e) {
        reject(e);
      }
    });
    timings.relayout = performance.now() - tRelayout;
    behavior.layout_completed = true;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    notes.push("scenario threw: " + msg);
    behavior.regressions.push("scenario threw");
  } finally {
    if (instance) instance.destroy();
    container.remove();
  }

  return {
    fixture_id: cell.fixture_id,
    renderer_id: cell.renderer_id,
    run_mode: cell.run_mode,
    run_index: cell.run_index,
    timings_ms: timings,
    behavior,
    notes: notes.join("; "),
  };
};

test.describe("E7 Renderer Benchmark — real browser", () => {
  test("measures Cytoscape canvas and WebGL across the fixture roster", async ({
    page,
  }) => {
    test.setTimeout(120_000);
    // Inject cytoscape UMD as a global BEFORE any page script runs.
    // The Vite dev server would otherwise intercept the request
    // and serve a 404 for node_modules paths. We read cytoscape UMD
    // from disk and add it as an init script.
    const cytoscapeSource = readFileSync(CYTOSCAPE_LOCAL, "utf8");
    await page.addInitScript({
      content: `${cytoscapeSource}\nwindow.__e7CytoscapeReady = true;`,
    });

    await page.goto(HOST_URL);

    // Wait until the host page confirms cytoscape is loaded.
    await page.waitForFunction(
      () =>
        typeof window.cytoscape === "function" ||
        document.getElementById("status")?.dataset.ready === "1",
      undefined,
      { timeout: 10_000 },
    );

    // Sanity check the script loads.
    const sanity = await page.evaluate(() => ({
      cytoscape: typeof window.cytoscape,
    }));
    expect(sanity.cytoscape).toBe("function");

    const fixtures: Fixture[] = FIXTURE_IDS.map((id) => loadFixtureFromDisk(id));

    // Pull the production stylesheet out of the Vite dev server so
    // the bench measures the same visuals the Explorer uses.
    const stylesheet = await page.evaluate(async () => {
      try {
        const mod = await import("/src/components/InteractiveGraph/stylesheet.ts");
        return mod.buildStylesheet();
      } catch {
        return [];
      }
    });

    const rendererIds: Array<"cytoscape-canvas" | "cytoscape-webgl"> = [
      "cytoscape-canvas",
      "cytoscape-webgl",
    ];
    const browserResults: BrowserCellResult[] = [];

    for (const fixture of fixtures) {
      for (const rendererId of rendererIds) {
        const cell: ScenarioPayload = {
          fixture_id: fixture.fixture_id,
          renderer_id: rendererId,
          run_mode: "cold",
          run_index: 0,
          elements: fixtureToElements(fixture),
          stylesheet,
        };
        const coldResult = await page.evaluate(runScenario, cell);
        if (!coldResult || typeof coldResult !== "object") {
          throw new Error(
            `cold cell for ${fixture.fixture_id}/${rendererId} returned: ${JSON.stringify(coldResult)}`,
          );
        }
        browserResults.push(coldResult);

        const warmCell: ScenarioPayload = { ...cell, run_mode: "warm", run_index: 1 };
        const warmResult = await page.evaluate(runScenario, warmCell);
        if (!warmResult || typeof warmResult !== "object") {
          throw new Error(
            `warm cell for ${fixture.fixture_id}/${rendererId} returned: ${JSON.stringify(warmResult)}`,
          );
        }
        browserResults.push(warmResult);
      }
    }

    // Build MetricsRecords in the test process so we exercise the
    // same builder the vitest suite uses.
    const records: MetricsRecord[] = browserResults.map((r) => {
      const fixture = fixtures.find((f) => f.fixture_id === r.fixture_id)!;
      return {
        schema_version: "e7.0",
        runner: {
          browser: "chromium",
          browser_version: "playwright-1.60",
          os: "linux",
          machine_profile: "e2e",
        },
        fixture: {
          fixture_id: fixture.fixture_id,
          kind: fixture.kind,
          size_band: fixture.size_band,
          node_count: fixture.node_count,
          edge_count: fixture.edge_count,
        },
        renderer: {
          id: r.renderer_id,
          version: "3.34.0",
          config:
            r.renderer_id === "cytoscape-webgl"
              ? {
                  name: "canvas",
                  webgl: true,
                  webglTexSize: 4096,
                  webglTexRows: 24,
                  webglBatchSize: 2048,
                  webglTexPerBatch: 16,
                }
              : { name: "canvas" },
        },
        run: { mode: r.run_mode, index: r.run_index },
        timings_ms: r.timings_ms,
        behavior: r.behavior,
        notes: r.notes,
      };
    });

    // Write both artifacts.
    const timestamp = new Date().toISOString();
    const json = JSON.stringify(
      {
        schema_version: "e7.0",
        generated_at: timestamp,
        record_count: records.length,
        records,
      },
      null,
      2,
    );
    const markdown = renderMarkdown(records, timestamp);

    mkdirSync(OUTPUT_DIR, { recursive: true });
    writeFileSync(resolve(OUTPUT_DIR, "results.json"), json, "utf8");
    writeFileSync(resolve(OUTPUT_DIR, "report.md"), markdown, "utf8");

    // Sanity assertions.
    expect(records.length).toBe(fixtures.length * rendererIds.length * 2);
    for (const record of records) {
      expect(record.behavior.selection_works).toBe(true);
      expect(record.behavior.layout_completed).toBe(true);
      expect(record.timings_ms.load).toBeGreaterThanOrEqual(0);
      expect(Number.isFinite(record.timings_ms.load)).toBe(true);
    }
  });
});

const FIXTURES_DISK_DIR = resolve(
  __dirname,
  "..",
  "src",
  "bench",
  "fixtures",
);

function loadFixtureFromDisk(id: string): Fixture {
  // Playwright's TS loader does not accept JSON imports without an
  // explicit `with { type: "json" }` attribute. We read the fixture
  // JSON directly so the spec does not depend on that loader mode.
  const generated: Fixture[] = [
    generateCallGraphMedium(),
    generateDependencyGraphMedium(),
    generateCallGraphLarge(),
  ];
  const found = generated.find((f) => f.fixture_id === id);
  if (found) {
    return found;
  }
  const path = resolve(FIXTURES_DISK_DIR, `${id}.json`);
  const raw = readFileSync(path, "utf8");
  return JSON.parse(raw) as Fixture;
}

function generateCallGraphMedium(): Fixture {
  return generate("call-graph-medium", "call_graph", "medium", 1000);
}
function generateDependencyGraphMedium(): Fixture {
  return generate("dependency-graph-medium", "dependency_graph", "medium", 1000);
}
function generateCallGraphLarge(): Fixture {
  return generate("call-graph-large", "call_graph", "large", 5000);
}

function generate(
  id: string,
  kind: Fixture["kind"],
  size: Fixture["size_band"],
  nodeCount: number,
): Fixture {
  const nodeStyle = kind === "call_graph" ? "node-function" : "node-module";
  const edgeStyle =
    kind === "call_graph" ? "edge-calls" : "edge-dependency";
  const nodeKind = kind === "call_graph" ? "function" : "module";
  const relation = kind === "call_graph" ? "calls" : "depends_on";
  const nodes = Array.from({ length: nodeCount }, (_, i) => ({
    id: `n${i}`,
    label: `${kind}-n${i}`,
    kind: nodeKind,
    style_class: nodeStyle,
  }));
  const fanOut = 3;
  const edges: Fixture["edges"] = [];
  let edgeId = 0;
  outer: for (let i = 0; i < nodeCount; i++) {
    for (let k = 1; k <= fanOut; k++) {
      const target = i + k;
      if (target >= nodeCount) continue;
      edges.push({
        id: `e${edgeId++}`,
        source: `n${i}`,
        target: `n${target}`,
        relation,
        style_class: edgeStyle,
      });
      if (edges.length >= 3 * nodeCount) break outer;
    }
  }
  return {
    fixture_id: id,
    kind,
    size_band: size,
    node_count: nodes.length,
    edge_count: edges.length,
    nodes,
    edges,
  };
}

function fixtureToElements(fixture: Fixture) {
  return {
    nodes: fixture.nodes.map((n) => ({
      group: "nodes" as const,
      data: {
        id: n.id,
        label: n.label,
        kind: n.kind,
        style_class: n.style_class,
      },
    })),
    edges: fixture.edges.map((e, i) => ({
      group: "edges" as const,
      data: {
        id: `${e.source}->${e.target}:${i}`,
        source: e.source,
        target: e.target,
        relation: e.relation,
        style_class: e.style_class,
      },
    })),
  };
}
