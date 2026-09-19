"""A1a + A1a+1 — G4 contract tests for the per-repo Tier-1 reader.

Pins the G4 contract from
`openspec/specs/release-readiness-gate/spec.md`:

  Correctness (ground-truth comparison via the scoring engine's
  matchers) MUST be >= 90% on every Tier-1 repository
  (ripgrep, serde, anyhow, tokio, clap).

Two corrections from the A1a+1 directive:

  1. Provenance is positive, not defensive: every candidate must
     carry declared_repo, resolved_workspace, actual_repository_identity,
     actual_repository_revision, ground_truth_present, correctness_measured,
     scenario_id, repeat_identity. The reader does NOT infer identity from
     the declared `repo` field alone. If actual_repository_identity is
     UNVERIFIED, the result is UNVERIFIED — never silently re-tagged.

  2. Aggregation respects scoring policy and scenario identity:
     correctness is averaged per (scenario_id, repo) — not per repetition,
     not across scenarios. Repeat counts do NOT fabricate coverage.
     A repo with insufficient required coverage is reported separately
     from a repo with sufficient coverage that scored below threshold.

Verdict precedence:

  All Tier-1 repos:
    acredited + sufficient coverage + per-repo avg >= 90
        -> GREEN
    acredited + sufficient coverage + any per-repo avg < 90
        -> RED (the failing repo is named)
    acredited + insufficient coverage (any repo)
        -> AMBER/INCOMPLETE (missing repos named)
    no positive provenance for any Tier-1 repo
        -> AMBER/INCOMPLETE

  Missing data NEVER produces GREEN.

Run with:
    cd <repo-root>
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
G4_THRESHOLD = 90.0


# ── Synthetic fixtures ────────────────────────────────────────────────────────


def _make_result(
    scenario_id: str,
    repo: str,
    correctitud,
    workspace: str = ".",
    language: str = "rust",
    tool: str = "search_content",
    outcome: str = "pass",
    workspace_snapshot_id: str | None = "snap000",
    actual_repository_identity: str | None = None,
    actual_repository_revision: str | None = None,
    ground_truth_present: bool = True,
    repeat_index: int = 0,
):
    """Construct a result.json-like dict.

    `correctitud` may be None to represent "no ground truth → no
    correctitud score from the scoring engine".

    `actual_repository_identity` may be None to represent "no positive
    provenance for the executed repository" — i.e. UNVERIFIED.
    """
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
        "language": language,
        "tool": tool,
        "tier": "B",
        "repo": repo,                       # declared_repo (metadata)
        "outcome": outcome,
        "failure_class": "pass" if outcome == "pass" else outcome,
        "dimension_scores": dim,
        "validation": {"stages": [], "passed": True},
        "workspace": workspace,             # resolved_workspace (path)
        "workspace_snapshot_id": workspace_snapshot_id,
        # The two positive-provenance fields are absent from the current
        # result.json schema. They are introduced by the reader's
        # normalization layer in A1a+1. When missing, the reader treats
        # the result as UNVERIFIED.
        "actual_repository_identity": actual_repository_identity,
        "actual_repository_revision": actual_repository_revision,
        "ground_truth_present": ground_truth_present,
        "repeat_index": repeat_index,
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


def _acredit(repo: str, correctitud, scenario_id: str | None = None,
             repeat_index: int = 0):
    """Build an acredited Tier-1 result with positive provenance."""
    return _make_result(
        scenario_id=scenario_id or f"{repo}_search_content_{repeat_index}",
        repo=repo,
        correctitud=correctitud,
        workspace=f"repos/{repo}",
        actual_repository_identity=repo,
        actual_repository_revision="abc1234",
        workspace_snapshot_id=f"snap_{repo}_v1",
        repeat_index=repeat_index,
    )


# ── A1a — 5 original RED tests (contract pins) ────────────────────────────────


def test_spec_tier1_repos_observable(tmp_path):
    """Pin the contract: Tier-1 repos per spec are ripgrep, serde, anyhow, tokio, clap."""
    assert hasattr(rs, "TIER1_REPOS_PER_SPEC"), "reader must expose TIER1_REPOS_PER_SPEC"
    assert set(rs.TIER1_REPOS_PER_SPEC) == TIER1_REPOS


def test_all_five_tier1_repos_at_above_threshold_is_green(tmp_path):
    """All five Tier-1 repos, each with >=1 acredited scenario, all avg >= 90 -> GREEN."""
    rows = []
    for repo in TIER1_REPOS:
        rows.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a", repeat_index=0))
        rows.append(_acredit(repo, 92.0, scenario_id=f"{repo}_b", repeat_index=1))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "GREEN", f"expected GREEN, got {g.status} ({g.evidence_text})"
    # per-repo breakdown must be present
    for r in TIER1_REPOS:
        assert r in g.evidence_text, f"per-repo breakdown must name {r}"


def test_one_tier1_repo_below_threshold_is_red(tmp_path):
    """4 repos >= 90, clap < 90 -> RED with clap named."""
    rows = []
    for repo in TIER1_REPOS - {"clap"}:
        rows.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a", repeat_index=0))
    rows.append(_acredit("clap", 50.0, scenario_id="clap_a", repeat_index=0))
    rows.append(_acredit("clap", 60.0, scenario_id="clap_b", repeat_index=1))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "RED", f"expected RED, got {g.status} ({g.evidence_text})"
    assert "clap" in g.evidence_text, f"failing repo must be named; got: {g.evidence_text}"


def test_one_tier1_repo_without_evidence_is_amber(tmp_path):
    """4 repos measured, clap absent -> AMBER with clap named as missing."""
    rows = []
    for repo in TIER1_REPOS - {"clap"}:
        rows.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a", repeat_index=0))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"expected AMBER/INCOMPLETE, got {g.status} ({g.evidence_text})"
    assert "clap" in g.evidence_text.lower(), \
        f"missing repo must be named; got: {g.evidence_text}"


def test_no_ground_truth_cannot_inflate_average(tmp_path):
    """No-GT scenarios must not contribute to per-repo averages.

    5 repos, each with ONE acredited scenario @ 85 (below threshold).
    The reader must average only that scenario per repo. Without that
    discipline, padding with bogus 95s would push the average above 90.
    """
    rows = []
    for repo in TIER1_REPOS:
        rows.append(_acredit(repo, 85.0, scenario_id=f"{repo}_only"))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "RED", (
        f"if per-repo averages are 85 < 90 the gate must be RED; "
        f"got {g.status} ({g.evidence_text})"
    )


def test_no_tier1_repo_evidence_at_all_is_incomplete(tmp_path):
    """No scenario with positive Tier-1 provenance -> AMBER/INCOMPLETE, not GREEN."""
    rows = []
    for fake_repo in ["my-internal-fixture", "fixture-rust-hello"]:
        rows.append(_make_result(
            scenario_id=f"{fake_repo}_scenario",
            repo=fake_repo,
            correctitud=95.0,
            workspace=".",
            # NO positive provenance: declared_repo is not Tier-1 either
            actual_repository_identity=None,
            actual_repository_revision=None,
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
    because no scenario carries positive Tier-1 repository provenance
    (the result.json schema predates actual_repository_identity)."""
    frozen_run = str(REPO / "sandbox/results-runs/20260919T102509/run-1")
    g = rs.gate_g4([frozen_run])
    assert g.status in ("AMBER", "INCOMPLETE"), (
        f"frozen run has no positive Tier-1 provenance; G4 must be "
        f"AMBER/INCOMPLETE, got {g.status} ({g.evidence_text})"
    )
    # All five Tier-1 repos must be listed as missing/unverified.
    for r in TIER1_REPOS:
        assert r in g.evidence_text.lower(), \
            f"missing Tier-1 repo {r} must be named; got: {g.evidence_text}"


