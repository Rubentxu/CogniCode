#!/usr/bin/env python3
"""Tres contratos sobre scripts del repositorio que el gate no ejecuta, ejercitados desde un contrato.

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

Parte C (TASK-74), absorbida de `test_harness_ab.py`:

El harness A/B y sus metas: lo que la medicion decide, y lo que no.

## El hueco, exacto

TASK-93 pide tres cosas: un comando de medicion a demanda que mida la
superficie que ve un agente (tokens, llamadas, coste, tiempo y exactitud
juzgada) con y sin la superficie v2; que una meta superada haga fallar ese
comando; y que los resultados se publiquen por release con su commit.

El comando vive FUERA de la suite y FUERA del gate —por eso es un comando y
no un test—, asi que su logica de decision no la vigila nadie. Este contrato
la vigila. Concretamente:

1. **El juez** decide si una respuesta es correcta. Si el juez fuera erroneo,
   el harness mediria la exactitud de otra cosa y publicaria una cifra falsa.
   El juez se comprueba contra los tres `symbols.golden` —el oracle escrito a
   mano que ya usa `language_symbols_goldens.rs`—: cada entrada del golden
   tiene que coincidir con el juez. El juez no se valida a si mismo.
2. **El veredicto** decide si el comando falla. Un veredicto que no falla es
   un comando verde por defecto, que es la forma que tiene un gate de no
   vigilar nada.
3. **La extraccion** lee las respuestas de las dos superficies, que no tienen
   la misma forma: `entries[].file` en el brazo v2 y `calls[].file` en el
   legacy. Una extraccion que leyera solo una de las dos mediria un brazo y
   dejaria el otro en cero.
4. **El comando no entra en el gate.** Es una parte del encargo (AC2: «no
   entra en el gate»), asi que se comprueba: si el gate lo invoca, el gate
   paga una medicion que nadie pidio.
5. **Cada medicion publicada nombra su commit y su version.** Sin eso, «publicado
   por release con su commit» no se puede comprobar.

## Que NO se comprueba, y por que no se disimula

- **Que el harness mida bien el producto.** Eso lo mide el harness, en vivo,
  contra los repos de referencia. Este contrato comprueba su LOGICA, con
  fixtures: que el juez coincide con el oracle, que el veredicto falla cuando
  debe, y que las extracciones leen las dos formas.
- **Que las metas de latencia se cumplan en la maquina del operador.** Es una
  medicion, no un contrato: depende del hardware, del binario y de la carga.
  La cumple o no la cumple el comando, con su rc.
- **Que el agente del harness sea un LLM.** No lo es: es una politica
  determinista, y el harness mide lo que le cuesta a un agente **entender y
  usar** la superficie, no como se comporta un modelo concreto. El numero de
  llamadas que haria un LLM depende de su politica, asi que el harness lo
  mide y lo publica como observacion, y no como meta.

## Mutaciones que este contrato tiene que detectar

- cambiar el juez para que acepte cualquier respuesta (o para que no acepte
  ninguna): el golden lo delata;
- cambiar el veredicto para que nunca falle: la guarda de suelo lo delata;
- cambiar la extraccion del brazo legacy para que lea un campo que no existe:
  la prueba de las dos formas lo delata;
- meter el harness en el gate: lo delata;
- publicar una medicion sin commit o sin version: lo delata.
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import subprocess
import re
import sys
import tempfile
import tomllib
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


# ===========================================================================
# Parte C — el harness A/B (absorbido de test_harness_ab.py)
# ===========================================================================

REPO_ROOT = ROOT

HARNESS = REPO_ROOT / "scripts" / "measure" / "harness_ab.py"
SLO_BUDGET = REPO_ROOT / "slo-budget.toml"
HARNESS_CONFIG = REPO_ROOT / "scripts" / "measure" / "harness.toml"
MEASUREMENTS = REPO_ROOT / "product" / "measurements"


def load_module():
    spec = importlib.util.spec_from_file_location("harness_ab", HARNESS)
    if spec is None or spec.loader is None:
        raise AssertionError(f"cannot load {HARNESS}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def read_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


# --- 1. El juez contra el oracle escrito a mano ------------------------------


def test_the_judge_agrees_with_the_hand_written_goldens() -> None:
    """Cada entrada del golden tiene que coincidir con el juez.

    El golden dice que `class Shape@Shapes.kt` —el fichero declara el tipo—.
    El juez tiene que decir lo mismo, y para un símbolo que el fichero NO
    declara tiene que decir que no.
    """
    harness = load_module()
    config = read_toml(HARNESS_CONFIG)
    languages = config.get("language", {})

    checked = 0
    for repo in config.get("repo", []):
        golden_path = REPO_ROOT / repo["path"] / "symbols.golden"
        if not golden_path.is_file():
            continue
        patterns = languages[repo["language"]]["declaration"]
        for line in golden_path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("@"):
                continue
            kind, _, rest = line.partition(" ")
            name, _, path = rest.partition("@")
            source = (REPO_ROOT / repo["path"] / path).read_text(encoding="utf-8")
            assert harness.declares(source, name, patterns), (
                f"the judge does not see `{name}` in {path}, which the hand-written "
                f"golden declares ({kind})"
            )
            checked += 1

    assert checked >= 20, (
        f"the judge was checked against only {checked} golden entries; a check "
        f"over nothing is not a check"
    )

    # Y al reves: un simbolo que el fichero no declara, el juez no lo ve.
    patterns = languages["kotlin"]["declaration"]
    source = (REPO_ROOT / "sandbox" / "fixtures" / "kotlin" / "Shapes.kt").read_text(
        encoding="utf-8"
    )
    assert not harness.declares(source, "UnSymboloQueNoExiste", patterns), (
        "the judge sees a symbol that the file does not declare"
    )


def test_the_judge_does_not_see_a_call_as_a_declaration() -> None:
    """Un fichero que LLAMA a un símbolo no lo DECLARA.

    La diferencia entre las dos es lo que hace que el juez del `trace` pueda
    excluir el fichero de la declaracion: si una llamada contara como
    declaracion, el juez no encontraria nunca un llamante.
    """
    harness = load_module()
    config = read_toml(HARNESS_CONFIG)
    patterns = config["language"]["rust"]["declaration"]
    call = config["language"]["rust"]["call"]

    source = "fn main() {\n    helper();\n}\n"
    assert not harness.declares(source, "helper", patterns), (
        "the judge sees a call as a declaration"
    )
    assert harness.calls(source, "helper", call), "the judge does not see a call"
    assert not harness.calls("fn other() {}\n", "helper", call), (
        "the judge sees a call that is not there"
    )


# --- 2. El veredicto ----------------------------------------------------------


def test_the_verdict_is_quiet_when_every_goal_is_met() -> None:
    """Con todas las cifras dentro de las metas, no hay violaciones."""
    harness = load_module()
    goals = harness.load_goals(SLO_BUDGET)
    measured = {
        "v2": {
            "tools_list_bytes": 4350,
            "status_p95_ms": 120.0,
            "find_p95_ms": 210.0,
            "inspect_p95_ms": 300.0,
            "context_p95_ms": 1500.0,
            "accuracy": 1.0,
        },
        "legacy": {"accuracy": 1.0},
    }

    violations = harness.check_goals(measured, goals)

    assert violations == [], f"a measurement inside every goal raised violations: {violations}"


def test_the_verdict_names_the_goal_and_the_figure_when_one_is_not_met() -> None:
    """Una meta superada falla, y el mensaje dice CUAL y CUANTO.

    El nombre de la meta y la cifra medida son lo que hace que el fallo sea
    accionable: «el gate esta rojo» no dice nada.
    """
    harness = load_module()
    goals = harness.load_goals(SLO_BUDGET)
    measured = {
        "v2": {
            "tools_list_bytes": 4350,
            "status_p95_ms": 120.0,
            "find_p95_ms": 412.0,
            "inspect_p95_ms": 300.0,
            "context_p95_ms": 1500.0,
            "accuracy": 1.0,
        },
        "legacy": {"accuracy": 1.0},
    }

    violations = harness.check_goals(measured, goals)

    assert len(violations) == 1, f"expected one violation, got {violations}"
    violation = violations[0]
    assert "find" in violation["goal"], f"the violation does not name the goal: {violation}"
    assert violation["measured"] == 412.0, f"the violation does not carry the figure: {violation}"
    assert violation["ceiling"] == 300, f"the violation does not carry the ceiling: {violation}"
    message = harness.violation_message(violation)
    assert "412" in message and "300" in message, f"the message does not carry both figures: {message}"


def test_the_verdict_compares_the_two_arms_when_the_goal_is_a_comparison() -> None:
    """Una meta de comparacion mira los dos brazos, no uno."""
    harness = load_module()
    goals = harness.load_goals(SLO_BUDGET)
    measured = {
        "v2": {"accuracy": 0.75, "find_p95_ms": 100.0},
        "legacy": {"accuracy": 1.0},
    }

    violations = harness.check_goals(measured, goals)

    names = [violation["goal"] for violation in violations]
    assert any("accuracy" in name for name in names), (
        f"the comparison goal did not fire with the v2 arm less accurate: {violations}"
    )


# --- 3. Las dos formas de respuesta ------------------------------------------


def test_the_extractors_read_both_surfaces() -> None:
    """Las dos superficies no tienen la misma forma, y las dos se leen.

    El brazo v2 responde `entries[].file`; el legacy, `calls[].file`. Una
    extraccion que leyera solo una mediria un brazo y dejaria el otro en
    cero, que es un numero que parece bueno.
    """
    harness = load_module()

    v2_trace = {"result": {"entries": [{"file": "/ws/a.rs", "name": "main"}]}}
    legacy_trace = {"result": {"calls": [{"file": "/ws/b.rs", "symbol": "main"}]}}
    assert harness.extract_trace_files(v2_trace, "v2") == ["/ws/a.rs"]
    assert harness.extract_trace_files(legacy_trace, "legacy") == ["/ws/b.rs"]

    v2_find = {"result": {"locations": [{"file": "/ws/a.rs", "line": 3}]}}
    legacy_find = {"result": {"locations": [{"file": "/ws/b.rs", "line": 7}]}}
    assert harness.extract_locations(v2_find) == ["/ws/a.rs"]
    assert harness.extract_locations(legacy_find) == ["/ws/b.rs"]

    v2_inspect = {"result": {"symbols": [{"name": "Shape", "file_path": "/ws/a.kt"}]}}
    legacy_inspect = {"result": {"symbols": [{"name": "Shape", "location": {"file": "/ws/b.kt"}}]}}
    assert harness.extract_symbol_names(v2_inspect) == ["Shape"]
    assert harness.extract_symbol_names(legacy_inspect) == ["Shape"]


def test_an_extraction_that_finds_nothing_is_an_empty_list_and_not_a_crash() -> None:
    """Una respuesta que no trae lo que se busca devuelve una lista vacia."""
    harness = load_module()
    assert harness.extract_trace_files({"result": {}}, "v2") == []
    assert harness.extract_locations({"result": {}}) == []
    assert harness.extract_symbol_names({"result": {}}) == []


# --- 4. El comando vive fuera del gate ---------------------------------------


def test_the_command_does_not_enter_the_gate() -> None:
    """AC2 dice que el comando NO entra en el gate.

    El gate cuesta minutos (el paso de mutantes es el 92% de esa hora), y una
    medicion que nadie ha pedido no puede estar en un carril que se corre en
    cada lote.
    """
    gate = GATE.read_text(encoding="utf-8")
    assert "harness_ab" not in gate, "the gate invokes the measurement command"
    assert "slo-budget" not in gate, "the gate reads the SLO budget"


# --- 5. Lo que se publica ----------------------------------------------------


def test_every_published_measurement_names_its_commit_and_its_version() -> None:
    """«Publicado por release con su commit» se puede comprobar o no."""
    if not MEASUREMENTS.is_dir():
        return
    published = sorted(MEASUREMENTS.glob("*.json"))
    assert published, "there is no published measurement at all"
    for path in published:
        document = read_json(path)
        for field in ("version", "commit", "measured_on", "arms"):
            assert field in document, f"{path.name} does not declare `{field}`"
        assert re.fullmatch(r"[0-9a-f]{7,40}", document["commit"]), (
            f"{path.name} does not name a commit: {document['commit']!r}"
        )
        assert document["version"], f"{path.name} does not name a version"
        assert document["arms"], f"{path.name} publishes no arms"


def test_the_published_measurements_are_not_empty_of_arms() -> None:
    """Guarda de suelo: una medicion publicada sin brazos no mide nada."""
    if not MEASUREMENTS.is_dir():
        return
    for path in sorted(MEASUREMENTS.glob("*.json")):
        document = read_json(path)
        for arm, metrics in document["arms"].items():
            assert metrics.get("questions"), f"{path.name}: the arm `{arm}` measured no question"
            assert metrics.get("discovery_bytes") is not None, (
                f"{path.name}: the arm `{arm}` did not measure the discovery"
            )


# --- 6. La guarda de suelo ----------------------------------------------------


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
    """El control negativo: si el veredicto no puede fallar, no vigila.

    Se mide sobre una copia del presupuesto con una meta imposible, y el
    veredicto tiene que verla. Si este test pasa porque las metas estan
    todas dentro, no prueba nada.
    """
    harness = load_module()
    goals = harness.load_goals(SLO_BUDGET)
    measured = {
        "v2": {
            "tools_list_bytes": 4350,
            "status_p95_ms": 120.0,
            "find_p95_ms": 210.0,
            "inspect_p95_ms": 300.0,
            "context_p95_ms": 1500.0,
            "accuracy": 1.0,
        },
        "legacy": {"accuracy": 1.0},
    }

    assert harness.check_goals(measured, goals) == [], (
        "the control measurement is inside every goal; the negative control "
        "below would not prove anything"
    )

    impossible = [
        harness.Goal(name="find_p95_ms", metric="find_p95_ms", ceiling=1.0, note="")
    ]
    violations = harness.check_goals(measured, impossible)

    assert len(violations) == 1, f"the detector did not see the violation: {violations}"
    assert violations[0]["measured"] == 210.0
    assert violations[0]["ceiling"] == 1.0


def test_the_harness_configuration_declares_four_reference_repositories() -> None:
    """El encargo dice cuatro repos de referencia; la configuracion, cuatro.

    Y el cuarto es el propio repositorio: un repos pequeno de pruebas mide
    la latencia de una biblioteca de pruebas, que no es la latencia del
    producto.
    """
    config = read_toml(HARNESS_CONFIG)
    repos = config.get("repo", [])
    assert len(repos) == 4, f"the harness measures {len(repos)} repos, not four"
    languages = {repo["language"] for repo in repos}
    assert languages == {"rust", "kotlin", "groovy", "csharp"}, (
        f"the reference repositories are {languages}, and the four languages of "
        f"the encargo are rust, kotlin, groovy and csharp"
    )
    assert any(repo["path"] == "." for repo in repos), (
        "no reference repository is the product itself"
    )


def test_every_question_declares_the_intent_and_the_judge_knows_it() -> None:
    """Cada pregunta nombra una intencion, y cada intencion tiene juez.

    Una intencion sin juez mediria llamadas y tiempo, pero su exactitud
    saldria como un cero que parece una medida.
    """
    config = read_toml(HARNESS_CONFIG)
    intents = set(config.get("judge", {}).get("intent", {}))
    questions = config.get("question", [])
    assert questions, "the harness asks no question"
    for question in questions:
        assert question["intent"] in intents, (
            f"the question {question!r} uses an intent with no judge: {sorted(intents)}"
        )
        assert question["repo"], f"the question {question!r} names no repository"
    for arm in config.get("arm", []):
        for intent in intents:
            assert intent in arm.get("call", {}), (
                f"the arm `{arm['name']}` cannot answer the intent `{intent}`"
            )


def _call(version: str, question: str, repeat: int = 0) -> dict:
    meta = {"version": version, "repo": "r", "question": question, "intent": "find", "repeat": repeat}
    return load_module().make_call(meta, ["x"], 1, "1G")


def _fake(cut_on: str):
    def run_one(argv, *, timeout_s, memory_max):
        cut = "timeout" if cut_on in argv else ""
        return {"seconds": 1.0, "max_rss_kb": 10, "exit_code": 124 if cut else 0,
                "cut": cut, "payload": {} if cut else {"call_ms": 5, "correct": True, "state": "complete"}}
    return run_one


def test_a_cut_call_is_recorded_and_the_run_continues() -> None:
    """Una llamada que excede el plazo queda como `cortada` y la siguiente corre."""
    harness = load_module()
    slow = harness.make_call({"version": "legacy", "repo": "r", "question": "slow", "intent": "find", "repeat": 0}, ["slow"], 1, "1G")
    fine = harness.make_call({"version": "v2", "repo": "r", "question": "fine", "intent": "find", "repeat": 0}, ["fine"], 1, "1G")
    sunk: list = []
    rows = harness.run_plan([slow, fine], sunk.append, run_one=_fake("slow"))
    assert [r["cut"] for r in rows] == ["timeout", ""], rows
    assert rows[0]["state"] == "timeout" and rows[1]["state"] == "complete"
    assert len(sunk) == 2, "every call must leave its row as soon as it ends"


def test_repetitions_of_a_cut_question_are_skipped() -> None:
    """Repetir lo que ya agotó el plazo sólo gasta el plazo otra vez."""
    harness = load_module()
    meta = lambda n: {"version": "legacy", "repo": "r", "question": "slow", "intent": "find", "repeat": n}
    plan = [harness.make_call(meta(n), ["slow"], 1, "1G") for n in range(3)]
    rows = harness.run_plan(plan, lambda row: None, run_one=_fake("slow"))
    assert len(rows) == 1, rows


def test_exit_codes_classify_as_timeout_or_memory() -> None:
    harness = load_module()
    assert harness.classify_cut(124) == "timeout"
    assert harness.classify_cut(137) == "memory"
    assert harness.classify_cut(0) == "" and harness.classify_cut(1) == ""


def test_time_report_is_parsed() -> None:
    harness = load_module()
    report = (
        "\tCommand terminated by signal 9\n"
        "\tElapsed (wall clock) time (h:mm:ss or m:ss): 1:02.50\n"
        "\tMaximum resident set size (kbytes): 2048\n"
    )
    parsed = harness.parse_time_report(report)
    assert parsed == {"seconds": 62.5, "max_rss_kb": 2048, "exit_code": 137}, parsed


def test_csv_sink_appends_one_row_per_call_with_one_header() -> None:
    import tempfile
    harness = load_module()
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "nested" / "calls.csv"
        sink = harness.CsvSink(path)
        rows = harness.run_plan([_call("v2", "a"), _call("v2", "b")], sink, run_one=_fake("none"))
        read = harness.read_rows(path)
        assert [r["question"] for r in read] == ["a", "b"] == [r["question"] for r in rows]
        assert path.read_text().count("version,repo") == 1
        assert set(harness.FIELDS) <= set(read[0])


def test_a_real_process_over_its_limit_is_cut_and_the_next_one_runs() -> None:
    """Con procesos de verdad: `sleep` pasa del plazo y `true` sigue."""
    harness = load_module()
    slow = harness.isolated_run(["sleep", "30"], timeout_s=1, memory_max="256M")
    assert slow["cut"] == "timeout", slow
    fine = harness.isolated_run(["true"], timeout_s=10, memory_max="256M")
    assert fine["cut"] == "" and fine["exit_code"] == 0, fine


def test_the_plan_limits_the_large_repo_to_its_own_repeats() -> None:
    harness = load_module()
    config = read_toml(HARNESS_CONFIG)
    plan = harness.build_plan(config, Path("/bin/true"), {"large"}, {"v2"})
    per_question = {}
    for call in plan:
        key = call["meta"]["question"]
        per_question[key] = per_question.get(key, 0) + 1
    assert per_question and set(per_question.values()) == {config["run"]["large_repeats"]}, per_question
    assert all(call["memory_max"] == config["run"]["memory_max"] for call in plan)


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
        except Exception as exc:  # noqa: BLE001 - una comprobacion que revienta es un hallazgo
            failures.append(f"{check_.__name__}: {type(exc).__name__}: {exc}")
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
