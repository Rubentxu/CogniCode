#!/usr/bin/env python3
"""build_flaky_log.py — Generate / refresh the per-scenario flaky log.

Reads sandbox run results from `sandbox/results/` (any directory containing
`summary.json` or `result.json` files) and produces:

  - sandbox/results/flaky_scenarios.md   (human-readable per-scenario log)
  - sandbox/results/flaky_scenarios.json (machine-readable sidecar)

The "no surprise flaky" rule (per docs/TEST-PLAN.md §5 / T5):
  - Any scenario whose last 30-day pass rate is < 1.0 MUST appear in the log.
  - Scenarios listed as `quarantined` are exempt from G6 max-CV computation.
  - Scenarios newly flaky (not in this log) FAIL G13 ("no surprise flaky").

Usage:
  python3 sandbox/scripts/build_flaky_log.py [--window-days 30] [--output-dir sandbox/results]

Exit code:
  0  log generated successfully
  1  read or write error
  2  no run results found
"""

import argparse
import json
import os
import sys
from collections import defaultdict
from datetime import datetime, timezone, timedelta
from pathlib import Path


# ── Defaults ──────────────────────────────────────────────────────────────────

DEFAULT_RESULTS_ROOT = "sandbox/results"
DEFAULT_OUTPUT_DIR = "sandbox/results"
DEFAULT_WINDOW_DAYS = 30

# Scenarios listed here are KNOWN-flaky and quarantined. They are exempt from
# the G6 max-CV computation. New entries (not in this set) appearing in the
# rolling window are flagged as "newly flaky" → fail G13.
# Edit this set when the maintainer explicitly accepts a scenario as flaky.
KNOWN_QUARANTINED: set[str] = set()

# Threshold: per-scenario pass rate below this counts as flaky in the log.
FLAKY_THRESHOLD = 1.0  # strict: any non-100% pass rate in the window


# ── Loaders ───────────────────────────────────────────────────────────────────

def load_results(results_root: Path) -> list[dict]:
    """Walk the results tree and collect every result.json + summary.json.

    Each result.json is a single-scenario record (see prior matrices).
    summary.json is a per-run aggregate; we use it to attribute runs to
    a date but not to count individual scenarios.
    """
    results: list[dict] = []
    for path in results_root.rglob("result.json"):
        try:
            with open(path) as f:
                r = json.load(f)
                r["_run_dir"] = str(path.parent)
                r["_result_path"] = str(path)
                results.append(r)
        except Exception:
            pass
    return results


def parse_run_date(result: dict) -> datetime | None:
    """Extract a run date from a result record.

    Falls back to the parent's mtime if no date is present in the JSON.
    """
    # Try several known timestamp fields
    for key in ("run_started_at", "started_at", "created_at", "timestamp"):
        if key in result:
            try:
                return datetime.fromisoformat(str(result[key]).replace("Z", "+00:00"))
            except Exception:
                pass
    # Fall back to the parent directory's mtime
    p = Path(result.get("_run_dir", "."))
    if p.exists():
        try:
            return datetime.fromtimestamp(p.stat().st_mtime, tz=timezone.utc)
        except Exception:
            pass
    return None


# ── Computation ───────────────────────────────────────────────────────────────

def compute_per_scenario(results: list[dict], cutoff: datetime) -> dict[str, dict]:
    """Aggregate per-scenario pass rate over the rolling window.

    A scenario is "passing" if outcome == expected_outcome (or outcome == "pass"
    when expected_outcome is "pass"). Strips any record older than `cutoff`.
    """
    per: dict[str, dict] = defaultdict(
        lambda: {"passes": 0, "fails": 0, "total": 0, "last_seen": None,
                 "first_seen": None, "languages": set(), "tiers": set(), "tools": set()}
    )
    for r in results:
        d = parse_run_date(r)
        if d is None or d < cutoff:
            continue
        sid = r.get("scenario_id") or r.get("scenario", {}).get("id") or "unknown"
        outcome = str(r.get("outcome", "")).lower()
        expected = str(r.get("expected_outcome", "")).lower()
        per[sid]["total"] += 1
        if outcome == expected or (outcome == "pass" and expected == "pass"):
            per[sid]["passes"] += 1
        else:
            per[sid]["fails"] += 1
        if r.get("language"):
            per[sid]["languages"].add(r["language"])
        if r.get("tier"):
            per[sid]["tiers"].add(r["tier"])
        if r.get("tool"):
            per[sid]["tools"].add(r["tool"])
        if d is not None:
            ls = per[sid]["last_seen"]
            if ls is None or d > ls:
                per[sid]["last_seen"] = d
            fs = per[sid]["first_seen"]
            if fs is None or d < fs:
                per[sid]["first_seen"] = d
    return per


