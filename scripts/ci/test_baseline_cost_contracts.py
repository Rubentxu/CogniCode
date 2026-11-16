#!/usr/bin/env python3
"""CAA-01 baseline contracts: what the instruments must keep declaring.

These tests check the instruments themselves, not the product: the corpus
is deterministic and holds the five change classes, the phases are the ones
the acceptance criteria name, and the status discipline can never fold a
failure into a PASS. They are contract tests like scripts/ci/test_gate_
contracts.py, not measurements.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "measure"))

import baseline_cost  # noqa: E402
import generate_baseline_corpus as corpus  # noqa: E402


def test_corpus_is_deterministic() -> None:
    first = corpus.corpus_files(6)
    second = corpus.corpus_files(6)
    assert first == second, "two generations of the same size differ"
    assert corpus.corpus_digest(first) == corpus.corpus_digest(second)
    assert corpus.corpus_digest(first) != corpus.corpus_digest(corpus.corpus_files(7))


def test_corpus_holds_the_five_change_classes() -> None:
    assert set(corpus.CHANGE_CLASSES) == {
        "function_body",
        "public_signature",
        "manifest",
        "terraform",
        "helm",
    }
    files = corpus.corpus_files(12)
    for change_class, (path, old, new) in corpus.CHANGE_CLASSES.items():
        assert path in files, f"{change_class} names a file the corpus does not have: {path}"
        assert old in files[path], f"{change_class} lost its anchor in {path}"
        assert old != new


def test_apply_change_moves_exactly_one_file() -> None:
    files = corpus.corpus_files(12)
    with tempfile.TemporaryDirectory() as root:
        workspace = Path(root) / "workspace"
        for change_class, (path, _old, new) in corpus.CHANGE_CLASSES.items():
            corpus.apply_change(workspace, change_class, 12)
            assert new in (workspace / path).read_text(encoding="utf-8")
            changed = [
                relative
                for relative in files
                if (workspace / relative).read_text(encoding="utf-8") != files[relative]
            ]
            assert changed == [path], f"{change_class} bled into {changed}"


def test_the_phases_are_the_ones_the_acceptance_names() -> None:
    assert baseline_cost.PHASES == [
        "cache_lookup_ms",
        "walk_ms",
        "parse_ms",
        "ingest_ms",
        "resolve_ms",
        "build_ms",
        "snapshot_serialize_ms",
    ]
    assert set(baseline_cost.SCENARIOS) == {"cold", "warm", "incremental"}


def test_a_failure_is_never_a_pass() -> None:
    assert baseline_cost.aggregate_status(["PASS", "PASS"]) == "PASS"
    assert baseline_cost.aggregate_status(["PASS", "FAIL"]) == "FAIL"
    assert baseline_cost.aggregate_status(["PASS", "BLOCKED"]) == "BLOCKED"
    assert baseline_cost.aggregate_status([]) == "NOT_RUN"
    assert baseline_cost.aggregate_status(["NOT_RUN"]) == "NOT_RUN"


def test_percentile_never_invents_a_number_of_nothing() -> None:
    assert baseline_cost.percentile([10.0], 0.5) == 10.0
    assert baseline_cost.percentile([0.0, 100.0], 0.5) == 50.0
    try:
        baseline_cost.percentile([], 0.95)
    except ValueError:
        return
    raise AssertionError("a percentile of nothing must not be published")


def test_the_gate_rows_are_distinguished_and_never_green() -> None:
    gate = baseline_cost.unmeasured_gate_rows()
    assert set(gate) == {"test_compilation", "ci_wait"}
    for name, row in gate.items():
        assert row["status"] == "NOT_RUN", f"{name} cannot be green from this instrument"
        assert row["reason"], f"{name} must say why it was not measured"


def test_a_partial_or_rebuilt_warm_answer_is_never_a_baseline() -> None:
    assert baseline_cost.baseline_honesty("complete", "built", False) is None
    assert baseline_cost.baseline_honesty("complete", "cache", True) is None
    partial = baseline_cost.baseline_honesty("partial", "built", False)
    assert partial is not None and "partial" in partial
    rebuilt = baseline_cost.baseline_honesty("complete", "built", True)
    assert rebuilt is not None and "rebuilt" in rebuilt


def test_run_graph_refuses_a_binary_that_cannot_start() -> None:
    result = baseline_cost.run_graph(
        Path("/nonexistent/cognicode"), Path("/tmp"), Path("/tmp"), 5.0
    )
    assert result["status"] in ("FAIL", "BLOCKED"), "a run that cannot start is never a PASS"


if __name__ == "__main__":
    tests = [each for name, each in sorted(globals().items()) if name.startswith("test_")]
    failed = 0
    for each in tests:
        try:
            each()
            print(f"ok  {each.__name__}")
        except Exception as error:  # noqa: BLE001
            failed += 1
            print(f"FAIL {each.__name__}: {error}")
    raise SystemExit(1 if failed else 0)
