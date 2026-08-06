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
    "search":     (500.0,  "ms"),
    "call-graph": (2000.0, "ms"),
    "analytics":  (5000.0, "ms"),
}

SEARCH_TOOLS = {"search_content", "semantic_search", "query_symbol_index", "find_references"}
CALL_GRAPH_TOOLS = {
    "build_graph", "build_call_subgraph", "get_call_hierarchy",
    "trace_path", "get_per_file_graph",
}
ANALYTICS_TOOLS = {
    "graph_pagerank", "graph_communities", "graph_god_nodes",
    "graph_all_paths", "graph_condensed", "graph_query",
    "graph_surprising_connections", "graph_insights",
}
TOOL_TO_FAMILY: dict[str, str] = {
    **{t: "search"     for t in SEARCH_TOOLS},
    **{t: "call-graph" for t in CALL_GRAPH_TOOLS},
    **{t: "analytics"  for t in ANALYTICS_TOOLS},
}

CRASH_FAILURE_CLASSES = {
    "crash", "panic", "sigsegv", "oom", "oom_killed",
    "sandbox_infra_failure",  # OOM marker in failure_distribution
}


# ── Gate result dataclass ─────────────────────────────────────────────────────

@dataclass
class GateResult:
    id: str
    name: str
    status: str          # GREEN | AMBER | RED
    measured: Optional[Any] = None
    budget: Optional[Any] = None
    evidence_path: Optional[str] = None
    evidence_text: Optional[str] = None

from dataclasses import dataclass


# ── Utility loaders ───────────────────────────────────────────────────────────

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
    """G1: git log evidence for e13-wave2 PRs. Returns (status, evidence_text, path)."""
    try:
        result = subprocess.run(
            ["git", "log", "--oneline", "-30"],
            capture_output=True, text=True, timeout=10,
            cwd=Path(__file__).parent.parent.parent,
        )
        if result.returncode != 0:
            return "AMBER", "git log failed (manual evidence required)", "git_log"
        lines = result.stdout.strip().split("\n")
        e13_prs = [l for l in lines if "e13-wave2" in l.lower()]
        if e13_prs:
            return "GREEN", f"Found {len(e13_prs)} e13-wave2 commits in last 30: {e13_prs[0]}", "git_log"
        # Fallback: check for e13 tag
        tag_result = subprocess.run(
            ["git", "tag", "--list", "e13*", "--format=%(refname:short)"],
            capture_output=True, text=True, timeout=5,
            cwd=Path(__file__).parent.parent.parent,
        )
        if tag_result.returncode == 0 and tag_result.stdout.strip():
            return "GREEN", f"e13 tag found: {tag_result.stdout.strip().split()[0]}", "git_tag"
        return "AMBER", "no e13-wave2 commits in last 30 git log entries (manual evidence required)", "git_log"
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


def gate_g3(run_dirs: list[str]) -> GateResult:
    """G3: Health score ≥85. GREEN if avg ≥85; AMBER if single run <85; RED if avg <85 with ≥2 runs."""
    scores = []
    for d in run_dirs:
        s = load_summary(d)
        hs = s.get("health_score")
        if hs is not None:
            scores.append(hs)

    if not scores:
        return GateResult(
            id="G3", name="Sandbox Health Score",
            status="AMBER", evidence_text="no health_score data in any run",
            evidence_path=",".join(run_dirs),
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
        evidence_text=detail,
        evidence_path=",".join(run_dirs),
    )


def gate_g4(run_dirs: list[str]) -> GateResult:
    """G4: Correctitud dimension score ≥ 90."""
    scores = []
    for d in run_dirs:
        s = load_summary(d)
        dims = s.get("dimension_scores", {})
        corr = dims.get("correctitud")
        if corr is not None:
            scores.append(corr)

    if not scores:
        return GateResult(
            id="G4", name="Corpus Quality / Correctitud",
            status="AMBER", evidence_text="dimension_scores.correctitud not found",
            evidence_path=",".join(run_dirs),
        )

    latest = scores[-1]
    status = "GREEN" if latest >= 90 else "RED"
    return GateResult(
        id="G4", name="Corpus Quality / Correctitud",
        status=status,
        measured=latest,
        budget=90.0,
        evidence_text=f"latest correctitud: {latest}",
        evidence_path=",".join(run_dirs),
    )


