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


if __name__ == "__main__":
    tests = [
        test_current_provenance_and_public_surface,
        test_generation_is_deterministic,
        test_support_claims_are_conservative,
        test_invalid_source_commit_is_rejected,
        test_committed_baseline_is_self_consistent,
    ]
    for test in tests:
        test()
        print(f"PASS {test.__name__}")
