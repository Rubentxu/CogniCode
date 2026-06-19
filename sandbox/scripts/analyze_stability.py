#!/usr/bin/env python3
"""analyze_stability.py — Stability / repeatability analyzer for sandbox campaigns.

Usage:
    python3 sandbox/scripts/analyze_stability.py <parent-run-dir>

Reads all result.json files from a parent run directory that contains one or more
run-N/ subdirectories (one per repeat). Computes per-scenario statistics across
repeats and writes stability.json to the parent directory.

Pure consumer — no mutations beyond writing stability.json.
"""

import json
import math
import os
import sys
from collections import defaultdict
from pathlib import Path


def load_results_from_dir(results_dir: str) -> list[dict]:
    """Load all result.json files recursively under results_dir."""
    results = []
    root = Path(results_dir)
    for path in root.rglob("result.json"):
        try:
            with open(path) as f:
                results.append(json.load(f))
        except Exception:
            pass
    return results


def group_by_scenario(results: list[dict]) -> dict[str, list[dict]]:
    """Group results by scenario_id (or scenario name)."""
    grouped = defaultdict(list)
    for r in results:
        sid = r.get("scenario_id") or r.get("scenario", {}).get("id", "unknown")
        grouped[sid].append(r)
    return dict(grouped)


def outcomes_across_runs(scenario_results: list[dict]) -> list[str]:
    """Extract outcome list from a scenario's results across repeats."""
    return [r.get("outcome", "unknown") for r in scenario_results]


def timing_stats(timings: list[float]) -> dict:
    """Compute timing statistics for a list of timing values (ms)."""
    if not timings:
        return {"mean": 0, "std_dev": 0, "p50": 0, "p95": 0, "p99": 0, "min": 0, "max": 0, "cv": 0}

    n = len(timings)
    mean = sum(timings) / n
    variance = sum((t - mean) ** 2 for t in timings) / n
    std_dev = math.sqrt(variance)
    cv = (std_dev / mean) if mean > 0 else 0

    sorted_t = sorted(timings)

    def pct(p: float) -> float:
        idx = int(n * p / 100)
        return sorted_t[min(idx, n - 1)]

    return {
        "mean": round(mean, 2),
        "std_dev": round(std_dev, 2),
        "p50": round(pct(50), 2),
        "p95": round(pct(95), 2),
        "p99": round(pct(99), 2),
        "min": round(min(timings), 2),
        "max": round(max(timings), 2),
        "cv": round(cv, 4),
    }


def is_pass(r: dict) -> bool:
    return r.get("outcome") in ("pass", "expected_fail", "capability_missing")


def pass_rate(scenario_results: list[dict]) -> float:
    """Fraction of runs where scenario passed."""
    if not scenario_results:
        return 0.0
    return sum(1 for r in scenario_results if is_pass(r)) / len(scenario_results)


def is_flaky(scenario_results: list[dict], threshold: float = 1.0) -> bool:
    """Flaky if passes are between 0% and 100% (not all pass, not all fail)."""
    if len(scenario_results) < 2:
        return False
    rate = pass_rate(scenario_results)
    return 0.0 < rate < threshold


