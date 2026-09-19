#!/usr/bin/env python3
"""
release_scorecard.py — Release Readiness Scorecard: 12-gate verdict engine.

Produces a machine-readable scorecard.json and human-readable scorecard.md
aggregating campaign results, baseline, stability, and coverage data.

Exit code: 0 always (gate REDs do not block the script; they are informational).
"""

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from dataclasses import dataclass
from typing import Any, Optional

# ── G5 family → tool mapping ───────────────────────────────────────────────────

FAMILY_BUDGETS: dict[str, tuple[float, str]] = {
    "search":     (30000.0, "ms"),
    "call-graph": (30000.0, "ms"),
    "analytics":  (5000.0,  "ms"),
    "navigation": (45000.0, "ms"),
}

# G4 contract — Tier-1 repositories per the release-readiness spec.
# Source: openspec/specs/release-readiness-gate/spec.md
#   Correctness (ground-truth comparison via the scoring engine's
#   matchers) MUST be >= 90% on every Tier-1 repository
#   (ripgrep, serde, anyhow, tokio, clap).
TIER1_REPOS_PER_SPEC: frozenset[str] = frozenset({
    "ripgrep", "serde", "anyhow", "tokio", "clap",
})
G4_THRESHOLD: float = 90.0

SEARCH_TOOLS = {"search_content", "semantic_search", "query_symbol_index"}
CALL_GRAPH_TOOLS = {
    "build_graph", "build_call_subgraph", "get_call_hierarchy",
    "trace_path", "get_per_file_graph",
}
ANALYTICS_TOOLS = {
    "graph_pagerank", "graph_communities", "graph_god_nodes",
    "graph_all_paths", "graph_condensed", "graph_query",
    "graph_surprising_connections", "graph_insights",
}
NAVIGATION_TOOLS = {"find_references", "hover", "go_to_definition"}
TOOL_TO_FAMILY: dict[str, str] = {
    **{t: "search"     for t in SEARCH_TOOLS},
    **{t: "call-graph" for t in CALL_GRAPH_TOOLS},
    **{t: "analytics"  for t in ANALYTICS_TOOLS},
    **{t: "navigation" for t in NAVIGATION_TOOLS},
}

CRASH_FAILURE_CLASSES = {
    "crash", "panic", "sigsegv", "oom", "oom_killed",
    # NOTE: sandbox_infra_failure is container/workspace availability, NOT a crash.
}


# ── Startup invariant: FAMILY_BUDGETS ⇄ TOOL_TO_FAMILY ────────────────────────

def assert_family_consistency() -> None:
    """Catch drift between TOOL_TO_FAMILY and FAMILY_BUDGETS at module load.

    Without this check, adding a new tool to a *_TOOLS set is silently safe
    (TOOL_TO_FAMILY is auto-derived), but assigning it to a NEW family in
    TOOL_TO_FAMILY directly would leave that family with no budget — and G5
    would degrade to AMBER with "no data for families" forever, instead of
    failing fast at startup.

    Invoked at module import so any consumer (CLI, tests, repl) fails fast.
    """
    families_in_use = set(TOOL_TO_FAMILY.values())
    families_with_budget = set(FAMILY_BUDGETS.keys())
    missing = families_in_use - families_with_budget
    if missing:
        raise RuntimeError(
            f"FAMILY_BUDGETS is missing entries for families referenced by "
            f"TOOL_TO_FAMILY: {sorted(missing)}. "
            f"Add a (budget_ms, unit) tuple for each to sandbox/scripts/"
            f"release_scorecard.py."
        )


# Run the invariant check at import time so any consumer (CLI, tests, repl)
# fails fast on drift instead of silently degrading G5 to AMBER with
# "no data for families".
assert_family_consistency()


# ── Gate result dataclass ─────────────────────────────────────────────────────

from dataclasses import dataclass


@dataclass
class GateResult:
    id: str
    name: str
    status: str          # GREEN | AMBER | RED
    measured: Optional[Any] = None
    budget: Optional[Any] = None
    evidence_path: Optional[str] = None
    evidence_text: Optional[str] = None


# ── Utility loaders ───────────────────────────────────────────────────────────

# ── Result-tree aggregation (post-D1) ─────────────────────────────────────────
#
# Pre-D1 campaigns wrote `summary.json` at the run root. Post-D1 the
# orchestrator writes one `result.json` per (scenario, repeat) under
# `run-N/{scenario_id}/{timestamp}/result.json` and the run_campaign
# harness removes the per-worker summary.json after the run. The
# scorecard reader needs to read either format transparently.

def _discover_result_jsons(run_dir: str) -> list[Path]:
    """Walk a run directory and return every result.json (post-D1 layout).

    Pre-D1 layout kept scenarios at the root; post-D1 nests them under
    `{scenario_id}/{timestamp}/`. We glob both.
    """
    p = Path(run_dir)
    if not p.exists():
        return []
    return sorted(p.rglob("result.json"))


