/**
 * CLI for the E7 benchmark harness.
 *
 * The CLI is split into two layers:
 *
 *   - `parseCli(argv, env)` -- pure. Takes argv and env, returns a
 *     `BenchConfig` and a list of CLI flags. Tested under vitest.
 *   - `runCli(argv, env, fs)` -- impure side. Calls the runner and
 *     writes the report. The entrypoint in `entry.ts` calls
 *     `runCli` with a real node fs.
 *
 * Environment variables:
 *
 *   - `BENCH_ENABLE_SIGMA=1` -- opt into the Sigma adapter.
 *   - `BENCH_OUTPUT_DIR`     -- override the report output directory.
 *   - `BENCH_WARM_RUNS=N`    -- number of warm runs per cell (default 2).
 *
 * CLI flags (after `--`):
 *
 *   - `--enable-sigma`        -- same as `BENCH_ENABLE_SIGMA=1`.
 *   - `--output-dir <path>`   -- same as `BENCH_OUTPUT_DIR`.
 *   - `--warm-runs <n>`       -- same as `BENCH_WARM_RUNS`.
 *   - `--no-cold`             -- skip cold runs.
 *   - `--no-warm`             -- skip warm runs.
 *   - `--fixture <id>`        -- limit the run to a single fixture
 *                                (repeatable).
 *   - `--help`                -- print usage.
 */

import {
  DEFAULT_BENCH_CONFIG,
  type BenchConfig,
} from "./index";
import { runBench, type BenchProgressEvent } from "./runner";
import { writeReport, type ReportFilesystem } from "./report";
import { loadFixture } from "./fixtures";

export interface ParsedCli {
  config: BenchConfig;
  fixtureIds: string[] | null;
  help: boolean;
  errors: string[];
}

/**
 * Parse argv + env into a `BenchConfig`. Pure function -- the test
 * suite drives this with synthetic inputs.
 */
export function parseCli(
  argv: readonly string[],
  env: NodeJS.ProcessEnv = process.env,
): ParsedCli {
  const errors: string[] = [];
  let enableSigma = env.BENCH_ENABLE_SIGMA === "1";
  let outputDir = env.BENCH_OUTPUT_DIR ?? DEFAULT_BENCH_CONFIG.output_dir;
  let warmRuns = parsePositiveInt(
    env.BENCH_WARM_RUNS,
    DEFAULT_BENCH_CONFIG.warm_runs,
    errors,
    "BENCH_WARM_RUNS",
  );
  let cold = DEFAULT_BENCH_CONFIG.cold;
  let warm = DEFAULT_BENCH_CONFIG.warm;
  let help = false;
  const fixtureIds: string[] = [];

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--help":
      case "-h":
        help = true;
        break;
      case "--enable-sigma":
        enableSigma = true;
        break;
      case "--output-dir": {
        const value = argv[i + 1];
        if (!value) {
          errors.push("--output-dir requires a path argument");
        } else {
          outputDir = value;
          i++;
        }
        break;
      }
      case "--warm-runs": {
        const value = argv[i + 1];
        const parsed = parsePositiveInt(
          value ?? "",
          DEFAULT_BENCH_CONFIG.warm_runs,
          errors,
          "--warm-runs",
        );
        if (parsed !== DEFAULT_BENCH_CONFIG.warm_runs || value) {
          warmRuns = parsed;
        }
        if (value) i++;
        break;
      }
      case "--no-cold":
        cold = false;
        break;
      case "--no-warm":
        warm = false;
        break;
      case "--fixture": {
        const value = argv[i + 1];
        if (!value) {
          errors.push("--fixture requires a fixture id argument");
        } else {
          fixtureIds.push(value);
          i++;
        }
        break;
      }
      default:
        if (arg?.startsWith("-")) {
          errors.push(`unknown flag: ${arg}`);
        }
        // Non-flag positional arguments are ignored for now.
        break;
    }
  }

  if (warmRuns < 1 && warm) {
    errors.push(
      "warm runs cannot be less than 1 when warm runs are enabled",
    );
  }

  return {
    config: {
      ...DEFAULT_BENCH_CONFIG,
      cold,
      warm,
      warm_runs: warmRuns,
      enable_sigma: enableSigma,
      output_dir: outputDir,
    },
    fixtureIds: fixtureIds.length === 0 ? null : fixtureIds,
    help,
    errors,
  };
}

