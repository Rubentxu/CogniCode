#!/usr/bin/env python3
"""Tests for the published-document schema validation gate."""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from validate_schemas import DOCUMENT_SCHEMA_PAIRS, ROOT, validate_all, validate_document


def _copy_product_tree(destination_root: Path, source_root: Path = ROOT) -> Path:
    """Copy ``product/`` into a scratch root so tests can mutate it freely.

    The copy has to be deep, including ``product/schemas/``: the tests
    mutate the document and then revalidate it against the real schema, so
    both halves have to travel together.
    """
    target = destination_root / "product"
    for path in (source_root / "product").rglob("*"):
        if path.is_file():
            destination = target / path.relative_to(source_root / "product")
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(path.read_text(encoding="utf-8"), encoding="utf-8")
    return destination_root


def test_every_published_document_validates() -> None:
    results = validate_all()
    for name, errors in results.items():
        assert not errors, f"{name}.json does not match its schema:\n  " + "\n  ".join(errors)


def test_every_document_on_disk_is_registered() -> None:
    """A document that exists but is not registered would never be checked.

    This is the failure mode the gate cannot see by itself: a sixth
    product document is added, nobody adds it to `DOCUMENT_SCHEMA_PAIRS`,
    and validation keeps reporting green while the new file is unvalidated.
    """
    documents = {
        path.stem
        for path in (ROOT / "product").glob("*.json")
        if path.name not in {"tool-catalog-runtime.json"}
    }
    registered = set(DOCUMENT_SCHEMA_PAIRS)
    assert documents == registered, (
        f"product/ has documents {sorted(documents)} but only "
        f"{sorted(registered)} are validated; update DOCUMENT_SCHEMA_PAIRS"
    )


def test_every_registered_schema_exists() -> None:
    for document, schema in DOCUMENT_SCHEMA_PAIRS.items():
        schema_path = ROOT / "product" / "schemas" / schema
        assert schema_path.exists(), f"{document}.json has no schema at {schema}"
        payload = json.loads(schema_path.read_text(encoding="utf-8"))
        # A schema with no `required` and open `additionalProperties`
        # accepts anything, so passing validation against it proves
        # nothing. These are the properties that make a check a check.
        assert payload.get("required"), f"{schema} has no top-level required list"
        assert payload.get("additionalProperties") is False, (
            f"{schema} allows additional properties, so an unmodelled "
            f"field would be silently accepted"
        )


def test_unmodelled_field_is_rejected() -> None:
    """The gate must catch exactly the drift it was written to catch.

    A-006 moved `profiles` into `artifacts` and added `install` and
    `stability` to each published profile. The schema was never updated,
    and because nothing validated the document against it, that drift sat
    in `main` unnoticed. This reproduces it: adding one unmodelled field
    has to fail.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = _copy_product_tree(Path(directory) / "root")
        manifest_path = root / "product" / "product-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["public_surface"]["profiles"][0]["invented_field"] = "surprise"
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        errors = validate_document("product-manifest", "product-manifest.v1.schema.json", root)
        assert errors, "an unmodelled field passed schema validation"
        assert any("invented_field" in error for error in errors), (
            f"the failure does not name the offending field: {errors}"
        )


def test_error_message_locates_the_offending_path() -> None:
    """A validation failure has to say where to look.

    These documents are large; "the document is invalid" turns a two
    second fix into a manual hunt.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = _copy_product_tree(Path(directory) / "root")
        manifest_path = root / "product" / "product-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        del manifest["public_surface"]["binaries"][0]["name"]
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        errors = validate_document("product-manifest", "product-manifest.v1.schema.json", root)
        assert errors, "a missing required field passed schema validation"
        assert any("public_surface/binaries" in error for error in errors), (
            f"the failure does not locate the path: {errors}"
        )


if __name__ == "__main__":
    tests = [
        test_every_published_document_validates,
        test_every_document_on_disk_is_registered,
        test_every_registered_schema_exists,
        test_unmodelled_field_is_rejected,
        test_error_message_locates_the_offending_path,
    ]
    for test in tests:
        test()
        print(f"PASS {test.__name__}")