def _aggregate_results(run_dirs: list[str]) -> dict:
    """Aggregate result.json files into a summary-shaped dict.

    Returns the same keys the legacy summary.json had:
      - health_score (avg of non-null per-scenario dimension scores)
      - dimension_scores.correctitud (avg of non-null values)
      - by_tool.<tool>.timing_p95_ms (per-tool p95 across scenarios)
      - failure_distribution (counts by failure_class)
      - regressions_vs_baseline (always empty — caller compares to baseline)
    """
    out: dict = {
        "health_score": None,
        "dimension_scores": {"correctitud": None},
        "by_tool": {},
        "failure_distribution": {},
        "regressions_vs_baseline": [],
    }

    health_vals: list[float] = []
    correctitud_vals: list[float] = []
    per_tool_total: dict[str, list[float]] = {}

    for d in run_dirs:
        for rj in _discover_result_jsons(d):
            try:
                with open(rj) as f:
                    r = json.load(f)
            except Exception:
                continue

            ds = r.get("dimension_scores") or {}
            if ds.get("correctitud") is not None:
                correctitud_vals.append(float(ds["correctitud"]))

            # Health = mean of the four non-correctitud dimensions (matches
            # the orchestrator's compute_health_from_averages heuristic).
            non_corr = [v for k, v in ds.items() if k != "correctitud" and v is not None]
            if non_corr:
                health_vals.append(sum(non_corr) / len(non_corr))

            tool = r.get("tool")
            timing = r.get("timing_ms") or {}
            t_ms = timing.get("tool_call_ms") or timing.get("total_ms")
            if tool and t_ms is not None and t_ms > 0:
                per_tool_total.setdefault(tool, []).append(float(t_ms))

            fc = r.get("failure_class", "pass")
            out["failure_distribution"][fc] = out["failure_distribution"].get(fc, 0) + 1

    if health_vals:
        out["health_score"] = sum(health_vals) / len(health_vals)
    if correctitud_vals:
        out["dimension_scores"]["correctitud"] = sum(correctitud_vals) / len(correctitud_vals)

    # Per-tool p95. With one sample it's that value; with N we take the
    # 95th-percentile by index (numpy-free, deterministic across runs).
    for tool, vals in per_tool_total.items():
        if len(vals) == 1:
            p95 = vals[0]
        else:
            s = sorted(vals)
            idx = max(0, int(round(0.95 * (len(s) - 1))))
            p95 = s[idx]
        out["by_tool"][tool] = {
            "timing_p95_ms": p95,
            "count": len(vals),
        }

    return out


def load_summary_or_aggregate(dir_path: str) -> dict:
    """Load summary.json if present; else aggregate from result.jsons.

    Returns {} only if neither source exists. This keeps the scorecard
    reader backward-compatible with pre-D1 layouts while consuming
    post-D1 runs that lack the legacy summary.json file.
    """
    summary = load_summary(dir_path)
    if summary:
        return summary
    return _aggregate_results([dir_path])


def load_summary_stability_or_aggregate(run_dirs: list[str], stability_path: str = "") -> dict:
    """Like load_summary but also folds stability.json's aggregate metrics.

    stability.json (post-D1) is a first-class artifact and is the canonical
    source for health_score, total_runs, pass_rate, etc. When present, it
    takes precedence over the per-run aggregation because it spans all
    repeats and is computed by analyze_stability.py, not the reader.
    """
    agg = _aggregate_results(run_dirs)
    if stability_path and Path(stability_path).exists():
        try:
            with open(stability_path) as f:
                stab = json.load(f)
            if stab.get("health_score") is not None:
                agg["health_score"] = stab["health_score"]
            if stab.get("pass_rate") is not None:
                agg["pass_rate"] = stab["pass_rate"] / 100.0  # stability stores percent
            if stab.get("total_runs") is not None:
                agg["total_runs"] = stab["total_runs"]
        except Exception:
            pass
    return agg


def load_summary(dir_path: str) -> dict:
    """Load summary.json from a run directory."""
    p = Path(dir_path) / "summary.json"
    if not p.exists():
        return {}
    with open(p) as f:
        return json.load(f)


def load_stability(stability_path: str) -> dict:
    if not stability_path or not Path(stability_path).exists():
        return {}
    with open(stability_path) as f:
        return json.load(f)


def load_coverage_matrix(coverage_path: str) -> dict:
    if not coverage_path or not Path(coverage_path).exists():
        return {}
    # YAML or JSON
    with open(coverage_path) as f:
        raw = f.read()
    try:
        import yaml
        return yaml.safe_load(raw) or {}
    except Exception:
        return json.loads(raw) if raw.strip().startswith("{") else {}


def load_g8_probe(g8_path: str) -> dict:
    """Load result.json files from a g8-probe directory to determine outcome."""
    if not g8_path or not Path(g8_path).exists():
        return {}
    results = {}
    for result_file in Path(g8_path).rglob("result.json"):
        try:
            with open(result_file) as f:
                data = json.load(f)
                sid = data.get("scenario_id", result_file.parent.name)
                results[sid] = data
        except Exception:
            pass
    return results


def git_logEvidence() -> tuple[str, str, str]:
    """G1: git log evidence for e13-wave2 PRs. Returns (status, evidence_text, path).

    The original check looked only at the last 30 commits, which is too
    narrow for a merge commit that landed weeks earlier. Post-fix: walk
    the full git log (depth-bounded to 5000 commits for safety), then
    look for either:
      - commit subject containing e13-wave2
      - a merge commit pulling in feat/e13-wave2-*
      - an e13* tag
    """
    project_root = Path(__file__).parent.parent.parent

    try:
        # Source 1: last 30 commits (fast path)
        result = subprocess.run(
            ["git", "log", "--oneline", "-30"],
            capture_output=True, text=True, timeout=10,
            cwd=project_root,
        )
        if result.returncode == 0:
            lines = result.stdout.strip().split("\n")
            e13_prs = [l for l in lines if "e13-wave2" in l.lower()]
            if e13_prs:
                return "GREEN", f"Found {len(e13_prs)} e13-wave2 commits in last 30: {e13_prs[0]}", "git_log"

        # Source 2: deeper walk — any commit with e13-wave2 in the message,
        # or merge commits of feat/e13-wave2-* branches.
        deep = subprocess.run(
            ["git", "log", "--oneline", "--all", "-5000",
             "--grep=e13-wave2", "--grep=e13 wave2", "-i"],
            capture_output=True, text=True, timeout=15,
            cwd=project_root,
        )
        if deep.returncode == 0:
            deep_lines = [l for l in deep.stdout.strip().split("\n") if l.strip()]
            if deep_lines:
                return "GREEN", (
                    f"Found {len(deep_lines)} e13-wave2 commits in full git log "
                    f"(depth 5000): {deep_lines[0]}"
                ), "git_log"

        # Source 3: explicit e13 tag check
        tag_result = subprocess.run(
            ["git", "tag", "--list", "e13*", "--format=%(refname:short)"],
            capture_output=True, text=True, timeout=5,
            cwd=project_root,
        )
        if tag_result.returncode == 0 and tag_result.stdout.strip():
            return "GREEN", f"e13 tag found: {tag_result.stdout.strip().split()[0]}", "git_tag"

        return "AMBER", "no e13-wave2 commits in git log or e13 tags (manual evidence required)", "git_log"
    except Exception as e:
        return "AMBER", f"git unavailable: {e} (manual evidence required)", "git_log"


