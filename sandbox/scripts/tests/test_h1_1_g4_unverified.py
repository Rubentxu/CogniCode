"""H1.1 — G4 reader defect: `missing_or_unverified` over-counts read_file scenarios.

Directive: 'Un escenario con procedencia válida y correctitud no medida
no debe invalidar otro escenario medido y acreditado del mismo
repositorio.'

The pre-fix reader did this:

    if measured is None:
        unverified_repos.add(actual_short)   # ALWAYS adds
        continue

A repo with one search scenario @ 100 and one read_file scenario
(correctitud=null) would have:
  - measurements: {repo: {search_scenario: [100]}}    -> acredited
  - unverified_repos: {repo}                          -> BUT also unverified
  - missing_or_unverified (5/5 listed)
  - unverified=0 (the displayed counter says 0)

The contradiction: `acredited=5/5 unverified=0 missing_or_unverified=5/5`.

Fix: only mark a repo as unverified when it has NO measured scenarios
with correctitud. The repo's accreditation status and the
unverified_repos set must be coherent.

Run with:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_h1_1_g4_unverified.py -v
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))  # sandbox/scripts

import release_scorecard as rs  # noqa: E402

TIER1_REPOS = {"ripgrep", "serde", "anyhow", "tokio", "clap"}
G4_THRESHOLD = 90.0


def _make_result(
    scenario_id: str,
    repo: str,
    correctitud,
    workspace: str = ".",
    tool: str = "search_content",
    outcome: str = "pass",
    workspace_snapshot_id: str | None = "snap000",
    repeat_index: int = 0,
):
    dim = None
    if correctitud is not None:
        dim = {
            "correctitud": correctitud,
            "latencia": 100.0,
            "escalabilidad": 100.0,
            "consistencia": 100.0,
            "robustez": 100.0,
        }
    return {
        "scenario_id": scenario_id,
        "language": "rust",
        "tool": tool,
        "tier": "B",
        "repo": repo,
        "outcome": outcome,
        "failure_class": "pass" if outcome == "pass" else outcome,
        "dimension_scores": dim,
        "validation": {"stages": [], "passed": True},
        "workspace": workspace,
        "workspace_snapshot_id": workspace_snapshot_id,
        "repo_provenance": {
            "actual_repository_identity": repo,
            "actual_repository_revision": "abc1234",
            "actual_workspace": f"/tmp/scratch/{repo}",
            "workspace_relative_path": repo,
        },
        "ground_truth_present": True,
        "repeat_index": repeat_index,
    }


def _write_run(tmpdir: Path, name: str, results: list[dict]) -> Path:
    run = tmpdir / name
    run.mkdir(parents=True, exist_ok=True)
    for r in results:
        sid = r["scenario_id"]
        ts = "2026-09-19T120000"
        d = run / sid / ts
        d.mkdir(parents=True, exist_ok=True)
        (d / "result.json").write_text(json.dumps(r))
    return run


# ── RED tests: pin the contract ──────────────────────────────────────────────


def test_unverified_counter_matches_evidence_text(tmp_path):
    """The displayed `unverified=N` counter must match the actual number of
    repos in `unverified_repos` (which forms `missing_or_unverified` together
    with `TIER1_REPOS_PER_SPEC - acredited`).

    Reproduces the pilot5 contradiction:
      acredited=5/5, unverified=0, missing_or_unverified=5/5
    """
    rows = []
    for repo in TIER1_REPOS:
        # 1 search scenario with measured correctitud (the credit)
        rows.append(_make_result(
            scenario_id=f"{repo}_search",
            repo=repo,
            correctitud=100.0,
            tool="search_content",
        ))
        # 1 read_file scenario with no correctitud (smoke matcher only)
        rows.append(_make_result(
            scenario_id=f"{repo}_read",
            repo=repo,
            correctitud=None,
            tool="read_file",
        ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # All 5 repos have a measured search scenario -> all 5 acredited.
    # None of them should appear in missing_or_unverified because
    # they all have positive provenance AND measured correctitud.
    assert "acredited=5/5" in g.evidence_text, (
        f"all 5 repos should be acredited; got: {g.evidence_text}"
    )
    # The defect: `missing_or_unverified` was listing all 5 due to the
    # read_file scenarios marking each repo as unverified.
    assert "missing_or_unverified" not in g.evidence_text or \
        not any(r in g.evidence_text.split("missing_or_unverified: ")[-1] for r in TIER1_REPOS), (
        f"no Tier-1 repo should appear in missing_or_unverified when all "
        f"have a measured scenario; got: {g.evidence_text}"
    )
    # The displayed `unverified=N` counter (if shown) must be zero.
    if "unverified=" in g.evidence_text:
        seg = g.evidence_text.split("unverified=")[1].split(";", 1)[0].split(",", 1)[0]
        assert int(seg) == 0, (
            f"displayed unverified count must be 0 when all repos have measured "
            f"correctitud; got {seg}; full text: {g.evidence_text}"
        )


def test_repo_with_only_unmeasured_scenarios_still_unverified(tmp_path):
    """Regression guard for the over-correction: if a repo has ONLY read_file
    scenarios (no measured correctitud), it MUST still appear in
    missing_or_unverified.

    Per directive: 'Verificar con regresiones que un repositorio sin
    ninguna medición obligatoria de correctitud continúa figurando como
    incompleto.'
    """
    rows = []
    # 4 repos have measured search scenarios.
    for repo in TIER1_REPOS - {"clap"}:
        rows.append(_make_result(
            scenario_id=f"{repo}_search",
            repo=repo,
            correctitud=100.0,
            tool="search_content",
        ))
    # clap has ONLY a read_file scenario (no measured correctitud).
    rows.append(_make_result(
        scenario_id="clap_read",
        repo="clap",
        correctitud=None,
        tool="read_file",
    ))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # 4/5 acredited, clap missing -> AMBER with clap named.
    assert "acredited=4/5" in g.evidence_text, (
        f"only 4 repos have measured correctitud; got: {g.evidence_text}"
    )
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"clap missing correctitud must produce AMBER; got {g.status} "
        f"({g.evidence_text})"
    )
    assert "clap" in g.evidence_text.lower(), (
        f"clap must be named as missing/unverified; got: {g.evidence_text}"
    )


def test_mixed_repo_with_both_search_and_read_acredited(tmp_path):
    """A repo with both a measured search scenario and a read_file scenario
    must be credited (no missing_or_unverified flag) AND have its
    per-repo average computed only from the search scenario."""
    rows = [
        # serde: 1 search @ 95 + 1 read (correctitud=None)
        _make_result("serde_search", "serde", 95.0, tool="search_content"),
        _make_result("serde_read", "serde", None, tool="read_file"),
        # 4 other repos: 1 search each @ 95
        _make_result("ripgrep_search", "ripgrep", 95.0, tool="search_content"),
        _make_result("anyhow_search", "anyhow", 95.0, tool="search_content"),
        _make_result("tokio_search", "tokio", 95.0, tool="search_content"),
        _make_result("clap_search", "clap", 95.0, tool="search_content"),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # All 5 repos have measured search scenarios -> GREEN.
    assert g.status == "GREEN", (
        f"all 5 repos have measured correctitud -> GREEN; "
        f"got {g.status} ({g.evidence_text})"
    )
    # The read_file scenario for serde must NOT drag its average down.
    # serde per-repo avg = mean of [95] = 95.0
    # The evidence_text contains "serde: avg=95.0 (n_scenarios=1, threshold=90)"
    assert "serde: avg=95.0" in g.evidence_text, (
        f"serde average must be 95.0 (search only); got: {g.evidence_text}"
    )


def test_acredited_count_equals_measurement_repos(tmp_path):
    """Internal-coherence check: `acredited` (from measurements.keys())
    must equal `len(measurements)` — a sanity guard against any future
    reader drift.
    """
    from release_scorecard import gate_g4
    rows = [
        _make_result("a", "serde", 95.0, tool="search_content"),
        _make_result("b", "anyhow", 95.0, tool="search_content"),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = gate_g4([str(run)])
    # Only 2 of 5 Tier-1 repos have measured correctitud.
    assert "acredited=2/5" in g.evidence_text
    # And the verdict is AMBER (missing 3).
    assert g.status in ("AMBER", "INCOMPLETE")
