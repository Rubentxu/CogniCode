#!/usr/bin/env python3
"""
generate_e2e_report.py — Explorer E2E HTML report generator.

Reads Playwright JSON reporter output and produces a self-contained HTML report
with KPI cards, per-file/per-spec breakdown, and (if present) stability data.

Usage:
    python3 apps/explorer-ui/scripts/generate_e2e_report.py [--results-dir <dir>] [--output <file>]

Defaults:
    --results-dir  — latest directory under apps/explorer-ui/e2e-runs/
    --output       — <results-dir>/report.html
"""

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# Data loading
# ---------------------------------------------------------------------------

def latest_run_dir(e2e_runs: Path) -> Path | None:
    """Return the most recent run directory by modification time."""
    if not e2e_runs.exists():
        return None
    dirs = sorted((p for p in e2e_runs.iterdir() if p.is_dir()), key=lambda p: p.stat().st_mtime)
    return dirs[-1] if dirs else None


def load_run(run_dir: Path, repeat: int = 1) -> dict[str, Any]:
    """Load the JSON reporter output for a specific repeat (default 1)."""
    path = run_dir / f"run-{repeat}.json"
    if not path.exists():
        raise FileNotFoundError(f"Run file not found: {path}")
    with open(path) as f:
        return json.load(f)


def load_summary(run_dir: Path) -> dict[str, Any] | None:
    path = run_dir / "summary.json"
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


def load_stability(run_dir: Path) -> dict[str, Any] | None:
    path = run_dir / "stability.json"
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


# ---------------------------------------------------------------------------
# Data extraction helpers
# ---------------------------------------------------------------------------

def walk_suites(suites: list[dict], depth: int = 0) -> list[dict]:
    """
    Flatten the Playwright JSON suite tree into a list of (file, spec, test)
    triples with relevant metadata.
    """
    rows = []
    for suite in suites:
        rows.extend(walk_suites(suite.get("suites", []) or [], depth + 1))
        for spec in suite.get("specs", []) or []:
            for test in spec.get("tests", []) or []:
                rows.append({
                    "file": suite.get("title", "") or spec.get("file", ""),
                    "spec": spec.get("title", ""),
                    "title": test.get("title", ""),
                    "status": test.get("status", ""),
                    "duration": test.get("duration", 0.0),
                    "errors": spec.get("errors", []),
                })
    return rows


def group_by_file(rows: list[dict]) -> dict[str, list[dict]]:
    groups: dict[str, list[dict]] = {}
    for row in rows:
        groups.setdefault(row["file"], []).append(row)
    return groups


def pct(n: float, d: float) -> str:
    if d == 0:
        return "—"
    return f"{n / d * 100:.1f}%"


# ---------------------------------------------------------------------------
# HTML fragments
# ---------------------------------------------------------------------------

TAILWIND_CDN = (
    '<script src="https://cdn.tailwindcss.io"></script>'
)


def kpi_cards(stats: dict[str, Any]) -> str:
    expected = stats.get("expected", 0)
    unexpected = stats.get("unexpected", 0)
    skipped = stats.get("skipped", 0)
    flaky = stats.get("flaky", 0)
    duration_ms = stats.get("duration", 0.0)
    total = expected + unexpected + skipped
    pass_rate = pct(expected, total)

    status_color = "bg-green-100 text-green-800" if unexpected == 0 else "bg-red-100 text-red-800"

    return f"""
  <div class="grid grid-cols-2 md:grid-cols-5 gap-4 mb-8">
    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
      <div class="text-2xl font-bold {status_color}">{pass_rate}</div>
      <div class="text-xs text-gray-500 mt-1">Pass Rate</div>
    </div>
    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
      <div class="text-2xl font-bold text-gray-900">{total}</div>
      <div class="text-xs text-gray-500 mt-1">Total Tests</div>
    </div>
    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
      <div class="text-2xl font-bold text-green-700">{expected}</div>
      <div class="text-xs text-gray-500 mt-1">Passed</div>
    </div>
    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
      <div class="text-2xl font-bold text-red-700">{unexpected}</div>
      <div class="text-xs text-gray-500 mt-1">Failed</div>
    </div>
    <div class="bg-white rounded-xl shadow-sm border border-gray-200 p-4">
      <div class="text-2xl font-bold text-gray-700">{duration_ms / 1000:.1f}s</div>
      <div class="text-xs text-gray-500 mt-1">Duration</div>
    </div>
  </div>"""