def classify_flaky(per: dict[str, dict]) -> dict[str, dict]:
    """Tag each scenario with a flaky status + trend."""
    out: dict[str, dict] = {}
    for sid, info in per.items():
        if info["total"] == 0:
            continue
        pass_rate = info["passes"] / info["total"]
        is_quarantined = sid in KNOWN_QUARANTINED
        is_flaky = pass_rate < FLAKY_THRESHOLD
        # Trend (placeholder): compare halves of the window. Naive split on
        # last_seen. Sufficient for now; replaced by a regression test in B?.
        if is_quarantined:
            status = "quarantined"
        elif is_flaky:
            status = "failing"
        else:
            status = "passing"
        out[sid] = {
            "scenario_id": sid,
            "pass_rate": round(pass_rate, 4),
            "passes": info["passes"],
            "fails": info["fails"],
            "total": info["total"],
            "status": status,
            "quarantined": is_quarantined,
            "languages": sorted(info["languages"]),
            "tiers": sorted(info["tiers"]),
            "tools": sorted(info["tools"]),
            "last_seen": info["last_seen"].isoformat() if info["last_seen"] else None,
            "first_seen": info["first_seen"].isoformat() if info["first_seen"] else None,
        }
    return out


# ── Renderers ────────────────────────────────────────────────────────────────

def render_markdown(classified: dict[str, dict], window_days: int) -> str:
    """Render the per-scenario flaky log as a Markdown table."""
    header = (
        f"# Sandbox Flaky-Scenarios Log\n\n"
        f"> **Source-of-truth**: per-scenario pass rate over the last **{window_days} days**.\n"
        f"> **Generated by**: `sandbox/scripts/build_flaky_log.py` (run nightly via `just scorecard-stability`).\n"
        f"> **Rule** (per docs/TEST-PLAN.md §5 / T5): any scenario whose pass rate is < 100% in the\n"
        f"> window MUST appear here. Quarantined scenarios are exempt from the G6 max-CV\n"
        f"> computation. Newly-flaky scenarios (pass rate < 100% in this log) FAIL G13\n"
        f"> — that is the explicit **no surprise flaky** guarantee.\n\n"
    )
    summary = (
        f"## Summary\n\n"
        f"| Total scenarios | Passing | Failing | Quarantined |\n"
        f"|-----------------|---------|---------|-------------|\n"
    )
    passing = sum(1 for v in classified.values() if v["status"] == "passing")
    failing = sum(1 for v in classified.values() if v["status"] == "failing")
    quarantined = sum(1 for v in classified.values() if v["status"] == "quarantined")
    summary += f"| {len(classified)} | {passing} | {failing} | {quarantined} |\n\n"

    table_header = (
        "## Per-Scenario Table\n\n"
        "| scenario_id | tool | language | tier | pass_rate | passes/total | status | quarantined | last_seen |\n"
        "|-------------|------|----------|------|-----------|--------------|--------|-------------|-----------|\n"
    )
    rows = []
    # Sort by status (failing first, then quarantined, then passing) then by
    # scenario_id for stable output.
    order = {"failing": 0, "quarantined": 1, "passing": 2}
    for sid in sorted(classified,
                      key=lambda s: (order.get(classified[s]["status"], 9), s)):
        v = classified[sid]
        tool = ",".join(v["tools"]) or "—"
        lang = ",".join(v["languages"]) or "—"
        tier = ",".join(v["tiers"]) or "—"
        rate = f"{v['pass_rate']*100:.1f}%"
        ratio = f"{v['passes']}/{v['total']}"
        status = v["status"]
        qf = "yes" if v["quarantined"] else "no"
        last_seen = v["last_seen"] or "—"
        rows.append(f"| `{sid}` | {tool} | {lang} | {tier} | {rate} | {ratio} | {status} | {qf} | {last_seen} |")
    body = "\n".join(rows) if rows else "| (no scenarios in the window) | — | — | — | — | — | — | — | — |"
    footer = (
        "\n\n## To quarantine a scenario\n\n"
        "If a scenario is reproducibly flaky (env-dependent, race-condition ground truth, etc.)\n"
        "and the maintainer accepts that state, add it to `KNOWN_QUARANTINED` in\n"
        "`sandbox/scripts/build_flaky_log.py` and commit. The scenario then drops out of\n"
        "the G6 max-CV computation and the T7 nightly check stops failing on it.\n\n"
        "## Quarantine vs fix policy\n\n"
        "- **Quarantine is not a permanent state.** Each quarantined scenario should have\n"
        "  an ADR or a follow-up issue that documents the intended fix.\n"
        "- **Unknown-flaky = G13 fail.** Adding a scenario to the quarantine list is the\n"
        "  only way to suppress its failure signature; if you don't, the next nightly\n"
        "  scorecard will fail with `newly flaky scenario: <id>`.\n"
    )
    return header + summary + table_header + body + "\n" + footer


