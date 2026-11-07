#!/usr/bin/env python3
"""Dos scripts del repositorio que el gate no ejecuta, ejercitados desde un contrato.

Los dos contratos cargan o lanzan el script real y comprueban su veredicto,
incluidos sus fallos, y comparten el mecanismo: una lista `failures` a la que
cada comprobacion anade lo que no se cumple. La parte A es el lint de
capacidades (`sandbox/scripts/capabilities_drift_lint.py`); la B, la medicion
de la recall de la seleccion (`scripts/affected-tests-recall.py`). Cada parte
conserva su docstring original y sus comprobaciones.

Ejecutar: python3 scripts/ci/test_script_drift_and_recall.py

# PARTE A — deriva de la declaracion de capacidades

La declaracion de capacidades no puede separarse de la autoridad.

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
gate lo habria detenido. El contrato cierra esa asimetria: el lint
pasa a formar parte del conjunto de contratos, que el gate ejecuta.

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


# PARTE B — la recall de la seleccion

The contract of the recall measurement: `scripts/affected-tests-recall.py`.

The measurement compares the selection the product publishes against the
tests a complete run proves reach the change. Both sides can be given, and
that is what makes the comparison testable without a compiler in the loop:
the selection is a JSON document, and the run is a command that prints the
tests that fail.

What these checks prove:

    - the recall is computed as it says: of the tests the execution
      reaches, the ones the selection names;
    - a miss is REPORTED, not swallowed: a selection that names nothing
      says so, and says which test it failed to name;
    - the tree is left as it was: the file whose contract was broken is
      restored byte for byte;
    - a measurement that cannot be made is an error, not a number: a
      symbol that does not exist stops the run.

"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from types import ModuleType

failures: list[str] = []

# ======================= PARTE A: deriva de capacidades ======================

ROOT = Path(__file__).resolve().parent.parent.parent
LINT = ROOT / "sandbox/scripts/capabilities_drift_lint.py"
MATRIX = "docs/roadmap/consumer-contracts/specs/CAPABILITIES-MATRIX.md"
CATALOG = "product/languages.json"


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


# ======================= PARTE B: recall de la seleccion =====================

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "affected-tests-recall.py"
GATE = REPO / "scripts" / "test-gate.sh"


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


LIB = """pub fn watched() -> u32 {
    41
}
"""

TEST = """#[test]
fn the_test_that_watches() {
    assert_eq!(1, 1);
}
"""

# The runner the fixture uses. It fails the test that reaches the change
# only when the contract of the symbol is broken, which is what a real
# complete run does.
#
# The line it prints is the line nextest prints, with the counter of the
# progress between the time and the binary: the reader of the output has to
# understand the format that the real runner writes, not one that is easier
# to parse. And it prints it on STDERR, which is where the real runner
# writes what it did —its stdout stays empty—.
RUNNER = """#!/usr/bin/env python3
import os
import sys
from pathlib import Path

fixture = Path(os.environ["RECALL_FIXTURE"])
source = (fixture / "src" / "lib.rs").read_text(encoding="utf-8")
if "affected-tests-recall" in source:
    print("        FAIL [   0.005s] (   1/2) demo it::the_test_that_watches", file=sys.stderr)
sys.exit(100)
"""


def write_fixture(root: Path) -> Path:
    (root / "src").mkdir(parents=True)
    (root / "tests" / "it").mkdir(parents=True)
    (root / "Cargo.toml").write_text(
        '[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2021"\n\n'
        "[workspace]\n",
        encoding="utf-8",
    )
    (root / "src" / "lib.rs").write_text(LIB, encoding="utf-8")
    (root / "tests" / "it" / "checks.rs").write_text(TEST, encoding="utf-8")
    runner = root / "runner.sh"
    runner.write_text(RUNNER, encoding="utf-8")
    runner.chmod(0o755)
    return runner


def measure(
    fixture: Path,
    runner: Path,
    selection: list[dict],
    symbol: str = "watched",
    report: Path | None = None,
) -> tuple[int, dict, str]:
    """Run the measurement over the fixture and return its verdict."""
    document = fixture / "selection.json"
    document.write_text(
        json.dumps({"kind": "impact", "affected_tests": selection}), encoding="utf-8"
    )
    command = [
        sys.executable,
        str(SCRIPT),
        "--package",
        "demo",
        "--symbol",
        symbol,
        "--manifest-path",
        str(fixture / "Cargo.toml"),
        "--runner",
        str(runner),
        "--selection",
        str(document),
        "--json",
    ]
    if report is not None:
        command += ["--report", str(report)]
    proc = subprocess.run(
        command,
        cwd=REPO,
        capture_output=True,
        text=True,
        env={**os.environ, "RECALL_FIXTURE": str(fixture)},
    )
    parsed: dict = {}
    if proc.stdout.strip():
        try:
            parsed = json.loads(proc.stdout)
        except json.JSONDecodeError:
            parsed = {}
    return proc.returncode, parsed, proc.stderr


def test_the_recall_is_the_share_of_the_reached_tests_the_selection_names() -> None:
    """The selection names the test the complete run proves reaches the
    change: the recall is the whole truth."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, _ = measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
        )
        check(code == 0, f"the measurement failed: {code}")
        check(
            report.get("reached") == 1,
            f"the execution reaches one test: {report.get('reached')}",
        )
        check(
            report.get("recall") == 1.0,
            f"the selection names the test that reaches the change: {report.get('recall')}",
        )
        check(
            report.get("precision") == 1.0,
            f"nothing extra is selected: {report.get('precision')}",
        )


