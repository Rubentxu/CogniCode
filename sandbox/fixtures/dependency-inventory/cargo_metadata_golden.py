#!/usr/bin/env python3
"""Regenerates or checks the `cargo metadata` golden of the Cargo fixture.

The inventory tests compare the offline lockfile reader against this golden,
which is the normalised output of `cargo metadata --locked --offline` on the
fixture workspace. The tests never launch cargo; this script is the only
place that does, and it runs outside the test suite.

    python3 -I cargo_metadata_golden.py            # rewrite the golden
    python3 -I cargo_metadata_golden.py --check    # fail if it is stale

Paths and absolute locations are removed so the golden is the same on every
machine: packages are `name@version`, sources are the lockfile strings, and a
path package has the source `path`.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "cargo-workspace"
GOLDEN = HERE / "cargo-workspace.metadata.golden.json"
DEADLINE_SECONDS = 180


def metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=FIXTURE,
        capture_output=True,
        text=True,
        timeout=DEADLINE_SECONDS,
        check=False,
    )
    if result.returncode != 0:
        sys.exit(f"cargo metadata failed:\n{result.stderr}")
    return json.loads(result.stdout)


def source_of(raw: dict) -> str:
    return raw["source"] if raw.get("source") else "path"


def normalise(raw: dict) -> dict:
    ids: dict[str, str] = {}
    packages = []
    for package in raw["packages"]:
        ids[package["id"]] = f"{package['name']}@{package['version']}"
        packages.append(
            {
                "id": ids[package["id"]],
                "name": package["name"],
                "version": package["version"],
                "source": source_of(package),
                "declared": sorted(
                    (
                        {
                            "package": dep["name"],
                            "rename": dep["rename"],
                            "requirement": dep["req"],
                            "kind": dep["kind"] or "normal",
                            "target": dep["target"],
                            "optional": dep["optional"],
                            "features": sorted(dep["features"]),
                            "source": source_of(dep),
                        }
                        for dep in package["dependencies"]
                    ),
                    key=lambda dep: json.dumps(dep, sort_keys=True),
                ),
            }
        )
    edges = []
    for node in raw["resolve"]["nodes"]:
        for dep in node["deps"]:
            edges.append(
                {
                    "from": ids[node["id"]],
                    "to": ids[dep["pkg"]],
                    "kinds": sorted(
                        (
                            {"kind": k["kind"] or "normal", "target": k["target"]}
                            for k in dep["dep_kinds"]
                        ),
                        key=lambda k: json.dumps(k, sort_keys=True),
                    ),
                }
            )
    return {
        "command": "cargo metadata --locked --offline --format-version 1",
        "workspace_members": sorted(ids[member] for member in raw["workspace_members"]),
        "packages": sorted(packages, key=lambda package: package["id"]),
        "edges": sorted(edges, key=lambda edge: (edge["from"], edge["to"])),
    }


def render() -> str:
    return json.dumps(normalise(metadata()), indent=2, sort_keys=True) + "\n"


def main() -> int:
    text = render()
    if "--check" in sys.argv[1:]:
        if not GOLDEN.exists() or GOLDEN.read_text() != text:
            print(f"{GOLDEN.name} is stale: rerun without --check", file=sys.stderr)
            return 1
        print(f"{GOLDEN.name} is current")
        return 0
    GOLDEN.write_text(text)
    print(f"wrote {GOLDEN.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
