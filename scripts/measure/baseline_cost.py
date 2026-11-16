#!/usr/bin/env python3
"""CAA-01: measure the real cost of cold/warm/incremental answers by phase.

This is an explicit measurement instrument, not a suite test. It generates
the deterministic corpus, drives the public CLI graph route over it, and
records what each stage cost — cache hit, walk, parse, ingest, resolution,
serialization, build, total — as p50/p95 plus peak child memory.

Disciplines the numbers obey:
  - Only a run whose CLI call succeeded and published a complete JSON answer
    counts as PASS. An Unknown, Failed, timeout, unparsable or partial answer
    is FAIL/BLOCKED, and it is never folded into a percentile or published as
    PASS: a partial graph never persists a durable snapshot, so a corpus that
    produces one measures rebuilds while claiming to measure warm reuse.
  - A warm run that did not serve from the durable snapshot is a FAIL, not a
    number: it measured a rebuild under the name "warm".
  - Test compilation and CI wait are separate rows. A row nobody ran says
    NOT_RUN with its reason; absence of a measurement is not zero and not
    green.
  - Every result carries its attribution: corpus digest, snapshot schema,
    binary version, analyzer commit — so a number can be traced to the
    snapshot and version that produced it.

Reuses scripts/generate_baseline_corpus.py; keeps XDG cache isolated per
sample exactly like scripts/measure/snapshot_incremental.py.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import generate_baseline_corpus as corpus  # noqa: E402

PHASES = [
    "cache_lookup_ms",
    "walk_ms",
    "parse_ms",
    "ingest_ms",
    "resolve_ms",
    "build_ms",
    "snapshot_serialize_ms",
]
CHANGE_CLASSES = list(corpus.CHANGE_CLASSES)
SCENARIOS = ["cold", "warm", "incremental"]


def percentile(values: list[float], p: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise ValueError("a percentile of nothing is not a measurement")
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * p
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    fraction = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def aggregate_status(statuses: list[str]) -> str:
    """The status of a set of samples: PASS only when every sample passed.

    A FAIL anywhere is a FAIL; a timeout anywhere is BLOCKED unless the set
    already failed; NOT_RUN is what an empty set means — nobody measured.
    """
    if not statuses:
        return "NOT_RUN"
    if "FAIL" in statuses:
        return "FAIL"
    if "BLOCKED" in statuses:
        return "BLOCKED"
    if all(status == "PASS" for status in statuses):
        return "PASS"
    return "NOT_RUN"


def _peak_rss_poll(stop: threading.Event, seen: list[int], pid: int) -> None:
    while not stop.is_set():
        try:
            with open(f"/proc/{pid}/status", encoding="utf-8") as status:
                for line in status:
                    if line.startswith("VmHWM:"):
                        kb = int(line.split()[1])
                        if kb > seen[0]:
                            seen[0] = kb
        except (OSError, ValueError):
            return
        stop.wait(0.02)


def baseline_honesty(answer_status: object, source: str, require_cache: bool) -> str | None:
    """Why this run may not be published as a baseline, or None if it may.

    The corpus is only a baseline while every answer is complete and every
    warm answer is served from the snapshot. Otherwise the numbers describe a
    different code path than the scenario names claim.
    """
    if answer_status != "complete":
        return (
            f"the answer says {answer_status!r}: a baseline needs a complete graph "
            "(partial graphs never persist a durable snapshot)"
        )
    if require_cache and source != "cache":
        return "a warm run rebuilt: the durable snapshot did not serve this query"
    return None


def run_graph(
    binary: Path, workspace: Path, cache: Path, timeout: float, require_cache: bool = False
) -> dict[str, object]:
    env = os.environ.copy()
    env["XDG_CACHE_HOME"] = str(cache)
    command = [str(binary), "--format", "json", "graph", "full", str(workspace)]
    started = time.perf_counter()
    try:
        popen = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
    except FileNotFoundError as error:
        return {"status": "FAIL", "reason": f"the binary cannot start: {error}"}
    except OSError as error:
        return {"status": "BLOCKED", "reason": f"the run could not be launched: {error}"}
    seen = [0]
    stop = threading.Event()
    poller = threading.Thread(target=_peak_rss_poll, args=(stop, seen, popen.pid), daemon=True)
    poller.start()
    try:
        stdout, stderr = popen.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        popen.kill()
        popen.communicate()
        stop.set()
        return {"status": "BLOCKED", "reason": f"timeout after {timeout}s"}
    stop.set()
    poller.join(timeout=1.0)
    elapsed_ms = (time.perf_counter() - started) * 1000
    if popen.returncode != 0:
        return {
            "status": "FAIL",
            "returncode": popen.returncode,
            "elapsed_ms": round(elapsed_ms, 3),
            "stderr": stderr[-2000:],
        }
    try:
        document = json.loads(stdout)
    except json.JSONDecodeError as error:
        return {"status": "FAIL", "reason": f"stdout is not the published JSON: {error}"}
    timings = document.get("timings_ms") or {}
    row: dict[str, object] = {"status": "PASS", "elapsed_ms": round(elapsed_ms, 3)}
    for phase in PHASES:
        if isinstance(timings.get(phase), (int, float)):
            row[phase] = float(timings[phase])
    row["source"] = "cache" if "Graph loaded from" in stderr else "built"
    row["peak_rss_kb"] = seen[0]
    row["answer_status"] = document.get("status")
    problem = baseline_honesty(document.get("status"), str(row["source"]), require_cache)
    if problem:
        return {
            "status": "FAIL",
            "reason": problem,
            "elapsed_ms": round(elapsed_ms, 3),
            "peak_rss_kb": seen[0],
            "answer_status": document.get("status"),
        }
    return row


def attribution(binary: Path) -> dict[str, object]:
    info: dict[str, object] = {"binary": str(binary)}
    try:
        version = subprocess.run(
            [str(binary), "--version"], capture_output=True, text=True, timeout=30, check=False
        )
        info["binary_version"] = version.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        info["binary_version"] = "unknown"
    schema = "unknown"
    try:
        source = (REPO_ROOT / "crates/cognicode-core/src/domain/value_objects/snapshot_key.rs").read_text(
            encoding="utf-8"
        )
        match = re.search(r'SNAPSHOT_SCHEMA[^=]*=\s*"([^"]+)"', source)
        if match:
            schema = match.group(1)
    except OSError:
        pass
    info["snapshot_schema"] = schema
    try:
        commit = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        info["analyzer_commit"] = commit.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        info["analyzer_commit"] = "unknown"
    info["measured_at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    return info


def unmeasured_gate_rows() -> dict[str, dict[str, str]]:
    """The gate rows this instrument distinguishes but does not run.

    Test compilation and CI wait are phases a baseline has to tell apart
    from the analysis stages. This instrument does not measure them, and
    it says so: NOT_RUN is neither zero nor green.
    """
    return {
        "test_compilation": {
            "status": "NOT_RUN",
            "reason": "not measured by this instrument; scripts/build-time.sh owns test-compile cost",
        },
        "ci_wait": {
            "status": "NOT_RUN",
            "reason": "no CI lane exists; the local authority is scripts/test-gate.sh",
        },
    }


def measure(binary: Path, samples: int, timeout: float, modules: int) -> dict[str, object]:
    rows: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="caa01-baseline-") as root:
        base = Path(root)
        for change_class in CHANGE_CLASSES:
            for scenario in SCENARIOS:
                for sample in range(1, samples + 1):
                    workspace = base / f"{change_class}-{scenario}-{sample}" / "workspace"
                    cache = workspace.parent / "cache"
                    cache.mkdir(parents=True)
                    corpus.write_corpus(workspace, modules)
                    if scenario == "cold":
                        result = run_graph(binary, workspace, cache, timeout)
                    elif scenario == "warm":
                        run_graph(binary, workspace, cache, timeout)  # build the snapshot
                        result = run_graph(
                            binary, workspace, cache, timeout, require_cache=True
                        )  # serve it, or fail honestly
                    else:
                        run_graph(binary, workspace, cache, timeout)  # snapshot of the base tree
                        corpus.apply_change(workspace, change_class, modules)
                        result = run_graph(binary, workspace, cache, timeout)
                    rows.append(
                        {
                            "change_class": change_class,
                            "scenario": scenario,
                            "sample": sample,
                            **result,
                        }
                    )

    pass_rows = [row for row in rows if row.get("status") == "PASS"]

    def summarize(change_class: str, scenario: str) -> dict[str, object]:
        selected = [
            row
            for row in pass_rows
            if row["change_class"] == change_class and row["scenario"] == scenario
        ]
        statuses = [str(row["status"]) for row in rows
                    if row["change_class"] == change_class and row["scenario"] == scenario]
        summary: dict[str, object] = {
            "status": aggregate_status(statuses),
            "n": len(selected),
            "wall": {
                "p50_ms": round(percentile([float(r["elapsed_ms"]) for r in selected], 0.50), 3),
                "p95_ms": round(percentile([float(r["elapsed_ms"]) for r in selected], 0.95), 3),
            }
            if selected
            else None,
            "peak_rss": {
                "p50_kb": round(percentile([float(r["peak_rss_kb"]) for r in selected], 0.50), 1),
                "max_kb": max(float(r["peak_rss_kb"]) for r in selected),
            }
            if selected
            else None,
            "phases": {},
        }
        for phase in PHASES:
            values = [float(row[phase]) for row in selected if phase in row]
            if values:
                summary["phases"][phase] = {
                    "n": len(values),
                    "p50_ms": round(percentile(values, 0.50), 3),
                    "p95_ms": round(percentile(values, 0.95), 3),
                }
            else:
                summary["phases"][phase] = {"n": 0, "status": "NOT_RUN"}
        return summary

    baseline = {
        change_class: summarize(change_class, "cold") for change_class in CHANGE_CLASSES
    }

    summary = {
        "tool": "baseline_cost",
        "version": 1,
        "attribution": attribution(binary),
        "corpus": {
            "modules": modules,
            "digest": corpus.corpus_digest(corpus.corpus_files(modules)),
        },
        "gate": unmeasured_gate_rows(),
        "rows": rows,
        "results": {
            f"{change_class}.{scenario}": summarize(change_class, scenario)
            for change_class in CHANGE_CLASSES
            for scenario in SCENARIOS
        },
        "baseline": baseline,
    }
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="built cognicode analysis CLI")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--modules", type=int, default=12)
    parser.add_argument("--out", type=Path, help="write the JSON evidence to this file too")
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    if not args.binary.is_file():
        parser.error(f"binary does not exist: {args.binary}")

    evidence = measure(args.binary.resolve(), args.samples, args.timeout, args.modules)
    text = json.dumps(evidence, indent=2, sort_keys=True)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text + "\n", encoding="utf-8")
    print(text)
    results = list(evidence["results"].values())
    return 0 if results and all(row["status"] == "PASS" for row in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