def test_a_selection_that_names_nothing_reports_the_miss() -> None:
    """The number is not the whole answer: the test the selection failed
    to name is listed, because a miss is either a defect of the selection
    or a limit of the oracle, and only a human can say which."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, _ = measure(fixture, runner, [])
        check(code == 0, f"the measurement failed: {code}")
        check(
            report.get("recall") == 0.0,
            f"an empty selection has no recall against the truth: {report.get('recall')}",
        )
        misses = report.get("misses") or []
        check(
            len(misses) == 1 and misses[0]["name"] == "the_test_that_watches",
            f"the missed test is named: {misses}",
        )


def test_the_tree_is_left_as_it_was() -> None:
    """The measurement breaks a contract on purpose, and it puts the tree
    back: a measurement that leaves the tree modified is worse than no
    measurement."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        source = fixture / "src" / "lib.rs"
        before = source.read_bytes()
        measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
        )
        check(
            source.read_bytes() == before,
            "the file whose contract was broken is restored byte for byte",
        )


def test_a_symbol_that_does_not_exist_is_an_error_and_not_a_number() -> None:
    """The measurement that cannot be made says so. A zero would be a
    figure, and a figure is believed."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, stderr = measure(fixture, runner, [], symbol="not_there")
        check(code != 0, f"the measurement of a symbol that does not exist fails: {code}")
        check(
            "not_there" in stderr,
            f"the error names the symbol that was not found: {stderr!r}",
        )
        check(
            not report,
            f"no report is printed when the measurement could not be made: {report}",
        )


def test_the_measurement_writes_the_artefact_the_receipt_cites() -> None:
    """The figure is cited with the SHA of the tree it was measured on, and
    the report is a file: a number in a journal with nothing behind it is a
    claim, not a measurement."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        artefact = fixture / "recall.json"
        code, _, _ = measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
            report=artefact,
        )
        check(code == 0, f"the measurement failed: {code}")
        check(artefact.exists(), "the report is written where it was asked for")
        if artefact.exists():
            written = json.loads(artefact.read_text(encoding="utf-8"))
            check(
                written.get("recall") == 1.0,
                f"the report carries the figure that was measured: {written}",
            )
            check(
                bool(written.get("head")),
                f"the report says the tree it was measured on: {written}",
            )


def test_the_measurement_stays_out_of_the_gate() -> None:
    """The measurement is asked for on purpose. A gate that runs it would
    pay its price on every batch, and the price is a complete run of the
    suite twice."""
    text = GATE.read_text(encoding="utf-8")
    check(
        "affected-tests-recall" not in text,
        "scripts/test-gate.sh does not run the recall measurement: it is asked for on purpose",
    )


def main() -> int:
    module = sys.modules[__name__]
    checks = sorted(
        (
            getattr(module, name)
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda check_: check_.__code__.co_firstlineno,
    )
    assert checks, "ningun test_ encontrado: el contrato no comprobaria nada"
    for check_ in checks:
        before = len(failures)
        try:
            check_()
        except AssertionError as exc:
            failures.append(f"{check_.__name__}: {exc}")
        print(f"  {'PASS' if len(failures) == before else 'FAIL'} {check_.__name__}")
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print(f"PASS — deriva de capacidades y recall de la seleccion ({len(checks)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