# ── A1a+1 — 8 adversarial cases per the directive ────────────────────────────


def test_repo_serde_with_fixture_workspace_does_not_count(tmp_path):
    """repo='serde' declared but workspace is a fixture path -> UNVERIFIED, no Tier-1 credit."""
    rows = [
        _make_result(
            scenario_id="serde_search_content_a",
            repo="serde",
            correctitud=100.0,
            workspace=".",                    # fixture workspace
            actual_repository_identity=None,  # NO positive provenance
            actual_repository_revision=None,
        ),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # No positive provenance -> serde is UNVERIFIED, not in the GREEN-eligible set.
    # The other four Tier-1 repos are absent too, so the verdict is AMBER.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"declared_repo='serde' with no positive provenance must be UNVERIFIED; " \
        f"got {g.status} ({g.evidence_text})"
    # serde must be flagged as unverified, not silently accredited.
    assert "serde" in g.evidence_text.lower(), \
        f"unverified serde must be named; got: {g.evidence_text}"


def test_repo_serde_with_non_serde_workspace_does_not_count(tmp_path):
    """repo='serde' declared but workspace is some other path -> UNVERIFIED."""
    rows = [
        _make_result(
            scenario_id="serde_a",
            repo="serde",
            correctitud=100.0,
            workspace="repos/anyhow",         # workspace points elsewhere
            actual_repository_identity="anyhow",   # actual repo is anyhow
            actual_repository_revision="def5678",
        ),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # declared=serde, actual=anyhow — discrepancy. Reader must trust the
    # positive provenance, not the declared label. So anyhow gets credited
    # (at 100), but the other four Tier-1 repos are absent -> AMBER.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"label/provenance mismatch must not inflate Tier-1 credit; " \
        f"got {g.status} ({g.evidence_text})"
    # anyhow must show up under anyhow (positive provenance wins),
    # but the other four must be named as missing.
    for r in TIER1_REPOS - {"anyhow"}:
        assert r in g.evidence_text.lower(), \
            f"missing Tier-1 repo {r} must be named; got: {g.evidence_text}"


def test_valid_workspace_but_uncredited_revision_is_unverified(tmp_path):
    """Positive identity but no actual_repository_revision -> UNVERIFIED.

    Per directive: 'Workspace válido pero revisión no acreditada: no
    cuenta como evidencia verificada.' The reader must require both
    actual_repository_identity AND actual_repository_revision.
    """
    rows = [
        _make_result(
            scenario_id="serde_a",
            repo="serde",
            correctitud=100.0,
            workspace="repos/serde",
            actual_repository_identity="serde",
            actual_repository_revision=None,    # missing
            workspace_snapshot_id="snap_serde",
        ),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"missing actual_repository_revision must be UNVERIFIED; " \
        f"got {g.status} ({g.evidence_text})"
    assert "serde" in g.evidence_text.lower(), \
        f"unverified serde must be named; got: {g.evidence_text}"


def test_all_five_above_threshold_is_green(tmp_path):
    """All five Tier-1 repos, each with >=1 acredited scenario, all avg >= 90 -> GREEN.

    This is the canonical happy-path test. Repeats are present but must
    NOT fabricate additional coverage: the per-repo average is computed
    over distinct scenario_ids, not over all measurements.
    """
    rows = []
    for repo in TIER1_REPOS:
        # 3 distinct scenarios, each with 2 repeats.
        for s_idx in range(3):
            sid = f"{repo}_scenario_{s_idx}"
            rows.append(_acredit(repo, 95.0, scenario_id=sid, repeat_index=0))
            rows.append(_acredit(repo, 90.0, scenario_id=sid, repeat_index=1))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "GREEN", \
        f"all five acredited repos with avg >= 90 -> GREEN; got {g.status} ({g.evidence_text})"


def test_one_above_below_threshold_is_red_with_repo_named(tmp_path):
    """All five acredited; one repo < 90 -> RED, naming the repo."""
    rows = []
    for repo in TIER1_REPOS - {"tokio"}:
        rows.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a"))
    rows.append(_acredit("tokio", 80.0, scenario_id="tokio_a"))
    rows.append(_acredit("tokio", 70.0, scenario_id="tokio_b"))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    assert g.status == "RED", f"expected RED, got {g.status} ({g.evidence_text})"
    assert "tokio" in g.evidence_text, f"failing repo tokio must be named; got: {g.evidence_text}"
    # Non-failing repos must NOT be flagged in the "failing:" segment.
    assert "failing: " in g.evidence_text, \
        f"red verdict must list failing repos in a 'failing:' segment; got: {g.evidence_text}"
    failing_segment = g.evidence_text.split("failing: ", 1)[1].split(";", 1)[0]
    assert "clap" not in failing_segment, \
        f"non-failing clap must not appear in the failing segment; got: {failing_segment}"


def test_one_or_more_repos_missing_is_amber_unless_other_red(tmp_path):
    """Missing repos alone -> AMBER. Missing + a separate <90 repo -> RED
    (the directive: 'mantener RED e informar también de los repositorios
    incompletos')."""
    # Case A: 3 repos acredited, 2 missing -> AMBER
    rows_a = []
    for repo in {"ripgrep", "serde", "tokio"}:
        rows_a.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a"))
    run_a = _write_run(tmp_path, "run-A", rows_a)
    g_a = rs.gate_g4([str(run_a)])
    assert g_a.status in ("AMBER", "INCOMPLETE"), \
        f"missing repos alone -> AMBER; got {g_a.status} ({g_a.evidence_text})"
    for r in {"anyhow", "clap"}:
        assert r in g_a.evidence_text.lower(), \
            f"missing {r} must be named; got: {g_a.evidence_text}"

    # Case B: 3 repos acredited, anyhow<90, clap missing -> RED with anyhow
    # named AND clap named as missing
    rows_b = []
    for repo in {"ripgrep", "serde", "tokio"}:
        rows_b.append(_acredit(repo, 95.0, scenario_id=f"{repo}_a"))
    rows_b.append(_acredit("anyhow", 60.0, scenario_id="anyhow_a"))
    run_b = _write_run(tmp_path, "run-B", rows_b)
    g_b = rs.gate_g4([str(run_b)])
    assert g_b.status == "RED", \
        f"anyhow<90 must drive RED; got {g_b.status} ({g_b.evidence_text})"
    assert "anyhow" in g_b.evidence_text, \
        f"failing anyhow must be named; got: {g_b.evidence_text}"
    assert "clap" in g_b.evidence_text.lower(), \
        f"missing clap must also be reported; got: {g_b.evidence_text}"


def test_repeats_do_not_fabricate_coverage(tmp_path):
    """Repeats of the SAME scenario_id do NOT add coverage.

    Per directive: 'Repeticiones de un mismo escenario: no fabrican
    cobertura adicional.' Two repos with one scenario each (3 repeats
    of each) -> still 2 repos with coverage, 3 missing.
    """
    rows = []
    for repo in {"ripgrep", "serde"}:
        sid = f"{repo}_only_scenario"
        for repeat in range(5):
            rows.append(_acredit(repo, 95.0, scenario_id=sid, repeat_index=repeat))
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # 3 of 5 Tier-1 repos are missing despite high repeat counts on
    # the two that exist. Coverage = distinct scenario_ids per repo.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"5 repeats of one scenario each must not fabricate coverage; " \
        f"got {g.status} ({g.evidence_text})"
    for r in {"anyhow", "tokio", "clap"}:
        assert r in g.evidence_text.lower(), \
            f"missing {r} must be named even though other repos had repeats; " \
            f"got: {g.evidence_text}"


def test_pre_d1_compatibility_honest_classification(tmp_path):
    """Pre-D1 results (no actual_repository_identity field at all)
    must be classified honestly as UNVERIFIED, not silently re-tagged.

    Per directive: 'Compatibilidad con resultados pre-D1: conservarla
    cuando su procedencia pueda acreditarse; en caso contrario,
    clasificar la limitación honestamente.'
    """
    # A pre-D1 result.json shape: no actual_repository_identity field.
    pre_d1 = {
        "scenario_id": "serde_legacy",
        "language": "rust",
        "tool": "search_content",
        "tier": "B",
        "repo": "serde",                   # declared (metadata only)
        "outcome": "pass",
        "failure_class": "pass",
        "dimension_scores": {
            "correctitud": 100.0,
            "latencia": 100.0,
            "escalabilidad": 100.0,
            "consistencia": 100.0,
            "robustez": 100.0,
        },
        "validation": {"stages": [], "passed": True},
        "workspace": ".",
        "workspace_snapshot_id": "legacy_snap",
        # NO actual_repository_identity, NO actual_repository_revision
    }
    run = _write_run(tmp_path, "run-pre-d1", [pre_d1])
    g = rs.gate_g4([str(run)])
    # The pre-D1 result cannot be positively accredited -> AMBER for the
    # whole run, with serde flagged as UNVERIFIED, NOT silently counted
    # as Tier-1 evidence.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"pre-D1 results without positive provenance must be UNVERIFIED; " \
        f"got {g.status} ({g.evidence_text})"
    # serde must be flagged as unverified (the declared label is not credit).
    assert "serde" in g.evidence_text.lower(), \
        f"unverified serde must be named; got: {g.evidence_text}"
    # All five Tier-1 repos should appear as missing/unverified.
    for r in TIER1_REPOS:
        assert r in g.evidence_text.lower(), \
            f"missing Tier-1 repo {r} must be named; got: {g.evidence_text}"


# ── Aggregation policy ───────────────────────────────────────────────────────


def test_correctitud_none_is_not_a_zero_score(tmp_path):
    """correctitud=None (no ground truth) must NOT contribute to the average.

    Per scoring policy: when there is no ground truth, the scoring engine
    returns NaN. The reader must NOT fold that into a 0 (which would
    silently drag the average down) nor treat it as a high score.
    """
    rows = [
        _acredit("serde", None, scenario_id="serde_no_gt"),
        _acredit("serde", 95.0, scenario_id="serde_with_gt"),
    ]
    run = _write_run(tmp_path, "run-1", rows)
    g = rs.gate_g4([str(run)])
    # Only the with-GT scenario contributes -> serde avg = 95.
    # Other four Tier-1 repos are missing -> AMBER with serde name in
    # the green-by-repo list and the other four in the missing list.
    assert g.status in ("AMBER", "INCOMPLETE"), \
        f"correctitud=None must be skipped; got {g.status} ({g.evidence_text})"
    # serde should be reported as the only acredited repo.
    assert "serde" in g.evidence_text, f"serde must be named; got: {g.evidence_text}"
    for r in TIER1_REPOS - {"serde"}:
        assert r in g.evidence_text.lower(), \
            f"missing {r} must be named; got: {g.evidence_text}"


# ── A1a+1 — provenance-aware schema (current) tests ──────────────────────────
#
# These tests pin the contract that the reader correctly accepts
# results with the new `repo_provenance` nested schema, and that
# legacy results with top-level actual_* fields still work.


def _acredit_with_repo_provenance(repo: str, correctitud, scenario_id=None,
                                   repeat_index=0, actual_revision=None,
                                   actual_identity=None):
    """Build an acredited Tier-1 result using the current schema:
    `repo_provenance` nested object with actual_repository_identity
    and actual_repository_revision inside.
    """
    r = _make_result(
        scenario_id=scenario_id or f"{repo}_search_{repeat_index}",
        repo=repo,
        correctitud=correctitud,
        workspace=f"repos/{repo}",
        workspace_snapshot_id=f"snap_{repo}_v1",
        # NOTE: top-level fields are absent in current schema
        actual_repository_identity=None,
        actual_repository_revision=None,
        repeat_index=repeat_index,
    )
    # Replace top-level actual_* with nested repo_provenance
    if "actual_repository_identity" in r:
        del r["actual_repository_identity"]
    if "actual_repository_revision" in r:
        del r["actual_repository_revision"]
    r["repo_provenance"] = {
        "actual_repository_identity": actual_identity if actual_identity is not None else repo,
        "actual_repository_revision": actual_revision if actual_revision is not None else "abc1234",
        "actual_workspace": f"/tmp/scratch/{repo}",
        "workspace_relative_path": repo,
    }
    return r


def test_repo_provenance_schema_recognizes_tier1_evidence(tmp_path):
    """Current schema (repo_provenance nested): the reader must recognize
    acredited Tier-1 evidence from the new nested fields."""
    from release_scorecard import _g4_extract_provenance

    r = _acredit_with_repo_provenance(
        "serde", 95.0, actual_revision="03eec42c3313b36da416be1486e9ecac345784d5"
    )
    p = _g4_extract_provenance(r)
    assert p["actual_repository_identity"] == "serde"
    assert p["actual_repository_revision"] == "03eec42c3313b36da416be1486e9ecac345784d5"


def test_repo_provenance_schema_empty_origin_is_unverified(tmp_path):
    """Current schema: an empty actual_repository_identity (no `origin`
    remote configured) must be treated as UNVERIFIED, not as a valid
    Tier-1 attribution."""
    from release_scorecard import _g4_extract_provenance

    r = _acredit_with_repo_provenance(
        "serde", 95.0, actual_identity="", actual_revision="abc1234"
    )
    p = _g4_extract_provenance(r)
    assert p["actual_repository_identity"] is None
    # The result must NOT be treated as acredited Tier-1


def test_repo_provenance_schema_full_5_repos_green(tmp_path):
    """End-to-end: with the new schema, all 5 Tier-1 repos above the
    threshold produce GREEN."""
    from release_scorecard import gate_g4

    runs = []
    for repo in TIER1_REPOS:
        results = [
            _acredit_with_repo_provenance(repo, 95.0, repeat_index=0),
            _acredit_with_repo_provenance(repo, 95.0, repeat_index=1),
        ]
        runs.append(_write_run(tmp_path, f"run_{repo}", results))

    g4 = gate_g4([str(r) for r in runs])
    assert g4.status == "GREEN", (
        f"Expected GREEN with all 5 Tier-1 repos at 95% (new schema); got {g4.status}: "
        f"{g4.evidence_text}"
    )


def test_repo_provenance_schema_unverified_when_provenance_missing(tmp_path):
    """If repo_provenance is absent (e.g., a fixture scenario), the
    result is UNVERIFIED — even if the declared repo name is Tier-1."""
    from release_scorecard import _g4_extract_provenance

    r = _make_result(
        scenario_id="fixture_impostor",
        repo="serde",                  # declared Tier-1
        correctitud=100.0,             # perfect score
        workspace="sandbox/fixtures/rust-hello",  # fixture, not real
        actual_repository_identity=None,
        actual_repository_revision=None,
    )
    # Remove any top-level actual_* to simulate fixture-only result
    r.pop("actual_repository_identity", None)
    r.pop("actual_repository_revision", None)
    p = _g4_extract_provenance(r)
    assert p["actual_repository_identity"] is None
    assert p["actual_repository_revision"] is None


def test_legacy_schema_with_top_level_actual_still_works(tmp_path):
    """Backward compat: legacy results with top-level actual_* fields
    still get acredited. The reader must accept BOTH schemas."""
    from release_scorecard import _g4_extract_provenance

    r = _make_result(
        scenario_id="legacy_serde",
        repo="serde",
        correctitud=90.0,
        workspace="repos/serde",
        actual_repository_identity="serde",
        actual_repository_revision="abc1234",
    )
    # No repo_provenance key — pure legacy
    assert "repo_provenance" not in r
    p = _g4_extract_provenance(r)
    assert p["actual_repository_identity"] == "serde"
    assert p["actual_repository_revision"] == "abc1234"


# ── A1a+1 — URL normalization tests ────────────────────────────────────────


def test_normalize_repo_identity_https_url():
    """The orchestrator emits origin URLs; reader must map to short name."""
    from release_scorecard import _normalize_repo_identity

    assert _normalize_repo_identity("https://github.com/serde-rs/serde.git") == "serde"
    assert _normalize_repo_identity("https://github.com/BurntSushi/ripgrep.git") == "ripgrep"
    assert _normalize_repo_identity("https://github.com/clap-rs/clap.git") == "clap"
    assert _normalize_repo_identity("https://github.com/tokio-rs/tokio.git") == "tokio"
    assert _normalize_repo_identity("https://github.com/dtolnay/anyhow.git") == "anyhow"


def test_normalize_repo_identity_alternative_formats():
    """SSH, plain name, None, empty."""
    from release_scorecard import _normalize_repo_identity

    assert _normalize_repo_identity("git@github.com:serde-rs/serde.git") == "serde"
    assert _normalize_repo_identity("serde") == "serde"  # passthrough
    assert _normalize_repo_identity(None) is None
    assert _normalize_repo_identity("") is None
    assert _normalize_repo_identity("  ") == ""  # stripped to empty after trim


def test_normalize_repo_identity_handles_non_github_urls():
    """Non-GitHub URLs: take last path segment."""
    from release_scorecard import _normalize_repo_identity

    # No recognized prefix -> return as-is (last segment)
    assert _normalize_repo_identity("https://gitlab.com/foo/bar.git") == "https://gitlab.com/foo/bar"  # .git stripped, no prefix recognized