def gate_g1() -> GateResult:
    status, evidence, path = git_logEvidence()
    return GateResult(
        id="G1", name="Git Hygiene / e13-wave2 PR Evidence",
        status=status, evidence_path=path, evidence_text=evidence,
    )


def gate_g2(coverage_path: str) -> GateResult:
    """G2: MCP tool coverage from coverage_matrix.yaml."""
    if not coverage_path or not Path(coverage_path).exists():
        return GateResult(
            id="G2", name="MCP Tool Coverage",
            status="AMBER", evidence_text="coverage_matrix.yaml not found",
            evidence_path=coverage_path or "coverage_matrix.yaml",
        )
    matrix = load_coverage_matrix(coverage_path)
    summary = matrix.get("summary", {})
    total = summary.get("total_tools", 0)
    covered = summary.get("covered", 0)
    uncovered = summary.get("uncovered_count", 0)
    pct = summary.get("coverage_percent", 0.0)

    if pct >= 100:
        status = "GREEN"
    elif pct >= 80:
        status = "AMBER"
    else:
        status = "RED"

    return GateResult(
        id="G2", name="MCP Tool Coverage",
        status=status,
        measured=f"{covered}/{total} ({pct}%)",
        budget="100%",
        evidence_text=f"{covered} covered, {uncovered} uncovered",
        evidence_path=coverage_path,
    )


def gate_g3(run_dirs: list[str], stability_path: str = "") -> GateResult:
    """G3: Health score ≥85. GREEN if avg ≥85; AMBER if single run <85; RED if avg <85 with ≥2 runs.

    Reads health_score from (in order):
      1. stability.json (post-D1 canonical source — spans all repeats)
      2. summary.json in each run_dir (pre-D1)
      3. Aggregated from result.json files (fallback when both above missing)
    """
    scores: list[float] = []
    sources: list[str] = []

    # Source 1: stability.json (highest precedence)
    if stability_path:
        stab = load_stability(stability_path)
        if stab.get("health_score") is not None:
            scores.append(float(stab["health_score"]))
            sources.append("stability.json")

    # Source 2 + 3: per-run summary.json, fall back to result.json aggregation
    for d in run_dirs:
        s = load_summary(d)
        if not s:
            s = _aggregate_results([d])
            sources.append(f"aggregate({Path(d).name})")
        else:
            sources.append(f"summary({Path(d).name})")
        hs = s.get("health_score")
        if hs is not None:
            scores.append(float(hs))

    if not scores:
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="AMBER", evidence_text="no health_score data in any source",
            evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
        )

    avg = sum(scores) / len(scores)
    if len(scores) == 1:
        status = "GREEN" if scores[0] >= 85 else "AMBER"
        detail = f"single run: {scores[0]:.1f}"
    else:
        status = "GREEN" if avg >= 85 else "RED"
        detail = f"avg {avg:.1f} across {len(scores)} runs"

    return GateResult(
        id="G3", name="Sandbox Health Score",
        status=status,
        measured=round(avg, 1),
        budget=85.0,
        evidence_text=f"{detail}; sources: {', '.join(sources)}",
        evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
    )


