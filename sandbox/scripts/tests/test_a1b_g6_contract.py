"""A1b — G6 contract tests for the run-to-run stability reader.

Pins the G6 contract from
`openspec/specs/release-readiness-gate/spec.md`:

  Run-to-run variance (via stability.json from --repeat >= 3) MUST
  be < 10% per dimension.

Plus the A1b.0 characterization findings:

  R-G6-1 reader accepts any repeat_count
    Spec: --repeat >= 3 required.
    Test: test_reader_requires_repeat_count_at_least_three

  R-G6-2 reader accepts runs < 3 per scenario
    Spec: variance via stability.json implies n >= 3.
    Test: test_reader_requires_at_least_three_samples_per_scenario

  R-G6-3 cv_warm == cv when n < 3 is silently passed as warm-cache
    Spec: warm-cache CV requires cold-cache identification, which is
          only meaningful with n >= 3.
    Test: test_cv_warm_equal_to_cv_with_insufficient_samples_must_be_flagged

  R-G6-4 no samples -> cv = 0 (silently green)
    Spec: empty measurements are NOT evidence.
    Test: test_empty_timings_does_not_produce_zero_cv_green

  R-G6-5 no campaign identity check (parent_dir, measured_source_head)
    Spec: comparisons within a campaign.
    Test: test_reader_requires_campagin_identity_match

  R-G6-6 mix per-dimension variance (only timing, no other dimensions)
    Spec: "variance ... per dimension".
    Test: test_per_dimension_variance_is_required (deferred to A1c+)

  R-G6-7 aggregate_cv from stability.json not consulted
    Spec: stability.json is the canonical source; aggregate_cv is the
          second-opinion summary.
    Test: test_aggregate_cv_is_a_second_opinion_signal

  R-G6-8 cv_warm == cv without warning in evidence_text
    Spec: evidence must distinguish warm-cache from full.
    Test: test_evidence_text_distinguishes_warm_vs_full

Verdict precedence for G6:

  repeat_count < 3 (campaign-level)        -> AMBER, insufficient_repeats
  no scenario with runs >= 3               -> AMBER, insufficient_samples
  cv_full >= 10% with sufficient samples   -> RED, dimension named
  cv_warm >= 10% (warm-cache applicable)   -> RED, dimension named
  cv_warm == cv == 0 with no samples       -> AMBER, no evidence
  all CV < 10% with sufficient samples     -> GREEN

Run with:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_a1b_g6_contract.py -v
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))  # sandbox/scripts

import release_scorecard as rs  # noqa: E402

REPO = Path("/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode")
G6_CV_THRESHOLD = 0.10
G6_MIN_REPEATS = 3


# ── Synthetic fixtures ────────────────────────────────────────────────────────


def _scenario_stats(
    scenario_id: str,
    timings: list[float],
    *,
    cold_cache_sample: bool | None = None,
    extra: dict | None = None,
) -> dict:
    """Build a scenario_stats entry compatible with analyze_stability output."""
    if not timings:
        return {
            "scenario_id": scenario_id,
            "runs": 0,
            "pass_rate": 1.0,
            "flaky": False,
            "timing": {
                "mean": 0, "std_dev": 0, "p50": 0, "p95": 0, "p99": 0,
                "min": 0, "max": 0, "cv": 0, "cv_warm": 0,
                "cold_cache_sample": False,
            },
            "outcome_distribution": {},
        }

    n = len(timings)
    mean = sum(timings) / n
    var = sum((t - mean) ** 2 for t in timings) / n
    std = var ** 0.5
    cv = (std / mean) if mean > 0 else 0
    sorted_t = sorted(timings)

    # Mirror analyze_stability.py: cv_warm applies only when n >= 3
    cv_warm = cv
    detected_cold = False
    if n >= 3:
        warm = sorted_t[:-1]
        warm_mean = sum(warm) / len(warm)
        warm_var = sum((t - warm_mean) ** 2 for t in warm) / len(warm)
        warm_std = warm_var ** 0.5
        warm_cv = (warm_std / warm_mean) if warm_mean > 0 else 0
        cv_warm = round(warm_cv, 4)
        if sorted_t[-1] >= 1.5 * warm_mean and sorted_t[-1] > 100:
            detected_cold = True

    return {
        "scenario_id": scenario_id,
        "runs": n,
        "pass_rate": 1.0,
        "flaky": False,
        "timing": {
            "mean": round(mean, 2),
            "std_dev": round(std, 2),
            "p50": sorted_t[min(int(n * 0.5), n - 1)],
            "p95": sorted_t[min(int(n * 0.95), n - 1)],
            "p99": sorted_t[min(int(n * 0.99), n - 1)],
            "min": round(min(timings), 2),
            "max": round(max(timings), 2),
            "cv": round(cv, 4),
            "cv_warm": cv_warm,
            "cold_cache_sample": (
                cold_cache_sample
                if cold_cache_sample is not None
                else detected_cold
            ),
        },
        "outcome_distribution": {"pass": n},
        **(extra or {}),
    }


def _stability(
    scenario_stats: list[dict],
    *,
    campaign_id: str = "20260919T102509",
    measured_source_head: str = "d1f771ba",
    repeat_count: int = 2,
    parent_dir: str = "sandbox/results-runs/20260919T102509",
    aggregate_cv: float | None = None,
    aggregate_health_score: float | None = None,
    pass_rate: float = 100.0,
) -> dict:
    """Build a stability.json-compatible dict."""
    cvs = [s["timing"]["cv_warm"] for s in scenario_stats if s["timing"]["cv_warm"] is not None]
    return {
        "parent_dir": parent_dir,
        "campaign_id": campaign_id,
        "measured_source_head": measured_source_head,
        "repeat_count": repeat_count,
        "total_scenarios": len(scenario_stats),
        "total_runs": sum(s["runs"] for s in scenario_stats),
        "pass_rate": pass_rate,
        "health_score": aggregate_health_score if aggregate_health_score is not None else 97.5,
        "timing_cv": aggregate_cv if aggregate_cv is not None else (
            round(sum(cvs) / len(cvs), 4) if cvs else 0
        ),
        "scenario_stats": scenario_stats,
    }


def _write_stability(tmp_path: Path, name: str, stab: dict) -> Path:
    p = tmp_path / name
    p.write_text(json.dumps(stab))
    return p


# ── Constants surface ────────────────────────────────────────────────────────


def test_g6_constants_observable():
    """Pin the contract as observable constants on the reader."""
    assert hasattr(rs, "G6_CV_THRESHOLD"), "reader must expose G6_CV_THRESHOLD"
    assert hasattr(rs, "G6_MIN_REPEATS"), "reader must expose G6_MIN_REPEATS"
    assert rs.G6_CV_THRESHOLD == 0.10, f"threshold must be 0.10, got {rs.G6_CV_THRESHOLD}"
    assert rs.G6_MIN_REPEATS == 3, f"min repeats must be 3, got {rs.G6_MIN_REPEATS}"


# ── Contract pins (RED tests against the current reader) ────────────────────


def test_reader_requires_repeat_count_at_least_three(tmp_path):
    """R-G6-1: spec says --repeat >= 3. Reader must reject campaigns with fewer.

    With repeat_count < 3, the gate cannot produce a release-stability
    verdict — that data is diagnostic, not release evidence.
    """
    stab = _stability(
        [_scenario_stats("a", [100, 110]),
         _scenario_stats("b", [200, 250])],
        repeat_count=2,                    # < 3
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"repeat_count=2 < spec minimum of 3 -> AMBER; got {g.status} ({g.evidence_text})"
    )
    assert "insufficient_repeats" in g.evidence_text.lower() or "repeat" in g.evidence_text.lower(), \
        f"AMBER reason must reference repeat count; got: {g.evidence_text}"


def test_reader_requires_at_least_three_samples_per_scenario(tmp_path):
    """R-G6-2: per-scenario stability requires n >= 3.

    Even if repeat_count >= 3 overall, individual scenarios with
    fewer samples cannot be evaluated for run-to-run variance.
    """
    stab = _stability(
        [
            _scenario_stats("good", [100, 110, 105]),     # n=3
            _scenario_stats("bad", [100, 110]),            # n=2 — insufficient
        ],
        repeat_count=3,
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    # Either:
    #   - the scenario with n=2 is excluded, and good gives a verdict
    #   - the scenario with n=2 forces AMBER for insufficient_samples
    # The reader MUST NOT silently include the n=2 scenario as evidence.
    if g.status in ("GREEN", "RED"):
        assert "bad" not in g.evidence_text or "insufficient" in g.evidence_text.lower(), \
            f"n=2 scenario must be flagged as insufficient if included; got: {g.evidence_text}"
    else:
        assert g.status in ("AMBER", "INCOMPLETE"), \
            f"expected AMBER if n<3 scenarios are present; got {g.status} ({g.evidence_text})"


def test_cv_warm_equal_to_cv_with_insufficient_samples_must_be_flagged(tmp_path):
    """R-G6-3: cv_warm == cv when n < 3 must be flagged, not silently accepted.

    analyze_stability.py returns cv_warm == cv when n < 3 because the
    warm-cache drop cannot be applied. The reader must NOT treat that
    as a warm-cache measurement; it must report it as warm-cache NOT
    applicable with the same value as cv.
    """
    # n=2 with high variance — exactly the frozen-run culprit pattern.
    stab = _stability(
        [_scenario_stats("rust_safe_refactor_rename_concrete_concrete",
                          [16242, 615])],
        repeat_count=2,
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    # The n=2 scenario alone cannot drive a G6 verdict. AMBER at minimum.
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"n=2 with high variance must be AMBER (insufficient samples), "
        f"not RED/GREEN; got {g.status} ({g.evidence_text})"
    )
    # The evidence must NOT call this a "warm-cache" measurement.
    # If the value is reported, it must be marked as warm-cache N/A.
    assert "warm-cache n/a" in g.evidence_text.lower() or \
           "warm_cache n/a" in g.evidence_text.lower() or \
           "warm-cache not applicable" in g.evidence_text.lower() or \
           "insufficient" in g.evidence_text.lower(), \
        f"evidence must distinguish warm-cache N/A from warm-cache applied; " \
        f"got: {g.evidence_text}"


def test_empty_timings_does_not_produce_zero_cv_green(tmp_path):
    """R-G6-4: empty measurements are NOT evidence.

    cv=0 because there are zero samples is not a measurement; it's a
    default value. The reader must distinguish "no measurement" from
    "measured CV of zero".
    """
    stab = _stability(
        [_scenario_stats("empty", [])],
        repeat_count=3,
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"no samples must be AMBER, not GREEN; got {g.status} ({g.evidence_text})"
    assert "empty" in g.evidence_text.lower() or "no sample" in g.evidence_text.lower(), \
        f"the scenario with no samples must be named; got: {g.evidence_text}"


def test_aggregate_cv_is_a_second_opinion_signal(tmp_path):
    """R-G6-7: aggregate_cv from stability.json is a stronger signal than max per-scenario.

    With n>=3 samples, the aggregate CV across all scenarios is a
    release-stability summary. The reader should consult it as a
    second opinion — when aggregate_cv >= 10%, even if no single
    per-scenario cv crosses 10%, the gate must surface that signal.
    """
    # 5 scenarios, all per-scenario cv warm < 10%, but aggregate_cv = 15%
    scenarios = []
    for i in range(5):
        # Each scenario: cv ~ 6% warm, but all are 6% so aggregate ~ 6% — too clean.
        # Use a different distribution: have one scenario high cv that brings
        # the aggregate up but stays under 10% per-scenario. Hard; instead
        # construct a case where aggregate_cv in stability.json disagrees
        # with the per-scenario mean — the disagreement itself is the signal.
        scenarios.append(_scenario_stats(f"sc{i}", [100, 105, 102]))
    stab = _stability(
        scenarios,
        repeat_count=3,
        aggregate_cv=0.42,                 # huge aggregate, far above 10%
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    # Per-scenario cv ~ 2-3% — but aggregate_cv=0.42 (42%) is a strong
    # warning. The reader must surface this, not declare GREEN.
    assert g.status in ("AMBER", "RED"), \
        f"aggregate_cv=42% must not be hidden behind per-scenario averages; " \
        f"got {g.status} ({g.evidence_text})"
    assert "aggregate_cv" in g.evidence_text.lower() or "aggregate" in g.evidence_text.lower(), \
        f"aggregate_cv must be cited; got: {g.evidence_text}"


def test_evidence_text_distinguishes_warm_vs_full(tmp_path):
    """R-G6-8: evidence_text must say whether warm-cache was applied.

    When cv_warm == cv (cold-cache policy not applied), the evidence
    must NOT advertise "warm-cache CV" — that's misleading. It must
    say "warm-cache N/A" or "full CV (n<3, warm-cache policy not applicable)".
    """
    stab = _stability(
        [_scenario_stats("a", [100, 200])],
        repeat_count=2,
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    # The current reader advertises warm-cache measurements for n<2 cases.
    # The test pins the honest distinction.
    assert "warm-cache n/a" in g.evidence_text.lower() or \
           "warm-cache not applicable" in g.evidence_text.lower() or \
           "(n<3, warm-cache policy not applicable)" in g.evidence_text.lower() or \
           "insufficient" in g.evidence_text.lower(), \
        f"evidence must explicitly distinguish warm-cache applied vs not; " \
        f"got: {g.evidence_text}"


# ── Verdict precedence ───────────────────────────────────────────────────────


def test_all_warm_cv_below_threshold_is_green(tmp_path):
    """All scenarios with n >= 3 and warm cv < 10% -> GREEN."""
    scenarios = []
    for i in range(5):
        # 3 samples each, low variance
        scenarios.append(_scenario_stats(f"sc{i}", [100 + i, 102 + i, 101 + i]))
    stab = _stability(scenarios, repeat_count=3)
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    assert g.status == "GREEN", \
        f"all warm cv < 10% with sufficient samples -> GREEN; got {g.status} ({g.evidence_text})"


def test_one_warm_cv_above_threshold_is_red_with_scenario_named(tmp_path):
    """5 scenarios with n>=3, one with cv warm >= 10% -> RED with scenario named."""
    scenarios = []
    for i in range(4):
        scenarios.append(_scenario_stats(f"sc{i}", [100 + i, 102 + i, 101 + i]))
    # Flaky scenario with high variance
    scenarios.append(_scenario_stats("flaky", [100, 200, 300]))  # mean 200, std ~81, cv ~0.4
    stab = _stability(scenarios, repeat_count=3)
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    assert g.status == "RED", \
        f"one warm cv >= 10% with sufficient samples -> RED; got {g.status} ({g.evidence_text})"
    assert "flaky" in g.evidence_text, \
        f"failing scenario must be named; got: {g.evidence_text}"


def test_warm_cache_applied_dropping_a_clear_outlier_lowers_cv(tmp_path):
    """When cold cache is detected, cv_warm should be lower than cv."""
    # Cold sample is 5x the warm ones. cv full is high, cv warm is low.
    stab = _stability(
        [_scenario_stats("a", [100, 105, 102, 99, 5000])],  # last sample is cold
        repeat_count=5,
    )
    p = _write_stability(tmp_path, "stability.json", stab)
    g = rs.gate_g6(str(p))
    # cv full would be > 100%, cv warm should be < 10%
    assert g.status == "GREEN", \
        f"warm-cache drop on a clear cold outlier -> GREEN; got {g.status} ({g.evidence_text})"
    # The evidence must show the cold-cache drop happened.
    assert "cold_cache_sample=true" in g.evidence_text or \
           "cold_cache" in g.evidence_text.lower() or \
           "dropped" in g.evidence_text.lower(), \
        f"evidence must surface the cold-cache drop; got: {g.evidence_text}"


# ── Frozen campaign integration ──────────────────────────────────────────────


def test_frozen_campaign_yields_amber_for_insufficient_repeats():
    """The frozen 20260919T102509 campaign has repeat_count=2, must yield AMBER."""
    p = REPO / "sandbox/results-runs/20260919T102509/stability.json"
    if not p.exists():
        # Skip if the frozen run dir isn't present (CI may not have it).
        return
    g = rs.gate_g6(str(p))
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"frozen run repeat_count=2 < spec minimum of 3 -> AMBER; "
        f"got {g.status} ({g.evidence_text})"
    )
    # Must NOT cite the 92.7% as warm-cache release-readiness evidence.
    # It can appear as diagnostic, but only with an insufficient_repeats marker.
    assert "insufficient_repeats" in g.evidence_text.lower() or \
           "insufficient" in g.evidence_text.lower() or \
           "repeat" in g.evidence_text.lower(), \
        f"AMBER reason must reference repeat count; got: {g.evidence_text}"


def test_frozen_campaign_max_cv_scenario_identified():
    """The 92.7% scenario must be named explicitly."""
    p = REPO / "sandbox/results-runs/20260919T102509/stability.json"
    if not p.exists():
        return
    g = rs.gate_g6(str(p))
    assert "rust_safe_refactor_rename_concrete_concrete" in g.evidence_text, \
        f"max-CV scenario must be named; got: {g.evidence_text}"