function parsePositiveInt(
  raw: string | undefined,
  fallback: number,
  errors: string[],
  label: string,
): number {
  if (raw === undefined || raw === "") return fallback;
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed < 0 || !Number.isInteger(parsed)) {
    errors.push(`${label} must be a non-negative integer; got "${raw}"`);
    return fallback;
  }
  return parsed;
}

/**
 * Print CLI usage to stdout.
 */
export function printCliUsage(out: (line: string) => void = console.log): void {
  out("E7 Renderer Benchmark CLI");
  out("");
  out("Usage: bench:renderer [--enable-sigma] [--output-dir <path>] [--warm-runs <n>]");
  out("                      [--no-cold] [--no-warm] [--fixture <id>...] [--help]");
  out("");
  out("Environment variables:");
  out("  BENCH_ENABLE_SIGMA=1   opt into the Sigma proof-of-concept adapter");
  out("  BENCH_OUTPUT_DIR=<path>  override the report output directory");
  out("  BENCH_WARM_RUNS=<n>    number of warm runs per cell (default 2)");
  out("");
  out("Flags:");
  out("  --enable-sigma          opt into the Sigma adapter");
  out("  --output-dir <path>     override the report output directory");
  out("  --warm-runs <n>         number of warm runs per cell");
  out("  --no-cold               skip cold runs");
  out("  --no-warm               skip warm runs");
  out("  --fixture <id>          limit the run to a single fixture (repeatable)");
  out("  --help, -h              print this message");
}

/**
 * Run the harness with the parsed CLI. Returns the number of
 * records produced. The caller supplies a `ReportFilesystem` so
 * the same code runs under vitest (in-memory fs) and in production
 * (node fs).
 */
export async function runCli(
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
  fs: ReportFilesystem,
  options: { stdout?: (line: string) => void; stderr?: (line: string) => void } = {},
): Promise<{ exit_code: number; record_count: number }> {
  const stdout = options.stdout ?? console.log;
  const stderr = options.stderr ?? console.error;
  const parsed = parseCli(argv, env);

  if (parsed.help) {
    printCliUsage(stdout);
    return { exit_code: 0, record_count: 0 };
  }

  if (parsed.errors.length > 0) {
    for (const error of parsed.errors) {
      stderr(`error: ${error}`);
    }
    printCliUsage(stderr);
    return { exit_code: 2, record_count: 0 };
  }

  // Validate fixture ids up front so we fail before running anything.
  // Validate fixture ids up front so we fail before running anything.
  let fixtures: ReturnType<typeof loadFixture>[] | undefined;
  if (parsed.fixtureIds) {
    try {
      fixtures = parsed.fixtureIds.map((id) => loadFixture(id));
    } catch (err) {
      stderr(`error: ${describeError(err)}`);
      return { exit_code: 2, record_count: 0 };
    }
  }

  const events: BenchProgressEvent[] = [];
  const records = await runBench(parsed.config, {
    fixtures,
    hooks: {
      onProgress: (event) => {
        events.push(event);
        stdout(
          `[bench] ${event.fixture_id} x ${event.renderer_id} (${event.run_mode} #${event.run_index}) -> ${event.status}`,
        );
      },
    },
  });

  const result = await writeReport({
    records,
    output_dir: parsed.config.output_dir,
    fs,
  });

  stdout(`[bench] wrote ${result.json_path}`);
  stdout(`[bench] wrote ${result.markdown_path}`);
  stdout(`[bench] ${records.length} records, ${events.length} progress events`);

  return { exit_code: 0, record_count: records.length };
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}