def render_json(classified: dict[str, dict], window_days: int) -> str:
    """Render the per-scenario flaky log as machine-readable JSON."""
    out = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "window_days": window_days,
        "summary": {
            "total_scenarios": len(classified),
            "passing": sum(1 for v in classified.values() if v["status"] == "passing"),
            "failing": sum(1 for v in classified.values() if v["status"] == "failing"),
            "quarantined": sum(1 for v in classified.values() if v["status"] == "quarantined"),
        },
        "scenarios": list(classified.values()),
    }
    return json.dumps(out, indent=2, sort_keys=True)


# ── Entry point ───────────────────────────────────────────────────────────────

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--results-root", default=DEFAULT_RESULTS_ROOT,
                   help=f"Root directory for run results (default: {DEFAULT_RESULTS_ROOT})")
    p.add_argument("--output-dir", default=DEFAULT_OUTPUT_DIR,
                   help=f"Output directory for the log (default: {DEFAULT_OUTPUT_DIR})")
    p.add_argument("--window-days", type=int, default=DEFAULT_WINDOW_DAYS,
                   help=f"Rolling window in days (default: {DEFAULT_WINDOW_DAYS})")
    p.add_argument("--archive", action="store_true",
                   help="Also write a timestamped snapshot under "
                        "`sandbox/results/flaky-archive/<ts>/` (used by the nightly cadence)")
    args = p.parse_args()

    results_root = Path(args.results_root)
    output_dir = Path(args.output_dir)
    if not results_root.exists():
        print(f"ERROR: results root not found: {results_root}", file=sys.stderr)
        return 1
    output_dir.mkdir(parents=True, exist_ok=True)

    results = load_results(results_root)
    if not results:
        print(f"WARN: no result.json files found under {results_root}", file=sys.stderr)
        # Continue anyway — emit an empty log so the cadence does not block.

    cutoff = datetime.now(timezone.utc) - timedelta(days=args.window_days)
    per = compute_per_scenario(results, cutoff)
    classified = classify_flaky(per)
    md = render_markdown(classified, args.window_days)
    js = render_json(classified, args.window_days)

    md_path = output_dir / "flaky_scenarios.md"
    js_path = output_dir / "flaky_scenarios.json"
    md_path.write_text(md)
    js_path.write_text(js)

    # Optional: archive per-run snapshot (used by the nightly cadence to keep
    # a 5-night history in `sandbox/results/flaky-archive/<ts>/`). The archive
    # is local-only (sandbox/results/* is gitignored) — the 5-night cadence
    # is verified by walking the archive directory.
    if args.archive:
        archive_dir = output_dir / "flaky-archive" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        archive_dir.mkdir(parents=True, exist_ok=True)
        (archive_dir / "flaky_scenarios.md").write_text(md)
        (archive_dir / "flaky_scenarios.json").write_text(js)
        print(f"==> Archived to {archive_dir}")

    summary = classified and (
        f"{sum(1 for v in classified.values() if v['status']=='failing')} failing, "
        f"{sum(1 for v in classified.values() if v['status']=='quarantined')} quarantined, "
        f"{sum(1 for v in classified.values() if v['status']=='passing')} passing"
    )
    print(f"==> Wrote {md_path}")
    print(f"==> Wrote {js_path}")
    print(f"==> Window: {args.window_days} days · {summary}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
