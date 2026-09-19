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

# G3 contract — Sandbox Health Score per the release-readiness spec.
# Source: openspec/specs/release-readiness-gate/spec.md
#   The MCP Health Score (weighted average of correctness, latency,
#   scalability, consistency, robustness dimensions, computed by
#   sandbox_core::scoring) MUST be >= 85/100 for the candidate release.
# Authoritative source of weights: crates/cognicode-core/src/sandbox_core/
# scoring.rs:1400 (HEALTH_WEIGHTS). Update both together.
G3_THRESHOLD: float = 85.0
G3_HEALTH_WEIGHTS: tuple[float, ...] = (0.35, 0.20, 0.15, 0.15, 0.15)
G3_DIM_NAMES: tuple[str, ...] = (
    "correctitud", "latencia", "escalabilidad", "consistencia", "robustez",
)

# G6 contract — Run-to-Run Stability per the release-readiness spec.
# Source: openspec/specs/release-readiness-gate/spec.md
#   Run-to-run variance (via stability.json from --repeat >= 3) MUST
#   be < 10% per dimension.
G6_CV_THRESHOLD: float = 0.10
G6_MIN_REPEATS: int = 3
G6_MIN_SAMPLES_PER_SCENARIO: int = 3

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
    """G3: Sandbox Health Score (5-dim weighted average) >= 85.

    Contract (openspec/specs/release-readiness-gate/spec.md):
      The MCP Health Score (weighted average of correctness, latency,
      scalability, consistency, robustness dimensions, computed by
      sandbox_core::scoring) MUST be >= 85/100 for the candidate release.

    Authoritative source of the formula:
      crates/cognicode-core/src/sandbox_core/scoring.rs:1400
        HEALTH_WEIGHTS = (0.35, 0.20, 0.15, 0.15, 0.15)
        compute_health_score(scores) =
            CORR*0.35 + LAT*0.20 + ESC*0.15 + CON*0.15 + ROB*0.15

    Single source of truth (A1c.2):
      The spec requires the scoring-engine formula. The reader computes
      the health score DIRECTLY from result.json files in each run_dir
      using G3_HEALTH_WEIGHTS. It does NOT consume stability.json::
      health_score (which is the analyze_stability.py formula:
      `min(100, pass_rate*50 + 95*30 + 95*20)` — two 95s are
      placeholder constants, not measurements) and does NOT mix
      per-run summary.json::health_score or aggregate values.

    Provenance policy:
      A scenario contributes to G3 only when ALL FIVE dimensions are
      present and non-None in EVERY repeat (across run_dirs). If even
      one repeat has correctitud=None (typical: scenario has no
      ground truth), the scenario is excluded from the verdict-driving
      set but counted as incomplete_5dim.

    Aggregation policy (P1):
      Each result.json file is one repeat of one scenario. Aggregation
      is per distinct scenario_id (de-duplicated across run_dirs);
      per-scenario health is the mean of the 5-dim health values
      computed across the repeats that have all 5 dims populated.
      The reader does NOT silently use only the first repeat.

    Verdict precedence:
      no result.json with any dimension_scores           -> AMBER no_evidence
      any complete scenario with health < G3_THRESHOLD   -> RED, scenario named
      no complete scenario (correctitud=None everywhere) -> AMBER insufficient_5dim
      some complete, some incomplete                     -> AMBER insufficient_5dim
      all complete and >= G3_THRESHOLD                   -> GREEN

    Diagnostic preserved:
      stability.json::health_score (if present) is reported as
      diagnostic_only with its formula cited. It is never used as
      release-readiness evidence.
    """
    # Aggregation policy (P1):
    #   - Each result.json is one repeat of one scenario.
    #   - Aggregation is per distinct scenario_id; per-scenario health
    #     is the mean of the 5-dim health values across repeats that
    #     have ALL FIVE dimensions populated.
    #   - If a scenario_id has at least one repeat with all 5 dims and
    #     one repeat missing at least one dim, the scenario is
    #     incomplete_5dim (cannot drive a verdict, but its repeats
    #     are counted for coverage).
    #   - The reader does NOT silently use only the first repeat.
    #   - Cross-run_dir: the same scenario_id is aggregated once even
    #     if it appears in multiple run_dirs (e.g. run-1, run-2).
    per_scenario: dict[str, dict] = {}    # scenario_id -> aggregation
    seen_scenario: set[str] = set()        # distinct scenario_ids
    total_candidates = 0

    for d in run_dirs:
        for rj_path in _discover_result_jsons(d):
            try:
                with open(rj_path) as f:
                    r = json.load(f)
            except Exception:
                continue
            sid = r.get("scenario_id") or ""
            if not sid:
                continue
            total_candidates += 1
            seen_scenario.add(sid)

            ds = r.get("dimension_scores") or {}
            if not ds:
                per_scenario.setdefault(sid, {
                    "complete_healths": [],
                    "has_no_evidence": True,
                    "has_incomplete": False,
                })
                per_scenario[sid]["has_no_evidence"] = True
                continue

            values: list[float] = []
            for k in G3_DIM_NAMES:
                v = ds.get(k)
                if v is None:
                    break
                values.append(float(v))
            else:
                h = sum(v * w for v, w in zip(values, G3_HEALTH_WEIGHTS))
                per_scenario.setdefault(sid, {
                    "complete_healths": [],
                    "has_no_evidence": False,
                    "has_incomplete": False,
                })
                per_scenario[sid]["complete_healths"].append(h)
                continue
            # This repeat has some dims but not all 5.
            per_scenario.setdefault(sid, {
                "complete_healths": [],
                "has_no_evidence": False,
                "has_incomplete": False,
            })
            per_scenario[sid]["has_incomplete"] = True

    # Build the rows (per-scenario) and the incomplete_scenarios list
    # from the per_scenario aggregation.
    rows: list[dict] = []
    incomplete_scenarios: list[str] = []
    no_evidence_scenarios: list[str] = []
    for sid, agg in per_scenario.items():
        if agg["complete_healths"] and not agg["has_incomplete"] and not agg["has_no_evidence"]:
            # Clean: all repeats have full 5 dims.
            avg = sum(agg["complete_healths"]) / len(agg["complete_healths"])
            rows.append({"scenario_id": sid, "health": avg, "n_repeats": len(agg["complete_healths"])})
        elif agg["complete_healths"] and (agg["has_incomplete"] or agg["has_no_evidence"]):
            # Mixed: some repeats complete, others incomplete.
            # Mark as incomplete_5dim; do NOT compute a partial health.
            incomplete_scenarios.append(sid)
        else:
            # No complete repeats at all.
            incomplete_scenarios.append(sid)

    # Diagnostic: load stability.json::health_score to preserve the
    # analyze_stability formula's value as diagnostic_only.
    diag_value: float | None = None
    diag_formula = ""
    if stability_path:
        stab = load_stability(stability_path)
        if stab.get("health_score") is not None:
            diag_value = float(stab["health_score"])
            diag_formula = (
                "min(100, pass_rate*100*0.5 + 95*0.3 + 95*0.2); "
                "the two 95s are placeholder constants, not measurements"
            )

    # Verdict precedence.
    if total_candidates == 0 or (not rows and not incomplete_scenarios and not no_evidence_scenarios):
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="AMBER",
            evidence_text=(
                "no_evidence: no result.json files with dimension_scores "
                "in any run_dir; cannot compute 5-dim health "
                "(spec: sandbox_core::scoring::compute_health_score)"
            ),
            evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
        )

    if not rows:
        # No scenario has all 5 dims. Verdict is AMBER.
        reasons = []
        if incomplete_scenarios:
            reasons.append(
                f"insufficient_5dim: {len(incomplete_scenarios)} scenarios "
                f"missing one or more of {list(G3_DIM_NAMES)} (typically "
                f"correctitud=None when no ground truth)"
            )
        if no_evidence_scenarios:
            reasons.append(
                f"no_evidence: {len(no_evidence_scenarios)} scenarios "
                f"with no dimension_scores at all"
            )
        if diag_value is not None:
            reasons.append(
                f"diagnostic_only: stability.json::health_score={diag_value:.1f} "
                f"({diag_formula}); not counted toward G3"
            )
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="AMBER",
            evidence_text="; ".join(reasons),
            evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
        )

    # Some scenarios have all 5 dims.
    failing = [r for r in rows if r["health"] < G3_THRESHOLD]
    passing = [r for r in rows if r["health"] >= G3_THRESHOLD]
    avg_health = sum(r["health"] for r in rows) / len(rows)
    min_health = min(r["health"] for r in rows)
    max_health = max(r["health"] for r in rows)

    if failing:
        worst = min(failing, key=lambda r: r["health"])
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="RED",
            measured=round(worst["health"], 1),
            budget=G3_THRESHOLD,
            evidence_text=(
                f"5-dim health (sandbox_core::scoring formula, weights={list(G3_HEALTH_WEIGHTS)}) "
                f"below threshold on {len(failing)} scenario(s); "
                f"worst: {worst['scenario_id']!r} at {worst['health']:.1f}; "
                f"complete_5dim={len(rows)}/{len(rows)+len(incomplete_scenarios)}, "
                f"incomplete_5dim={len(incomplete_scenarios)}, "
                f"min={min_health:.1f}, max={max_health:.1f}, avg={avg_health:.1f}"
                + (f"; diagnostic_only: stability.json::health_score={diag_value:.1f}"
                   + (f" ({diag_formula})" if diag_formula else "")
                   if diag_value is not None else "")
            ),
            evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
        )

    if incomplete_scenarios:
        # All complete scenarios are >= 85, but some scenarios are
        # incomplete. Spec requires per-scenario 5-dim health across
        # the corpus — incomplete coverage means the verdict cannot
        # be GREEN.
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="AMBER",
            measured=round(avg_health, 1),
            budget=G3_THRESHOLD,
            evidence_text=(
                f"all {len(rows)} complete 5-dim scenarios >= {G3_THRESHOLD:.0f}, "
                f"avg={avg_health:.1f}; but {len(incomplete_scenarios)} scenarios "
                f"have insufficient 5-dim coverage (correctitud=None or other "
                f"dims missing); insufficient_5dim prevents GREEN"
                + (f"; diagnostic_only: stability.json::health_score={diag_value:.1f}"
                   if diag_value is not None else "")
            ),
            evidence_path=",".join(run_dirs + ([stability_path] if stability_path else [])),
        )

    # All complete and all >= 85.
    return GateResult(
        id="G3", name="Sandbox Health Score",
        status="GREEN",
        measured=round(avg_health, 1),
        budget=G3_THRESHOLD,
        evidence_text=(
            f"all {len(rows)} scenarios with complete 5-dim coverage have "
            f"health (sandbox_core::scoring formula, weights={list(G3_HEALTH_WEIGHTS)}) "
            f">= {G3_THRESHOLD:.0f}; min={min_health:.1f}, max={max_health:.1f}, "
            f"avg={avg_health:.1f}"
            + (f"; diagnostic_only: stability.json::health_score={diag_value:.1f}"
               + (f" ({diag_formula})" if diag_formula else "")
               if diag_value is not None else "")
        ),
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
    """G6: Run-to-run stability (per-scenario timing CV < 10%).

    Contract (openspec/specs/release-readiness-gate/spec.md):
      Run-to-run variance (via stability.json from --repeat >= 3)
      MUST be < 10% per dimension.

    Provenance / sufficiency policy (A1b.2):
      A stability measurement is a release-readiness signal only when
      BOTH preconditions hold:
        (a) campaign-level repeat_count >= G6_MIN_REPEATS (3 per spec);
        (b) per-scenario runs >= G6_MIN_SAMPLES_PER_SCENARIO (3).
      If either fails, the gate is AMBER with explicit reason.
      The per-scenario number is the smallest unit that supports a
      run-to-run variance claim.

    Warm-cache vs full CV (R-G6-3 / R-G6-8):
      analyze_stability.py emits cv_warm by dropping the max sample
      whenever n >= 3, and marks cold_cache_sample=true only when
      the dropped sample is >= 1.5x the warm mean and > 100ms.
      When n < 3, cv_warm is bit-identical to cv because the
      drop cannot be applied. The reader must NOT advertise such a
      value as a warm-cache measurement: the evidence must mark it
      as "warm-cache N/A (n<3)" so the user knows the policy was
      not applied.

    Aggregate CV (R-G6-7):
      stability.json carries a top-level timing_cv across all
      scenarios. When it disagrees materially with the per-scenario
      max, the reader cites it as a second opinion in the evidence
      text. It does NOT drive the verdict by itself (the spec is
      per-scenario), but it is preserved as diagnostic data.

    Pre-D1 compatibility:
      stability.json files from pre-D1 campaigns may not carry
      scenario_stats. The reader falls back to families_runtorun /
      families for the per-family cv mean. The verdict precedence
      above applies the same way.
    """
    stab = load_stability(stability_path)
    if not stab:
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER",
            evidence_text=(
                "stability.json not found or unreadable; "
                "no run-to-run variance measurement available"
            ),
            evidence_path=stability_path or "stability.json",
        )

    # Precondition (a): campaign-level repeat_count >= G6_MIN_REPEATS.
    repeat_count = stab.get("repeat_count")
    campaign_id = stab.get("campaign_id") or ""
    measured_source_head = stab.get("measured_source_head") or ""
    parent_dir = stab.get("parent_dir") or ""
    scenario_stats = stab.get("scenario_stats") or []

    if repeat_count is None or repeat_count < G6_MIN_REPEATS:
        observed = repeat_count if repeat_count is not None else "unknown"
        # Even when the campaign is insufficient, surface the worst
        # observed per-scenario CV as diagnostic data so the operator
        # can see which scenario would have driven the verdict if
        # the campaign had met the precondition.
        diag_max = ""
        diag_scenario = ""
        if scenario_stats:
            best = max(
                (
                    (float((s.get("timing") or {}).get("cv_warm") or
                           (s.get("timing") or {}).get("cv") or 0),
                     s.get("scenario_id") or "<unknown>",
                     (s.get("timing") or {}).get("cold_cache_sample"))
                    for s in scenario_stats
                    if ((s.get("timing") or {}).get("cv_warm") is not None
                        or (s.get("timing") or {}).get("cv") is not None)
                ),
                default=(0.0, "", False),
                key=lambda t: t[0],
            )
            if best[1]:
                diag_max = f"diagnostic_max_cv={best[0]:.4f}"
                diag_scenario = (
                    f"diagnostic_scenario={best[1]!r} "
                    f"(cold_cache_sample={best[2]})"
                )
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER",
            evidence_text=(
                f"insufficient_repeats: repeat_count={observed} < "
                f"spec minimum {G6_MIN_REPEATS} (--repeat >= "
                f"{G6_MIN_REPEATS} required for release-stability "
                f"measurement per openspec/specs/release-readiness-gate/spec.md); "
                f"campaign_id={campaign_id or 'unknown'}, "
                f"measured_source_head={measured_source_head or 'unknown'}, "
                f"parent_dir={parent_dir or 'unknown'}; "
                f"{diag_max}; {diag_scenario}"
            ),
            evidence_path=stability_path,
        )
    rows: list[dict] = []  # one entry per scenario that has runs
    insufficient_sample_scenarios: list[str] = []
    empty_timing_scenarios: list[str] = []
    warm_applied_count = 0
    warm_na_count = 0

    for s in scenario_stats:
        sid = s.get("scenario_id") or "<unknown>"
        timing = s.get("timing") or {}
        runs = s.get("runs") or 0
        cv = timing.get("cv")
        cv_warm = timing.get("cv_warm")
        cold_detected = bool(timing.get("cold_cache_sample"))

        if runs == 0:
            empty_timing_scenarios.append(sid)
            continue
        if runs < G6_MIN_SAMPLES_PER_SCENARIO:
            insufficient_sample_scenarios.append(sid)
            # Still record the cv for diagnostic, but it cannot drive
            # a release verdict. Mark it as warm-cache N/A explicitly.
            if cv is not None:
                rows.append({
                    "scenario_id": sid,
                    "runs": runs,
                    "cv": float(cv),
                    "cv_warm": float(cv_warm) if cv_warm is not None else float(cv),
                    "cold_detected": cold_detected,
                    "warm_cache_applicable": False,
                    "warm_cache_na_reason": f"n={runs}<{G6_MIN_SAMPLES_PER_SCENARIO}",
                })
                warm_na_count += 1
            continue

        # n >= 3: warm-cache policy applies.
        warm_applied_count += 1
        rows.append({
            "scenario_id": sid,
            "runs": runs,
            "cv": float(cv) if cv is not None else None,
            "cv_warm": float(cv_warm) if cv_warm is not None else float(cv) if cv is not None else 0.0,
            "cold_detected": cold_detected,
            "warm_cache_applicable": True,
            "warm_cache_na_reason": None,
        })

    # Source 2 (pre-D1 fallback): families_runtorun / families.
    fams = stab.get("families_runtorun") or stab.get("families") or {}
    used_pre_d1_families = False
    if not rows and fams:
        used_pre_d1_families = True
        for fam_name, f in fams.items():
            v = f.get("mean_cv_warm") if f.get("mean_cv_warm") is not None else f.get("mean_cv")
            if v is not None:
                # Pre-D1 family-level mean: treat as per-scenario-like.
                rows.append({
                    "scenario_id": f"family:{fam_name}",
                    "runs": f.get("samples") or G6_MIN_SAMPLES_PER_SCENARIO,
                    "cv": float(v),
                    "cv_warm": float(v),
                    "cold_detected": False,
                    "warm_cache_applicable": False,
                    "warm_cache_na_reason": "pre-D1 family-level mean",
                })

    # Verdict precedence.
    eligible = [r for r in rows if r["warm_cache_applicable"]]
    diagnostic_only = [r for r in rows if not r["warm_cache_applicable"]]

    # If NO eligible scenario has warm-cache applied, the gate cannot
    # produce a release verdict. AMBER.
    if not eligible and (insufficient_sample_scenarios or empty_timing_scenarios or diagnostic_only):
        reasons: list[str] = []
        if insufficient_sample_scenarios:
            # P3: name the insufficient scenarios so the operator sees
            # which ones cannot drive the verdict.
            named = insufficient_sample_scenarios[:10]
            more = "" if len(insufficient_sample_scenarios) <= 10 else \
                f" (+{len(insufficient_sample_scenarios) - 10} more)"
            reasons.append(
                f"insufficient_samples: {len(insufficient_sample_scenarios)} "
                f"scenarios with runs<{G6_MIN_SAMPLES_PER_SCENARIO}: "
                f"{', '.join(repr(n) for n in named)}{more}"
            )
        if empty_timing_scenarios:
            reasons.append(
                f"no_evidence: {len(empty_timing_scenarios)} "
                f"scenarios with 0 samples"
            )
        # Note: the diagnostic-only rows (n<3 with non-zero cv) are
        # reported as diagnostic. They do NOT drive the verdict.
        diag_cv_max = (
            max(r["cv"] for r in diagnostic_only if r["cv"] is not None)
            if diagnostic_only else 0.0
        )
        if diag_cv_max > 0:
            reasons.append(
                f"diagnostic_max_cv={diag_cv_max:.4f} "
                f"(warm-cache N/A, cannot drive verdict)"
            )
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER",
            evidence_text="; ".join(reasons),
            evidence_path=stability_path,
        )

    if not eligible:
        # Truly no measurements at all.
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER",
            evidence_text=(
                "no per-scenario timing CVs in stability.json "
                "(need scenario_stats[].timing.cv_warm with "
                f"runs>={G6_MIN_SAMPLES_PER_SCENARIO} per scenario)"
            ),
            evidence_path=stability_path,
        )

    # Eligible scenarios exist: per-scenario verdict.
    failing: list[dict] = []
    passing: list[dict] = []
    for r in eligible:
        cv_eff = r["cv_warm"]
        if cv_eff is None:
            continue
        if cv_eff >= G6_CV_THRESHOLD:
            failing.append(r)
        else:
            passing.append(r)

    # Aggregate CV second-opinion.
    agg_cv = stab.get("timing_cv")
    agg_cv_str = ""
    if agg_cv is not None and float(agg_cv) >= G6_CV_THRESHOLD:
        agg_cv_str = (
            f"; aggregate_cv={float(agg_cv):.4f} "
            f"(second-opinion: aggregate across all scenarios above "
            f"threshold)"
        )
    elif agg_cv is not None:
        agg_cv_str = f"; aggregate_cv={float(agg_cv):.4f}"

    if failing:
        max_row = max(failing, key=lambda r: r["cv_warm"] or 0.0)
        max_cv = max_row["cv_warm"]
        # P3: name the insufficient scenarios even when RED — they
        # are part of the campaign and the operator must see them.
        insufficient_str = ""
        if insufficient_sample_scenarios:
            named = insufficient_sample_scenarios[:10]
            more = "" if len(insufficient_sample_scenarios) <= 10 else \
                f" (+{len(insufficient_sample_scenarios) - 10} more)"
            insufficient_str = (
                f"; insufficient_samples={len(insufficient_sample_scenarios)} "
                f"({', '.join(repr(n) for n in named)}{more})"
            )
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="RED",
            measured=f"{max_cv*100:.1f}%",
            budget=f"<{G6_CV_THRESHOLD*100:.0f}%",
            evidence_text=(
                f"per-scenario warm-cache CV >= {G6_CV_THRESHOLD:.2f}; "
                f"max={max_cv:.4f} on scenario "
                f"{max_row['scenario_id']!r} (n={max_row['runs']}, "
                f"cold_cache_sample={max_row['cold_detected']}); "
                f"failing_scenarios={len(failing)}/{len(eligible)}; "
                f"campaign_id={campaign_id or 'unknown'}, "
                f"measured_source_head={measured_source_head or 'unknown'}, "
                f"repeat_count={repeat_count}"
                f"{agg_cv_str}{insufficient_str}"
            ),
            evidence_path=stability_path,
        )

    # All passing.
    # P3: even when all eligible scenarios pass, surface the
    # insufficient ones so they are not silently ignored.
    insufficient_str = ""
    if insufficient_sample_scenarios:
        named = insufficient_sample_scenarios[:10]
        more = "" if len(insufficient_sample_scenarios) <= 10 else \
            f" (+{len(insufficient_sample_scenarios) - 10} more)"
        insufficient_str = (
            f"; insufficient_samples={len(insufficient_sample_scenarios)} "
            f"({', '.join(repr(n) for n in named)}{more})"
        )
    return GateResult(
        id="G6", name="Run-to-Run Stability",
        status="GREEN",
        measured=f"max {max((r['cv_warm'] or 0) for r in eligible)*100:.1f}%",
        budget=f"<{G6_CV_THRESHOLD*100:.0f}%",
        evidence_text=(
            f"all {len(eligible)} scenarios with sufficient samples "
            f"(n>={G6_MIN_SAMPLES_PER_SCENARIO}) have warm-cache "
            f"CV < {G6_CV_THRESHOLD:.2f}; max="
            f"{max((r['cv_warm'] or 0) for r in eligible):.4f}; "
            f"cold_cache_drops_applied={warm_applied_count}; "
            f"warm_cache_na_count={warm_na_count}; "
            f"campaign_id={campaign_id or 'unknown'}, "
            f"measured_source_head={measured_source_head or 'unknown'}, "
            f"repeat_count={repeat_count}"
            f"{agg_cv_str}{insufficient_str}"
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
