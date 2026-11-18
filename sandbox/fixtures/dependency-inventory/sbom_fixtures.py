#!/usr/bin/env python3
"""Regenerates or checks the SBOM fixtures of the Cargo fixture workspace.

Two of the three SBOMs next to the fixture are produced by real exporters and
are reproduced here, sanitised so they are the same on every machine:

* `cargo-workspace.cargo-cyclonedx.cdx.json`: `cargo cyclonedx` (CycloneDX
  format, JSON 1.5) run on a copy of the `app` member, with the timestamp,
  serial number and absolute paths replaced by fixed values.
* `cargo-workspace.guard.cdx.json`: the CogniCode Guard release SBOM
  generator, `scripts/sbom_from_lock.py` of the sibling `cognicode-guard`
  repository, run on the fixture lockfile.

`external-image.cdx.json` is written by hand in the shape an image scanner
emits (an artifact SBOM with an operating-system package, a container subject
with a digest, a missing hash, a wrong hash and a component the lockfile does
not resolve); it is an input of the tests, not an export, and is not
regenerated.

The tests never launch these tools. This script is the only place that does,
outside the test suite:

    python3 -I sbom_fixtures.py --guard-script PATH         # rewrite both
    python3 -I sbom_fixtures.py --guard-script PATH --check # fail if stale

`cargo-cyclonedx` must be installed; the exporter version is recorded in the
fixture itself.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "cargo-workspace"
CARGO_CYCLONEDX = HERE / "cargo-workspace.cargo-cyclonedx.cdx.json"
GUARD = HERE / "cargo-workspace.guard.cdx.json"
DEADLINE_SECONDS = 180
FIXED_TIMESTAMP = "2024-01-01T00:00:00Z"
FIXED_SERIAL = "urn:uuid:00000000-0000-4000-8000-000000000001"
FIXED_ROOT = "/fixture"


def run(command: list[str], cwd: Path) -> None:
    result = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=DEADLINE_SECONDS,
        check=False,
    )
    if result.returncode != 0:
        sys.exit(f"{' '.join(command)} failed:\n{result.stderr}")


def sanitised(text: str, root: Path) -> str:
    document = json.loads(text.replace(str(root), FIXED_ROOT))
    document["serialNumber"] = FIXED_SERIAL
    document["metadata"]["timestamp"] = FIXED_TIMESTAMP
    return json.dumps(document, indent=2) + "\n"


def cargo_cyclonedx() -> str:
    with tempfile.TemporaryDirectory() as scratch:
        workspace = Path(scratch) / "ws"
        shutil.copytree(FIXTURE, workspace)
        run(
            ["cargo", "cyclonedx", "--format", "json", "--spec-version", "1.5"],
            workspace,
        )
        return sanitised((workspace / "app" / "app.cdx.json").read_text(), workspace)


def guard(script: Path) -> str:
    with tempfile.TemporaryDirectory() as scratch:
        output = Path(scratch) / "guard.cdx.json"
        run(
            [
                sys.executable,
                "-I",
                str(script),
                "--lock",
                str(FIXTURE / "Cargo.lock"),
                "--component-name",
                "app",
                "--component-version",
                "0.3.0",
                "--output",
                str(output),
            ],
            HERE,
        )
        return output.read_text()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--guard-script", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()

    produced = {
        CARGO_CYCLONEDX: cargo_cyclonedx(),
        GUARD: guard(arguments.guard_script),
    }
    stale = [
        path.name
        for path, text in produced.items()
        if not path.exists() or path.read_text() != text
    ]
    if arguments.check:
        if stale:
            sys.exit(f"stale SBOM fixtures: {', '.join(stale)}")
        return 0
    for path, text in produced.items():
        path.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
