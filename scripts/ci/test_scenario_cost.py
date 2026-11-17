#!/usr/bin/env python3
"""Contracts of the scenario-cost instrument: what it may never fold together.

These tests check the instrument, not the product. They pin the percentile
definition, the separation of lock wait from compute, the rule that a failed
run is never folded into a time as if it passed, and the overlay-to-edits
translation the guard measurement depends on.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "measure"))

import scenario_cost as cost  # noqa: E402


def test_percentile_is_nearest_rank_and_p95_of_few_runs_is_the_maximum() -> None:
    values = [5.0, 1.0, 3.0, 2.0, 4.0]
    assert cost.percentile(values, 50) == 3.0
    assert cost.percentile(values, 95) == 5.0
    assert cost.percentile([7.0], 50) == 7.0


def test_a_percentile_of_nothing_is_refused() -> None:
    try:
        cost.percentile([], 50)
    except ValueError:
        return
    raise AssertionError("an empty set must not yield a number")


def test_lock_wait_is_the_gap_after_the_lock_message_and_never_negative_compute() -> None:
    stamped = [(0.0, "Compiling a"), (1.0, "Blocking waiting for file lock on build directory"), (3.5, "Compiling b")]
    assert cost.lock_wait_ms(stamped, 4.0) == 2500.0
    last = [(1.0, "Blocking waiting for file lock on package cache")]
    assert cost.lock_wait_ms(last, 2.0) == 1000.0
    assert cost.lock_wait_ms([(0.0, "ok")], 9.0) == 0.0
    sample = cost.Sample(wall_ms=1000.0, lock_wait_ms=2500.0, exit_code=0)
    assert sample.compute_ms == 0.0


def test_summary_counts_exit_codes_beside_the_times() -> None:
    samples = [cost.Sample(100.0, 0.0, 0), cost.Sample(300.0, 50.0, 101)]
    summary = cost.summarise(samples)
    assert summary["exit_codes"] == {"0": 1, "101": 1}
    assert summary["runs_that_waited_for_a_lock"] == 1
    assert summary["compute_ms"]["max_ms"] == 250.0
    assert summary["wall_ms"]["n"] == 2


def test_a_gate_run_fails_if_any_stage_failed() -> None:
    stages = [cost.Sample(10.0, 0.0, 0), cost.Sample(20.0, 5.0, 1)]
    total = cost.run_total(stages)
    assert (total.wall_ms, total.lock_wait_ms, total.exit_code) == (30.0, 5.0, 1)


def test_pr_cost_times_finished_checks_and_counts_unfinished_ones() -> None:
    pulls = [{
        "created_at": "2026-01-01T00:00:00Z",
        "merged_at": "2026-01-01T01:00:00Z",
        "check_runs": [
            {"name": "guard", "started_at": "2026-01-01T00:00:10Z", "completed_at": "2026-01-01T00:01:10Z", "conclusion": "success"},
            {"name": "guard", "started_at": "2026-01-01T00:10:00Z", "completed_at": None, "conclusion": None},
        ],
    }]
    report = cost.pr_cost(pulls)
    assert report["check_runs"]["guard"]["p50_ms"] == 60000.0
    assert report["check_runs"]["guard"]["conclusions"] == {"success": 1, "None": 1}
    assert report["unfinished_check_runs"] == 1
    assert report["open_to_merge"]["p50_ms"] == 3_600_000.0


def test_overlay_becomes_edits_with_deletions_as_null_and_no_oracle() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        overlay = Path(tmp)
        (overlay / "src").mkdir()
        (overlay / "src" / "lib.rs").write_text("fn a() {}\n")
        (overlay / "DELETE").write_text("src/old.rs\n")
        (overlay / "ORACLE").write_text("ignored\n")
        assert cost.edits_json(overlay) == {"src/old.rs": None, "src/lib.rs": "fn a() {}\n"}


def test_applying_an_overlay_deletes_and_writes() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        base, overlay = Path(tmp) / "base", Path(tmp) / "overlay"
        (base / "src").mkdir(parents=True)
        (base / "src" / "old.rs").write_text("old")
        (overlay / "src").mkdir(parents=True)
        (overlay / "src" / "new.rs").write_text("new")
        (overlay / "DELETE").write_text("src/old.rs\n")
        cost.apply_overlay(overlay, base)
        assert not (base / "src" / "old.rs").exists()
        assert (base / "src" / "new.rs").read_text() == "new"
        assert not (base / "DELETE").exists()


def test_delta_outcome_keeps_the_verdict_next_to_the_speed() -> None:
    answer = (
        '{"delta": {"status": "inconclusive", "fully_observed": false,'
        ' "candidate": {"impact_status": "partial"}, "summary": {"introduced": 0}}}'
    )
    assert cost.delta_outcome(answer) == {
        "status": "inconclusive", "fully_observed": False,
        "impact_status": "partial", "summary": {"introduced": 0},
    }


def test_the_real_corpus_has_scenarios_and_every_language_without_a_gate_says_why() -> None:
    found = cost.scenarios(REPO_ROOT / cost.CORPUS)
    assert found, "the corpus has no scenarios"
    for part, _klass, _overlay in found:
        gated = part in ("rust", "cross", "helm")
        assert gated or part in cost.LANGUAGE_TOOLS, f"part {part} has neither a gate nor a stated reason"


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"ok   {name}")
            except Exception as error:  # noqa: BLE001
                failures += 1
                print(f"FAIL {name}: {error!r}")
    sys.exit(1 if failures else 0)