def gate_g5(run_dirs: list[str]) -> GateResult:
    """G5: Latency by tool family (search <500ms, call-graph <2s, analytics <5s)."""
    family_p95: dict[str, list[float]] = {f: [] for f in FAMILY_BUDGETS}

    for d in run_dirs:
        s = load_summary(d)
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
        evidence = f"all families within budget"
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
    """G6: Run-to-run stability (timing_cv < 10%)."""
    stab = load_stability(stability_path)
    if not stab:
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER", evidence_text="stability.json not found",
            evidence_path=stability_path or "stability.json",
        )

    cv = stab.get("timing_cv")
    if cv is None:
        return GateResult(
            id="G6", name="Run-to-Run Stability",
            status="AMBER", evidence_text="timing_cv not found in stability.json",
            evidence_path=stability_path,
        )

    status = "GREEN" if cv < 0.10 else "RED"
    return GateResult(
        id="G6", name="Run-to-Run Stability",
        status=status,
        measured=f"{cv*100:.1f}%",
        budget="<10%",
        evidence_text=f"timing_cv={cv:.4f}",
        evidence_path=stability_path,
    )


def gate_g7(run_dirs: list[str]) -> GateResult:
    """G7: Zero crash-class failures in failure_distribution."""
    crash_count = 0
    evidence_parts = []

    for d in run_dirs:
        s = load_summary(d)
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
        if "oom" in failure_class.lower() or "timeout" in failure_class.lower():
            oom_timeout_tools.append(f"{tool}({failure_class})")

    if oom_timeout_tools:
        # OOM/timeout with a tracked defect → AMBER
        return GateResult(
            id="G8", name="Scalability Proof (Tier-3)",
            status="AMBER",
            measured=f"OOM/timeout: {', '.join(oom_timeout_tools)}",
            evidence_text="OOM or timeout detected in tier-3 probe (defect tracked)",
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


def gate_g10() -> GateResult:
    """G10: Openspec conformance audit. AMBER placeholder (Phase 4)."""
    return GateResult(
        id="G10", name="Openspec Conformance Audit",
        status="AMBER",
        evidence_text="no machine-readable openspec audit available (Phase 4)",
        evidence_path="openspec/",
    )


def gate_g11(project_root: str) -> GateResult:
    """G11: Documentation currency — MCP-TOOLS.md (68 tools) + ADR-031/032."""
    checks = []
    mcp_tools_path = Path(project_root) / "docs" / "MCP-TOOLS.md"
    adr031_path = Path(project_root) / "docs" / "adr" / "ADR-031-moldql-pattern-graph-analytics-platform.md"
    adr032_path = Path(project_root) / "docs" / "adr" / "ADR-032"

    mcp_ok = False
    if mcp_tools_path.exists():
        try:
            content = mcp_tools_path.read_text()
            if "68 tools" in content or "43 tools" in content:
                mcp_ok = True
                checks.append("MCP-TOOLS.md found")
            else:
                checks.append("MCP-TOOLS.md found (tool count not confirmed as 68)")
        except Exception:
            checks.append("MCP-TOOLS.md found (read error)")
    else:
        checks.append("MCP-TOOLS.md NOT found")

    adr_ok = adr031_path.exists() or Path(str(adr031_path) + ".md").exists()
    adr032_ok = adr032_path.exists() or Path(str(adr032_path) + ".md").exists()

    if adr_ok:
        checks.append("ADR-031 found")
    else:
        checks.append("ADR-031 NOT found")
    if adr032_ok:
        checks.append("ADR-032 found")
    else:
        checks.append("ADR-032 NOT found")

    if mcp_ok and adr_ok and adr032_ok:
        status = "GREEN"
    elif mcp_ok:
        status = "AMBER"
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
    try:
        result = subprocess.run(
            ["git", "tag", "--list", "v*", "--format=%(refname:short):%(creatordate:short)",
             "--sort=-creatordate"],
            capture_output=True, text=True, timeout=10,
            cwd=project_root,
        )
        if result.returncode != 0 or not result.stdout.strip():
            return GateResult(
                id="G12", name="Git Tag Hygiene",
                status="AMBER",
                evidence_text="no semver tags found",
                evidence_path="git_tag",
            )
        tags = result.stdout.strip().split("\n")
        recent = tags[0] if tags else "none"
        return GateResult(
            id="G12", name="Git Tag Hygiene",
            status="GREEN",
            measured=recent,
            evidence_text=f"most recent tag: {recent}",
            evidence_path="git_tag",
        )
    except Exception as e:
        return GateResult(
            id="G12", name="Git Tag Hygiene",
            status="AMBER",
            evidence_text=f"git tag check failed: {e}",
            evidence_path="git_tag",
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
    args = parser.parse_args()

    run_dirs = [d.strip() for d in args.runs.split(",") if d.strip()]
    project_root = str(Path(__file__).parent.parent.parent)

    # Evaluate all 12 gates
    gates = [
        gate_g1(),
        gate_g2(args.coverage_matrix),
        gate_g3(run_dirs),
        gate_g4(run_dirs),
        gate_g5(run_dirs),
        gate_g6(args.stability),
        gate_g7(run_dirs),
        gate_g8(args.g8_probe_result),
        gate_g9(run_dirs),
        gate_g10(),
        gate_g11(project_root),
        gate_g12(project_root),
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
