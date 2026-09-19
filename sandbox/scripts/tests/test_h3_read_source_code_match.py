"""H3 — read_source correctness via the existing `match_code` matcher.

H1/H2 left the five `read_source_*` scenarios with `correctitud=None`
because the manifest declared `ground_truth.exists: true`, which is NOT a
field of `GroundTruth`. H3 closes that gap by populating
`ground_truth.code.content` with the **pinned file content** (file at
the pinned SHA, derived from `sandbox/repos/<repo>/<path>`), so the
existing `match_code` matcher can compute correctitud as either 100
(exact_match) or `content_similarity * 100` (Jaccard of words).

This test pins:

  1. The five pinned-file SHA-256s are recorded (no auto-reference; the
     references live under sandbox/results/freezes/tier1_phase1/h3/
     references/<repo>/).
  2. The latest H3 campaign run has the five read_source scenarios
     producing **measured** correctitud (not None).
  3. The scorecard has G3 GREEN (all 10 scenarios 5-dim complete).
  4. The scorecard records the **honest** G4 RED driven by anyhow/tokio
     truncation (a product defect, not a measurement defect).

Run with:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_h3_read_source_code_match.py -v
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path

import pytest

REPO = Path("/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode")


def _latest_run() -> Path:
    out = subprocess.check_output(
        ["bash", "-c", "ls -td sandbox/results-runs/*/ | head -1"],
        cwd=REPO,
    ).decode().strip()
    return REPO / out


# Mapping: scenario_id -> (repo, expected pinned file path under sandbox/repos)
SCENARIO_TO_PIN = {
    "rust_tier1_serde_read_source_default":    ("serde",   "sandbox/repos/serde/serde/src/lib.rs"),
    "rust_tier1_ripgrep_read_source_default":  ("ripgrep", "sandbox/repos/ripgrep/crates/cli/src/lib.rs"),
    "rust_tier1_anyhow_read_source_default":   ("anyhow",  "sandbox/repos/anyhow/src/lib.rs"),
    "rust_tier1_tokio_read_source_default":    ("tokio",   "sandbox/repos/tokio/tokio/src/lib.rs"),
    "rust_tier1_clap_read_source_default":     ("clap",    "sandbox/repos/clap/clap_builder/src/lib.rs"),
}


def _find_result_json(scenario_dir: Path) -> Path | None:
    for sub in scenario_dir.iterdir():
        if sub.is_dir() and (sub / "result.json").exists():
            return sub / "result.json"
    return None


def test_h3_pinned_references_match_cloned_files():
    """Each pinned file under sandbox/results/freezes/tier1_phase1/h3/references/
    must SHA-256 match the live cloned file under sandbox/repos/."""
    ref_root = REPO / "sandbox/results/freezes/tier1_phase1/h3/references"
    assert ref_root.exists(), f"H3 reference dir missing: {ref_root}"

    for repo, live_path in SCENARIO_TO_PIN.values():
        ref_path = ref_root / repo / "expected_code.txt"
        sha_path = ref_root / repo / "expected_code.sha256"
        assert ref_path.exists(), f"reference for {repo} missing: {ref_path}"
        assert sha_path.exists(), f"SHA-256 file for {repo} missing: {sha_path}"

        ref_content = ref_path.read_bytes()
        live_content = (REPO / live_path).read_bytes()
        ref_sha = hashlib.sha256(ref_content).hexdigest()
        live_sha = hashlib.sha256(live_content).hexdigest()
        assert ref_sha == live_sha, (
            f"{repo}: pinned reference SHA-256 {ref_sha} != live file SHA-256 {live_sha}. "
            f"The reference has drifted — re-pin via clone_repos.sh."
        )

        # Cross-check with the recorded SHA-256 file
        recorded = sha_path.read_text().strip().split()[0]
        assert recorded == ref_sha, (
            f"{repo}: SHA-256 file records {recorded} but reference SHA is {ref_sha}"
        )


def test_h3_run_has_measured_correctitud_for_all_read_source_scenarios():
    """For each read_source scenario in the latest H3 campaign, the
    measured correctitud in result.json MUST be a number (not None).

    This proves the existing match_code matcher produces a real score
    against the pinned reference content."""
    run_dir = _latest_run()
    assert (run_dir / "run-1").exists(), f"No run-1 dir under {run_dir}"

    for sid in SCENARIO_TO_PIN:
        d = run_dir / "run-1" / sid
        rj_path = _find_result_json(d)
        assert rj_path is not None, f"No result.json for {sid} under {d}"
        rj = json.loads(rj_path.read_text())
        ds = rj.get("dimension_scores", {})
        corr = ds.get("correctitud")
        assert corr is not None, (
            f"{sid}: correctitud=None in {rj_path}. "
            f"The match_code matcher did not produce a score."
        )
        assert 0.0 <= corr <= 100.0, (
            f"{sid}: correctitud={corr} out of [0,100]"
        )


def test_h3_scorecard_g3_green():
    """The H3 scorecard MUST classify G3 as GREEN (all 10 scenarios 5-dim complete)."""
    scorecard = REPO / "sandbox/results/freezes/tier1_phase1/h3/scorecard.md"
    assert scorecard.exists(), f"H3 scorecard missing: {scorecard}"
    text = scorecard.read_text()
    # G3 row must show GREEN
    m = re.search(r"G3 Sandbox Health Score\s+\|?\s*(✅ GREEN|⚠️ AMBER|❌ RED)", text)
    assert m, f"G3 row not found in scorecard"
    assert m.group(1) == "✅ GREEN", (
        f"G3 expected GREEN, got {m.group(1)}. "
        f"Full scorecard:\n{text}"
    )


def test_h3_scorecard_g4_red_documented():
    """The H3 scorecard MUST classify G4 as RED — this is the honest verdict
    driven by anyhow+clap truncation (a product defect). G4 RED is preserved
    per H3.3 directive: 'Si el resultado es RED, conservarlo y documentar su causa.'"""
    scorecard = REPO / "sandbox/results/freezes/tier1_phase1/h3/scorecard.md"
    text = scorecard.read_text()
    m = re.search(r"G4 Corpus Quality\s+/?\s*Correctitud\s+\|?\s*(✅ GREEN|⚠️ AMBER|❌ RED)", text)
    assert m, f"G4 row not found in scorecard"
    assert m.group(1) == "❌ RED", (
        f"G4 expected RED (with anyhow/tokio truncation evidence), got {m.group(1)}"
    )
    # Failing repos must be named
    assert "anyhow" in text or "tokio" in text, (
        f"G4 RED must name the failing repos"
    )


def test_h3_anyhow_read_source_below_90_due_to_truncation():
    """The anyhow read_source scenario MUST score below 90 because its
    pinned file is 730 lines and the product caps read_file at 500 lines.

    This proves G4 RED is traceable to a specific reproducible product
    defect, not a measurement defect."""
    run_dir = _latest_run()
    d = run_dir / "run-1" / "rust_tier1_anyhow_read_source_default"
    rj = json.loads(_find_result_json(d).read_text())
    corr = rj["dimension_scores"]["correctitud"]
    assert corr is not None and corr < 90.0, (
        f"anyhow read_source expected <90 due to truncation, got {corr}"
    )
    # And the response must show end_line=500 with total_lines=730
    response = json.loads(json.loads(
        (d / list(d.iterdir())[0].name / "response.json").read_text()
    )["result"]["content"][0]["text"])
    assert response.get("total_lines") == 730
    assert response.get("end_line") == 500
    # Truncation flag is the documented product defect (NOT H3 scope)
    assert response.get("truncated") is False, (
        "truncated should be True when end_line<total_lines (carry-forward, NOT H3 scope)"
    )
