#!/usr/bin/env python3
"""Focused black-box tests for the public profile contract."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_profiles.py"
SCHEMA = ROOT / "product/schemas/profiles.v1.schema.json"
BASELINE = "73235889409afea43bc18cb0122a3966676dbb85"


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), "--root", str(ROOT), *args],
        text=True,
        check=False,
        capture_output=True,
    )


def test_generation_and_classification() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "profiles.json"
        result = run("--output", str(output), "--source-commit", BASELINE)
        assert result.returncode == 0, result.stderr
        document = json.loads(output.read_text(encoding="utf-8"))
        assert document["schema_version"] == "cognicode.profiles/v1"
        ids = [profile["id"] for profile in document["profiles"]]
        assert ids == sorted(ids)
        assert ids == ["core", "developer", "experimental", "reviewer"]

        by_id = {profile["id"]: profile for profile in document["profiles"]}

        assert by_id["core"]["stability"] == "stable"
        assert by_id["core"]["install"] is True
        assert by_id["core"]["mutating"] is False
        assert by_id["core"]["components"] == ["cogh", "cognicode"]
        assert by_id["core"]["skill_bundles"] == ["cognicode"]

        assert by_id["reviewer"]["stability"] == "stable"
        assert by_id["reviewer"]["install"] is True
        assert by_id["reviewer"]["mutating"] is False
        assert by_id["reviewer"]["components"] == ["cogh", "cognicode", "cognicode-mcp"]
        assert by_id["reviewer"]["skill_bundles"] == ["cognicode", "cognicode-mcp"]

        assert by_id["developer"]["stability"] == "experimental"
        assert by_id["developer"]["install"] is False
        assert by_id["developer"]["mutating"] is True
        assert "cogh" in by_id["developer"]["components"]
        assert "cognicode-developer" in by_id["developer"]["skill_bundles"]

        assert by_id["experimental"]["stability"] == "experimental"
        assert by_id["experimental"]["install"] is False
        assert by_id["experimental"]["mutating"] is False
        assert by_id["experimental"]["components"] == []
        assert by_id["experimental"]["skill_bundles"] == []


def test_schemas_and_check_mode() -> None:
    import jsonschema

    document = json.loads((ROOT / "product/profiles.json").read_text(encoding="utf-8"))
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    jsonschema.validate(document, schema)

    committed = (ROOT / "product/profiles.json").read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory() as directory:
        fresh = Path(directory) / "profiles.json"
        result = run("--output", str(fresh), "--source-commit", BASELINE)
        assert result.returncode == 0, result.stderr
        assert fresh.read_text(encoding="utf-8") == committed

    check = run("--output", "product/profiles.json", "--source-commit", BASELINE, "--check")
    assert check.returncode == 0, check.stderr


def test_invalid_documents_are_rejected() -> None:
    import jsonschema

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    document = json.loads((ROOT / "product/profiles.json").read_text(encoding="utf-8"))

    bad_alpha = json.loads(json.dumps(document))
    bad_alpha["profiles"][0]["stability"] = "alpha"
    try:
        jsonschema.validate(bad_alpha, schema)
    except jsonschema.ValidationError:
        pass
    else:
        raise AssertionError("alpha stability must be rejected by the schema")

    bad_install = json.loads(json.dumps(document))
    bad_install["profiles"][1]["install"] = True
    bad_install["profiles"][1]["components"] = []
    try:
        jsonschema.validate(bad_install, schema)
    except jsonschema.ValidationError:
        pass
    else:
        raise AssertionError("install=true with empty components must be rejected")


def test_invalid_sha_and_drift_fail_without_overwriting() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "profiles.json"
        result = run("--output", str(output), "--source-commit", "not-a-sha")
        assert result.returncode != 0
        assert not output.exists()

    drift = run("--output", "product/profiles.json", "--source-commit", BASELINE, "--check")
    assert drift.returncode == 0, drift.stderr


def test_check_tolerates_a_stale_provenance_stamp() -> None:
    """A commit stamp from an older HEAD is not drift.

    The committed artefact records the commit it was generated from, which
    is necessarily not the current HEAD after any later commit. Comparing
    whole documents made `--check` fail on every commit, so it could never
    be relied on and had to be skipped. Content is the property that must
    match; the stamp only has to be well-formed.
    """
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "profiles.json"
        assert run("--output", str(output), "--source-commit", BASELINE).returncode == 0

        # Re-check the same content while pretending HEAD has moved on.
        stale = run("--output", str(output), "--check")
        assert stale.returncode == 0, stale.stderr

        # Content drift is still caught, and the message names the field so
        # a reader knows where to look instead of diffing by hand.
        document = json.loads(output.read_text(encoding="utf-8"))
        document["profiles"] = document["profiles"][:-1]
        output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
        content_drift = run("--output", str(output), "--check")
        assert content_drift.returncode != 0
        assert "profiles" in content_drift.stderr, content_drift.stderr


def test_check_rejects_a_malformed_or_missing_provenance_stamp() -> None:
    """The stamp is validated for shape, not ignored entirely.

    Loosening the comparison must not turn into not caring: a missing or
    malformed stamp is the one thing `--check` can still verify about
    provenance, so it verifies it.
    """
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "profiles.json"
        assert run("--output", str(output), "--source-commit", BASELINE).returncode == 0
        document = json.loads(output.read_text(encoding="utf-8"))

        for label, stamp in (("malformed", "not-a-sha"), ("missing", None)):
            mutated = dict(document)
            if stamp is None:
                mutated.pop("source_commit", None)
            else:
                mutated["source_commit"] = stamp
            output.write_text(json.dumps(mutated, indent=2) + "\n", encoding="utf-8")
            result = run("--output", str(output), "--check")
            assert result.returncode != 0, f"{label} stamp was accepted"
            assert "source_commit" in result.stderr, result.stderr


def test_developer_and_experimental_are_not_installable() -> None:
    document = json.loads((ROOT / "product/profiles.json").read_text(encoding="utf-8"))
    by_id = {profile["id"]: profile for profile in document["profiles"]}
    assert by_id["developer"]["install"] is False
    assert by_id["experimental"]["install"] is False
    # README documents the installer behaviour explicitly.
    readme = (ROOT / "product/README.md").read_text(encoding="utf-8").lower()
    assert "--profile developer" in readme
    assert "--profile experimental" in readme


def test_manifest_derives_profiles_from_profiles_json() -> None:
    profiles = json.loads((ROOT / "product/profiles.json").read_text(encoding="utf-8"))
    manifest = json.loads((ROOT / "product/product-manifest.json").read_text(encoding="utf-8"))
    contract_ids = [profile["id"] for profile in profiles["profiles"]]
    manifest_ids = [profile["name"] for profile in manifest["public_surface"]["profiles"]]
    assert manifest_ids == contract_ids

    by_contract_name = {profile["id"]: profile for profile in profiles["profiles"]}
    for entry in manifest["public_surface"]["profiles"]:
        contract = by_contract_name[entry["name"]]
        assert entry["mutating"] == contract["mutating"]
        assert entry["install"] == contract["install"]
        assert entry["stability"] == contract["stability"]


if __name__ == "__main__":
    # Standalone entry point; see scripts/run_contract_tests.py for why
    # this must not depend on pytest being installed.
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from run_contract_tests import main as run_main

    raise SystemExit(run_main([__file__]))