def gate_g4(run_dirs: list[str]) -> GateResult:
    """G4: Correctitud dimension score >= 90 on every Tier-1 repository.

    Contract (openspec/specs/release-readiness-gate/spec.md):
      Correctness (ground-truth comparison via the scoring engine's
      matchers) MUST be >= 90% on every Tier-1 repository
      (ripgrep, serde, anyhow, tokio, clap).

    Provenance policy (A1a+1):
      Every result candidate carries eight provenance fields:
        declared_repo, resolved_workspace, actual_repository_identity,
        actual_repository_revision, ground_truth_present,
        correctness_measured, scenario_id, repeat_identity.
      The reader does NOT trust the declared `repo` field. It only
      accredits a result when positive provenance is present:
        actual_repository_identity in TIER1_REPOS_PER_SPEC
        AND actual_repository_revision is not None
        AND ground_truth_present is True
        AND correctness_measured is not None
      Results without positive provenance are classified UNVERIFIED
      and contribute neither to coverage nor to the average.

    Aggregation policy:
      The per-repo average is computed over distinct scenario_ids
      (one value per scenario_id, even across repeats). Repeat counts
      do NOT fabricate coverage. A repo with one scenario and three
      repeats has coverage of one scenario, not three.

    Pre-D1 compatibility:
      Legacy result.json / summary.json files do not carry
      actual_repository_identity or actual_repository_revision. They
      are classified UNVERIFIED honestly, not silently re-tagged as
      Tier-1.

    Verdict precedence:
      1. All Tier-1 repos acredited with coverage >= 1 scenario each
         AND every per-repo average >= G4_THRESHOLD
             -> GREEN
      2. Some Tier-1 repo acredited with coverage >= 1 scenario
         AND any per-repo average < G4_THRESHOLD
             -> RED (failing repo named)
      3. No Tier-1 repo acredited, OR some repo missing coverage
             -> AMBER / INCOMPLETE (missing/unverified repos named)
      Missing data NEVER produces GREEN.
    """
    measurements: dict[str, dict[str, list[float]]] = {}  # repo -> scenario_id -> [scores]
    coverage: dict[str, set[str]] = {}                    # repo -> set of scenario_ids
    unverified_repos: set[str] = set()
    scored_pre_d1_fixtures: list[float] = []              # diagnostic only, never drives verdict
    total_candidates = 0
    total_unverified = 0

    for d in run_dirs:
        for rj_path in _discover_result_jsons(d):
            try:
                with open(rj_path) as f:
                    r = json.load(f)
            except Exception:
                continue
            total_candidates += 1

            ds = r.get("dimension_scores") or {}
            measured = ds.get("correctitud")

            provenance = _g4_extract_provenance(r)
            declared = provenance["declared_repo"]
            actual = provenance["actual_repository_identity"]
            revision = provenance["actual_repository_revision"]
            gt_present = provenance["ground_truth_present"]
            sid = provenance["scenario_id"]

            # No positive provenance -> UNVERIFIED. If declared_repo is
            # Tier-1, that repo is named as unverified for the evidence
            # text, but it does NOT count toward coverage or the average.
            if actual is None or revision is None or not gt_present:
                total_unverified += 1
                # If the declared label names a Tier-1 repo, surface it
                # as an unverified Tier-1 candidate. Otherwise it stays
                # out of the verdict entirely.
                if declared in TIER1_REPOS_PER_SPEC:
                    unverified_repos.add(declared)
                # Pre-D1 fixtures carry a correctitud score but no
                # positive provenance. Preserve the score as a diagnostic
                # signal so we can report it as "fixture correctitud"
                # separately — never as Tier-1 evidence.
                if measured is not None:
                    scored_pre_d1_fixtures.append(float(measured))
                continue

            if actual not in TIER1_REPOS_PER_SPEC:
                # Credited to a non-Tier-1 repo. Does not affect G4.
                if measured is not None:
                    scored_pre_d1_fixtures.append(float(measured))
                continue

            # Positive Tier-1 credit.
            if measured is None:
                # GT was declared as present but no score came back. Treat
                # as unverified for this Tier-1 repo.
                unverified_repos.add(actual)
                continue

            score = float(measured)
            measurements.setdefault(actual, {}).setdefault(sid, []).append(score)
            coverage.setdefault(actual, set()).add(sid)

    acredited = sorted(measurements.keys())
    missing_or_unverified = sorted(
        (TIER1_REPOS_PER_SPEC - set(acredited)) | unverified_repos
    )

    # Compute per-repo averages over distinct scenario_ids. Each
    # scenario_id contributes the mean of its repeats (matches the
    # scoring engine's aggregation policy).
    per_repo_avg: dict[str, float] = {}
    per_repo_scenarios: dict[str, int] = {}
    for repo, scenarios in measurements.items():
        per_scenario_means = [sum(vals) / len(vals) for vals in scenarios.values()]
        per_repo_avg[repo] = sum(per_scenario_means) / len(per_scenario_means)
        per_repo_scenarios[repo] = len(scenarios)

    # Verdict precedence.
    failing_repos: list[str] = []
    sufficient = {r for r in acredited if per_repo_scenarios.get(r, 0) >= 1}

    if acredited and not missing_or_unverified:
        # Every Tier-1 repo is acredited.
        for repo in acredited:
            if per_repo_avg[repo] < G4_THRESHOLD:
                failing_repos.append(repo)
        if failing_repos:
            status = "RED"
        else:
            status = "GREEN"
    elif acredited and missing_or_unverified:
        # Some repos acredited, some missing. If any acredited repo is
        # below threshold, RED wins (per directive: do not hide RED
        # behind missing evidence).
        for repo in acredited:
            if per_repo_avg[repo] < G4_THRESHOLD:
                failing_repos.append(repo)
        if failing_repos:
            status = "RED"
        else:
            status = "AMBER"
    else:
        # No repo acredited.
        status = "AMBER"

    # Build evidence text.
    lines: list[str] = []
    if per_repo_avg:
        for repo in sorted(per_repo_avg.keys()):
            n = per_repo_scenarios.get(repo, 0)
            lines.append(
                f"{repo}: avg={per_repo_avg[repo]:.1f} "
                f"(n_scenarios={n}, threshold={G4_THRESHOLD:.0f})"
            )
    if missing_or_unverified:
        lines.append(
            "missing_or_unverified: " + ", ".join(missing_or_unverified)
        )
    if scored_pre_d1_fixtures:
        avg_fixture = sum(scored_pre_d1_fixtures) / len(scored_pre_d1_fixtures)
        lines.append(
            f"fixture_diagnostic_only: avg={avg_fixture:.1f} "
            f"across {len(scored_pre_d1_fixtures)} pre-D1/no-provenance "
            f"scenarios (does not count toward G4)"
        )
    lines.insert(
        0,
        f"acredited={len(acredited)}/{len(TIER1_REPOS_PER_SPEC)} "
        f"tier1_repos; candidates={total_candidates}; "
        f"unverified={total_unverified}",
    )
    if failing_repos:
        lines.insert(1, "failing: " + ", ".join(sorted(failing_repos)))

    evidence_text = "; ".join(lines)
    measured_value = (
        sum(per_repo_avg.values()) / len(per_repo_avg) if per_repo_avg else None
    )

    return GateResult(
        id="G4", name="Corpus Quality / Correctitud",
        status=status,
        measured=(
            f"{measured_value:.1f} avg across "
            f"{len(per_repo_avg)}/{len(TIER1_REPOS_PER_SPEC)} tier1 repos"
            if measured_value is not None
            else None
        ),
        budget=G4_THRESHOLD,
        evidence_text=evidence_text,
        evidence_path=",".join(run_dirs),
    )


