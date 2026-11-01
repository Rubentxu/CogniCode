#!/usr/bin/env python3
"""Contrato del clasificador que decide que carril respalda el estado `merge-gate`.

## Lo que esta en juego

`publish-merge-gate.sh` corre `docs-gate.pipeline.kts` en lugar del gate
completo cuando `scripts/ci/change-class.sh` dice `docs`. Un `docs` equivocado
es un cambio de producto que entra en `main` sin compilar ni probar nada. Por
eso el clasificador falla cerrado, y este contrato fija los dos lados:

  - lo que DEBE ser `docs` (si no, el carril corto nunca se usa y nadie lo nota);
  - lo que NUNCA puede ser `docs` (codigo, pipelines, Markdown que viaja dentro
    de un artefacto, un diff vacio).

Y comprueba las dos condiciones que hacen seguro saltarse la compilacion: que
ningun crate haga `include_str!` de algo que el clasificador llame `docs`, y
que el carril de docs ejecute los tests que leen documentos del repositorio.

Run:
    python3 scripts/ci/test_change_class.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts/ci/change-class.sh"
READERS = REPO_ROOT / "scripts/ci/docs-readers.sh"
PUBLISH = REPO_ROOT / "scripts/ci/publish-merge-gate.sh"
DOCS_LANE = REPO_ROOT / "docs-gate.pipeline.kts"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def classify(*files: str) -> str:
    done = subprocess.run(
        ["bash", str(SCRIPT), "--files", *files],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    return done.stdout.strip()


def test_documentation_changes_are_docs() -> None:
    for files in (
        ("docs/roadmap/JOURNAL.md",),
        ("docs/roadmap/ROADMAP.md", "ROADMAP.md", "goal-compass.json"),
        ("odd/tasks/x.md", "evidence/u21/run.jsonl", "plans/001.md"),
        ("AGENTS.md",),
    ):
        got = classify(*files)
        check(got == "docs", f"{files} es solo documentacion y sale '{got}'")


def test_anything_else_is_code() -> None:
    for files in (
        ("crates/cognicode-core/src/lib.rs",),
        ("docs/roadmap/JOURNAL.md", "crates/cognicode-core/src/lib.rs"),
        ("merge-gate.pipeline.kts",),
        ("docs-gate.pipeline.kts",),
        ("scripts/ci/change-class.sh",),
        ("Cargo.toml",),
        ("Cargo.lock",),
        ("crates/cognicode-cli/README.md",),
        ("skills/cognicode-core/SKILL.md",),
        ("product/tools.json",),
        ("apps/explorer-ui/README.md",),
        ("sandbox/fixtures/x.md",),
        ("justfile",),
    ):
        got = classify(*files)
        check(got == "code", f"{files} toca producto o el propio gate y sale '{got}'")


def test_an_empty_change_is_code() -> None:
    got = classify()
    check(got == "code", f"un diff vacio debe dar 'code' (fallar cerrado) y da '{got}'")


def test_no_crate_compiles_in_a_docs_file() -> None:
    """Si un crate hiciera include_str! de un fichero `docs`, saltarse la
    compilacion dejaria de ser seguro."""
    pattern = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"')
    offenders = []
    for rs in REPO_ROOT.glob("crates/**/*.rs"):
        if "spike-" in str(rs):
            continue
        for target in pattern.findall(rs.read_text(encoding="utf-8", errors="replace")):
            resolved = (rs.parent / target).resolve()
            try:
                rel = resolved.relative_to(REPO_ROOT).as_posix()
            except ValueError:
                continue
            if classify(rel) == "docs":
                offenders.append(f"{rs.relative_to(REPO_ROOT)} -> {rel}")
    check(not offenders, f"estos crates compilan dentro un fichero que el clasificador llama docs: {offenders}")


def test_the_docs_lane_runs_the_tests_that_read_docs() -> None:
    lane = DOCS_LANE.read_text(encoding="utf-8")
    check("docs-readers.sh" in lane, "docs-gate.pipeline.kts no ejecuta los tests de Rust que leen documentos")
    check("run-all-contracts.sh" in lane, "docs-gate.pipeline.kts no ejecuta los contratos")
    check("change-class.sh" in lane, "docs-gate.pipeline.kts no comprueba por si mismo que el cambio sea docs")
    readers = READERS.read_text(encoding="utf-8")
    for crate, test in re.findall(r'"(cognicode-[a-z-]+) ([a-z0-9_]+)"', readers):
        check((REPO_ROOT / "crates" / crate / "tests" / f"{test}.rs").is_file(),
              f"docs-readers.sh nombra {crate}/{test}, que no existe")


def test_the_status_names_the_lane() -> None:
    text = PUBLISH.read_text(encoding="utf-8")
    check("change-class.sh" in text, "publish-merge-gate.sh no consulta el clasificador")
    check('description="$LANE passed' in text,
          "la descripcion del estado no dice que carril lo produjo: un verde de docs se leeria como uno completo")


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — change-class: docs solo cuando nada compilado cambia; todo lo demas, gate completo")
    return 0


if __name__ == "__main__":
    sys.exit(main())
