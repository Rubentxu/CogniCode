#!/usr/bin/env python3
"""Print the mermaid edges between the SDK library crates, one per line.

The libraries are the crates `docs/distribution/SDK_SURFACES.json` classifies
as `library`; the edges are their normal dependencies on one another, read
from `cargo metadata`. The output is the body of the diagram in
`docs/distribution/SDK_ARCHITECTURE.md`.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DECLARATION = ROOT / "docs" / "distribution" / "SDK_SURFACES.json"


def edge_lines():
    declared = json.loads(DECLARATION.read_text(encoding="utf-8"))["crates"]
    libraries = sorted(n for n, i in declared.items() if i["class"] == "library")
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True, timeout=180,
    ).stdout
    packages = {p["name"]: p for p in json.loads(out)["packages"]}
    ident = lambda name: name.replace("-", "_")  # noqa: E731 (mermaid ids)
    return [
        f"  {ident(name)} --> {ident(dep['name'])}"
        for name in libraries
        for dep in sorted(packages[name]["dependencies"], key=lambda d: d["name"])
        if dep["kind"] is None and dep["name"] in libraries
    ]


if __name__ == "__main__":
    print("\n".join(edge_lines()))
    sys.exit(0)