def _g4_extract_provenance(r: dict) -> dict:
    """Extract provenance fields from a result.json dict.

    The orchestrator's current schema predates actual_repository_identity
    and actual_repository_revision, so they may be missing entirely.
    In that case the reader treats the result as UNVERIFIED rather than
    silently inferring identity from declared_repo.
    """
    return {
        "declared_repo": r.get("repo"),
        "resolved_workspace": r.get("workspace"),
        "actual_repository_identity": r.get("actual_repository_identity"),
        "actual_repository_revision": r.get("actual_repository_revision"),
        "ground_truth_present": bool(r.get("ground_truth_present", True)),
        "correctness_measured": (
            (r.get("dimension_scores") or {}).get("correctitud")
        ),
        "scenario_id": r.get("scenario_id") or "",
        "repeat_identity": r.get("repeat_index"),
    }


def gate_g5(run_dirs: list[str]) -> GateResult:
    """G5: Latency by tool family (search <500ms, call-graph <2s, analytics <5s).

    Aggregates per-tool latencies from result.json files (post-D1) or
    summary.json (pre-D1) and groups by family using TOOL_TO_FAMILY.
    Each family has a per-tool worst-case p95; the family p95 is the max
    across all tools in that family.
    """
    family_p95: dict[str, list[float]] = {f: [] for f in FAMILY_BUDGETS}

    # Source: result.json aggregation post-D1 (preferred) or summary.json
    # pre-D1. _aggregate_results returns by_tool.timing_p95_ms in either case.
    for d in run_dirs:
        s = load_summary(d)
        if not s:
            s = _aggregate_results([d])
        by_tool = s.get("by_tool", {})
        for tool_name, tool_data in by_tool.items():
            family = TOOL_TO_FAMILY.get(tool_name)
            if family:
                p95 = tool_data.get("timing_p95_ms")
                if p95 is not None and p95 > 0:
                    family_p95[family].append(p95)

    worst_status = "GREEN"
    violations: list[str] = []
    no_data_families: list[str] = []

    for family, budget_ms in FAMILY_BUDGETS.items():
        budget = budget_ms[0]
        p95_list = family_p95[family]
        if not p95_list:
            no_data_families.append(family)
            continue
        worst = max(p95_list)
        if worst > budget:
            worst_status = "RED"
            violations.append(f"{family}: p95={worst:.0f}ms > budget {budget:.0f}ms")

    if worst_status == "GREEN" and no_data_families:
        worst_status = "AMBER"

    if worst_status == "GREEN":
        evidence = "all families within budget"
    elif worst_status == "RED":
        evidence = "; ".join(violations)
    else:
        evidence = f"no data for families: {', '.join(no_data_families)}"

    return GateResult(
        id="G5", name="Latency Budget by Tool Family",
        status=worst_status,
        measured=evidence,
        evidence_path=",".join(run_dirs),
    )


def gate_g6(stability_path: str) -> GateResult:
    """G6: Run-to-run stability (timing_cv < 10%).

    Reads per-scenario timing CVs from stability.json/scenario_stats[].timing.cv
    (post-D1 canonical source — emitted by analyze_stability.py across
    all repeats). Falls back to families_runtorun / families if the
    older key is present (pre-D1 compatibility).
    """
    stab = load_stability(stability_path)
    if not stab:
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER", evidence_text="stability.json not found",
            evidence_path=stability_path or "stability.json",
        )

    # Source 1 (post-D1): per-scenario CVs in scenario_stats
    scenario_stats = stab.get("scenario_stats") or []
    cvs: list[float] = []
    source_label = ""
    if scenario_stats:
        for s in scenario_stats:
            timing = s.get("timing") or {}
            # Prefer cv_warm (E31-E) when present; fall back to cv otherwise.
            v = timing.get("cv_warm") if timing.get("cv_warm") is not None else timing.get("cv")
            if v is not None:
                cvs.append(float(v))
        if cvs:
            source_label = "scenario_stats"

    # Source 2 (pre-D1 fallback): families_runtorun / families
    if not cvs:
        fams = stab.get("families_runtorun") or stab.get("families") or {}
        for f in fams.values():
            v = f.get("mean_cv_warm") if f.get("mean_cv_warm") is not None else f.get("mean_cv")
            if v is not None:
                cvs.append(float(v))
        if cvs:
            source_label = "families (pre-D1)"

    if not cvs:
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER",
            evidence_text=(
                "no per-scenario timing CVs in stability.json "
                "(need scenario_stats[].timing.cv or families_runtorun)"
            ),
            evidence_path=stability_path,
        )

    max_cv = max(cvs)
    status = "GREEN" if max_cv < 0.10 else "RED"
    cv_label = "warm-cache" if any(
        (s.get("timing") or {}).get("cv_warm") is not None for s in scenario_stats
    ) else "full"
    return GateResult(
        id="G6", name="Run-to-Run Stability",
        status=status,
        measured=f"{max_cv*100:.1f}%",
        budget="<10%",
        evidence_text=(
            f"max per-scenario run-to-run CV={max_cv:.4f} ({cv_label}) "
            f"across {len(cvs)} scenarios ({source_label})"
        ),
        evidence_path=stability_path,
    )


def gate_g7(run_dirs: list[str]) -> GateResult:
    """G7: Zero crash-class failures in failure_distribution.

    Reads failure_distribution from summary.json (pre-D1) or aggregates
    from result.json files (post-D1). Both shapes use a flat
    {failure_class: count} dict.
    """
    crash_count = 0
    evidence_parts = []

    for d in run_dirs:
        s = load_summary(d)
        if not s:
            s = _aggregate_results([d])
        failure_dist = s.get("failure_distribution", {})
        for cls, count in failure_dist.items():
            if cls.lower() in CRASH_FAILURE_CLASSES or any(
                c in cls.lower() for c in ["crash", "panic", "oom", "sigsegv"]
            ):
                crash_count += count
                evidence_parts.append(f"{cls}={count}")

    if crash_count > 0:
        status = "RED"
        evidence = f"crash-class failures: {', '.join(evidence_parts)}"
    else:
        status = "GREEN"
        evidence = "no crash-class failures detected"

    return GateResult(
        id="G7", name="Robustness — Zero Crashes",
        status=status,
        measured=crash_count,
        budget=0,
        evidence_text=evidence,
        evidence_path=",".join(run_dirs),
    )