def status_badge(status: str) -> str:
    colors = {
        "expected": "bg-green-100 text-green-800",
        "unexpected": "bg-red-100 text-red-800",
        "skipped": "bg-gray-100 text-gray-500",
        "flaky": "bg-yellow-100 text-yellow-800",
    }
    color = colors.get(status, "bg-gray-100 text-gray-600")
    labels = {
        "expected": "PASS",
        "unexpected": "FAIL",
        "skipped": "SKIP",
        "flaky": "FLAKY",
    }
    label = labels.get(status, status.upper())
    return f'<span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium {color}">{label}</span>'


def per_file_table(rows: list[dict]) -> str:
    by_file = group_by_file(rows)
    rows_html = ""
    for fname, specs in sorted(by_file.items()):
        passed = sum(1 for s in specs if s["status"] == "expected")
        failed = sum(1 for s in specs if s["status"] == "unexpected")
        total = len(specs)
        duration = sum(s["duration"] for s in specs)
        rows_html += f"""
        <tr class="border-b border-gray-100 hover:bg-gray-50">
          <td class="px-4 py-2 font-medium text-gray-900 text-sm">{fname}</td>
          <td class="px-4 py-2 text-center text-sm">{total}</td>
          <td class="px-4 py-2 text-center text-sm text-green-700">{passed}</td>
          <td class="px-4 py-2 text-center text-sm text-red-700">{failed}</td>
          <td class="px-4 py-2 text-center text-sm text-gray-500">{duration / 1000:.1f}s</td>
        </tr>"""

    return f"""
  <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden mb-8">
    <div class="px-4 py-3 border-b border-gray-200 bg-gray-50">
      <h2 class="text-sm font-semibold text-gray-700">Per-File Breakdown</h2>
    </div>
    <table class="min-w-full text-sm">
      <thead>
        <tr class="text-left text-xs text-gray-500 uppercase tracking-wider bg-gray-50">
          <th class="px-4 py-2">File</th>
          <th class="px-4 py-2 text-center">Tests</th>
          <th class="px-4 py-2 text-center">Passed</th>
          <th class="px-4 py-2 text-center">Failed</th>
          <th class="px-4 py-2 text-center">Duration</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-gray-100">{rows_html}
      </tbody>
    </table>
  </div>"""


def per_spec_table(rows: list[dict]) -> str:
    rows_html = ""
    for row in rows:
        duration_ms = row["duration"]
        rows_html += f"""
        <tr class="border-b border-gray-100 hover:bg-gray-50">
          <td class="px-4 py-2">
            <div class="font-medium text-gray-800 text-sm">{row["spec"]}</div>
            <div class="text-xs text-gray-400">{row["title"]}</div>
          </td>
          <td class="px-4 py-2">{status_badge(row["status"])}</td>
          <td class="px-4 py-2 text-right text-sm text-gray-500">{duration_ms:.0f}ms</td>
        </tr>"""

    return f"""
  <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden mb-8">
    <div class="px-4 py-3 border-b border-gray-200 bg-gray-50">
      <h2 class="text-sm font-semibold text-gray-700">Per-Spec Results</h2>
    </div>
    <table class="min-w-full text-sm">
      <thead>
        <tr class="text-left text-xs text-gray-500 uppercase tracking-wider bg-gray-50">
          <th class="px-4 py-2">Test</th>
          <th class="px-4 py-2">Status</th>
          <th class="px-4 py-2 text-right">Duration</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-gray-100">{rows_html}
      </tbody>
    </table>
  </div>"""


