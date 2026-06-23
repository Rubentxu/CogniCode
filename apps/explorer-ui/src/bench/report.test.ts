import { describe, it, expect } from "vitest";

import { renderMarkdown, writeReport } from "./report";
import type { MetricsRecord } from "./metrics";
import { makeMetricsRecord } from "./metrics";

function makeRecord(overrides: Partial<{
  fixture_id: string;
  renderer_id: "cytoscape-canvas" | "cytoscape-webgl" | "sigma-poc";
  mode: "cold" | "warm";
  index: number;
  load: number;
  select: number;
  pan: number;
  zoom: number;
  relayout: number;
  selection_works: boolean;
  edge_highlight_works: boolean;
  layout_completed: boolean;
  regressions: string[];
  notes: string;
}> = {}): MetricsRecord {
  const fixtureId = overrides.fixture_id ?? "call-graph-small";
  const rendererId = overrides.renderer_id ?? "cytoscape-canvas";
  const mode = overrides.mode ?? "cold";
  const index = overrides.index ?? 0;
  const selectionWorks = overrides.selection_works ?? true;
  const edgeHighlightWorks = overrides.edge_highlight_works ?? true;
  const layoutCompleted = overrides.layout_completed ?? true;
  const regressions = overrides.regressions ?? [];
  return makeMetricsRecord({
    runner: {
      browser: "chromium",
      browser_version: "120",
      os: "linux",
      machine_profile: "ci",
    },
    fixture: {
      fixture_id: fixtureId,
      kind: "call_graph",
      size_band: "small",
      node_count: 12,
      edge_count: 11,
    },
    renderer: {
      id: rendererId,
      version: "3.34.0",
      config: {},
    },
    run: { mode, index },
    timings_ms: {
      load: overrides.load ?? 1,
      first_render: 0,
      fit: 0,
      select: overrides.select ?? 2,
      pan: overrides.pan ?? 3,
      zoom: overrides.zoom ?? 4,
      relayout: overrides.relayout ?? 5,
    },
    behavior: {
      selection_works: selectionWorks,
      edge_highlight_works: edgeHighlightWorks,
      layout_completed: layoutCompleted,
      regressions,
    },
    notes: overrides.notes ?? "",
  });
}

describe("renderMarkdown header", () => {
  it("renders a heading, timestamp, and record count", () => {
    const md = renderMarkdown([makeRecord()], "2026-06-21T00:00:00.000Z");
    expect(md).toContain("# E7 Renderer Benchmark Report");
    expect(md).toContain("Generated at: 2026-06-21T00:00:00.000Z");
    expect(md).toContain("Record count: 1");
  });

  it("renders an empty report cleanly", () => {
    const md = renderMarkdown([], "2026-06-21T00:00:00.000Z");
    expect(md).toContain("No records were produced.");
  });
});

describe("renderMarkdown grouping", () => {
  it("groups by renderer then by fixture", () => {
    const records = [
      makeRecord({ renderer_id: "cytoscape-canvas", fixture_id: "call-graph-small" }),
      makeRecord({ renderer_id: "cytoscape-canvas", fixture_id: "call-graph-medium" }),
      makeRecord({ renderer_id: "cytoscape-webgl", fixture_id: "call-graph-small" }),
    ];
    const md = renderMarkdown(records, "2026-06-21T00:00:00.000Z");

    expect(md).toContain("## Renderer: `cytoscape-canvas`");
    expect(md).toContain("## Renderer: `cytoscape-webgl`");
    expect(md.indexOf("cytoscape-canvas")).toBeLessThan(
      md.indexOf("cytoscape-webgl"),
    );
  });

  it("renders a per-fixture table with all timings", () => {
    const record = makeRecord({
      load: 12.5,
      select: 3.4,
      pan: 2.1,
      zoom: 1.5,
      relayout: 7.8,
    });
    const md = renderMarkdown([record], "2026-06-21T00:00:00.000Z");

    expect(md).toContain("| run | mode | load (ms)");
    expect(md).toContain("12.50");
    expect(md).toContain("3.40");
    expect(md).toContain("2.10");
    expect(md).toContain("1.50");
    expect(md).toContain("7.80");
  });

  it("formats non-finite timings as n/a", () => {
    const record = makeRecord({ load: Number.NaN });
    const md = renderMarkdown([record], "2026-06-21T00:00:00.000Z");
    expect(md).toContain("n/a");
  });
});

describe("renderMarkdown regressions section", () => {
  it("lists every failing run", () => {
    const records = [
      makeRecord({
        selection_works: false,
        regressions: ["select returned false"],
        notes: "node missing",
      }),
      makeRecord({ layout_completed: false, regressions: ["layout incomplete"] }),
      makeRecord(), // valid
    ];
    const md = renderMarkdown(records, "2026-06-21T00:00:00.000Z");

    expect(md).toContain("Total failing runs: 2 of 3");
    expect(md).toContain("select returned false");
    expect(md).toContain("node missing");
    expect(md).toContain("layout incomplete");
  });

  it("reports zero regressions when all runs are valid", () => {
    const records = [makeRecord(), makeRecord({ mode: "warm", index: 1 })];
    const md = renderMarkdown(records, "2026-06-21T00:00:00.000Z");
    expect(md).toContain("No regressions detected.");
  });
});

describe("writeReport", () => {
  it("emits both files into the configured output directory", async () => {
    const files: Array<{ path: string; content: string }> = [];
    const fs = {
      writeFile: async (path: string, content: string) => {
        files.push({ path, content });
      },
    };

    const result = await writeReport({
      records: [makeRecord()],
      output_dir: "artifacts/e7-renderer-bench",
      fs,
      timestamp: "2026-06-21T00:00:00.000Z",
    });

    expect(result.json_path).toBe("artifacts/e7-renderer-bench/results.json");
    expect(result.markdown_path).toBe("artifacts/e7-renderer-bench/report.md");
    expect(files).toHaveLength(2);
    expect(files[0]!.path).toBe(result.json_path);
    expect(files[1]!.path).toBe(result.markdown_path);
  });

  it("json payload includes schema version and record_count", async () => {
    const files: Array<{ path: string; content: string }> = [];
    const fs = {
      writeFile: async (path: string, content: string) => {
        files.push({ path, content });
      },
    };

    await writeReport({
      records: [makeRecord()],
      output_dir: "/tmp/bench",
      fs,
      timestamp: "2026-06-21T00:00:00.000Z",
    });

    const json = JSON.parse(files[0]!.content);
    expect(json.schema_version).toBe("e7.0");
    expect(json.record_count).toBe(1);
    expect(json.records).toHaveLength(1);
    expect(json.records[0]!.renderer.id).toBe("cytoscape-canvas");
  });

  it("handles trailing slashes in the output directory", async () => {
    const files: Array<{ path: string; content: string }> = [];
    const fs = {
      writeFile: async (path: string, content: string) => {
        files.push({ path, content });
      },
    };
    const result = await writeReport({
      records: [makeRecord()],
      output_dir: "artifacts/e7-renderer-bench/",
      fs,
      timestamp: "2026-06-21T00:00:00.000Z",
    });
    expect(result.json_path).toBe("artifacts/e7-renderer-bench/results.json");
  });

  it("rejects when the filesystem fails to write", async () => {
    const fs = {
      writeFile: () => {
        throw new Error("disk full");
      },
    };
    await expect(
      writeReport({
        records: [makeRecord()],
        output_dir: "artifacts/e7-renderer-bench",
        fs,
        timestamp: "2026-06-21T00:00:00.000Z",
      }),
    ).rejects.toThrow(/disk full/);
  });
});