def analyze_stability(parent_dir: str) -> dict:
    """Main analysis: compute stability metrics for a parent run directory."""
    # Discover repeat subdirs (run-1, run-2, ...)
    parent = Path(parent_dir)
    repeat_dirs = sorted(parent.glob("run-[0-9]*"), key=lambda p: p.name)

    if len(repeat_dirs) < 2:
        print(f"  ⚠️  Fewer than 2 repeat subdirs found in {parent_dir}, skipping stability analysis")
        return {}

    # Load all results from each repeat
    all_results: list[dict] = []
    repeat_results: dict[int, list[dict]] = {}
    for i, rdir in enumerate(repeat_dirs, start=1):
        results = load_results_from_dir(str(rdir))
        repeat_results[i] = results
        all_results.extend(results)

    if not all_results:
        print("  ⚠️  No result.json files found")
        return {}

    # Group by scenario
    by_scenario = group_by_scenario(all_results)

    scenario_stats = []
    timing_data: list[float] = []

    for scenario_id, s_results in by_scenario.items():
        rate = pass_rate(s_results)
        timing_vals = [
            r.get("timing_ms", {}).get("total_ms", 0)
            for r in s_results
            if r.get("timing_ms", {}).get("total_ms", 0) > 0
        ]
        t_stats = timing_stats(timing_vals)
        timing_data.extend(timing_vals)

        flaky = is_flaky(s_results)

        # Outcome distribution
        outcomes = outcomes_across_runs(s_results)
        outcome_dist: dict[str, int] = defaultdict(int)
        for o in outcomes:
            outcome_dist[o] += 1

        # Determine scenario language and tool from first result
        first = s_results[0]
        lang = first.get("language", "unknown")
        tool = first.get("tool", "unknown")

        scenario_stats.append({
            "scenario_id": scenario_id,
            "language": lang,
            "tool": tool,
            "runs": len(s_results),
            "pass_rate": round(rate, 4),
            "flaky": flaky,
            "timing": t_stats,
            "outcome_distribution": dict(outcome_dist),
        })

    # Overall timing
    overall_timing = timing_stats(timing_data)

    # Per-scenario timing CV for ranking
    cv_ranked = [
        (s["scenario_id"], s["timing"]["cv"], s)
        for s in scenario_stats
        if s["runs"] >= 2 and s["timing"]["cv"] > 0
    ]
    cv_ranked.sort(key=lambda x: x[1], reverse=True)

    # Per-language stability summary
    by_lang: dict[str, dict] = defaultdict(lambda: {"total": 0, "flaky": 0, "scenarios": []})
    for s in scenario_stats:
        lang = s["language"]
        by_lang[lang]["total"] += 1
        by_lang[lang]["scenarios"].append(s["scenario_id"])
        if s["flaky"]:
            by_lang[lang]["flaky"] += 1

    # Flaky scenarios list
    flaky_scenarios = [s for s in scenario_stats if s["flaky"]]
    flaky_scenarios.sort(key=lambda s: s["pass_rate"])

    # Top flakiest and highest-variance
    top_flaky = flaky_scenarios[:10]
    top_cv = cv_ranked[:10]

    # Overall KPIs
    total_scenarios = len(by_scenario)
    flaky_count = len(flaky_scenarios)
    stable_count = total_scenarios - flaky_count
    overall_pass_rate = sum(1 for r in all_results if is_pass(r)) / len(all_results) if all_results else 0

    # Health score (similar formula to generate_html_report)
    health = min(100, overall_pass_rate * 100 * 0.5 + 95 * 0.3 + 95 * 0.2)

    return {
        "parent_dir": str(parent_dir),
        "repeat_count": len(repeat_dirs),
        "total_scenarios": total_scenarios,
        "total_runs": len(all_results),
        "pass_rate": round(overall_pass_rate * 100, 2),
        "health_score": round(health, 2),
        "flaky_count": flaky_count,
        "stable_count": stable_count,
        "flaky_percentage": round(flaky_count / total_scenarios * 100, 2) if total_scenarios > 0 else 0,
        "timing_p50_ms": overall_timing["p50"],
        "timing_p95_ms": overall_timing["p95"],
        "timing_p99_ms": overall_timing["p99"],
        "timing_cv": overall_timing["cv"],
        "flaky_scenarios": [s["scenario_id"] for s in flaky_scenarios],
        "top_flaky_scenarios": top_flaky,
        "top_high_variance_scenarios": [{"scenario_id": sid, "cv": round(cv, 4)} for sid, cv, _ in top_cv],
        "by_language": {lang: {
            "total": d["total"],
            "flaky": d["flaky"],
            "stable": d["total"] - d["flaky"],
            "flaky_rate": round(d["flaky"] / d["total"] * 100, 2) if d["total"] > 0 else 0,
        } for lang, d in by_lang.items()},
        "scenario_stats": scenario_stats,
    }


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 analyze_stability.py <parent-run-dir>")
        sys.exit(1)

    parent_dir = sys.argv[1]

    if not os.path.isdir(parent_dir):
        print(f"Error: {parent_dir} is not a directory")
        sys.exit(1)

    print(f"📂 Analyzing stability: {parent_dir}")

    stability = analyze_stability(parent_dir)

    if not stability:
        print("  ⚠️  No stability data produced")
        sys.exit(0)

    output_path = os.path.join(parent_dir, "stability.json")
    with open(output_path, "w") as f:
        json.dump(stability, f, indent=2)

    print(f"  ✅ stability.json written ({len(stability.get('scenario_stats', []))} scenarios, "
          f"{stability.get('repeat_count')} repeats)")
    print(f"  📊 Flaky: {stability.get('flaky_count')}/{stability.get('total_scenarios')} "
          f"({stability.get('flaky_percentage')}%)")
    print(f"  📊 Pass rate: {stability.get('pass_rate')}%")


if __name__ == "__main__":
    main()