def stability_section(stability: dict[str, Any]) -> str:
    flaky = stability.get("flaky", [])
    if not flaky:
        return ""

    rows_html = ""
    for entry in flaky:
        pass_rate_pct = entry["passRate"] * 100
        rows_html += f"""
        <tr class="border-b border-gray-100 hover:bg-yellow-50">
          <td class="px-4 py-2">
            <div class="font-medium text-gray-800 text-sm">{entry["spec"]}</div>
            <div class="text-xs text-gray-400">{entry["title"]}</div>
          </td>
          <td class="px-4 py-2 text-center">
            <div class="text-sm font-bold text-yellow-700">{pass_rate_pct:.0f}%</div>
            <div class="text-xs text-gray-400">{entry["passCount"]}/{entry["total"]}</div>
          </td>
          <td class="px-4 py-2 text-right text-sm text-gray-500">
            {entry["meanDurationMs"]:.0f}ms avg
          </td>
        </tr>"""

    return f"""
  <div class="bg-yellow-50 rounded-xl shadow-sm border border-yellow-200 overflow-hidden mb-8">
    <div class="px-4 py-3 border-b border-yellow-200 bg-yellow-100">
      <h2 class="text-sm font-semibold text-yellow-900">
        ⚠️ Flaky Tests ({len(flaky)} of {stability.get("totalTests", 0)})
      </h2>
      <p class="text-xs text-yellow-700 mt-0.5">
        Tests with pass rate &lt; 100% across {stability.get("repeat", "?")} repeat runs.
      </p>
    </div>
    <table class="min-w-full text-sm">
      <thead>
        <tr class="text-left text-xs text-yellow-700 uppercase tracking-wider bg-yellow-50">
          <th class="px-4 py-2">Test</th>
          <th class="px-4 py-2 text-center">Pass Rate</th>
          <th class="px-4 py-2 text-right">Avg Duration</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-yellow-100">{rows_html}
      </tbody>
    </table>
  </div>"""


# ---------------------------------------------------------------------------
# Main HTML document
# ---------------------------------------------------------------------------

def build_report(
    stats: dict[str, Any],
    rows: list[dict],
    stability: dict[str, Any] | None,
    run_id: str,
    repeat: int,
) -> str:
    summary_meta = (
        f'<p class="text-sm text-gray-500">'
        f'Run ID: <code class="font-mono bg-gray-100 px-1 rounded">{run_id}</code> &nbsp;·&nbsp; '
        f'Repeats: {repeat}'
        f'</p>'
    )

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Explorer E2E Report — {run_id}</title>
  {TAILWIND_CDN}
  <style>
    body {{ font-family: ui-sans-serif, system-ui, -apple-system, sans-serif; }}
  </style>
</head>
<body class="bg-gray-50 min-h-screen pb-12">
  <div class="max-w-5xl mx-auto px-4 py-8">

    <header class="mb-8">
      <h1 class="text-2xl font-bold text-gray-900">🎭 Explorer E2E Report</h1>
      {summary_meta}
    </header>

    {kpi_cards(stats)}
    {stability_section(stability) if stability else ""}
    {per_file_table(rows)}
    {per_spec_table(rows)}

  </div>
</body>
</html>"""


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="Generate Explorer E2E HTML report.")
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=None,
        help="Run directory (default: latest under apps/explorer-ui/e2e-runs/)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output HTML path (default: <results-dir>/report.html)",
    )
    args = parser.parse_args()

    # Resolve results directory
    e2e_runs = Path(__file__).parent.parent / "e2e-runs"
    if args.results_dir:
        run_dir = args.results_dir
    else:
        run_dir = latest_run_dir(e2e_runs)
        if run_dir is None:
            print(f"ERROR: No run directories found under {e2e_runs}", file=sys.stderr)
            sys.exit(1)
        print(f"Using latest run: {run_dir}")

    # Load data
    summary = load_summary(run_dir)
    stability = load_stability(run_dir)

    # Load run-1.json (primary run for pass/fail breakdown)
    run_data = load_run(run_dir, repeat=1)
    stats = run_data.get("stats", {})
    rows = walk_suites(run_data.get("suites", []) or [])

    run_id = summary.get("runId", run_dir.name) if summary else run_dir.name
    repeat = summary.get("repeat", 1) if summary else 1

    # Generate output path
    output_path = args.output or (run_dir / "report.html")
    html = build_report(stats, rows, stability, run_id, repeat)
    with open(output_path, "w") as f:
        f.write(html)

    print(f"✅ Report written to: {output_path}")
    print(f"   stats: {stats}")
    if stability:
        print(f"   stability: {len(stability.get('flaky', []))} flaky / {stability.get('totalTests', 0)} total")


if __name__ == "__main__":
    main()
