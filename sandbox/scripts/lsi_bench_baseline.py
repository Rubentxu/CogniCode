#!/usr/bin/env python3
"""lsi_bench_baseline.py — LSI M0 benchmark baseline artifact + delta compare (E36 WU-2).

Runs `cargo bench -p cognicode-core --bench graph_benchmarks` with Criterion's
bencher output format (the same parse path proven by scripts/perf-budget-check.sh)
and either:

  capture   (default) writes the machine-readable baseline artifact
            sandbox/results/lsi-baseline/baseline.json with per-benchmark mean
            timings plus environment metadata (os, rustc, cargo, git commit).
  compare   re-runs the bench and emits a per-benchmark delta report
            {name, baseline_mean_us, current_mean_us, delta_pct} against the
            committed baseline. Exits non-zero with a clean report when no
            baseline exists (task 2.1 RED contract), and with `--fail-above`
            when any benchmark regresses beyond the threshold (scorecard gate).

Subprocess safety (threat matrix): fixed argv constants, shell=False, cwd
pinned to the project root, no user-supplied paths except output overrides.

Exit codes: 0 green, 1 contract failure (missing baseline / regression above
threshold / self-test failure), 2 environment error (bench command failed).
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = 1

RESULTS_DIR = "sandbox/results/lsi-baseline"
BASELINE_FILE = "baseline.json"
BENCH_TIMEOUT_S = 1800

BENCH_ARGV = [
    "cargo",
    "bench",
    "-p",
    "cognicode-core",
    "--bench",
    "graph_benchmarks",
    "--",
    "--output-format",
    "bencher",
]

# Criterion bencher format:
#   test <name> ... bench:         1,234.56 ns/iter (+/- 100)
BENCH_LINE_RE = re.compile(
    r"^\s*test\s+(\S+)\s+\.\.\.\s+bench:\s+([\d,]+(?:\.\d+)?)\s+(\w+)/iter"
)

UNIT_TO_US = {"ns": 0.001, "us": 1.0, "µs": 1.0, "ms": 1000.0, "s": 1_000_000.0}


# ── Parsing ──────────────────────────────────────────────────────────────────

def parse_bencher_output(text: str) -> dict[str, float]:
    """Parse Criterion bencher output into {name: mean_us}."""
    results: dict[str, float] = {}
    for line in text.splitlines():
        match = BENCH_LINE_RE.match(line)
        if not match:
            continue
        name, raw_value, unit = match.groups()
        value = float(raw_value.replace(",", ""))
        if unit not in UNIT_TO_US:
            raise ValueError(f"unknown bench unit {unit!r} in line: {line.strip()[:120]}")
        results[name] = value * UNIT_TO_US[unit]
    return results


def run_bench(project_root: Path) -> dict[str, float]:
    """Run the fixed-argv bench command and parse its output."""
    proc = subprocess.run(
        BENCH_ARGV,
        cwd=str(project_root),
        capture_output=True,
        text=True,
        timeout=BENCH_TIMEOUT_S,
        shell=False,
    )
    if proc.returncode != 0:
        tail = "\n".join(proc.stderr.strip().splitlines()[-15:])
        raise RuntimeError(f"cargo bench failed (exit {proc.returncode}):\n{tail}")
    parsed = parse_bencher_output(proc.stdout)
    if not parsed:
        raise RuntimeError(
            "cargo bench produced no parseable bencher output — "
            "expected 'test <name> ... bench: <value> <unit>/iter' lines"
        )
    return parsed


# ── Environment metadata ─────────────────────────────────────────────────────

def _run_version_cmd(argv: list[str], project_root: Path) -> str:
    try:
        proc = subprocess.run(
            argv,
            cwd=str(project_root),
            capture_output=True,
            text=True,
            timeout=30,
            shell=False,
        )
        if proc.returncode == 0:
            return proc.stdout.strip().splitlines()[0]
    except (OSError, subprocess.SubprocessError):
        pass
    return "unknown"


def collect_env(project_root: Path) -> dict[str, str]:
    return {
        "os": f"{platform.system()} {platform.release()} {platform.machine()}",
        "rustc": _run_version_cmd(["rustc", "--version"], project_root),
        "cargo": _run_version_cmd(["cargo", "--version"], project_root),
        "commit": _run_version_cmd(["git", "rev-parse", "HEAD"], project_root),
    }


# ── Capture / compare modes ──────────────────────────────────────────────────

def capture_baseline(
    project_root: Path, baseline_path: Path, bench_output_file: Path | None = None
) -> tuple[int, str]:
    """Run the bench and write the baseline artifact. Returns (exit_code, report)."""
    if bench_output_file is not None:
        means = parse_bencher_output(bench_output_file.read_text())
        if not means:
            raise ValueError(f"no bencher lines parsed from {bench_output_file}")
    else:
        means = run_bench(project_root)
    baseline = {
        "schema_version": SCHEMA_VERSION,
        "captured_at_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "commit": collect_env(project_root)["commit"],
        "env": collect_env(project_root),
        "benchmarks": [
            {"name": name, "mean_us": round(means[name], 4), "unit": "us"}
            for name in sorted(means)
        ],
    }
    baseline_path.parent.mkdir(parents=True, exist_ok=True)
    with open(baseline_path, "w") as f:
        json.dump(baseline, f, indent=2, sort_keys=True)
        f.write("\n")
    lines = [
        "LSI benchmark baseline — capture",
        f"  wrote {baseline_path}",
        f"  benchmarks: {len(baseline['benchmarks'])}",
        f"  commit: {baseline['commit']}",
        "  RESULT: OK",
    ]
    return 0, "\n".join(lines)


def compare_to_baseline(
    project_root: Path,
    baseline_path: Path,
    bench_output_file: Path | None = None,
    delta_output: Path | None = None,
    fail_above_pct: float | None = None,
) -> tuple[int, str]:
    """Compare a fresh bench run against the committed baseline.

    Emits per-benchmark {name, baseline_mean_us, current_mean_us, delta_pct}.
    Returns (exit_code, report): exit 1 with a clean report when the baseline
    is missing; with `--fail-above` set, exit 1 when any delta exceeds it.
    """
    if not baseline_path.exists():
        report = "\n".join(
            [
                "LSI benchmark baseline — compare",
                f"  RESULT: FAIL — baseline not found at {baseline_path}",
                "  Capture one first with: "
                "python3 sandbox/scripts/lsi_bench_baseline.py capture",
            ]
        )
        return 1, report

    with open(baseline_path) as f:
        baseline = json.load(f)
    baseline_rows = {
        b["name"]: b["mean_us"] for b in baseline.get("benchmarks", [])
    }
    if not baseline_rows:
        raise ValueError(
            f"baseline artifact {baseline_path} has no benchmarks entries — "
            "recapture it (schema drift is fail-closed)"
        )

    if bench_output_file is not None:
        current = parse_bencher_output(bench_output_file.read_text())
        if not current:
            raise ValueError(f"no bencher lines parsed from {bench_output_file}")
    else:
        current = run_bench(project_root)

    deltas = []
    for name in sorted(set(baseline_rows) | set(current)):
        base = baseline_rows.get(name)
        cur = current.get(name)
        if base is None or cur is None:
            delta_pct = None
        else:
            delta_pct = round((cur - base) / base * 100.0, 2) if base else None
        deltas.append(
            {
                "name": name,
                "baseline_mean_us": base,
                "current_mean_us": round(cur, 4) if cur is not None else None,
                "delta_pct": delta_pct,
            }
        )

    regressions = [
        row["name"]
        for row in deltas
        if fail_above_pct is not None
        and row["delta_pct"] is not None
        and row["delta_pct"] > fail_above_pct
    ]

    delta_artifact = {
        "schema_version": SCHEMA_VERSION,
        "baseline": str(baseline_path),
        "fail_above_pct": fail_above_pct,
        "regressions": regressions,
        "deltas": deltas,
    }
    if delta_output is not None:
        delta_output.parent.mkdir(parents=True, exist_ok=True)
        with open(delta_output, "w") as f:
            json.dump(delta_artifact, f, indent=2, sort_keys=True)
            f.write("\n")

    lines = [
        "LSI benchmark baseline — compare",
        f"  baseline: {baseline_path}",
        f"  benchmarks compared: {len(deltas)}",
    ]
    for row in deltas:
        base = "n/a" if row["baseline_mean_us"] is None else f"{row['baseline_mean_us']:.3f}"
        cur = "n/a" if row["current_mean_us"] is None else f"{row['current_mean_us']:.3f}"
        pct = "n/a" if row["delta_pct"] is None else f"{row['delta_pct']:+.2f}%"
        lines.append(f"    {row['name']:<40} {base:>12} {cur:>12} {pct:>10}")
    if fail_above_pct is None:
        lines.append("  RESULT: OK — delta report emitted (no --fail-above threshold set)")
        return 0, "\n".join(lines)
    if regressions:
        lines.append(
            f"  RESULT: FAIL — {len(regressions)} benchmark(s) regressed beyond "
            f"{fail_above_pct}%: {', '.join(regressions)}"
        )
        return 1, "\n".join(lines)
    lines.append(
        f"  RESULT: OK — no benchmark regressed beyond {fail_above_pct}%"
    )
    return 0, "\n".join(lines)


# ── Self-test ────────────────────────────────────────────────────────────────

SAMPLE_BENCH_OUTPUT = """\
test add_node ... bench:         1,234 ns/iter (+/- 50)
test shortest_path ... bench:           2.5 us/iter (+/- 0.1)
test call_graph_10k_lines_python ... bench:           3 ms/iter (+/- 0.2)
"""


def self_test(project_root: Path) -> tuple[int, str]:
    results: list[tuple[str, bool, str]] = []

    def record(name: str, ok: bool, detail: str = "") -> None:
        results.append((name, ok, detail))

    def safe(fn, *a, **kw) -> tuple[int, str]:
        try:
            return fn(*a, **kw)
        except Exception as exc:  # noqa: BLE001 — contract must fail cleanly
            return 1, f"ERROR: {type(exc).__name__}: {exc}"

    # Unit: bencher parsing (comma separators + unit normalization to us).
    parsed = parse_bencher_output(SAMPLE_BENCH_OUTPUT)
    parse_ok = (
        set(parsed) == {"add_node", "shortest_path", "call_graph_10k_lines_python"}
        and abs(parsed["add_node"] - 1.234) < 1e-9
        and abs(parsed["shortest_path"] - 2.5) < 1e-9
        and abs(parsed["call_graph_10k_lines_python"] - 3000.0) < 1e-9
    )
    record(
        "bencher_parse_normalizes_units",
        parse_ok,
        f"parsed={ {k: round(v, 3) for k, v in parsed.items()} }",
    )

    # Unit: delta math.
    with tempfile.TemporaryDirectory(prefix="lsi-bench-selftest-") as tmp:
        tmp_path = Path(tmp)
        baseline_path = tmp_path / BASELINE_FILE
        with open(baseline_path, "w") as f:
            json.dump(
                {
                    "schema_version": SCHEMA_VERSION,
                    "benchmarks": [
                        {"name": "add_node", "mean_us": 1.0, "unit": "us"},
                        {"name": "get_node", "mean_us": 4.0, "unit": "us"},
                    ],
                },
                f,
            )
        bench_file = tmp_path / "bench.txt"
        bench_file.write_text(
            "test add_node ... bench:           2 us/iter (+/- 0)\n"
            "test get_node ... bench:           1 us/iter (+/- 0)\n"
            "test new_bench ... bench:           1 us/iter (+/- 0)\n"
        )
        exit_c, report_c = safe(
            compare_to_baseline,
            project_root,
            baseline_path,
            bench_file,
            tmp_path / "delta.json",
            25.0,
        )
        deltas_ok = False
        if exit_c == 1 and "Traceback" not in report_c:
            try:
                delta = json.loads((tmp_path / "delta.json").read_text())
                rows = {r["name"]: r for r in delta["deltas"]}
                deltas_ok = (
                    abs(rows["add_node"]["delta_pct"] - 100.0) < 1e-9
                    and abs(rows["get_node"]["delta_pct"] + 75.0) < 1e-9
                    and rows["new_bench"]["baseline_mean_us"] is None
                    and delta["regressions"] == ["add_node"]
                )
            except (OSError, json.JSONDecodeError, KeyError, TypeError):
                deltas_ok = False
        record(
            "compare_emits_delta_report_and_flags_regression",
            exit_c == 1 and "Traceback" not in report_c and deltas_ok,
            f"exit={exit_c} deltas_ok={deltas_ok} regressed_beyond_25pct",
        )

        # Contract: compare WITHOUT baseline -> clean failure, no writes.
        empty_dir = tmp_path / "no-baseline"
        empty_dir.mkdir()
        exit_a, report_a = safe(
            compare_to_baseline,
            project_root,
            empty_dir / BASELINE_FILE,
            bench_file,
            empty_dir / "delta.json",
            None,
        )
        files_after = sorted(p.name for p in empty_dir.rglob("*") if p.is_file())
        clean = (
            exit_a != 0
            and "Traceback" not in report_a
            and "ERROR" not in report_a
            and "baseline" in report_a.lower()
        )
        record(
            "compare_without_baseline_fails_clean",
            clean and files_after == [],
            f"exit={exit_a} clean_report={clean} files_written={files_after}",
        )

        # Subprocess safety: bench argv is a fixed list, shell=False, in-repo.
        argv_ok = all(isinstance(part, str) for part in BENCH_ARGV) and not any(
            ch in part for part in BENCH_ARGV for ch in ("&&", ";", "|", "`")
        )
        record(
            "bench_argv_fixed_no_shell",
            argv_ok and BENCH_ARGV[0] == "cargo" and "--bench" in BENCH_ARGV,
            f"argv_ok={argv_ok}",
        )

        # Capture: writes a schema-complete baseline from a fixed output file.
        cap_path = tmp_path / "out" / BASELINE_FILE
        cap_path.parent.mkdir(parents=True, exist_ok=True)
        fixed_bench = tmp_path / "fixed.txt"
        fixed_bench.write_text(SAMPLE_BENCH_OUTPUT)
        exit_b, _ = safe(
            capture_baseline, project_root, cap_path, fixed_bench
        )
        schema_ok = False
        if exit_b == 0:
            with open(cap_path) as f:
                artifact = json.load(f)
            schema_ok = (
                artifact["schema_version"] == SCHEMA_VERSION
                and "commit" in artifact
                and set(artifact["env"]) == {"os", "rustc", "cargo", "commit"}
                and artifact["benchmarks"]
                and all(
                    set(b) == {"name", "mean_us", "unit"} and b["unit"] == "us"
                    for b in artifact["benchmarks"]
                )
            )
        record(
            "baseline_artifact_schema_complete",
            exit_b == 0 and schema_ok,
            f"exit={exit_b} schema_ok={schema_ok}",
        )

    failed = sum(1 for _, ok, _ in results if not ok)
    lines = ["LSI bench baseline self-test:"]
    for name, ok, detail in results:
        lines.append(f"  {'PASS' if ok else 'FAIL'}  {name}  {detail}")
    lines.append(f"  {len(results) - failed}/{len(results)} passed")
    return (1 if failed else 0), "\n".join(lines)


# ── Main ─────────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(
        description="LSI M0 benchmark baseline + delta compare (E36 WU-2)",
    )
    parser.add_argument(
        "mode",
        nargs="?",
        default="capture",
        choices=["capture", "compare"],
        help="capture: write baseline artifact; compare: delta report vs baseline",
    )
    parser.add_argument(
        "--baseline",
        default=None,
        help=f"baseline artifact path (default: {RESULTS_DIR}/{BASELINE_FILE})",
    )
    parser.add_argument(
        "--bench-output",
        default=None,
        help="parse an existing `cargo bench --output-format bencher` output "
        "file instead of running cargo bench",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="compare mode: also write the delta report JSON to this path",
    )
    parser.add_argument(
        "--fail-above",
        type=float,
        default=None,
        metavar="PCT",
        help="compare mode: exit 1 when any benchmark regresses beyond PCT%%",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run in-process checks (parser, delta math, contracts)",
    )
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parent.parent.parent
    baseline_path = (
        Path(args.baseline)
        if args.baseline
        else project_root / RESULTS_DIR / BASELINE_FILE
    )
    bench_file = Path(args.bench_output) if args.bench_output else None
    delta_output = Path(args.output) if args.output else None

    if args.self_test:
        code, report = self_test(project_root)
        print(report)
        return code

    try:
        if args.mode == "capture":
            code, report = capture_baseline(project_root, baseline_path, bench_file)
        else:
            code, report = compare_to_baseline(
                project_root, baseline_path, bench_file, delta_output, args.fail_above
            )
        print(report)
        return code
    except (RuntimeError, ValueError, OSError, subprocess.SubprocessError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
