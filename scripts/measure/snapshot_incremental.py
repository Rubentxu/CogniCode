#!/usr/bin/env python3
"""Measure snapshot cold/warm/edit behavior on the real CLI graph route.

This is an explicit measurement instrument, not a suite test. It keeps the
workspace and XDG cache isolated, then records the public graph source reported
by the CLI. ``built`` means the route rebuilt the graph; ``cache`` means it
reused the complete durable snapshot. No result is labeled incremental delta:
the current route has no delta source.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

BASE = b"pub fn entry() { helper(); }\npub fn helper() {}\n"
EDITED = b"pub fn entry() { caller(); }\npub fn caller() {}\n"


def percentile(values: list[float], p: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * p
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    fraction = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def run_graph(binary: Path, workspace: Path, cache: Path, timeout: float) -> dict[str, object]:
    env = os.environ.copy()
    env["XDG_CACHE_HOME"] = str(cache)
    command = [str(binary), "graph", "full", "--format", "json", str(workspace)]
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            env=env,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {"status": "BLOCKED", "reason": f"timeout after {timeout}s", "stderr": str(error)}
    elapsed_ms = (time.perf_counter() - started) * 1000
    if completed.returncode != 0:
        return {
            "status": "FAIL",
            "returncode": completed.returncode,
            "elapsed_ms": round(elapsed_ms, 3),
            "stderr": completed.stderr[-2000:],
        }
    source = "cache" if "Graph loaded from" in completed.stderr else "built"
    return {
        "status": "PASS",
        "source": source,
        "classification": "warm reuse" if source == "cache" else "full rebuild",
        "elapsed_ms": round(elapsed_ms, 3),
    }


def run_scanner(workspace: Path, timeout: float) -> dict[str, object]:
    repo_root = Path(__file__).resolve().parents[2]
    command = [
        "cargo",
        "run",
        "--quiet",
        "-p",
        "cognicode-core",
        "--example",
        "measure_incremental_scanner",
        "--",
        str(workspace),
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            cwd=repo_root,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {"status": "BLOCKED", "reason": f"timeout after {timeout}s", "stderr": str(error)}
    if completed.returncode != 0:
        return {
            "status": "FAIL",
            "returncode": completed.returncode,
            "stderr": completed.stderr[-2000:],
        }
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        return {
            "status": "BLOCKED",
            "reason": f"scanner output is not JSON: {error}",
            "stdout": completed.stdout[-2000:],
            "stderr": completed.stderr[-2000:],
        }
    return {"status": "PASS", "evidence": result}


    rows: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="cognicode-h5-") as root:
        root_path = Path(root)
        workspace = root_path / "workspace"
        source = workspace / "src" / "lib.rs"
        cache = root_path / "cache"
        scanner_workspace = root_path / "scanner"
        source.parent.mkdir(parents=True)
        workspace.joinpath("Cargo.toml").write_text(
            "[package]\nname = \"h5_fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
            encoding="utf-8",
        )
        for sample in range(1, samples + 1):
            shutil.rmtree(cache, ignore_errors=True)
            cache.mkdir()
            source.write_bytes(BASE)
            original_stat = source.stat()
            for phase in ("cold", "warm"):
                result = run_graph(binary, workspace, cache, timeout)
                rows.append({"sample": sample, "phase": phase, **result})
            source.write_bytes(EDITED)
            os.utime(source, ns=(original_stat.st_atime_ns, original_stat.st_mtime_ns))
            result = run_graph(binary, workspace, cache, timeout)
            rows.append({"sample": sample, "phase": "edit", **result})

        scanner = run_scanner(scanner_workspace, timeout)

    pass_rows = [row for row in rows if row.get("status") == "PASS"]
    summary: dict[str, object] = {
        "samples": samples,
        "rows": rows,
        "graph_semantics": {
            "status": "NOT_RUN",
            "instrument_observation": "NOT_RUN: CLI route emits no graph JSON on stdout",
        },
        "scanner": scanner,
    }
    for phase in ("cold", "warm", "edit"):
        values = [
            float(row["elapsed_ms"])
            for row in pass_rows
            if row["phase"] == phase
        ]
        sources = sorted({str(row["source"]) for row in pass_rows if row["phase"] == phase})
        summary[phase] = {
            "n": len(values),
            "p50_ms": round(percentile(values, 0.50), 3) if values else None,
            "p95_ms": round(percentile(values, 0.95), 3) if values else None,
            "sources": sources,
            "delta": False,
        }
    summary["delta_evidence"] = "NOT_RUN: load_or_build_graph exposes built/cache, not IncrementalScanner delta"
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="built cognicode analysis CLI")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=600.0)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    if not args.binary.is_file():
        parser.error(f"binary does not exist: {args.binary}")
    print(json.dumps(measure(args.binary.resolve(), args.samples, args.timeout), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
