#!/usr/bin/env python3
"""Focused black-box tests for the product manifest generator."""

from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_product_manifest.py"
BASELINE = "73235889409afea43bc18cb0122a3966676dbb85"


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
        assert manifest["version"] == "0.99.2"
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
        assert [item["target"] for item in manifest["platforms"]] == [
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
        ]
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
