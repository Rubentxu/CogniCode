#!/usr/bin/env python3
"""Focused black-box tests for the product manifest generator."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_product_manifest.py"
BASELINE = "73235889409afea43bc18cb0122a3966676dbb85"

# `import release_lane` resolves the same way the generator does: the contract
# suite loads this file by path, so its own directory is not on the path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_lane  # noqa: E402


def release_targets() -> list[str]:
    """The cargo targets the release lane declares, sorted.

    The same reasoning as `workspace_version` below, applied to the platform
    list. This assertion used to carry its own copy of the two GNU targets, so
    the fact "which platforms are certified" was stated in the release lane, in
    the generator, and here — and the generator's copy was the one that drifted
    silently, because nothing compared the three.

    It still has teeth. It compares the generated manifest against the lane
    read independently here, so a generator that stops reading the lane, or
    starts inventing a platform, fails. What it no longer does is require
    someone to remember to edit a literal when a platform is added — which is
    exactly the edit that gets forgotten, and the failure it causes is a red
    gate on a release day rather than a stale claim in a published manifest.
    """
    return sorted(release_lane.release_targets(ROOT))


def workspace_version() -> str:
    """The version the manifest is required to publish.

    Read from ``Cargo.toml`` rather than hardcoded. A hardcoded literal made
    this assertion fail on every version bump, which is a real cost: the
    natural response to a red gate is to edit the artefact, and that is how a
    published contract drifts away from the binary that produces it.

    The assertion still has teeth. It compares the generated manifest against
    the workspace version read independently here, so if the generator ever
    derives the version from anywhere else — or stops propagating it — this
    fails. What it no longer does is demand that someone remember to edit a
    test file on release day.
    """
    workspace = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(
        r'(?ms)^\[workspace\.package\]\s+.*?^version\s*=\s*["\']([^"\']+)["\']',
        workspace,
    )
    assert match, "could not read the workspace version from Cargo.toml"
    return match.group(1)


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), "--root", str(ROOT), *args],
        text=True,
        capture_output=True,
        check=False,
    )


def test_current_provenance_and_public_surface() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "manifest.json"
        result = run("--output", str(output), "--source-commit", BASELINE)
        assert result.returncode == 0, result.stderr
        manifest = json.loads(output.read_text(encoding="utf-8"))
        assert manifest["schema_version"] == "cognicode.product/v1"
        assert manifest["version"] == workspace_version()
        assert manifest["source_commit"] == BASELINE
        assert [item["name"] for item in manifest["public_surface"]["binaries"]] == [
            "cogh",
            "cognicode",
            "cognicode-mcp",
        ]
        assert [item["name"] for item in manifest["public_surface"]["profiles"]] == [
            "core",
            "developer",
            "experimental",
            "reviewer",
        ]
        assert manifest["public_surface"]["profiles"][0]["install"] is True
        assert manifest["public_surface"]["profiles"][1]["install"] is False
        assert manifest["public_surface"]["mcp"]["protocol_revision"] == "2025-03-26"


def test_generation_is_deterministic() -> None:
    with tempfile.TemporaryDirectory() as directory:
        first = Path(directory) / "first.json"
        second = Path(directory) / "second.json"
        assert run("--output", str(first), "--source-commit", BASELINE).returncode == 0
        assert run("--output", str(second), "--source-commit", BASELINE).returncode == 0
        assert first.read_bytes() == second.read_bytes()


def test_support_claims_are_conservative() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "manifest.json"
        result = run("--output", str(output), "--source-commit", BASELINE)
        assert result.returncode == 0, result.stderr
        manifest = json.loads(output.read_text(encoding="utf-8"))
        assert manifest["languages"]
        assert {item["support_level"] for item in manifest["languages"]} == {"experimental"}
        # Compared as sorted lists because order is not the claim being made.
        # The manifest lists the lane's declaration order; the property is that
        # it claims exactly the targets the lane builds, no more and no fewer.
        assert sorted(item["target"] for item in manifest["platforms"]) == release_targets()
        assert {item["support_level"] for item in manifest["platforms"]} == {"certified"}
        assert "tools" not in manifest


def test_invalid_source_commit_is_rejected() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "manifest.json"
        result = run("--output", str(output), "--source-commit", "not-a-sha")
        assert result.returncode != 0
        assert not output.exists()


def test_committed_baseline_is_self_consistent() -> None:
    result = run("--output", "product/product-manifest.json", "--source-commit", BASELINE, "--check")
    assert result.returncode == 0, result.stderr


def test_check_does_not_invalidate_itself_on_a_newer_head() -> None:
    """``--check`` must pass with no ``--source-commit`` given.

    This is the property that was missing. Every other test in this file
    pins ``--source-commit`` explicitly, which is why the defect survived:
    the self-invalidation only shows up when the generator falls back to
    ``git rev-parse HEAD``, and the committed stamp always names an older
    commit than the one being checked.

    ``source_commit`` is provenance, not derived content. Requiring it to
    equal ``HEAD`` makes the check unsatisfiable on every fresh commit, and
    a gate that always fails is a gate nobody reads.
    """
    result = run("--output", "product/product-manifest.json", "--check")
    assert result.returncode == 0, result.stderr


def test_check_still_detects_real_content_drift() -> None:
    """The relaxation above must not weaken real drift detection.

    Content is compared strictly; only the provenance stamp is exempt. A
    change to any derived field has to fail, and the message has to name
    the field so a reader knows where to look instead of diffing by hand.
    """
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "manifest.json"
        assert run("--output", str(output), "--source-commit", BASELINE).returncode == 0
        manifest = json.loads(output.read_text(encoding="utf-8"))
        manifest["mcp_protocol_revision"] = "1999-01-01"
        output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        result = run("--output", str(output), "--check")
        assert result.returncode != 0
        assert "mcp_protocol_revision" in result.stderr


def test_check_rejects_malformed_provenance() -> None:
    """Provenance is validated for shape even though not for equality.

    Exempting the stamp from comparison must not exempt it from
    validation: a missing or malformed ``source_commit`` is a real defect in
    the artefact, not a formatting preference.
    """
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "manifest.json"
        assert run("--output", str(output), "--source-commit", BASELINE).returncode == 0
        manifest = json.loads(output.read_text(encoding="utf-8"))
        manifest["source_commit"] = "not-a-sha"
        output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        result = run("--output", str(output), "--check")
        assert result.returncode != 0
        assert "source_commit" in result.stderr


if __name__ == "__main__":
    tests = [
        test_current_provenance_and_public_surface,
        test_generation_is_deterministic,
        test_support_claims_are_conservative,
        test_invalid_source_commit_is_rejected,
        test_committed_baseline_is_self_consistent,
        test_check_does_not_invalidate_itself_on_a_newer_head,
        test_check_still_detects_real_content_drift,
        test_check_rejects_malformed_provenance,
    ]
    for test in tests:
        test()
        print(f"PASS {test.__name__}")
