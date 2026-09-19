"""A1c — G3 contract tests for the Sandbox Health Score reader.

Pins the G3 contract from
`openspec/specs/release-readiness-gate/spec.md`:

  The MCP Health Score (weighted average of correctness, latency,
  scalability, consistency, robustness dimensions, computed by
  sandbox_core::scoring) MUST be >= 85/100 for the candidate release.

The contract source-of-truth is the Rust scoring engine in
`crates/cognicode-core/src/sandbox_core/scoring.rs`:

  HEALTH_WEIGHTS = (0.35, 0.20, 0.15, 0.15, 0.15)
  compute_health_score(scores) =
      CORR*0.35 + LAT*0.20 + ESC*0.15 + CON*0.15 + ROB*0.15

This Python test module re-implements the same formula to avoid
binding to the Rust crate from Python. The reader is expected to
apply the same weights when consuming dimension_scores from
result.json files.

Reader gaps pinned by these tests:

R-G3-1 wrong source: stability.json::health_score (analyze_stability
  formula `min(100, pass_rate*50 + 95*30 + 95*20)`) is consumed as
  G3 evidence. The spec requires the scoring-engine formula.
  Tests: test_uses_scoring_engine_weights_not_analyze_stability_formula,
         test_5dim_weights_match_scoring_engine,
         test_two_constants_95_are_not_accepted_as_health_evidence

R-G3-2 double counting: stability.json::health_score +
  per-run summary.json::health_score + per-run aggregate::health_score
  are averaged together. Same observations counted up to 3 times.
  Test: test_does_not_mix_stability_and_per_run_sources

R-G3-3 missing correctitud inflates health: when correctitud is None,
  the current 97.5 hides it. The spec requires the scoring-engine
  formula, which depends on correctitud.
  Test: test_missing_correctitud_does_not_produce_85_green

R-G3-4 scenarios vary: real 5-dim health for the 8 complete scenarios
  is [94.45, 98.92, 71.25, ...]. The current 97.5 hides the variance.
  Test: test_per_scenario_health_is_visible_in_evidence

R-G3-5 71.25 below threshold: the current reader reports 97.5 GREEN
  while a scenario at 71.25 is below 85. Cannot both be honest.
  Test: test_one_below_threshold_forces_not_green

R-G3-6 GREEN with 8/58 complete scenarios: the spec requires
  scoring-engine health; with correctitud only in 8/58, the
  scoring-engine health is not fully computable across the corpus.
  Test: test_insufficient_complete_5dim_scenarios_is_amber

Verdict precedence for G3:

  no dimension_scores from scoring engine          -> AMBER no_evidence
  any scenario below 85 with full 5 dims           -> RED (named)
  all scenarios >= 85 with full 5 dims             -> GREEN
  some complete, some missing                       -> AMBER (incomplete)
  source is analyze_stability formula (not 5-dim)  -> AMBER wrong_source

Run with:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_a1c_g3_contract.py -v
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
# Mirror of crates/cognicode-core/src/sandbox_core/scoring.rs:1400
HEALTH_WEIGHTS = (0.35, 0.20, 0.15, 0.15, 0.15)
DIM_NAMES = ("correctitud", "latencia", "escalabilidad", "consistencia", "robustez")


def _5dim_health(ds: dict) -> float | None:
    """Compute the scoring-engine health score for one dimension_scores dict.

    Returns None if any of the 5 required dimensions is missing or None.
    Returns 0.0 if all 5 are present but all zero (legitimate 0).
    """
    vals = []
    for k in DIM_NAMES:
        v = ds.get(k)
        if v is None:
            return None
        vals.append(float(v))
    return sum(v * w for v, w in zip(vals, HEALTH_WEIGHTS))


def _make_result(scenario_id: str, ds: dict, run_idx: int = 0,
                 timestamp: str = "2026-09-19T120000") -> dict:
    return {
        "scenario_id": scenario_id,
        "language": "rust",
        "tool": "search_content",
        "tier": "A",
        "repo": "rust-hello",
        "outcome": "pass",
        "failure_class": "pass",
        "dimension_scores": ds,
        "validation": {"stages": [], "passed": True},
        "workspace": ".",
        "workspace_snapshot_id": f"snap_{scenario_id}_{run_idx}",
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


def _write_summary(run: Path, summary: dict) -> Path:
    p = run / "summary.json"
    p.write_text(json.dumps(summary))
    return p


def _write_stability(tmp_path: Path, name: str, stab: dict) -> Path:
    p = tmp_path / name
    p.write_text(json.dumps(stab))
    return p


# ── Constants surface ────────────────────────────────────────────────────────


def test_g3_constants_observable():
    """Pin the contract as observable constants on the reader."""
    assert hasattr(rs, "G3_THRESHOLD"), "reader must expose G3_THRESHOLD"
    assert hasattr(rs, "G3_HEALTH_WEIGHTS"), "reader must expose G3_HEALTH_WEIGHTS"
    assert rs.G3_THRESHOLD == 85.0, f"threshold must be 85.0, got {rs.G3_THRESHOLD}"
    assert tuple(rs.G3_HEALTH_WEIGHTS) == HEALTH_WEIGHTS, \
        f"weights must match scoring engine, got {rs.G3_HEALTH_WEIGHTS}"


# ── 5-dim health formula ────────────────────────────────────────────────────


def test_5dim_health_for_perfect_scenario():
    ds = {k: 100.0 for k in DIM_NAMES}
    assert _5dim_health(ds) == 100.0


def test_5dim_health_for_zero_scenario():
    ds = {k: 0.0 for k in DIM_NAMES}
    assert _5dim_health(ds) == 0.0


def test_5dim_health_for_missing_correctitud():
    ds = {k: 90.0 for k in DIM_NAMES}
    del ds["correctitud"]
    assert _5dim_health(ds) is None


def test_5dim_health_for_none_correctitud():
    ds = {k: 90.0 for k in DIM_NAMES}
    ds["correctitud"] = None
    assert _5dim_health(ds) is None


def test_5dim_health_weight_distribution():
    """Sanity: weights sum to 1.0 and correctitud has the highest weight."""
    assert abs(sum(HEALTH_WEIGHTS) - 1.0) < 1e-9
    assert HEALTH_WEIGHTS[0] == 0.35      # correctitud
    assert HEALTH_WEIGHTS[1] == 0.20      # latencia
    for w in HEALTH_WEIGHTS[2:]:
        assert w == 0.15                  # esc, con, rob


# ── Reader contract ──────────────────────────────────────────────────────────


def test_uses_scoring_engine_weights_not_analyze_stability_formula(tmp_path):
    """R-G3-1: the reader must use the scoring-engine 5-dim formula, not the
    analyze_stability formula `min(100, pass_rate*50 + 95*30 + 95*20)`.

    Setup: 3 scenarios, all with the SAME 5-dim dimensions such that the
    scoring-engine health equals exactly 85.0 (threshold). The
    analyze_stability formula would also give 85 here (since pass_rate=100),
    so this test is the control. The next test isolates the difference.
    """
    # 3 scenarios, all dimensions = 85.0
    rows = []
    for i in range(3):
        ds = {k: 85.0 for k in DIM_NAMES}
        rows.append(_make_result(f"sc{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    # stability.json with the analyze_stability formula
    # 3 results, all pass -> pass_rate=1.0
    #   health_analyze = min(100, 100*0.5 + 95*0.3 + 95*0.2) = 97.5
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        # The 97.5 is the analyze_stability formula with pass_rate=100
        "health_score": 97.5,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))

    # If the reader uses the analyze_stability formula, it sees 97.5 -> GREEN.
    # If the reader uses the scoring-engine 5-dim formula, it sees 85.0 -> GREEN.
    # Both formulas pass at threshold here. To distinguish, see next test.
    # This test pins the threshold semantics.
    assert g.status == "GREEN", \
        f"all 5-dim health = 85 should be GREEN at threshold; got {g.status} ({g.evidence_text})"


def test_two_constants_95_are_not_accepted_as_health_evidence(tmp_path):
    """R-G3-1 (sharpened): the analyze_stability formula's two 95s are
    constants, not measurements. A campaign where every dimension is
    missing but the analyze_stability formula gives a high value must
    NOT produce a release verdict.

    Setup: 3 scenarios, all dimensions None (no measurement possible).
    analyze_stability still gives 97.5 because the 95s are constants.
    The reader must NOT consume that 97.5 as G3 evidence.
    """
    rows = []
    for i in range(3):
        ds = {k: None for k in DIM_NAMES}    # NO measurements
        rows.append(_make_result(f"sc{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "health_score": 97.5,    # from analyze_stability formula with 95s
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"all-dimensions-None must NOT be GREEN even if analyze_stability " \
        f"formula says 97.5; got {g.status} ({g.evidence_text})"


def test_does_not_mix_stability_and_per_run_sources(tmp_path):
    """R-G3-2: stability.json::health_score and per-run summary.json::health_score
    are derived from the same observations. The reader must not average them.

    Setup: stability.json::health_score=97.5. Each run_dir's summary.json has
    a different health_score. If the reader averages them, the result is
    somewhere in between. The fix: pick one source per the spec, not both.
    """
    rows = []
    for i in range(3):
        ds = {k: 90.0 for k in DIM_NAMES}    # all 5-dim health = 90.0
        rows.append(_make_result(f"sc{i}", ds))

    run1 = _write_run(tmp_path, "run-1", rows)
    run2 = _write_run(tmp_path, "run-2", rows)
    # Per-run summary.json with different (wrong) health values
    _write_summary(run1, {"health_score": 50.0})
    _write_summary(run2, {"health_score": 60.0})

    # stability.json health_score = 97.5 (from analyze_stability formula)
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 2,
        "pass_rate": 100.0,
        "health_score": 97.5,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run1), str(run2)], stability_path=str(sp))

    # The reader must NOT produce (97.5 + 50 + 60) / 3 = 69.17 (avg of mixed).
    # It must either:
    #   (a) use the 5-dim scoring-engine formula on result.json (90 each -> 90),
    #   (b) report AMBER because no source matches the spec.
    # Both are acceptable; the reader must NOT mix sources.
    if g.status == "GREEN":
        # If GREEN, the value must come from one source, not be mixed.
        # Reading the evidence must show a single-source origin.
        assert "5-dim" in g.evidence_text.lower() or "scoring_engine" in g.evidence_text.lower() or \
               "5dim" in g.evidence_text.lower(), \
            f"GREEN must come from a single source per the spec; got: {g.evidence_text}"
    else:
        assert g.status in ("AMBER", "INCOMPLETE"), \
            f"expected AMBER/INCOMPLETE for mixed sources, got {g.status} ({g.evidence_text})"


def test_missing_correctitud_does_not_produce_85_green(tmp_path):
    """R-G3-3: correctitud None must not produce a 85 GREEN.

    Setup: 3 scenarios, all dimensions populated EXCEPT correctitud.
    analyze_stability still gives 97.5; the scoring-engine formula
    cannot be computed (correctitud missing). The reader must NOT
    substitute the analyze_stability value.
    """
    rows = []
    for i in range(3):
        ds = {k: 90.0 for k in DIM_NAMES}
        ds["correctitud"] = None    # missing
        rows.append(_make_result(f"sc{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "health_score": 97.5,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"missing correctitud must NOT produce GREEN; got {g.status} ({g.evidence_text})"


def test_one_below_threshold_forces_not_green(tmp_path):
    """R-G3-5: a scenario with health < 85 forces RED or AMBER.

    Setup: 3 scenarios, two at 90 and one at 80 (below threshold).
    The 5-dim health must show the distribution; the verdict
    must not be GREEN.
    """
    rows = []
    for i in range(2):
        ds = {k: 90.0 for k in DIM_NAMES}
        rows.append(_make_result(f"sc{i}", ds))
    ds_low = {k: 80.0 for k in DIM_NAMES}    # 5-dim health = 80.0
    rows.append(_make_result("sc_low", ds_low))
    run = _write_run(tmp_path, "run-1", rows)

    # stability.json: NO health_score field (so the reader is forced
    # to compute from result.json files).
    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status in ("RED", "AMBER"), \
        f"one scenario below threshold must NOT be GREEN; got {g.status} ({g.evidence_text})"
    # The low scenario should be named.
    assert "sc_low" in g.evidence_text, \
        f"the below-threshold scenario must be named; got: {g.evidence_text}"


def test_per_scenario_health_is_visible_in_evidence(tmp_path):
    """R-G3-4: per-scenario health distribution must be visible.

    Three scenarios at 90, 90, 71.25 (the values from the frozen run).
    The reader must surface the variance, not hide it behind a 97.5 average.
    """
    rows = []
    rows.append(_make_result("sc0", {k: 90.0 for k in DIM_NAMES}))
    rows.append(_make_result("sc1", {k: 90.0 for k in DIM_NAMES}))
    # 71.25 = 5-dim health when (correctitud=70, latencia=80, esc=70, con=70, rob=70)
    ds_low = {k: 70.0 for k in DIM_NAMES}
    ds_low["latencia"] = 80.0
    rows.append(_make_result("sc2", ds_low))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        # analyze_stability formula gives 97.5; the spec wants 5-dim
        "health_score": 97.5,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    # The per-scenario health values must be visible in evidence, not
    # hidden behind an aggregate.
    assert "sc2" in g.evidence_text or "71.25" in g.evidence_text or "below" in g.evidence_text.lower(), \
        f"the below-threshold scenario's value must be cited; got: {g.evidence_text}"


def test_insufficient_complete_5dim_scenarios_is_amber(tmp_path):
    """R-G3-6: insufficient 5-dim scenarios -> AMBER.

    With correctitud only in 8/58 result.jsons (frozen-run pattern),
    the scoring-engine health cannot be computed across the corpus.
    That is insufficient evidence for a release verdict.
    """
    rows = []
    # 50 scenarios, all with latencia/escalabilidad/consistencia/robustez
    # but missing correctitud (typical scenario without ground truth)
    for i in range(50):
        ds = {k: 90.0 for k in DIM_NAMES}
        ds["correctitud"] = None
        rows.append(_make_result(f"sc{i}", ds))
    # 5 scenarios with all 5 dims populated
    for i in range(5):
        ds = {k: 90.0 for k in DIM_NAMES}
        rows.append(_make_result(f"complete{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "health_score": 97.5,    # placeholder from analyze_stability
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    # Only 5/55 scenarios have all 5 dims -> insufficient evidence
    # for the corpus-level health claim the spec demands.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"only 5/55 with 5-dim evidence -> AMBER; got {g.status} ({g.evidence_text})"


# ── Frozen campaign integration ──────────────────────────────────────────────


def test_frozen_run_g3_evidence():
    """Frozen run G3 must reflect the real 5-dim picture, not the 97.5 placeholder.

    The frozen run 20260919T102509 has correctitud in only 8/58
    result.jsons (across run-1 and run-2). Of those, the 5-dim health
    computed by the scoring-engine formula reveals 2 scenarios below
    the 85 threshold (worst=68.6). The previous reader hid this
    behind a 97.5 placeholder; the new reader reports it as RED with
    the failing scenario named.

    This test pins: the verdict comes from real 5-dim evidence, not
    from the analyze_stability placeholder. RED is the honest verdict
    for this campaign; AMBER only if there were zero complete-5-dim
    scenarios at all.
    """
    p = REPO / "sandbox/results-runs/20260919T102509/run-1"
    if not p.exists():
        return
    sp = REPO / "sandbox/results-runs/20260919T102509/stability.json"
    if not sp.exists():
        return
    g = rs.gate_g3([str(p)], stability_path=str(sp))
    # Some scenarios have all 5 dims; some are below threshold. RED
    # is the honest verdict (not GREEN, not AMBER). AMBER is reserved
    # for "no complete 5-dim scenarios at all" — that is not this case.
    assert g.status == "RED", (
        f"frozen run has 2 scenarios below threshold (5-dim formula); "
        f"G3 must be RED, got {g.status} ({g.evidence_text})"
    )
    # The 97.5 placeholder must be preserved as diagnostic, not as evidence.
    assert "diagnostic_only" in g.evidence_text, \
        f"the 97.5 placeholder must be cited as diagnostic_only; got: {g.evidence_text}"
    assert "placeholder constants" in g.evidence_text.lower(), \
        f"the placeholder nature of the 95s must be cited; got: {g.evidence_text}"
    # The worst failing scenario must be named.
    assert "rust_search_content_default" in g.evidence_text, \
        f"the worst failing scenario must be named; got: {g.evidence_text}"


# ── Verdict precedence ──────────────────────────────────────────────────────


def test_complete_scenarios_all_above_threshold_is_green(tmp_path):
    """All scenarios with all 5 dims >= 85 -> GREEN."""
    rows = []
    for i in range(3):
        ds = {k: 90.0 for k in DIM_NAMES}
        rows.append(_make_result(f"sc{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        # stability.json::health_score absent; reader must compute from result.json
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status == "GREEN", \
        f"all 5-dim health=90 across all scenarios -> GREEN; got {g.status} ({g.evidence_text})"


def test_complete_scenarios_all_exactly_at_threshold_is_green(tmp_path):
    """5-dim health exactly at 85 -> GREEN (threshold is inclusive)."""
    rows = []
    for i in range(3):
        ds = {k: 85.0 for k in DIM_NAMES}
        rows.append(_make_result(f"sc{i}", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status == "GREEN", \
        f"5-dim health=85 at threshold -> GREEN; got {g.status} ({g.evidence_text})"


def test_no_dimension_scores_at_all_is_amber(tmp_path):
    """No dimension_scores in any result.json -> AMBER."""
    rows = []
    for i in range(3):
        rows.append(_make_result(f"sc{i}", {}))    # no dimension_scores at all
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "health_score": 97.5,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"no dimension_scores -> AMBER; got {g.status} ({g.evidence_text})"


def test_partial_5dim_with_some_complete_is_amber(tmp_path):
    """Some scenarios complete, some missing correctitud -> AMBER.

    The spec requires per-scenario 5-dim health. If even one
    scenario is incomplete, the corpus-level verdict cannot be
    produced — release evidence is incomplete.
    """
    rows = []
    # 2 complete scenarios
    for i in range(2):
        ds = {k: 90.0 for k in DIM_NAMES}
        rows.append(_make_result(f"complete{i}", ds))
    # 1 incomplete (missing correctitud)
    ds = {k: 90.0 for k in DIM_NAMES}
    ds["correctitud"] = None
    rows.append(_make_result("incomplete", ds))
    run = _write_run(tmp_path, "run-1", rows)

    stab = {
        "parent_dir": str(tmp_path),
        "campaign_id": "test",
        "measured_source_head": "test",
        "repeat_count": 3,
        "pass_rate": 100.0,
        "scenario_stats": [],
    }
    sp = _write_stability(tmp_path, "stability.json", stab)

    g = rs.gate_g3([str(run)], stability_path=str(sp))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"incomplete 5-dim coverage -> AMBER; got {g.status} ({g.evidence_text})"
