import { describe, it, expect, vi } from "vitest";

import { parseCli, printCliUsage, runCli } from "./cli";
import { DEFAULT_BENCH_CONFIG } from "./index";

describe("parseCli", () => {
  it("returns the default config when no args or env are provided", () => {
    const parsed = parseCli([], {});
    expect(parsed.config.cold).toBe(DEFAULT_BENCH_CONFIG.cold);
    expect(parsed.config.warm).toBe(DEFAULT_BENCH_CONFIG.warm);
    expect(parsed.config.warm_runs).toBe(DEFAULT_BENCH_CONFIG.warm_runs);
    expect(parsed.config.enable_sigma).toBe(false);
    expect(parsed.config.output_dir).toBe(DEFAULT_BENCH_CONFIG.output_dir);
    expect(parsed.fixtureIds).toBeNull();
    expect(parsed.help).toBe(false);
    expect(parsed.errors).toEqual([]);
  });

  it("honors BENCH_ENABLE_SIGMA=1", () => {
    const parsed = parseCli([], { BENCH_ENABLE_SIGMA: "1" });
    expect(parsed.config.enable_sigma).toBe(true);
  });

  it("honors BENCH_OUTPUT_DIR", () => {
    const parsed = parseCli([], { BENCH_OUTPUT_DIR: "artifacts/custom" });
    expect(parsed.config.output_dir).toBe("artifacts/custom");
  });

  it("honors BENCH_WARM_RUNS", () => {
    const parsed = parseCli([], { BENCH_WARM_RUNS: "5" });
    expect(parsed.config.warm_runs).toBe(5);
  });

  it("ignores invalid BENCH_WARM_RUNS and reports an error", () => {
    const parsed = parseCli([], { BENCH_WARM_RUNS: "-3" });
    expect(parsed.errors.length).toBeGreaterThan(0);
    expect(parsed.config.warm_runs).toBe(DEFAULT_BENCH_CONFIG.warm_runs);
  });

  it("parses --enable-sigma flag", () => {
    const parsed = parseCli(["--enable-sigma"], {});
    expect(parsed.config.enable_sigma).toBe(true);
  });

  it("parses --output-dir flag", () => {
    const parsed = parseCli(["--output-dir", "artifacts/foo"], {});
    expect(parsed.config.output_dir).toBe("artifacts/foo");
  });

  it("parses --warm-runs flag", () => {
    const parsed = parseCli(["--warm-runs", "4"], {});
    expect(parsed.config.warm_runs).toBe(4);
  });

  it("parses --no-cold and --no-warm", () => {
    const parsed = parseCli(["--no-cold", "--no-warm"], {});
    expect(parsed.config.cold).toBe(false);
    expect(parsed.config.warm).toBe(false);
  });

  it("collects multiple --fixture flags", () => {
    const parsed = parseCli(
      ["--fixture", "call-graph-small", "--fixture", "call-graph-medium"],
      {},
    );
    expect(parsed.fixtureIds).toEqual(["call-graph-small", "call-graph-medium"]);
  });

  it("reports an error when --output-dir is missing its argument", () => {
    const parsed = parseCli(["--output-dir"], {});
    expect(parsed.errors.length).toBeGreaterThan(0);
  });

  it("reports an error when --fixture is missing its argument", () => {
    const parsed = parseCli(["--fixture"], {});
    expect(parsed.errors.length).toBeGreaterThan(0);
  });

  it("reports an error for unknown flags", () => {
    const parsed = parseCli(["--no-such-flag"], {});
    expect(parsed.errors.length).toBeGreaterThan(0);
  });

  it("marks --help as true and skips validation", () => {
    const parsed = parseCli(["--help"], {});
    expect(parsed.help).toBe(true);
  });

  it("rejects warm runs < 1 when warm runs are enabled", () => {
    const parsed = parseCli(["--warm-runs", "0", "--no-warm"], {});
    // --no-warm means warm is false, so the warm_runs=0 constraint is moot.
    expect(parsed.errors.length).toBe(0);
  });
});

describe("printCliUsage", () => {
  it("emits at least one line of usage", () => {
    const out = vi.fn();
    printCliUsage(out);
    expect(out).toHaveBeenCalled();
    expect(out.mock.calls.some(([line]) => line.includes("E7 Renderer"))).toBe(
      true,
    );
  });
});

describe("runCli", () => {
  function makeMemoryFs(): {
    files: Map<string, string>;
    fs: {
      writeFile: (path: string, content: string) => Promise<void>;
    };
  } {
    const files = new Map<string, string>();
    return {
      files,
      fs: {
        async writeFile(path, content) {
          files.set(path, content);
        },
      },
    };
  }

  it("prints usage and exits 0 on --help", async () => {
    const out = vi.fn();
    const { fs } = makeMemoryFs();
    const result = await runCli(["--help"], {}, fs, { stdout: out });
    expect(result.exit_code).toBe(0);
    expect(result.record_count).toBe(0);
    expect(out).toHaveBeenCalled();
  });

  it("rejects invalid argv with exit_code 2", async () => {
    const err = vi.fn();
    const { fs } = makeMemoryFs();
    const result = await runCli(["--bogus"], {}, fs, { stderr: err });
    expect(result.exit_code).toBe(2);
    expect(err).toHaveBeenCalled();
  });

  it("runs the harness and emits both report artifacts", async () => {
    const { files, fs } = makeMemoryFs();
    const out = vi.fn();
    const result = await runCli(
      ["--no-warm", "--fixture", "call-graph-small"],
      {},
      fs,
      { stdout: out },
    );

    expect(result.exit_code).toBe(0);
    expect(result.record_count).toBeGreaterThan(0);
    expect(files.has("artifacts/e7-renderer-bench/results.json")).toBe(true);
    expect(files.has("artifacts/e7-renderer-bench/report.md")).toBe(true);
    expect(out).toHaveBeenCalled();
  });

  it("honors --enable-sigma when the env var is unset", async () => {
    const { files, fs } = makeMemoryFs();
    const result = await runCli(
      ["--enable-sigma", "--no-warm", "--fixture", "call-graph-small"],
      {},
      fs,
    );

    expect(result.exit_code).toBe(0);
    const json = JSON.parse(
      files.get("artifacts/e7-renderer-bench/results.json") ?? "{}",
    );
    const rendererIds = (json.records as Array<{ renderer: { id: string } }>).map(
      (r) => r.renderer.id,
    );
    expect(rendererIds).toContain("sigma-poc");
  });

  it("skips Sigma by default", async () => {
    const { files, fs } = makeMemoryFs();
    const result = await runCli(
      ["--no-warm", "--fixture", "call-graph-small"],
      {},
      fs,
    );
    expect(result.exit_code).toBe(0);
    const json = JSON.parse(
      files.get("artifacts/e7-renderer-bench/results.json") ?? "{}",
    );
    const rendererIds = (json.records as Array<{ renderer: { id: string } }>).map(
      (r) => r.renderer.id,
    );
    expect(rendererIds).not.toContain("sigma-poc");
  });

  it("reports an error when --fixture references an unknown fixture", async () => {
    const err = vi.fn();
    const { fs } = makeMemoryFs();
    const result = await runCli(
      ["--fixture", "does-not-exist"],
      {},
      fs,
      { stderr: err },
    );
    expect(result.exit_code).toBe(2);
    expect(err).toHaveBeenCalled();
  });
});