def gate_g8(g8_probe_dir: str) -> GateResult:
    """G8: Tier-3 scalability probe (build_graph on 652M LOC typescript repo)."""
    results = load_g8_probe(g8_probe_dir)

    if not results:
        return GateResult(
            id="G8", name="Scalability Proof (Tier-3)",
            status="AMBER", evidence_text="no g8-probe results found",
            evidence_path=g8_probe_dir or "g8-probe/",
        )

    # Check for OOM/timeout classified as such
    oom_timeout_tools = []
    pass_tools = []
    for sid, res in results.items():
        outcome = res.get("outcome", "")
        failure_class = res.get("failure_class", "")
        tool = res.get("tool", sid)
        if outcome in ("pass", "expected_fail"):
            pass_tools.append(tool)
        if "oom" in str(failure_class).lower() or "timeout" in str(failure_class).lower():
            oom_timeout_tools.append(f"{tool}({failure_class})")

    if oom_timeout_tools:
        # OOM/timeout with a tracked defect → AMBER (defect SCAL-001 / INC-004)
        # Per E31-F: evidence text must reference the defect ID explicitly so
        # monitors can correlate the AMBER with the upstream tracking issue.
        return GateResult(
            id="G8", name="Scalability Proof (Tier-3)",
            status="AMBER",
            measured=f"OOM/timeout: {', '.join(oom_timeout_tools)}",
            evidence_text="OOM or timeout detected in tier-3 probe (defect SCAL-001 / INC-004: typescript tier-3 652M-LOC timeout; container 1G→4G mitigation applied; see ~/.sddk-knowledge/CogniCode/incidences/INC-004-scal-001-scalability-typescript.md)",
            evidence_path=g8_probe_dir,
        )
    elif pass_tools:
        return GateResult(
            id="G8", name="Scalability Proof (Tier-3)",
            status="GREEN",
            measured=f"pass: {', '.join(pass_tools)}",
            evidence_text=f"{len(pass_tools)} tier-3 tools passed",
            evidence_path=g8_probe_dir,
        )
    else:
        return GateResult(
            id="G8", name="Scalability Proof (Tier-3)",
            status="RED",
            evidence_text="tier-3 probe had failures but none classified as OOM/timeout",
            evidence_path=g8_probe_dir,
        )


def gate_g9(run_dirs: list[str]) -> GateResult:
    """G9: No regressions vs baseline."""
    all_regressions = []
    for d in run_dirs:
        s = load_summary(d)
        if not s:
            s = _aggregate_results([d])
        regs = s.get("regressions_vs_baseline", [])
        all_regressions.extend(regs)

    if all_regressions:
        return GateResult(
            id="G9", name="No Regressions vs Baseline",
            status="RED",
            measured=f"{len(all_regressions)} regressions",
            budget=0,
            evidence_text=", ".join(all_regressions[:5]) + (" ..." if len(all_regressions) > 5 else ""),
            evidence_path=",".join(run_dirs),
        )
    else:
        return GateResult(
            id="G9", name="No Regressions vs Baseline",
            status="GREEN",
            measured="0 regressions",
            budget=0,
            evidence_text="regressions_vs_baseline is empty in all runs",
            evidence_path=",".join(run_dirs),
        )


def gate_g10(matrix_path: str = "sandbox/reports/conformance_matrix.yaml") -> GateResult:
    """G10: Openspec conformance audit — reads the conformance matrix."""
    mp = Path(matrix_path)
    if not mp.exists():
        return GateResult(
            id="G10", name="Openspec Conformance Audit",
            status="AMBER", evidence_text="conformance_matrix.yaml not found",
            evidence_path=str(mp),
        )
    try:
        import yaml
        data = yaml.safe_load(mp.read_text())
    except Exception as e:
        return GateResult(
            id="G10", name="Openspec Conformance Audit",
            status="AMBER", evidence_text=f"matrix parse error: {e}",
            evidence_path=str(mp),
        )
    summary = data.get("summary", {})
    legacy_obsolete = summary.get("legacy_obsolete", 0)
    active_total = summary.get("total", 0) - legacy_obsolete
    pct_v = (summary.get("verified", 0) / active_total * 100) if active_total else 0.0
    pct_t = (summary.get("verified", 0) + legacy_obsolete) / summary.get("total", 0) * 100 if summary.get("total", 0) else 0.0
    if pct_v >= 90.0 and pct_t >= 100.0:
        status = "GREEN"
    elif pct_v >= 90.0 or pct_t >= 100.0:
        status = "AMBER"
    else:
        status = "RED"
    return GateResult(
        id="G10", name="Openspec Conformance Audit",
        status=status,
        measured=f"verified {pct_v}% / triaged {pct_t}%",
        budget=">=90% verified, 100% triaged",
        evidence_text=(
            f"total={summary.get('total')} verified={summary.get('verified')} "
            f"legacy_obsolete={legacy_obsolete} "
            f"pct_verified={pct_v:.1f}% (denom=total−legacy_obsolete={active_total}, per ADR-031 §4)"
        ),
        evidence_path=str(mp),
    )


