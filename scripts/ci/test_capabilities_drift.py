#!/usr/bin/env python3
"""La matriz de capacidades y el codigo no pueden separarse.

El producto publica que lenguajes analiza y con que precision. Esa
declaracion vive en la matriz de capacidades; la capacidad real vive en
`ALL_TREE_SITTER_LANGS`, en el crate. El lint
`sandbox/scripts/capabilities_drift_lint.py` compara las dos cosas y falla
si divergen.

Este contrato existe por una asimetria concreta. El lint era stage de
merge-gate y nada mas: el gate local (`scripts/test-full.sh`) no lo
ejecutaba, asi que el veredicto que el operador ve antes de abrir el PR
podia ser verde con la matriz borrada. Una poda de documentacion borro el
fichero que el lint lee, el lint sigo dando verde en local y solo el
merge-gate lo habria detenido. El contrato cierra esa asimetria: el lint
pasa a formar parte del conjunto de contratos, que el gate local y el
merge-gate ejecutan los dos.

No reimplementa la comparacion. Carga el lint y lo ejercita, incluidos
sus dos fallos. Un guard que solo sabe decir "todo bien" no es un guard:
por eso hay controles negativos.

## Lo que se comprueba

- El arbol real esta alineado: la lista de la matriz coincide con la del
  codigo, el header declara el numero correcto, y el lint en modo estricto
  sale con codigo 0.
- El fichero de la matriz esta versionado. Un fichero que solo existe en
  la maquina del autor no es una autoridad para nadie mas.
- El lint falla con codigo 2 cuando la matriz no esta (no puede leerla).
- El lint falla con codigo 1 cuando la lista y el header no coinciden.
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import subprocess
import sys
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parent.parent.parent
LINT = ROOT / "sandbox/scripts/capabilities_drift_lint.py"
MATRIX = "docs/roadmap/consumer-contracts/specs/CAPABILITIES-MATRIX.md"

failures: list[str] = []


def _load_lint() -> ModuleType:
    spec = importlib.util.spec_from_file_location("capabilities_drift_lint", LINT)
    if spec is None or spec.loader is None:
        raise AssertionError(f"no se puede cargar el lint en {LINT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _run_lint(module: ModuleType, argv: list[str]) -> tuple[int, str]:
    """Ejecuta el lint capturando su salida. Devuelve (codigo, salida)."""
    saved_argv = sys.argv[:]
    buffer = io.StringIO()
    sys.argv = argv
    try:
        with contextlib.redirect_stdout(buffer), contextlib.redirect_stderr(buffer):
            try:
                code = module.main()
            except SystemExit as exc:
                code = int(exc.code or 0)
    finally:
        sys.argv = saved_argv
    return code, buffer.getvalue()


def test_the_matrix_matches_the_code() -> None:
    """La lista de la matriz y la del codigo son la misma lista."""
    module = _load_lint()
    code_langs = module.parse_code_langs()
    doc_langs, header_count = module.parse_doc_langs()
    if code_langs != doc_langs:
        only_code = sorted(set(code_langs) - set(doc_langs))
        only_doc = sorted(set(doc_langs) - set(code_langs))
        failures.append(
            f"la matriz de capacidades y el codigo declaran lenguajes "
            f"distintos. Solo en codigo: {only_code}. Solo en la matriz: "
            f"{only_doc}."
        )
    if header_count is not None and header_count != len(doc_langs):
        failures.append(
            f"la matriz declara ({header_count} lenguajes) y su lista tiene "
            f"{len(doc_langs)}."
        )


def test_the_lint_accepts_the_real_tree() -> None:
    """El lint en modo estricto sale con codigo 0 sobre el arbol real."""
    module = _load_lint()
    code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    if code != 0:
        failures.append(
            f"el lint de capacidades falla sobre el arbol real con codigo "
            f"{code}:\n{output}"
        )


def test_the_matrix_is_versioned() -> None:
    """La matriz existe en un clon limpio, no solo en esta maquina."""
    result = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--error-unmatch", MATRIX],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        failures.append(
            f"{MATRIX} no esta versionado. Un guard que lee un fichero sin "
            f"versionar pasa en la maquina de su autor y falla en todas las "
            f"demas."
        )


def test_the_lint_fails_when_the_matrix_is_missing() -> None:
    """Control negativo: sin matriz, el lint sale con codigo 2."""
    module = _load_lint()
    original = module.DOC_FILE
    module.DOC_FILE = ROOT / "docs/roadmap/consumer-contracts/specs/NO-EXISTE.md"
    try:
        code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    finally:
        module.DOC_FILE = original
    if code != 2:
        failures.append(
            f"con la matriz ausente el lint deberia salir con codigo 2 y ha "
            f"salido con {code}. Un guard que no puede fallar no es un "
            f"guard.\n{output}"
        )


def test_the_lint_fails_when_the_header_does_not_match() -> None:
    """Control negativo: header y lista en desacuerdo, el lint sale con 1."""
    module = _load_lint()
    original = module.DOC_FILE
    fake = ROOT / "scripts/ci/.capabilities-drift-lint-fixture.md"
    fake.write_text(
        "## Lenguajes con parser tree-sitter\n"
        "\n"
        "Lista oficial (3 lenguajes):\n"
        "\n"
        "```\n"
        "Python, Rust, JavaScript\n"
        "```\n",
        encoding="utf-8",
    )
    module.DOC_FILE = fake
    try:
        code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    finally:
        module.DOC_FILE = original
        fake.unlink(missing_ok=True)
    if code != 1:
        failures.append(
            f"con el header en desacuerdo el lint deberia salir con codigo 1 "
            f"y ha salido con {code}.\n{output}"
        )


def main() -> int:
    for func in (
        test_the_matrix_matches_the_code,
        test_the_lint_accepts_the_real_tree,
        test_the_matrix_is_versioned,
        test_the_lint_fails_when_the_matrix_is_missing,
        test_the_lint_fails_when_the_header_does_not_match,
    ):
        try:
            func()
        except AssertionError as exc:
            failures.append(str(exc))

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1

    print("PASS — la matriz de capacidades coincide con el codigo.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
