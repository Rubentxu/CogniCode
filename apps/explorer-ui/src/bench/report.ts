/**
 * Bench report writer.
 *
 * Accepts a list of `MetricsRecord` and emits two artifacts:
 *
 *   - `<output_dir>/results.json` — the raw records
 *   - `<output_dir>/report.md`    — a Markdown comparative summary
 *
 * The Markdown report groups runs by renderer, then by fixture, and
 * ends with a regressions section listing every run where the
 * behavior checks failed.
 *
 * The writer is pure: it does not import any node-only API. The CLI
 * in T9 wires it to the real filesystem. Tests can drive the writer
 * against an in-memory directory by supplying a `Fs` implementation.
 */

import type { MetricsRecord } from "./metrics";

export interface ReportFilesystem {
  writeFile(path: string, content: string): Promise<void> | void;
}

export interface WriteReportArgs {
  records: readonly MetricsRecord[];
  output_dir: string;
  fs?: ReportFilesystem;
  /** ISO timestamp written into the report header. Default: now. */
  timestamp?: string;
}

export interface WriteReportResult {
  json_path: string;
  markdown_path: string;
}

const DEFAULT_FS: ReportFilesystem = {
  writeFile: (path, content) => {
    // Best-effort default. Real CLI consumers (T9) supply their own
    // fs implementation that knows where to write on disk.
    console.log(`[bench] would write ${path} (${content.length} chars)`);
  },
};

export async function writeReport(args: WriteReportArgs): Promise<WriteReportResult> {
  const fs = args.fs ?? DEFAULT_FS;
  const timestamp = args.timestamp ?? new Date().toISOString();

  const jsonPath = joinPath(args.output_dir, "results.json");
  const markdownPath = joinPath(args.output_dir, "report.md");

  const json = JSON.stringify(
    {
      schema_version: "e7.0",
      generated_at: timestamp,
      record_count: args.records.length,
      records: args.records,
    },
    null,
    2,
  );

  const markdown = renderMarkdown(args.records, timestamp);

  await fs.writeFile(jsonPath, json);
  await fs.writeFile(markdownPath, markdown);

  return { json_path: jsonPath, markdown_path: markdownPath };
}

function joinPath(dir: string, file: string): string {
  if (dir.endsWith("/") || dir.endsWith("\\")) {
    return `${dir}${file}`;
  }
  return `${dir}/${file}`;
}

/**
 * Build the Markdown report. Pure function -- exercised by tests
 * without touching the filesystem.
 */
export function renderMarkdown(
  records: readonly MetricsRecord[],
  timestamp: string,
): string {
  const lines: string[] = [];
  lines.push("# E7 Renderer Benchmark Report");
  lines.push("");
  lines.push(`Generated at: ${timestamp}`);
  lines.push(`Record count: ${records.length}`);
  lines.push("");

  if (records.length === 0) {
    lines.push("No records were produced. The harness ran zero cells.");
    lines.push("");
    return lines.join("\n");
  }

  const grouped = groupByRenderer(records);
  for (const [rendererId, byFixture] of grouped) {
    lines.push(`## Renderer: \`${rendererId}\``);
    lines.push("");

    for (const [fixtureId, fixtureRecords] of byFixture) {
      lines.push(`### Fixture: \`${fixtureId}\``);
      lines.push("");
      lines.push(
        "| run | mode | load (ms) | select (ms) | pan (ms) | zoom (ms) | relayout (ms) | valid | regressions |",
      );
      lines.push(
        "|-----|------|-----------|-------------|-----------|-----------|----------------|-------|-------------|",
      );

      for (const record of fixtureRecords) {
        const valid = isBehaviorValid(record);
        const regressions = record.behavior.regressions.length;
        lines.push(
          `| ${record.run.index} | ${record.run.mode} | ${fmt(record.timings_ms.load)} | ${fmt(record.timings_ms.select)} | ${fmt(record.timings_ms.pan)} | ${fmt(record.timings_ms.zoom)} | ${fmt(record.timings_ms.relayout)} | ${valid ? "yes" : "no"} | ${regressions} |`,
        );
      }

      lines.push("");
    }
  }

  const failures = records.filter((r) => !isBehaviorValid(r));
  lines.push("## Regressions");
  lines.push("");

  if (failures.length === 0) {
    lines.push("No regressions detected.");
    lines.push("");
  } else {
    lines.push(
      `Total failing runs: ${failures.length} of ${records.length}`,
    );
    lines.push("");
    for (const failure of failures) {
      lines.push(
        `- \`${failure.renderer.id}\` x \`${failure.fixture.fixture_id}\` (run ${failure.run.index}, mode ${failure.run.mode}): ${failure.behavior.regressions.join("; ")}${failure.notes ? ` -- ${failure.notes}` : ""}`,
      );
    }
    lines.push("");
  }

  return lines.join("\n");
}

function isBehaviorValid(record: MetricsRecord): boolean {
  return (
    record.behavior.selection_works &&
    record.behavior.edge_highlight_works &&
    record.behavior.layout_completed
  );
}

function fmt(value: number): string {
  if (!Number.isFinite(value)) return "n/a";
  return value.toFixed(2);
}

function groupByRenderer(
  records: readonly MetricsRecord[],
): Map<string, Map<string, MetricsRecord[]>> {
  const outer = new Map<string, Map<string, MetricsRecord[]>>();
  for (const record of records) {
    let inner = outer.get(record.renderer.id);
    if (!inner) {
      inner = new Map();
      outer.set(record.renderer.id, inner);
    }
    const fixtureId = record.fixture.fixture_id;
    const list = inner.get(fixtureId) ?? [];
    list.push(record);
    inner.set(fixtureId, list);
  }
  return outer;
}