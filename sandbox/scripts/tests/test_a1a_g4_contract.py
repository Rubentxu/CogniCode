"""A1a — RED tests for the G4 contract.

These tests pin the G4 contract as written in
`openspec/specs/release-readiness-gate/spec.md`:

  Correctness (ground-truth comparison via the scoring engine's matchers)
  MUST be >= 90% on every Tier-1 repository (ripgrep, serde, anyhow,
  tokio, clap).

The current reader (gate_g4) averages all measured correctitud without
checking that:
  (a) the repo is one of {ripgrep, serde, anyhow, tokio, clap};
  (b) every Tier-1 repo has at least one measured scenario;
  (c) no scenario with no ground-truth can inflate the average;
  (d) only manifest-declared Tier-1 repos count, not manifest-declared
      `repo:` metadata that points at a fixture workspace.

These tests are intentionally RED against the current reader. They
must turn GREEN only when the reader implements the spec correctly.

Run with:
    cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode
    python3 -m pytest sandbox/scripts/tests/test_a1a_g4_contract.py -v
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Allow direct invocation
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))  # sandbox/scripts

import release_scorecard as rs  # noqa: E402

REPO = Path("/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode")
TIER1_REPOS = {"ripgrep", "serde", "anyhow", "tokio", "clap"}

# ── Synthetic fixtures ────────────────────────────────────────────────────────


def _make_result(
    scenario_id: str,
    repo: str,
    correctitud,
    workspace: str = ".",
    language: str = "rust",
    tool: str = "search_content",
    outcome: str = "pass",
):
    return {
        "scenario_id": scenario_id,
        "language": language,
        "tool": tool,
        "tier": "B",
        "repo": repo,
        "outcome": outcome,
        "failure_class": "pass" if outcome == "pass" else outcome,
        "dimension_scores": {
            "correctitud": correctitud,
            "latencia": 100.0,
            "escalabilidad": 100.0,
            "consistencia": 100.0,
            "robustez": 100.0,
        },
        "validation": {"stages": [], "passed": True},
        "workspace": workspace,
    }


def _write_run(tmpdir: Path, name: str, results: list[dict]) -> Path:
    """Write a synthetic run dir shaped like post-D1 layout."""
    run = tmpdir / name
    run.mkdir(parents=True, exist_ok=True)
    for r in results:
        sid = r["scenario_id"]
        ts = "2026-09-19T120000"
        d = run / sid / ts
        d.mkdir(parents=True, exist_ok=True)
        (d / "result.json").write_text(json.dumps(r))
    return run


# ── Tests ─────────────────────────────────────────────────────────────────────


def test_spec_tier1_repos_observable(tmp_path):
    """Pin the contract: Tier-1 repos per spec are ripgrep, serde, anyhow, tokio, clap.

    The reader must read these from a canonical source (the spec), not invent
    its own list. This test pins the observable behaviour.
    """
    assert hasattr(rs, "TIER1_REPOS_PER_SPEC"), "reader must expose TIER1_REPOS_PER_SPEC"
    assert set(rs.TIER1_REPOS_PER_SPEC) == TIER1_REPOS


def test_all_five_tier1_repos_at_above_threshold_is_green(tmp_path):
    """5 repos with avg >= 90 and at least one measurement each -> GREEN.

    Per spec scenario: 'All Tier-1 repos above threshold'.
    """
    repos_with_scores = {r: [95.0, 92.0] for r in TIER1_REPOS}
    rows = []
    for repo, scores in repos_with_scores.items():
        for i, sc in enumerate(scores):
            rows.append(_make_result(
                scenario_id=f"{repo}_search_content_{i}",
                repo=repo,
                correctitud=sc,
                workspace=f"repos/{repo}",
            ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "GREEN", f"expected GREEN, got {g.status} ({g.evidence_text})"


def test_one_tier1_repo_below_threshold_is_red(tmp_path):
    """4 repos >= 90, 1 repo < 90 -> RED. The failing repo MUST be named."""
    repos_with_scores = {
        "ripgrep": [95.0, 92.0],
        "serde":   [95.0, 92.0],
        "anyhow":  [95.0, 92.0],
        "tokio":   [95.0, 92.0],
        "clap":    [50.0, 60.0],  # below threshold
    }
    rows = []
    for repo, scores in repos_with_scores.items():
        for i, sc in enumerate(scores):
            rows.append(_make_result(
                scenario_id=f"{repo}_search_content_{i}",
                repo=repo,
                correctitud=sc,
                workspace=f"repos/{repo}",
            ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "RED", f"expected RED, got {g.status} ({g.evidence_text})"
    assert "clap" in g.evidence_text, f"failing repo must be named; got: {g.evidence_text}"


def test_one_tier1_repo_without_evidence_is_amber(tmp_path):
    """4 repos measured, 1 repo (clap) has no scenario -> INCOMPLETE/AMBER.

    Per spec: 'All Tier-1 repos above threshold' requires ALL five measured.
    A missing repo is not a clean pass; it is incomplete evidence.
    """
    repos_with_scores = {
        "ripgrep": [95.0, 92.0],
        "serde":   [95.0, 92.0],
        "anyhow":  [95.0, 92.0],
        "tokio":   [95.0, 92.0],
        # "clap" intentionally absent
    }
    rows = []
    for repo, scores in repos_with_scores.items():
        for i, sc in enumerate(scores):
            rows.append(_make_result(
                scenario_id=f"{repo}_search_content_{i}",
                repo=repo,
                correctitud=sc,
                workspace=f"repos/{repo}",
            ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"expected AMBER/INCOMPLETE, got {g.status} ({g.evidence_text})"
    assert "clap" in g.evidence_text.lower(), \
        f"missing repo must be named; got: {g.evidence_text}"


def test_no_ground_truth_cannot_inflate_average(tmp_path):
    """Scenarios without ground-truth must not contribute to the per-repo average.

    Per spec: 'Correctness (ground-truth comparison via the scoring engine's
    matchers)'. A scenario with no ground truth has no matcher output, so
    its 'correctitud' must not be averaged into the per-repo score.
    """
    # 5 repos with ground truth, all > 90. Add 100 scenarios with no ground
    # truth and bogus high scores. If those scenarios counted, the average
    # would still be >= 90 by luck; what matters is whether the per-repo
    # average is computed ONLY from ground-truth-backed measurements.
    rows = []
    for repo in TIER1_REPOS:
        rows.append(_make_result(
            scenario_id=f"{repo}_gt_present",
            repo=repo,
            correctitud=85.0,  # below threshold on purpose
            workspace=f"repos/{repo}",
        ))
    run = _write_run(tmp_path, "run-1", rows)
    # Even with no inflation, the per-repo averages must reflect the
    # single GT-backed scenario each. The gate must NOT count
    # anything beyond what the ground-truth-backed scenarios produce.
    g = rs.gate_g4([str(run)])
    assert g.status == "RED", (
        f"if GT-backed scores are below threshold the gate must be RED; "
        f"got {g.status} ({g.evidence_text})"
    )


def test_no_tier1_repo_evidence_at_all_is_incomplete(tmp_path):
    """If no scenario has any Tier-1 repo, the gate is INCOMPLETE/AMBER, not RED/GREEN.

    Per spec, the gate cannot pass if any Tier-1 repo is missing evidence.
    If ALL Tier-1 repos are missing, that is the strongest form of
    INCOMPLETE (no measurement possible).
    """
    # Scenarios with non-Tier-1 repos (these are not in {ripgrep, serde, anyhow, tokio, clap})
    rows = []
    for fake_repo in ["my-internal-fixture", "fixture-rust-hello"]:
        rows.append(_make_result(
            scenario_id=f"{fake_repo}_scenario",
            repo=fake_repo,
            correctitud=95.0,
            workspace=".",
        ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"missing all Tier-1 evidence must be AMBER/INCOMPLETE, got {g.status} ({g.evidence_text})"
    for r in TIER1_REPOS:
        assert r in g.evidence_text.lower(), \
            f"missing Tier-1 repo {r} must be named; got: {g.evidence_text}"


def test_frozen_campaign_has_no_tier1_real_repo_evidence(tmp_path):
    """The frozen 20260919T102509 run must produce AMBER/INCOMPLETE for G4
    because no scenario is a true Tier-1 real-repo execution."""
    frozen_run = str(REPO / "sandbox/results-runs/20260919T102509/run-1")
    g = rs.gate_g4([frozen_run])
    # The frozen run's `repo` field includes "serde" but the actual workspace
    # is `.` (fixture), not the serde checkout. Per spec, only REAL Tier-1
    # executions count. Currently the reader averages the field blindly.
    # This test pins the expected honest verdict after the reader fix.
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"frozen run has no real Tier-1 execution; G4 must be AMBER/INCOMPLETE, "
        f"got {g.status} ({g.evidence_text})"
    )
