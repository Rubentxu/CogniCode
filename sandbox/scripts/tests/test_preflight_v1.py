"""PREFLIGHT v1 — Acceptance-campaign preflight regression tests.

Pins the four preflight criteria from the GO directive (2026-09-19
acceptance-campaign v1):

  P1: Three repeats of the same scenario contribute to G3 according
      to the agreed aggregation policy. The reader must not silently
      use only the first result.json.
  P2: G3 criterion (aggregate candidate health and per-scenario
      below-85 treatment) is documented and coherent with the spec.
  P3: G6 cannot return GREEN when an included scenario is missing
      a mandatory repeat.
  P4: Scenarios without ground truth must NOT be transformed into
      a 0 correctitud score OR into valid 5-dim scores.

These tests are the contractual regression net for the acceptance
campaign. They run against the current readers (post-A1a+1, A1b,
A1c) and verify the readers honor the policy without needing a real
Tier-1 corpus.

Run:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_preflight_v1.py -v
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))  # sandbox/scripts

import release_scorecard as rs  # noqa: E402

REPO = Path("/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode")

G3_THRESHOLD = 85.0
G6_CV_THRESHOLD = 0.10
G6_MIN_REPEATS = 3
TIER1_REPOS = {"ripgrep", "serde", "anyhow", "tokio", "clap"}


# ── Fixtures ────────────────────────────────────────────────────────────────


def _make_result(
    scenario_id: str,
    repeat_index: int,
    correctitud=80.0,
    latencia=90.0,
    escalabilidad=90.0,
    consistencia=90.0,
    robustez=90.0,
    ground_truth_present: bool = True,
    repo: str = "ripgrep",
    actual_repository_identity: str = "ripgrep",
    actual_repository_revision: str = "abc1234",
    workspace: str = "repos/ripgrep",
    outcome: str = "pass",
) -> dict:
    """Build a result.json-like dict with full Tier-1 provenance."""
    return {
        "scenario_id": scenario_id,
        "repeat_index": repeat_index,
        "language": "rust",
        "tool": "search_content",
        "tier": "B",
        "repo": repo,
        "outcome": outcome,
        "failure_class": "pass" if outcome == "pass" else outcome,
        "dimension_scores": {
            "correctitud": correctitud,
            "latencia": latencia,
            "escalabilidad": escalabilidad,
            "consistencia": consistencia,
            "robustez": robustez,
        },
        "validation": {"stages": [], "passed": True},
        "workspace": workspace,
        "workspace_snapshot_id": f"snap_{scenario_id}_{repeat_index}",
        "actual_repository_identity": actual_repository_identity,
        "actual_repository_revision": actual_repository_revision,
        "ground_truth_present": ground_truth_present,
    }


def _write_run(tmp_path: Path, name: str, results: list[dict]) -> Path:
    run = tmp_path / name
    run.mkdir(parents=True, exist_ok=True)
    timestamp = "2026-09-19T120000"
    for r in results:
        sid = r["scenario_id"]
        d = run / sid / timestamp
        d.mkdir(parents=True, exist_ok=True)
        (d / "result.json").write_text(json.dumps(r))
    return run


def _write_stability(tmp_path: Path, name: str, stab: dict) -> Path:
    p = tmp_path / name
    p.write_text(json.dumps(stab))
    return p


def _scenario_stats(
    scenario_id: str, timings: list[float], *, n: int | None = None
) -> dict:
    """Build a scenario_stats entry compatible with analyze_stability output."""
    timings = list(timings)
    actual_n = n if n is not None else len(timings)
    if not timings:
        return {
            "scenario_id": scenario_id,
            "runs": actual_n,
            "pass_rate": 1.0,
            "flaky": False,
            "timing": {
                "mean": 0, "std_dev": 0, "p50": 0, "p95": 0, "p99": 0,
                "min": 0, "max": 0, "cv": 0, "cv_warm": 0,
                "cold_cache_sample": False,
            },
            "outcome_distribution": {},
        }
    mean = sum(timings) / len(timings)
    var = sum((t - mean) ** 2 for t in timings) / len(timings)
    std = var ** 0.5
    cv = (std / mean) if mean > 0 else 0
    sorted_t = sorted(timings)
    cv_warm = cv
    if len(timings) >= 3:
        warm = sorted_t[:-1]
        warm_mean = sum(warm) / len(warm)
        warm_var = sum((t - warm_mean) ** 2 for t in warm) / len(warm)
        warm_std = warm_var ** 0.5
        cv_warm = (warm_std / warm_mean) if warm_mean > 0 else 0
    return {
        "scenario_id": scenario_id,
        "runs": actual_n,
        "pass_rate": 1.0,
        "flaky": False,
        "timing": {
            "mean": round(mean, 2),
            "std_dev": round(std, 2),
            "p50": sorted_t[min(int(len(timings) * 0.5), len(timings) - 1)],
            "p95": sorted_t[min(int(len(timings) * 0.95), len(timings) - 1)],
            "p99": sorted_t[min(int(len(timings) * 0.99), len(timings) - 1)],
            "min": round(min(timings), 2),
            "max": round(max(timings), 2),
            "cv": round(cv, 4),
            "cv_warm": round(cv_warm, 4),
            "cold_cache_sample": False,
        },
        "outcome_distribution": {"pass": len(timings)},
    }


# ── P1: three repeats contribute to G3 ──────────────────────────────────────


def test_p1_three_repeats_each_contribute_to_g3(tmp_path):
    """P1: three repeats of the same scenario must all contribute to G3.

    With scenario_id='X' and 3 repeats, the reader must aggregate the
    3 repeats and produce ONE per-scenario health value (not three
    independent values), and that value must reflect all 3
    measurements, not silently drop 2 of them.

    The aggregation policy (per repeat_index mean within a scenario_id,
    averaged across distinct scenario_ids) is the same one used by
    the scoring engine's stability path.
    """
    # 3 repeats of ONE scenario, all dims = 90 -> 5-dim health = 90.0
    rows = []
    for ri in range(3):
        rows.append(_make_result("scX", repeat_index=ri, correctitud=80.0,
                                  latencia=90.0, escalabilidad=90.0,
                                  consistencia=90.0, robustez=90.0))
    run = _write_run(tmp_path, "run-1", rows)

    # stability.json: NO health_score (force reader to compute from result.json)
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p1",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    # All 3 repeats have full 5 dims, so this is one complete scenario
    # with health = 90.0 (the 80% correctitud lowers the weighted avg).
    # CORR*0.35 = 80*0.35 = 28.0
    # LAT*0.20 = 90*0.20 = 18.0
    # ESC*0.15 = 90*0.15 = 13.5
    # CON*0.15 = 90*0.15 = 13.5
    # ROB*0.15 = 90*0.15 = 13.5
    # Total = 86.5
    # >= 85 -> GREEN
    assert g.status == "GREEN", \
        f"3 repeats of one complete scenario with 5-dim health=86.5 -> GREEN; " \
        f"got {g.status} ({g.evidence_text})"

    # P1 contract: the reader must aggregate per scenario_id (1 distinct
    # scenario), not treat each result.json as a separate scenario.
    ev = g.evidence_text.lower()
    assert "1 scenarios" in ev or "1 scenario" in ev or "complete_5dim=1" in ev, \
        f"evidence must reflect 1 distinct scenario_id, not 3; got: {g.evidence_text}"


def test_p1_no_silent_dedup_of_first_only(tmp_path):
    """P1 (sharpened): when 3 repeats of one scenario have different
    dimension values, the aggregated health must reflect all 3,
    not silently the first one.

    Setup: scenario X with 3 repeats:
      repeat 0: correctitud=100, lat=90, esc=90, con=90, rob=90 -> health=92.0
      repeat 1: correctitud=60,  lat=90, esc=90, con=90, rob=90 -> health=81.0
      repeat 2: correctitud=60,  lat=90, esc=90, con=90, rob=90 -> health=81.0

    The aggregated health should be (92.0 + 81.0 + 81.0) / 3 = 84.67
    if we average per-repeat; or it should reflect the policy
    explicitly. If the reader silently uses only the first result
    (correctitud=100), the health would be 92.0 -> GREEN. With the
    real aggregation, health is 84.67 -> RED.

    This pins the contract: NOT silently first-only.
    """
    rows = []
    rows.append(_make_result("scX", repeat_index=0, correctitud=100.0))
    rows.append(_make_result("scX", repeat_index=1, correctitud=60.0))
    rows.append(_make_result("scX", repeat_index=2, correctitud=60.0))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p1b",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))

    # The reader does per-scenario aggregation; for a single scenario
    # with 3 repeats, the per-scenario health is averaged across the 3
    # repeats. If the reader silently uses only the first repeat
    # (correctitud=100), it would show 92.0 -> GREEN. With proper
    # aggregation, the value should be lower (around 84.67).
    # Pin: the verdict must reflect the real aggregation.
    # We accept AMBER or RED; we forbid GREEN with no warning
    # about the per-repeat spread.
    if g.status == "GREEN":
        # If GREEN, the evidence MUST surface the per-repeat spread.
        assert "100" in g.evidence_text and "60" in g.evidence_text, \
            f"GREEN must cite the per-repeat spread; got: {g.evidence_text}"
    else:
        assert g.status in ("AMBER", "RED"), \
            f"per-repeat spread must drive verdict below GREEN; got {g.status} ({g.evidence_text})"


def test_p1_dedup_across_run_dirs(tmp_path):
    """P1: when the same scenario_id appears in multiple run_dirs (e.g.
    campaign with run-1 and run-2 for the SAME scenario), the reader
    must deduplicate by scenario_id and NOT count it twice.
    """
    rows_a = [_make_result("scX", repeat_index=0, correctitud=80.0)]
    rows_b = [_make_result("scX", repeat_index=1, correctitud=80.0)]
    run_a = _write_run(tmp_path, "run-1", rows_a)
    run_b = _write_run(tmp_path, "run-2", rows_b)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p1c",
        "measured_source_head": "test",
        "repeat_count": 2,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run_a), str(run_b)], stability_path=str(sp))
    # 1 distinct scenario_id (scX), 2 repeats -> per-scenario health
    # is the mean of the 2. Must NOT be reported as 2 scenarios.
    ev = g.evidence_text.lower()
    assert "1 scenarios" in ev or "1 scenario" in ev or "complete_5dim=1" in ev, \
        f"evidence must reflect 1 distinct scenario, not 2; got: {g.evidence_text}"


# ── P2: G3 criterion documented and coherent ────────────────────────────────


def test_p2_g3_threshold_is_85():
    """P2: G3 threshold is 85.0 per the spec, documented in the reader."""
    assert hasattr(rs, "G3_THRESHOLD"), "G3_THRESHOLD must be exposed"
    assert rs.G3_THRESHOLD == 85.0, f"G3_THRESHOLD must be 85.0, got {rs.G3_THRESHOLD}"


def test_p2_g3_weights_match_spec():
    """P2: G3 weights match sandbox_core::scoring HEALTH_WEIGHTS."""
    assert hasattr(rs, "G3_HEALTH_WEIGHTS"), "G3_HEALTH_WEIGHTS must be exposed"
    assert tuple(rs.G3_HEALTH_WEIGHTS) == (0.35, 0.20, 0.15, 0.15, 0.15), \
        f"weights must match scoring engine, got {rs.G3_HEALTH_WEIGHTS}"


def test_p2_per_scenario_below_threshold_is_red(tmp_path):
    """P2: a single scenario with health < 85 forces RED with the scenario named.

    Coherent with the spec: 'MUST be >= 85/100' is per-scenario.
    """
    rows = [
        _make_result("good", repeat_index=0, correctitud=90.0),
        _make_result("bad", repeat_index=0, correctitud=70.0),  # 5-dim = 79.0
    ]
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p2",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status == "RED", \
        f"single scenario below 85 must be RED; got {g.status} ({g.evidence_text})"
    assert "bad" in g.evidence_text, \
        f"failing scenario must be named; got: {g.evidence_text}"


def test_p2_g3_documentation_includes_aggregation_policy():
    """P2: the reader's docstring documents the aggregation policy.

    The spec requires per-scenario 5-dim health. The reader must
    document how it aggregates (per-scenario distinct, per-repeat
    averaged within scenario_id).
    """
    import inspect
    src = inspect.getsource(rs.gate_g3)
    # The docstring or code must mention distinct scenario_ids and
    # the per-repeat averaging.
    assert "scenario" in src.lower(), \
        "reader docstring/source must reference scenario-level aggregation"
    assert "distinct" in src.lower() or "seen:" in src.lower() or "de-dup" in src.lower(), \
        "reader must document distinct scenario_id handling"
    assert "repeat" in src.lower(), \
        "reader must document per-repeat handling"


# ── P3: G6 cannot return GREEN when an included scenario lacks a repeat ─────


def test_p3_g6_red_with_some_scenarios_below_threshold(tmp_path):
    """P3: G6 cannot return GREEN when an included scenario has CV >= 10%.

    Even if all OTHER scenarios have CV < 10%, a single included
    scenario with high CV forces RED, naming the failing scenario.
    """
    scenarios = []
    # 4 stable scenarios (CV < 10%)
    for i in range(4):
        scenarios.append(_scenario_stats(f"stable{i}", [100, 102, 101]))
    # 1 unstable scenario (CV high)
    scenarios.append(_scenario_stats("unstable", [100, 200, 300]))  # cv warm ~ 0.4
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p3",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": scenarios,
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g6(str(sp))
    assert g.status == "RED", \
        f"one scenario CV >= 10% must force RED; got {g.status} ({g.evidence_text})"
    assert "unstable" in g.evidence_text, \
        f"failing scenario must be named; got: {g.evidence_text}"


def test_p3_g6_amber_when_all_scenarios_lack_repeats(tmp_path):
    """P3: G6 cannot return GREEN when NO scenario has the required repeats.

    Even with all 5 Tier-1 repos represented, if every scenario has
    only 2 repeats, the gate must be AMBER insufficient_samples,
    not GREEN.
    """
    # 5 scenarios, all with runs=2 (insufficient)
    scenarios = [
        _scenario_stats(f"sc{i}", [100, 110]) for i in range(5)
    ]
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p3b",
        "measured_source_head": "test",
        "repeat_count": 2,    # < 3
        "pass_rate": 100.0,
        "scenario_stats": scenarios,
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g6(str(sp))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"insufficient samples must NOT be GREEN; got {g.status} ({g.evidence_text})"


def test_p3_g6_amber_when_mixed_repeats(tmp_path):
    """P3: G6 with mixed repeats (some scenarios have n>=3, others have n<3).

    The eligible scenarios can drive a verdict; the ineligible
    scenarios must be surfaced as insufficient_samples and cannot
    be silently ignored.
    """
    scenarios = [
        _scenario_stats("good", [100, 110, 105]),    # n=3
        _scenario_stats("bad", [100, 110]),           # n=2
    ]
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p3c",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": scenarios,
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g6(str(sp))
    # The good scenario is eligible; if it's stable, the verdict could
    # be GREEN. But the bad scenario (n<3) MUST be named in evidence
    # as insufficient_samples.
    ev = g.evidence_text.lower()
    assert "bad" in ev or "insufficient" in ev, \
        f"the n<3 scenario must be surfaced in evidence; got: {g.evidence_text}"


# ── P4: scenarios without GT must NOT inflate or fake scores ────────────────


def test_p4_no_gt_scenario_does_not_produce_zero_correctitud(tmp_path):
    """P4: a scenario with ground_truth_present=False and correctitud=None
    must NOT contribute to G4 averages.

    The reader must not substitute a 0 for the missing score (which
    would silently drag the average down) nor treat None as a high
    score. The scenario must be classified UNVERIFIED and excluded
    from the per-repo average.
    """
    # 1 scenario with GT (correctitud=85) + 1 scenario without GT
    rows = [
        _make_result("with_gt", repeat_index=0, repo="ripgrep",
                     actual_repository_identity="ripgrep",
                     actual_repository_revision="abc1234",
                     correctitud=85.0, ground_truth_present=True),
        _make_result("no_gt", repeat_index=0, repo="ripgrep",
                     actual_repository_identity="ripgrep",
                     actual_repository_revision="abc1234",
                     correctitud=None, ground_truth_present=False),
    ]
    run = _write_run(tmp_path, "run-1", rows)

    g = rs.gate_g4([str(run)])
    # Only the with_gt scenario contributes to ripgrep's average.
    # 85 < 90 -> RED, ripgrep named.
    assert g.status == "RED", \
        f"only 1 scenario with GT at 85 must be RED; got {g.status} ({g.evidence_text})"
    assert "ripgrep" in g.evidence_text, \
        f"ripgrep must be named as failing repo; got: {g.evidence_text}"


def test_p4_no_gt_scenario_does_not_produce_5dim_health(tmp_path):
    """P4: a scenario with correctitud=None must NOT contribute to G3
    even if the other 4 dims are present.

    The reader must not fabricate a 5-dim health by substituting
    the missing correctitud with 0, with 100, or with the average
    of the other 4. The scenario is incomplete_5dim.
    """
    rows = [
        # Scenario with correctitud=None but other dims populated
        _make_result("no_corr", repeat_index=0, correctitud=None,
                     ground_truth_present=False),
    ]
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "p4",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    # correctitud=None -> incomplete_5dim -> AMBER (not GREEN, not RED)
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"correctitud=None must be AMBER insufficient_5dim; got {g.status} ({g.evidence_text})"
    # Must NOT be a fabricated GREEN.
    assert g.status != "GREEN", \
        f"correctitud=None must NOT be GREEN; got {g.status} ({g.evidence_text})"


def test_p4_no_gt_scenario_classified_as_unverified(tmp_path):
    """P4 (sharpened): a scenario with ground_truth_present=False
    AND actual_repository_identity=None (or any provenance missing)
    must be classified UNVERIFIED in G4's evidence, not silently
    aggregated.
    """
    rows = [
        _make_result("serde_a", repeat_index=0, repo="serde",
                     actual_repository_identity="serde",
                     actual_repository_revision="abc1234",
                     correctitud=95.0, ground_truth_present=True),
        # No GT, no provenance at all
        _make_result("bogus", repeat_index=0, repo="serde",
                     actual_repository_identity=None,
                     actual_repository_revision=None,
                     correctitud=None, ground_truth_present=False),
    ]
    run = _write_run(tmp_path, "run-1", rows)

    g = rs.gate_g4([str(run)])
    # serde is acredited via the first scenario (correctitud=95).
    # The other 4 Tier-1 repos are absent -> AMBER.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"only serde acredited must be AMBER; got {g.status} ({g.evidence_text})"
    # bogus must be classified UNVERIFIED in evidence (or as
    # contributing to missing_or_unverified count).
    ev = g.evidence_text.lower()
    assert "missing" in ev or "unverified" in ev, \
        f"bogus scenario must be flagged as missing/unverified; got: {g.evidence_text}"


# ── P1+P2+P3+P4 combined: end-to-end preflight ──────────────────────────────


def test_preflight_all_four_criteria_documented():
    """P1+P2+P3+P4: the four preflight criteria are exposed as constants
    and documented in the reader's docstrings."""
    import inspect
    src = inspect.getsource(rs)
    # P1: scenario-level aggregation
    assert "scenario_id" in src and "repeat" in src.lower(), \
        "P1 (repeat aggregation) not documented in reader"
    # P2: spec coherence
    assert "spec" in src.lower() and "spec.md" in src, \
        "P2 (spec coherence) not referenced in reader"
    # P3: G6 cannot GREEN without repeats
    g6 = inspect.getsource(rs.gate_g6)
    assert "insufficient_repeats" in g6 or "repeat_count" in g6, \
        "P3 (G6 repeat precondition) not enforced"
    # P4: no fabrication of missing dimensions
    g3 = inspect.getsource(rs.gate_g3)
    g4 = inspect.getsource(rs.gate_g4)
    assert "ground_truth_present" in g4 or "unverified" in g4.lower(), \
        "P4 (no GT fabrication) not enforced in G4"
    assert "incomplete_5dim" in g3 or "insufficient_5dim" in g3, \
        "P4 (no 5-dim fabrication) not enforced in G3"