def gate_g11(project_root: str) -> GateResult:
    """G11: Documentation currency — MCP-TOOLS.md (68 tools) + ADR-031/032."""
    checks = []
    mcp_tools_path = Path(project_root) / "docs" / "MCP-TOOLS.md"
    adr031_path = Path(project_root) / "docs" / "adr" / "ADR-031-release-1.0.0-definition.md"
    adr032_path = Path(project_root) / "docs" / "adr" / "ADR-032-sandbox-validation-system.md"

    mcp_ok = False
    if mcp_tools_path.exists():
        try:
            content = mcp_tools_path.read_text()
            if "68 tools" in content:
                mcp_ok = True
                checks.append("MCP-TOOLS.md found (68 tools)")
            else:
                checks.append("MCP-TOOLS.md found (tool count NOT 68)")
        except Exception:
            checks.append("MCP-TOOLS.md found (read error)")
    else:
        checks.append("MCP-TOOLS.md NOT found")

    def adr_ok(path: Path) -> tuple[bool, str]:
        if not path.exists():
            return False, "NOT found"
        try:
            text = path.read_text()
            if "ACEPTADO" in text.upper() or "accepted" in text.lower():
                return True, "found (ACEPTADO)"
            return True, "found (status NOT accepted)"
        except Exception:
            return True, "found (read error)"

    adr031_ok, adr031_note = adr_ok(adr031_path)
    adr032_ok, adr032_note = adr_ok(adr032_path)
    checks.append(f"ADR-031 {adr031_note}")
    checks.append(f"ADR-032 {adr032_note}")

    roadmap_ok = (Path(project_root) / "docs" / "ROADMAP.md").exists()
    checks.append("ROADMAP.md found" if roadmap_ok else "ROADMAP.md NOT found")

    if not mcp_ok or not adr031_ok or not adr032_ok:
        status = "RED"
    elif adr031_note.endswith("(ACEPTADO)") and adr032_note.endswith("(ACEPTADO)") and roadmap_ok:
        status = "GREEN"
    else:
        status = "AMBER"

    return GateResult(
        id="G11", name="Documentation Currency",
        status=status,
        evidence_text="; ".join(checks),
        evidence_path="docs/MCP-TOOLS.md, docs/adr/ADR-031*, docs/adr/ADR-032*",
    )


def gate_g12(project_root: str) -> GateResult:
    """G12: Git branch/tag hygiene — recent semver tag."""
    checks = []
    try:
        result = subprocess.run(
            ["git", "tag", "--sort=-v:refname", "--list", "v*"],
            capture_output=True, text=True, timeout=10, cwd=project_root,
        )
        tags = [t for t in result.stdout.strip().split("\n") if t]
        recent = tags[0] if tags else "none"
        checks.append(f"latest semver tag: {recent}")
    except Exception as e:
        recent = "unknown"
        checks.append(f"git tag check failed: {e}")

    changelog_ok = (Path(project_root) / "CHANGELOG.md").exists()
    checks.append("CHANGELOG.md found" if changelog_ok else "CHANGELOG.md MISSING")

    try:
        merged = subprocess.run(
            ["git", "branch", "-r", "--merged", "origin/main"],
            capture_output=True, text=True, timeout=15, cwd=project_root,
        )
        stale = [l.strip() for l in merged.stdout.split("\n") if l.strip() and "origin/main" not in l and "origin/HEAD" not in l]
        stale_count = len(stale)
        checks.append(f"stale merged remote branches: {stale_count}")
    except Exception:
        stale_count = -1
        checks.append("branch check failed")

    if changelog_ok and stale_count >= 0 and stale_count <= 20:
        status = "GREEN"
    elif changelog_ok and stale_count <= 50:
        status = "AMBER"
    else:
        status = "RED"

    return GateResult(
        id="G12", name="Git Hygiene (tags/changelog/branches)",
        status=status,
        measured=f"tag={recent} stale_branches={stale_count}",
        evidence_text="; ".join(checks),
        evidence_path="git_tag, CHANGELOG.md, git_branch",
    )


def gate_g13_lsi(
    baseline_path: str, delta_path: str, fail_above_pct: float = 25.0
) -> GateResult:
    """G13 (optional, non-blocking): LSI M0 benchmark baseline + compare deltas.

    Reads the E36 LSI baseline artifact (lsi_bench_baseline.py capture) and its
    delta report (lsi_bench_baseline.py compare --output). AMBER when artifacts
    are missing (baseline not captured yet — expected before M1 kernel work);
    RED only when a committed delta report flags regressions beyond the
    threshold. Absent artifacts never fail the scorecard run.
    """
    bp = Path(baseline_path)
    if not bp.exists():
        return GateResult(
            id="G13",
            name="LSI M0 Benchmark Baseline",
            status="AMBER",
            evidence_text=(
                f"baseline not found at {baseline_path} — run "
                "`just lsi-baseline` (capture) before M1 kernel changes; "
                "optional gate, non-blocking"
            ),
            evidence_path=baseline_path,
        )
    try:
        with open(bp) as f:
            baseline = json.load(f)
    except Exception as e:
        return GateResult(
            id="G13",
            name="LSI M0 Benchmark Baseline",
            status="AMBER",
            evidence_text=f"baseline parse error: {e}",
            evidence_path=baseline_path,
        )
    bench_count = len(baseline.get("benchmarks", []))
    commit = baseline.get("commit", "unknown")

    dp = Path(delta_path)
    if not dp.exists():
        return GateResult(
            id="G13",
            name="LSI M0 Benchmark Baseline",
            status="AMBER",
            measured=f"{bench_count} benchmarks baselined @ {commit[:12]}",
            evidence_text=(
                f"baseline present ({bench_count} benchmarks) but no compare "
                f"delta report at {delta_path} — run `just lsi-baseline compare`"
            ),
            evidence_path=baseline_path,
        )
    try:
        with open(dp) as f:
            delta = json.load(f)
    except Exception as e:
        return GateResult(
            id="G13",
            name="LSI M0 Benchmark Baseline",
            status="AMBER",
            evidence_text=f"delta report parse error: {e}",
            evidence_path=delta_path,
        )

    regressions = delta.get("regressions", [])
    if regressions:
        return GateResult(
            id="G13",
            name="LSI M0 Benchmark Baseline",
            status="RED",
            measured=f"{len(regressions)} regressions",
            budget=f"no benchmark >{fail_above_pct}% vs baseline",
            evidence_text=(
                f"regressed beyond {fail_above_pct}% vs baseline @ {commit[:12]}: "
                + ", ".join(str(r) for r in regressions[:8])
            ),
            evidence_path=delta_path,
        )
    return GateResult(
        id="G13",
        name="LSI M0 Benchmark Baseline",
        status="GREEN",
        measured=f"{len(delta.get('deltas', []))} benchmarks compared",
        budget=f"no benchmark >{fail_above_pct}% vs baseline",
        evidence_text=(
            f"delta report clean vs baseline @ {commit[:12]} "
            f"(threshold {fail_above_pct}%)"
        ),
        evidence_path=delta_path,
    )


