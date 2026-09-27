#!/usr/bin/env python3
"""Focused black-box tests for the language/platform support matrices."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
from pathlib import Path

import jsonschema

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_support_matrix.py"
BASELINE = "7c624d016475056205de62405bcf125943bb30a7"


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
    assert len(platforms["platforms"]) == 6
    assert sum(item["support_level"] == "supported" for item in languages["languages"]) == 18
    assert sum(item["support_level"] == "experimental" for item in languages["languages"]) == 12
    assert [item["id"] for item in languages["languages"]] == sorted(item["id"] for item in languages["languages"])
    assert {item["target"] for item in platforms["platforms"] if item["support_level"] == "certified"} == {
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
    }
    assert any(item["target"] == "x86_64-unknown-linux-musl" and item["support_level"] == "experimental" for item in platforms["platforms"])
    assert sum(item["support_level"] == "unsupported" for item in platforms["platforms"]) == 3


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
