#!/usr/bin/env python3
"""La declaracion de capacidades no puede separarse de la autoridad.

El producto publica que lenguajes analiza y con que precision. Esa
declaracion vive en las capacidades de cada tool; la capacidad real vive
en el registro de parsers, que es lo que liga un lenguaje a su gramatica
compilada. El lint `sandbox/scripts/capabilities_drift_lint.py` mide la
una contra la otra y falla si divergen.

Este contrato existe por una asimetria concreta. El lint era stage de
merge-gate y nada mas: el gate local (`scripts/test-full.sh`) no lo
ejecutaba, asi que el veredicto que el operador ve antes de abrir el PR
podia ser verde con la matriz borrada. Una poda de documentacion borro el
fichero que el lint lee, el lint sigo dando verde en local y solo el
merge-gate lo habria detenido. El contrato cierra esa asimetria: el lint
pasa a formar parte del conjunto de contratos, que el gate local y el
merge-gate ejecutan los dos.

No reimplementa la comparacion. Carga el lint y lo ejercita, incluidos
sus fallos. Un guard que solo sabe decir "todo bien" no es un guard: por
eso hay controles negativos.

## Lo que se comprueba

- El arbol real esta alineado: el documento no declara lista propia, la
  derivacion coincide con el catalogo publicado, y el lint en modo
  estricto sale con codigo 0.
- El documento y el catalogo estan versionados. Un fichero que solo
  existe en la maquina del autor no es una autoridad para nadie mas.
- El lint falla con codigo 2 cuando el documento no esta (no puede
  leerlo), y cuando el catalogo publicado no esta.
- El lint falla con codigo 1 cuando el documento vuelve a declarar una
  lista que no coincide con el catalogo: esa lista seria una tercera
  fuente de verdad.
- El lint falla con codigo 2 cuando el codigo deja de derivar la lista.
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
CATALOG = "product/languages.json"

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


def test_the_document_declares_no_list_of_its_own() -> None:
    """El documento no vuelve a enumerar los lenguajes.

    Enumerarlos aqui era una tercera fuente de verdad, y las tres se
    separaron sin que nadie se enterara.
    """
    module = _load_lint()
    doc_langs, header_count = module.parse_doc_langs()
    if doc_langs:
        failures.append(
            f"el documento declara una lista propia de lenguajes ({doc_langs}). "
            f"La lista se deriva del registro y se publica en {CATALOG}; "
            f"declararla aqui es una tercera fuente de verdad."
        )
    if header_count is not None:
        failures.append(
            f"el documento declara un recuento ({header_count} lenguajes). "
            f"Un recuento es una lista que hay que mantener."
        )


def test_the_derivation_matches_the_published_catalog() -> None:
    """Lo derivado y lo publicado son el mismo conjunto."""
    module = _load_lint()
    catalog_langs = module.parse_catalog_langs()
    if not catalog_langs:
        failures.append("el catalogo publicado no declara ningun lenguaje")
    code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    if code != 0:
        failures.append(
            f"el lint de capacidades falla sobre el arbol real con codigo "
            f"{code}:\n{output}"
        )


def test_the_documents_are_versioned() -> None:
    """El documento y el catalogo existen en un clon limpio."""
    for path in (MATRIX, CATALOG):
        result = subprocess.run(
            ["git", "-C", str(ROOT), "ls-files", "--error-unmatch", path],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            failures.append(
                f"{path} no esta versionado. Un guard que lee un fichero sin "
                f"versionar pasa en la maquina de su autor y falla en todas las "
                f"demas."
            )


def test_the_lint_fails_when_the_matrix_is_missing() -> None:
    """Control negativo: sin documento, el lint sale con codigo 2."""
    module = _load_lint()
    original = module.DOC_FILE
    module.DOC_FILE = ROOT / "docs/roadmap/consumer-contracts/specs/NO-EXISTE.md"
    try:
        code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    finally:
        module.DOC_FILE = original
    if code != 2:
        failures.append(
            f"con el documento ausente el lint deberia salir con codigo 2 y ha "
            f"salido con {code}. Un guard que no puede fallar no es un "
            f"guard.\n{output}"
        )


def test_the_lint_fails_when_the_catalog_is_missing() -> None:
    """Control negativo: sin catalogo publicado, el lint sale con codigo 2."""
    module = _load_lint()
    original = module.CATALOG_FILE
    module.CATALOG_FILE = ROOT / "product/NO-EXISTE.json"
    try:
        code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    finally:
        module.CATALOG_FILE = original
    if code != 2:
        failures.append(
            f"sin el catalogo publicado el lint deberia salir con codigo 2 y ha "
            f"salido con {code}. La autoridad tiene que estar ahi para que la "
            f"derivacion se pueda medir.\n{output}"
        )


def test_the_lint_fails_when_the_document_declares_a_different_list() -> None:
    """Control negativo: el documento vuelve a declarar una lista ajena.

    Es el defecto exacto que ya ocurrio y que el lint anterior no veia:
    el documento y el codigo declaraban la misma lista de veinte y una
    entradas, y las dos mentian frente al registro.
    """
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
            f"con el documento declarando una lista ajena el lint deberia salir "
            f"con codigo 1 y ha salido con {code}.\n{output}"
        )


def test_the_lint_fails_when_the_code_stops_deriving() -> None:
    """Control negativo: sin derivacion en el codigo, el lint sale con 2.

    Es el otro modo de romper el contrato: no que las listas se separen,
    sino que la derivacion desaparezca y la lista vuelva a ser escrita a
    mano.
    """
    module = _load_lint()
    original = module.CODE_FILE
    fake = ROOT / "scripts/ci/.capabilities-drift-code-fixture.rs"
    fake.write_text("pub const ALL_TREE_SITTER_LANGS: &[&str] = &[\"python\"];\n", encoding="utf-8")
    module.CODE_FILE = fake
    try:
        code, output = _run_lint(module, ["capabilities_drift_lint.py", "--strict"])
    finally:
        module.CODE_FILE = original
        fake.unlink(missing_ok=True)
    if code != 2:
        failures.append(
            f"sin derivacion en el codigo el lint deberia salir con codigo 2 y "
            f"ha salido con {code}.\n{output}"
        )


def main() -> int:
    for func in (
        test_the_document_declares_no_list_of_its_own,
        test_the_derivation_matches_the_published_catalog,
        test_the_documents_are_versioned,
        test_the_lint_fails_when_the_matrix_is_missing,
        test_the_lint_fails_when_the_catalog_is_missing,
        test_the_lint_fails_when_the_document_declares_a_different_list,
        test_the_lint_fails_when_the_code_stops_deriving,
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

    print("PASS — la declaracion de capacidades coincide con la autoridad.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