# ── Markdown table renderer ───────────────────────────────────────────────────

def render_markdown(gates: list[GateResult], generated_at: str) -> str:
    lines = [
        "# Release Readiness Scorecard",
        "",
        f"**Generated**: {generated_at}",
        "",
        "| Gate | Status | Measured | Budget | Evidence |",
        "|------|--------|----------|--------|----------|",
    ]
    for g in gates:
        badge = {"GREEN": "✅", "AMBER": "⚠️", "RED": "❌"}.get(g.status, "?")
        measured = str(g.measured) if g.measured is not None else "—"
        budget = str(g.budget) if g.budget is not None else "—"
        evidence = g.evidence_text or g.evidence_path or "—"
        lines.append(
            f"| {g.id} {g.name} | {badge} {g.status} | {measured} | {budget} | {evidence} |"
        )
    return "\n".join(lines)


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(
        description="Release Readiness Scorecard — 12-gate verdict engine",
    )
    parser.add_argument(
        "--runs",
        required=True,
        help="Comma-separated list of run directories containing summary.json",
    )
    parser.add_argument(
        "--baseline",
        required=False,
        default=None,
        help="Baseline run directory (for G9 regressions_vs_baseline)",
    )
    parser.add_argument(
        "--stability",
        required=False,
        default=None,
        help="Path to stability.json (from analyze_stability.py)",
    )
    parser.add_argument(
        "--coverage-matrix",
        required=False,
        default=None,
        help="Path to coverage_matrix.yaml (from generate_tool_coverage.py)",
    )
    parser.add_argument(
        "--g8-probe-result",
        required=False,
        default=None,
        help="Directory containing tier-3 probe results (result.json files)",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output prefix for scorecard.json and scorecard.md",
    )
    parser.add_argument(
        "--results-dir",
        required=False,
        default=None,
        help="Base results directory — auto-discovers full-run-N, full/, or root",
    )
    parser.add_argument(
        "--lsi-baseline",
        required=False,
        default=None,
        help="Path to the LSI M0 baseline artifact (G13, optional gate; "
        "default: sandbox/results/lsi-baseline/baseline.json)",
    )
    parser.add_argument(
        "--lsi-delta",
        required=False,
        default=None,
        help="Path to the LSI compare delta report (G13, optional gate; "
        "default: sandbox/results/lsi-baseline/delta.json)",
    )
    parser.add_argument(
        "--lsi-fail-above",
        required=False,
        type=float,
        default=25.0,
        help="G13 regression threshold in percent (default: 25)",
    )
    args = parser.parse_args()

    # Auto-discovery: if --results-dir is provided, find run subdirectories
    if args.results_dir:
        base = Path(args.results_dir)
        candidates = [
            base / "full-run-1",
            base / "full-run-2",
            base / "full-run-3",
            base / "full",
            base,
        ]
        discovered = [str(d) for d in candidates if d.exists()]
        run_dirs = discovered if discovered else [str(base)]
    else:
        run_dirs = [d.strip() for d in args.runs.split(",") if d.strip()]
    project_root = str(Path(__file__).parent.parent.parent)

    # G13 is optional and non-blocking: absent LSI artifacts degrade to AMBER.
    lsi_baseline = args.lsi_baseline or f"{project_root}/sandbox/results/lsi-baseline/baseline.json"
    lsi_delta = args.lsi_delta or f"{project_root}/sandbox/results/lsi-baseline/delta.json"

    # Evaluate all 13 gates
    gates = [
        gate_g1(),
        gate_g2(args.coverage_matrix),
        gate_g3(run_dirs, args.stability),
        gate_g4(run_dirs),
        gate_g5(run_dirs),
        gate_g6(args.stability),
        gate_g7(run_dirs),
        gate_g8(args.g8_probe_result),
        gate_g9(run_dirs),
        gate_g10(f"{project_root}/sandbox/reports/conformance_matrix.yaml"),
        gate_g11(project_root),
        gate_g12(project_root),
        gate_g13_lsi(lsi_baseline, lsi_delta, args.lsi_fail_above),
    ]

    generated_at = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # Build JSON output
    scorecard = {
        "generated_at": generated_at,
        "gates": [
            {
                "id": g.id,
                "name": g.name,
                "status": g.status,
                "measured": g.measured,
                "budget": g.budget,
                "evidence_path": g.evidence_path,
                "evidence_text": g.evidence_text,
            }
            for g in gates
        ],
    }

    # Write scorecard.json
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    json_path = output_path.with_suffix(".json")
    with open(json_path, "w") as f:
        json.dump(scorecard, f, indent=2)

    # Write scorecard.md
    md_path = output_path.with_suffix(".md")
    with open(md_path, "w") as f:
        f.write(render_markdown(gates, generated_at))

    # Print summary to stdout
    print(f"Release Readiness Scorecard")
    print(f"  Generated: {generated_at}")
    print(f"  Gates: {len(gates)}")
    red_count = sum(1 for g in gates if g.status == "RED")
    amber_count = sum(1 for g in gates if g.status == "AMBER")
    green_count = sum(1 for g in gates if g.status == "GREEN")
    print(f"  GREEN: {green_count}  AMBER: {amber_count}  RED: {red_count}")
    print(f"  JSON:  {json_path}")
    print(f"  Markdown: {md_path}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
