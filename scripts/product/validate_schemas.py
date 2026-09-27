#!/usr/bin/env python3
"""Validate every published product document against its own JSON Schema.

Why this exists
---------------
The repository ships five schemas under ``product/schemas/`` and five
documents under ``product/``. A schema nobody executes is a comment with
JSON syntax, and that is exactly what these were: a schema can drift away
from the document it describes and nothing notices, because nothing ever
reads the schema.

This module runs the check. When it was first added, the very first run
failed on ``product-manifest.json`` with five errors — ``additionalProperties
is not allowed ('profiles' was unexpected)`` — introduced when A-006 moved
the profiles out of ``artifacts`` and into ``public_surface``. The schema
was never updated, and no test failed, because no test validated the
document against it. The drift was real and had been sitting in
``main`` the whole time.

The same check covers the other four pairs as well, because a validation
that only ever looks at one document is a spot check, not a gate.

What this is and is not
-----------------------
It proves the documents match their declared schemas. It does *not* prove
the schemas are the right schemas: a schema can be internally consistent
and describe the wrong contract. That gap is why the schemas use
``additionalProperties: false`` — an unmodelled field fails loudly instead
of being silently accepted — but it is still a human judgement, and this
check cannot make it.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

try:
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover - exercised only on a bare interpreter
    Draft202012Validator = None  # type: ignore[assignment]

ROOT = Path(__file__).resolve().parents[2]

#: Every published document paired with the schema that is supposed to
#: describe it. Adding a document without adding it here would leave it
#: unvalidated, which is the failure mode this module exists to prevent, so
#: the test below pins this list against the files actually on disk.
DOCUMENT_SCHEMA_PAIRS: dict[str, str] = {
    "tools": "tools.v1.schema.json",
    "profiles": "profiles.v1.schema.json",
    "product-manifest": "product-manifest.v1.schema.json",
    "languages": "languages.v1.schema.json",
    "platforms": "platforms.v1.schema.json",
}


def _load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _error_path(error: Any) -> str:
    """Render an error location as a JSON-pointer-ish path.

    A bare "the document is invalid" tells a reader nothing about where to
    look, and these documents are large enough that the difference between
    a useful message and a useless one is the difference between a two
    second fix and a manual hunt.
    """
    parts = [str(part) for part in error.absolute_path]
    return "/" + "/".join(parts) if parts else "<root>"


def validate_document(document_name: str, schema_name: str, root: Path = ROOT) -> list[str]:
    """Return a list of human-readable validation errors, empty if valid.

    An empty list means the document satisfies the schema. A non-empty list
    names every offending path, because fixing one error at a time in a
    five-error document is needless work for whoever has to do it.
    """
    if Draft202012Validator is None:
        raise RuntimeError(
            "jsonschema is required to validate published documents. "
            "Install it or add it to the project's test dependencies."
        )

    document_path = root / "product" / f"{document_name}.json"
    schema_path = root / "product" / "schemas" / schema_name
    document = _load(document_path)
    schema = _load(schema_path)

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(document), key=lambda e: list(e.absolute_path))
    return [
        f"{document_path.relative_to(root)} at {_error_path(error)}: {error.message}"
        for error in errors
    ]


def validate_all(root: Path = ROOT) -> dict[str, list[str]]:
    """Validate every published document. Returns ``{document: [errors]}``."""
    return {
        name: validate_document(name, schema, root)
        for name, schema in DOCUMENT_SCHEMA_PAIRS.items()
    }


def main() -> int:
    results = validate_all()
    failed = False
    for name, errors in results.items():
        if errors:
            failed = True
            print(f"FAIL {name}: {len(errors)} schema violation(s)")
            for error in errors:
                print(f"  - {error}")
        else:
            print(f"PASS {name}: valid against {DOCUMENT_SCHEMA_PAIRS[name]}")
    if failed:
        print(
            "\nA published document does not match its schema. Either the "
            "document is wrong, or the schema was not updated when the "
            "contract changed. Both are defects; the schema is not the "
            "authority over the document it describes."
        )
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
