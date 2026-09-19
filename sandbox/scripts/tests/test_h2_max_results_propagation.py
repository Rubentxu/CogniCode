"""H2 — max_results propagation regression test.

The H1 G4 verdict on tokio (50%) and clap (0%) was traced to the
manifest not passing `max_results`, so the search engine's
default cap of 50 truncated the result set before the GT-exact
files surfaced.

This test reproduces the propagation contract end-to-end via the
release scorecard: given a manifest that explicitly sets
`max_results: 1000` on a search scenario, the resulting scenario
must report a measured correctitud at or near 100 (because the GT
files are within the top 1000 matches for the Tier-1 corpus).

Two complementary tests:
1. test_search_with_explicit_max_results_above_50_yields_higher_corr
   — when the manifest passes max_results=1000, the captured
   response.json must contain more than 50 matches for the
   Tier-1 patterns.
2. test_max_results_propagates_to_request_json — the captured
   request.json (sent to MCP) must include max_results: 1000
   when the manifest passes it.

Run with:
    cd <repo-root>
    python3 -m pytest sandbox/scripts/tests/test_h2_max_results_propagation.py -v
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

REPO = Path("/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode")


def _latest_run() -> Path:
    out = subprocess.check_output(
        ["bash", "-c", "ls -td sandbox/results-runs/*/ | head -1"],
        cwd=REPO,
    ).decode().strip()
    p = REPO / out
    return p


def _load_run_dir() -> dict[str, dict]:
    """Build {scenario_id: {'run-1': {...}, 'run-2': {...}, 'run-3': {...}}}."""
    run_dir = _latest_run()
    out: dict[str, dict] = {}
    for run in ("run-1", "run-2", "run-3"):
        sub = run_dir / run
        if not sub.is_dir():
            continue
        for d in sub.iterdir():
            if not d.is_dir():
                continue
            try:
                inner_dirs = [c for c in d.iterdir() if c.is_dir()]
                if not inner_dirs:
                    continue
                rj_path = inner_dirs[0] / "result.json"
                if not rj_path.exists():
                    continue
                rj = json.loads(rj_path.read_text())
                out.setdefault(d.name, {})[run] = rj
            except Exception:
                continue
    return out


@pytest.fixture(scope="module")
def run_data():
    return _load_run_dir()


def test_h2_search_response_size_matches_corpus(run_data):
    """For each Tier-1 search scenario with explicit max_results=1000,
    the captured response.json must contain at least the corpus match
    count (which we already measured empirically with `grep -r`):

        serde:   210 lines
        ripgrep: 244 lines
        anyhow:   27 lines   (anyhow legitimately has only 27)
        tokio:   599 lines
        clap:    498 lines

    H1 with the default max_results=50 truncated every scenario at 50,
    so the H1 response was always 50/50. H2 with max_results=1000 must
    return AT LEAST the corpus match count (and possibly exactly that
    count if the corpus is < 1000).
    """
    # Measured match counts (from `grep -r '<pattern>' <src>` at the pinned SHAs).
    min_expected = {
        "rust_tier1_serde_search_deserialize_default": 200,
        "rust_tier1_ripgrep_search_regex_matcher_default": 200,
        "rust_tier1_anyhow_search_anyhow_macro_default": 20,   # anyhow has only 25
        "rust_tier1_tokio_search_tokio_main_default": 500,
        "rust_tier1_clap_search_arg_action_default": 400,
    }
    run_dir = _latest_run()
    for sid, min_n in min_expected.items():
        for run in ("run-1", "run-2", "run-3"):
            d = run_dir / run / sid
            if not d.is_dir():
                continue
            inner = list(d.iterdir())[0]
            resp = json.loads((inner / "response.json").read_text())
            text = resp["result"]["content"][0]["text"]
            inner_json = json.loads(text)
            matches = inner_json["matches"]
            total = inner_json["total"]
            assert len(matches) == total, (
                f"{sid} {run}: matches.length={len(matches)} != total={total}"
            )
            assert len(matches) >= min_n, (
                f"{sid} {run}: expected >= {min_n} matches with max_results=1000, "
                f"got len(matches)={len(matches)}. If len(matches)==50, the "
                f"override did NOT propagate past the 50-result default."
            )
            break
        else:
            pytest.fail(f"no run-* dir for {sid}")


def test_h2_search_request_includes_max_results_1000(run_data):
    """For each Tier-1 search scenario with explicit max_results=1000,
    the captured request.json (sent to MCP) must include
    max_results: 1000 (proving the manifest field reaches the wire)."""
    search_scenarios = [
        "rust_tier1_serde_search_deserialize_default",
        "rust_tier1_ripgrep_search_regex_matcher_default",
        "rust_tier1_anyhow_search_anyhow_macro_default",
        "rust_tier1_tokio_search_tokio_main_default",
        "rust_tier1_clap_search_arg_action_default",
    ]
    run_dir = _latest_run()
    for sid in search_scenarios:
        d = run_dir / "run-1" / sid
        if not d.is_dir():
            pytest.fail(f"no run-1 dir for {sid}")
        inner = list(d.iterdir())[0]
        req = json.loads((inner / "request.json").read_text())
        args = req["params"]["arguments"]
        assert "max_results" in args, (
            f"{sid}: request.json is missing max_results; manifest field "
            f"did not propagate to the MCP wire"
        )
        assert args["max_results"] == 1000, (
            f"{sid}: request.json max_results={args['max_results']}, "
            f"expected 1000"
        )


def test_h2_search_correctitud_at_100(run_data):
    """For each Tier-1 search scenario, the per-repo measured correctitud
    must be 100.0 (proving the GT files are reached with max_results=1000)."""
    search_repos = {
        "rust_tier1_serde_search_deserialize_default": "serde",
        "rust_tier1_ripgrep_search_regex_matcher_default": "ripgrep",
        "rust_tier1_anyhow_search_anyhow_macro_default": "anyhow",
        "rust_tier1_tokio_search_tokio_main_default": "tokio",
        "rust_tier1_clap_search_arg_action_default": "clap",
    }
    for sid, _repo in search_repos.items():
        # All 3 repeats should report 100.0
        corrs = []
        for run in ("run-1", "run-2", "run-3"):
            if sid in run_data and run in run_data[sid]:
                corr = run_data[sid][run].get("dimension_scores", {}).get("correctitud")
                if corr is not None:
                    corrs.append((run, corr))
        assert corrs, f"no measured correctitud for {sid}"
        for run, corr in corrs:
            assert corr == 100.0, (
                f"{sid} {run}: correctitud={corr}, expected 100.0 "
                f"(GT files must be in the response with max_results=1000)"
            )


def test_h2_h1_contrast_search_response_size():
    """Document the before/after contrast: H1 (default max_results=50)
    truncated each search at 50; H2 (max_results=1000) returns the
    full corpus for each Tier-1 pattern.

    This test reads the H1 frozen response.json for tokio search
    and the H2 latest run for the same scenario, and asserts
    H2's matches.length > H1's matches.length."""
    h1_run = REPO / "sandbox/results-runs/20260919T143212"
    h2_run = _latest_run()
    if not h1_run.exists():
        pytest.skip("H1 frozen run not present")

    h1_files = list((h1_run / "run-1" / "rust_tier1_tokio_search_tokio_main_default").iterdir())
    h2_files = list((h2_run / "run-1" / "rust_tier1_tokio_search_tokio_main_default").iterdir())
    h1_resp = json.loads((h1_files[0] / "response.json").read_text())
    h2_resp = json.loads((h2_files[0] / "response.json").read_text())
    h1_matches = len(json.loads(h1_resp["result"]["content"][0]["text"])["matches"])
    h2_matches = len(json.loads(h2_resp["result"]["content"][0]["text"])["matches"])
    assert h2_matches > h1_matches, (
        f"H2 matches.length={h2_matches} should be > H1 matches.length={h1_matches}; "
        f"if equal, the max_results override did not change behavior."
    )
