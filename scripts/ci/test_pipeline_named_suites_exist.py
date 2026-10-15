#!/usr/bin/env python3
"""Every `cargo test --test <name>` a pipeline names must name a file that exists.

MEDIDO 2026-10-15. `merge-gate.pipeline.kts` carried two stages for
`prf_h06_adversarial_e2e`:

    stage("core-adversarial-e2e") {
        sh("cargo test -p cognicode-core --test prf_h06_adversarial_e2e")
    }
    ...
    stage("mcp-prf-h06-adversarial-e2e") {
        sh("cargo test -p cognicode-mcp --test prf_h06_adversarial_e2e")
    }

The suite had moved to `cognicode-mcp` because `CARGO_BIN_EXE_cognicode-mcp`
only exists for the package that declares that binary. The new stage was
added and the old one was left, and the gate failed on it:

    error: no test target named `prf_h06_adversarial_e2e` in `cognicode-core`
    package

That is not the interesting part. The interesting part is how long the
gate had been able to carry a command that cannot work. Before the
fail-fast fix, a stage whose command fails mid-run did not stop the
stage, so the same stale line had been harmless on every previous run —
and nobody had to look at it, because nothing was ever reported red.

The cost of that line was a 6-minute gate run that ended on it. This
contract is the cheap half: the answer is in the filesystem, so it does
not need a run to be found.

Two directions, both required:

  * Every suite a pipeline names exists under the crate it names. Catches
    typos, and suites that moved between crates.
  * The reverse — every suite that exists is named by some gate or is
    deliberately excluded — already lives in
    `cli_gate_coverage_contract`. This file does not duplicate it; it is
    the other half, and the two together mean the name sets are equal.

Run:
    python3 scripts/ci/test_pipeline_named_suites_exist.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# `cargo test -p cognicode-cli --test a015_onboarding_gate`, con los
# argumentos en el orden que escribe la gente. El crate puede ir antes o
# despues del selector de target.
INVOCATION = re.compile(
    r"-p\s+(\S+)[^\n]*?--test\s+([A-Za-z0-9_]+)"
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def pipeline_files() -> list[Path]:
    return sorted(REPO_ROOT.glob("*.pipeline.kts"))


def suites_of(path: Path) -> set[tuple[str, str]]:
    """(crate, suite) que el pipeline nombra. Comentarios fuera: no ejecutan."""
    out: set[tuple[str, str]] = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line.startswith("//") or line.startswith("#"):
            continue
        for crate, suite in INVOCATION.findall(line):
            out.add((crate, suite))
    return out


def test_every_named_suite_exists_under_the_crate_it_names() -> None:
    """`(crate, suite)` -> `crates/<crate>/tests/<suite>.rs`.

    Un target de integracion de cargo es exactamente ese fichero. Si la suite
    esta en otro crate, el nombre no basta: `-p` dice donde mira cargo, y
    escribir el par equivocado no es un fallo de estilo sino un stage que
    nunca puede pasar.
    """
    named = set()
    for pipeline in pipeline_files():
        named |= {(p.name, *pair) for pair in suites_of(pipeline) for p in [pipeline]}

    check(
        len(named) >= 30,
        f"solo se han encontrado {len(named)} suites nombradas por los "
        f"pipelines. Si el extractor dejo de verlas, este contrato no esta "
        f"midiendo nada y pasaria siempre.",
    )

    missing = []
    for pipeline, crate, suite in sorted(named):
        candidate = REPO_ROOT / "crates" / crate / "tests" / f"{suite}.rs"
        if not candidate.is_file():
            actual = [
                c.name
                for c in (REPO_ROOT / "crates").iterdir()
                if c.is_dir() and (c / "tests" / f"{suite}.rs").is_file()
            ]
            where = f" (la suite vive en {actual[0]})" if actual else ""
            missing.append(f"{pipeline}: {suite} nombrado bajo -p {crate}{where}")

    check(
        not missing,
        f"estos `cargo test --test` nombran un target que no existe:\n  "
        + "\n  ".join(missing)
        + "\ncargo responde `no test target named ... in ... package` y la etapa "
        "muere. Antes de que el guard `set -e` existiera, ese fallo no paraba "
        "la etapa y el gate pasaba igual: una linea rota puede ser "
        "portadora durante meses.",
    )


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a failing check is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        "PASS — every suite a pipeline names exists under the crate it names; a "
        "stage cannot name a target cargo would reject"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())