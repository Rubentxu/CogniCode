#!/usr/bin/env python3
"""Focused black-box tests for the language/platform support matrices."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import jsonschema

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_support_matrix.py"
BASELINE = "7c624d016475056205de62405bcf125943bb30a7"

# Resolved the way the generator resolves it: this file is loaded by path, so
# its own directory is not on the path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_lane  # noqa: E402


def load_generator():
    spec = importlib.util.spec_from_file_location("support_matrix_generator", GENERATOR)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), "--root", str(ROOT), *args],
        text=True,
        capture_output=True,
        check=False,
    )


def test_generation_and_classification() -> None:
    generator = load_generator()
    languages, platforms = generator.build_documents(ROOT, BASELINE)
    assert len(languages["languages"]) == 30
    assert sum(item["support_level"] == "supported" for item in languages["languages"]) == 18
    assert sum(item["support_level"] == "experimental" for item in languages["languages"]) == 12
    assert [item["id"] for item in languages["languages"]] == sorted(item["id"] for item in languages["languages"])
    # The platform count is not a literal. It is the certified set (whatever the
    # lane builds), plus the musl experiment, plus the deferred platforms the
    # generator appends to prove their absence is real. On 2026-10-13 this read
    # `== 6`, written when the lane built two targets; commit `e3ab9c4e` dropped
    # aarch64 to x86_64-only and the number became 6 against a generated 5, so
    # the suite was red for a day with nobody looking because the *program* had
    # gone 11 contracts down at the same time.
    #
    # A derived count is what catches the failures that matter: a platform lost
    # from the lane lowers it, a duplicate entry raises it, and a platform the
    # generator forgets to classify does not move it at all — which is why the
    # per-classification assertions below still pin the shape, not just the size.
    assert len(platforms["platforms"]) == (
        len(release_lane.release_targets(ROOT))
        + 1
        + len(generator.DEFERRED_PUBLIC_PLATFORMS)
    )
    # The certified set is whatever the release lane builds, read here
    # independently of the generator. Asserting two literals instead would be a
    # third copy of the same fact — the lane declares it, the generator reads
    # it, and a test repeated it — and the copy nobody reconciles is the one
    # that goes stale when a platform is added.
    assert {
        item["target"] for item in platforms["platforms"] if item["support_level"] == "certified"
    } == set(release_lane.release_targets(ROOT))
    assert any(item["target"] == "x86_64-unknown-linux-musl" and item["support_level"] == "experimental" for item in platforms["platforms"])
    assert sum(item["support_level"] == "unsupported" for item in platforms["platforms"]) == len(generator.DEFERRED_PUBLIC_PLATFORMS)
    # A platform cannot be certified and deferred at once. `deferred_evidence`
    # raises when the lane builds a target listed here as unsupported, so this
    # is the invariant that check exists to protect, stated where a reader of
    # the counts above will see it.
    certified_targets = {
        item["target"] for item in platforms["platforms"] if item["support_level"] == "certified"
    }
    deferred_targets = {entry["target"] for entry in generator.DEFERRED_PUBLIC_PLATFORMS}
    assert not (certified_targets & deferred_targets), (
        f"targets claimed both certified and unsupported: {sorted(certified_targets & deferred_targets)}"
    )


def test_schemas_and_check_mode() -> None:
    with tempfile.TemporaryDirectory() as directory:
        languages_output = Path(directory) / "languages.json"
        platforms_output = Path(directory) / "platforms.json"
        result = run(
            "--source-commit",
            BASELINE,
            "--languages-output",
            str(languages_output),
            "--platforms-output",
            str(platforms_output),
        )
        assert result.returncode == 0, result.stderr
        languages = json.loads(languages_output.read_text(encoding="utf-8"))
        platforms = json.loads(platforms_output.read_text(encoding="utf-8"))
        jsonschema.validate(languages, json.loads((ROOT / "product/schemas/languages.v1.schema.json").read_text()))
        jsonschema.validate(platforms, json.loads((ROOT / "product/schemas/platforms.v1.schema.json").read_text()))
        checked = run(
            "--source-commit",
            BASELINE,
            "--languages-output",
            str(languages_output),
            "--platforms-output",
            str(platforms_output),
            "--check",
        )
        assert checked.returncode == 0, checked.stderr


def test_invalid_documents_are_rejected() -> None:
    generator = load_generator()
    languages, platforms = generator.build_documents(ROOT, BASELINE)
    duplicate = json.loads(json.dumps(languages))
    duplicate["languages"].append(json.loads(json.dumps(duplicate["languages"][0])))
    try:
        generator.validate_documents(duplicate, platforms)
    except ValueError as error:
        assert "duplicate ids" in str(error)
    else:
        raise AssertionError("duplicate language ids must be rejected")

    invalid_level = json.loads(json.dumps(platforms))
    invalid_level["platforms"][0]["support_level"] = "maybe"
    try:
        generator.validate_documents(languages, invalid_level)
    except ValueError as error:
        assert "invalid support level" in str(error)
    else:
        raise AssertionError("invalid support levels must be rejected")

    missing_evidence = json.loads(json.dumps(languages))
    missing_evidence["languages"][0]["evidence"] = []
    try:
        generator.validate_documents(missing_evidence, platforms)
    except ValueError as error:
        assert "missing evidence" in str(error)
    else:
        raise AssertionError("missing evidence must be rejected")


def test_invalid_sha_and_drift_fail_without_overwriting() -> None:
    result = run("--source-commit", "not-a-sha")
    assert result.returncode != 0
    assert "source_commit" in result.stderr

    with tempfile.TemporaryDirectory() as directory:
        languages_output = Path(directory) / "languages.json"
        platforms_output = Path(directory) / "platforms.json"
        result = run(
            "--source-commit",
            BASELINE,
            "--languages-output",
            str(languages_output),
            "--platforms-output",
            str(platforms_output),
        )
        assert result.returncode == 0, result.stderr
        original = languages_output.read_text(encoding="utf-8")
        languages_output.write_text(original.replace('"product": "cognicode"', '"product": "drift"'), encoding="utf-8")
        checked = run(
            "--source-commit",
            BASELINE,
            "--languages-output",
            str(languages_output),
            "--platforms-output",
            str(platforms_output),
            "--check",
        )
        assert checked.returncode != 0
        assert "generated languages differ" in checked.stderr
