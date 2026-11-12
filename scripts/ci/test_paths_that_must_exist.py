#!/usr/bin/env python3
"""Contrato: lo que el árbol vigila o cita tiene que existir.

Dos contratos con el mismo defecto de fondo —una ruta que no está— sobre
sujetos distintos, reunidos en un solo fichero. No comparten un sujeto único:
cada sección vigila el suyo.

  I.  El `build.rs` de `cognicode-core`: no vigila rutas de git que un
      worktree no tiene, y toda entrada que hashea como fuente del analizador
      existe.
  II. Las configuraciones ejecutables (`playwright.config.ts`, `scripts/*.sh`,
      `*.pipeline.kts`...): no citan documentos de `docs/` que no existen.

## I. Un build.rs no vigila rutas de git que un worktree no tiene

`cargo:rerun-if-changed=<ruta>` sobre una ruta que NO existe hace que Cargo
vuelva a ejecutar el script en cada invocacion, y con el script, que
recompile el crate. En un worktree `.git` es un fichero, no un directorio, asi
que `../../.git/HEAD` no existe: dos `cargo build` identicos seguidos
recompilaban `cognicode-core` los dos (58 s y 52 s medidos). El gate corre
varias veces `cognicode-core` con features distintas, y cada una pagaba esa
recompilacion de mas.

La ruta correcta la dice git (`git rev-parse --git-path HEAD` y
`--git-common-dir`), y solo se vigila si existe.

El script vigila ademas las entradas que hashea como fuente del analizador
(`INPUTS` en `crates/cognicode-core/analyzer_source.rs`). Una entrada que no
existe no se vigila —el script la registra como ausente—, asi que una errata
en la lista no recompilaria nada de mas: dejaria esa fuente fuera de la
identidad sin decirlo. Por eso cada entrada tiene que existir en el checkout.

## II. Una configuración ejecutable no puede apuntar a un documento que no existe

La poda documental se llevó `docs/inventory/`. Lo que dejó detrás eran
configuraciones ejecutables que apuntaban allí:

  - `apps/explorer-ui/playwright.config.ts` declaraba
    `coverageMatrix: "docs/inventory/e17-coverage-matrix.md"`;
  - `apps/explorer-ui/scripts/coverage-matrix.ts` decía producir ese mismo
    fichero, y lo escribía con `writeFileSync` en un directorio que ya no
    existe;
  - dos specs de `apps/explorer-ui/e2e/` citaban
    `docs/inventory/e17-deferred-bugs.md`.

Las tres se retiraron el 2026-11-04, con la suite de navegador y su
configuración. Los globs de `CONFIG_GLOBS` siguen nombrándolas: un glob que
no casa ningún fichero no falla, y un fichero que vuelve tiene que volver
sometido a la misma regla.

El daño que este contrato evita no es que el documento falte. El daño es que
una configuración que apunta a un documento ausente describe mal lo que
ejecuta: el generador levanta, recorre el árbol, y falla al escribir en un
directorio que no está. El fallo aparece tarde y habla de permisos o de
rutas, no del documento que faltaba.

Para cada fichero de configuración ejecutable, cada ruta de `docs/` que cita
debe existir en el árbol.

  - una ruta citada que no existe                            -> 1 rojo
  - (control) un fichero con una cita inexistente sintética -> 1 rojo
  - (control) todo verde sobre el árbol                      -> 0 rojos

Los dos controles importan. Sin el primero, una regla que no puede fallar
sería indistinguible de una regla que nunca falla: bastaría con que el
extractor de citas dejara de encontrar citas para que el contrato pasara
siempre. Con él, se demuestra que el extractor ve citas y que sabe decir que
una no resuelve.

Lo que NO se comprueba:

  - **Que el documento citado sea el mejor.** Solo que exista. Si la cita
    apunta a un documento equivocado, eso es otra unidad.
  - **El contenido de las citas en prosa.** Solo se miran las rutas, que es
    lo que se puede comprobar sin depender de como esté redactada la frase.
  - **Las citas de los ficheros que no son configuración ejecutable.** Un
    documento puede citar a otro documento que se haya movido; eso lo
    vigila otro contrato, con otras reglas.

## III. `openspec_conformance.py --validate-paths` termina y nombra lo que falta

Sobre un árbol sano sin directorio de especificaciones termina con código 0;
con una ruta de evidencia ausente falla con código 1 y la nombra.

Run:
    python3 scripts/ci/test_paths_that_must_exist.py
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Los hallazgos se acumulan aquí y `main()` los convierte en un código de
# salida. Un contrato que informa de un fallo y sale con cero informa de un
# verde, que es peor que no tener contrato.
failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


# ---------------------------------------------------------------------------
# I. Un build.rs no vigila rutas de git que un worktree no tiene
# ---------------------------------------------------------------------------

# Descubiertos, no listados: un build.rs nuevo entra en el contrato sin que
# nadie se acuerde de añadirlo.
BUILD_SCRIPTS = tuple(sorted(str(p.relative_to(REPO_ROOT)) for p in REPO_ROOT.glob("crates/*/build.rs")))
CORE_PACKAGE = "crates/cognicode-core"
ANALYZER_SOURCE = f"{CORE_PACKAGE}/analyzer_source.rs"


def watched_literals(text: str) -> list[str]:
    live = "\n".join(l for l in text.splitlines() if not l.strip().startswith("//"))
    return re.findall(r'cargo:rerun-if-changed=([^"{}]+)"', live)


def test_the_build_scripts_are_discovered() -> None:
    check(f"{CORE_PACKAGE}/build.rs" in BUILD_SCRIPTS, f"el descubrimiento no encuentra {CORE_PACKAGE}/build.rs: {BUILD_SCRIPTS}")


def test_no_script_hardcodes_a_dot_git_directory() -> None:
    for rel in BUILD_SCRIPTS:
        text = (REPO_ROOT / rel).read_text(encoding="utf-8")
        bad = [p for p in watched_literals(text) if ".git" in p]
        check(not bad, f"{rel} vigila {bad}: en un worktree esa ruta no existe y Cargo recompila siempre")


def missing_literals(text: str, package_dir: Path) -> list[str]:
    return [p for p in watched_literals(text) if not (package_dir / p).exists()]


def test_no_script_watches_a_literal_path_that_does_not_exist() -> None:
    for rel in BUILD_SCRIPTS:
        script = REPO_ROOT / rel
        missing = missing_literals(script.read_text(encoding="utf-8"), script.parent)
        check(not missing, f"{rel} vigila {missing}, que no existen: Cargo vuelve a ejecutar el script en cada build")


def test_the_detector_sees_a_planted_missing_literal() -> None:
    planted = 'println!("cargo:rerun-if-changed=no-such-file.txt");'
    check(missing_literals(planted, REPO_ROOT / CORE_PACKAGE) == ["no-such-file.txt"], "el detector no ve un literal ausente plantado")


def test_the_detector_sees_a_planted_hardcoded_path() -> None:
    planted = 'println!("cargo:rerun-if-changed=../../.git/HEAD");'
    check(watched_literals(planted) == ["../../.git/HEAD"], "el detector no ve una ruta fija plantada")


def analyzer_inputs(text: str) -> list[str]:
    block = re.search(r"pub const INPUTS: &\[&str\] = &\[(.*?)\];", text, re.S)
    return re.findall(r'"([^"]+)"', block.group(1)) if block else []


def absent_inputs(inputs: list[str], package_dir: Path) -> list[str]:
    return [entry for entry in inputs if not (package_dir / entry).exists()]


def test_every_analyzer_input_exists() -> None:
    inputs = analyzer_inputs((REPO_ROOT / ANALYZER_SOURCE).read_text(encoding="utf-8"))
    check(bool(inputs), f"{ANALYZER_SOURCE}: no se encuentra la lista `INPUTS`")
    missing = absent_inputs(inputs, REPO_ROOT / CORE_PACKAGE)
    check(not missing, f"{ANALYZER_SOURCE} hashea {missing}, que no existen: esas fuentes quedan fuera de la identidad del analizador")


def test_the_detector_sees_a_planted_absent_input() -> None:
    planted = 'pub const INPUTS: &[&str] = &[\n    "src",\n    "../no-such-crate/src",\n];'
    inputs = analyzer_inputs(planted)
    check(inputs == ["src", "../no-such-crate/src"], f"el extractor no lee la lista plantada: {inputs}")
    check(absent_inputs(inputs, REPO_ROOT / CORE_PACKAGE) == ["../no-such-crate/src"], "el detector no ve una entrada ausente plantada")


# ---------------------------------------------------------------------------
# II. Una configuración ejecutable no cita un documento que no existe
# ---------------------------------------------------------------------------

# Configuraciones ejecutables: ficheros que el producto lee al arrancar o
# al ejecutarse. Un documento puede citar lo que quiera; una configuración
# no, porque lo que dice es lo que hace.
CONFIG_GLOBS = (
    "apps/explorer-ui/playwright.config.ts",
    "apps/explorer-ui/vite.config.ts",
    "apps/explorer-ui/vitest.config.ts",
    "apps/explorer-ui/scripts/*.ts",
    "apps/explorer-ui/scripts/*.mjs",
    "apps/explorer-ui/e2e/*.spec.ts",
    "scripts/*.sh",
    "scripts/ci/*.sh",
    "*.pipeline.kts",
)

# Una cita de ruta dentro de acentos graves, de un enlace markdown o de una
# cadena entre comillas. Las tres formas aparecen en configuración
# ejecutable, y las tres significan lo mismo: «este fichero existe en este
# sitio». La forma entre comillas es la que declara una ruta que el programa
# va a leer o a escribir.
CITED = re.compile(
    r"`(docs/[A-Za-z0-9._/-]+)`"
    r"|\]\((docs/[A-Za-z0-9._/-]+)\)"
    r"|[\"'](docs/[A-Za-z0-9._/-]+)[\"']"
)

# Una cita a un documento por su nombre de fichero, sin ruta. Es la forma
# que aparece cuando el documento se cita sin decir dónde está.
CITED_BY_NAME = re.compile(r"\b([A-Za-z0-9][A-Za-z0-9._-]*\.md)\b")

# El árbol donde se busca un documento citado por su nombre.
DOCUMENT_ROOTS = ("docs",)

# Los nombres que el programa PRODUCE. Un informe que el programa escribe
# no existe hasta que se escribe, así que exigirlo lo haría fallar
# siempre. Se declaran aquí porque es una decisión del contrato, no algo
# que se pueda deducir de un fichero que todavía no está.
PRODUCED_NAMES = ("report.md", "coverage-matrix.md")


def config_files() -> list[Path]:
    seen: dict[Path, None] = {}
    for pattern in CONFIG_GLOBS:
        for path in sorted(REPO_ROOT.glob(pattern)):
            if path.is_file():
                seen[path] = None
    return list(seen)


def cited_paths(text: str) -> set[str]:
    """Las rutas con prefijo `docs/` que cita el texto."""
    out: set[str] = set()
    for match in CITED.finditer(text):
        out.add(next(group for group in match.groups() if group))
    return out


def cited_names(text: str) -> set[str]:
    """Los nombres de documento que cita el texto, sin ruta."""
    return set(CITED_BY_NAME.findall(text))


def unresolved(
    paths: set[str], names: set[str], root: Path = REPO_ROOT
) -> list[str]:
    """Las citas que no resuelven a ningún fichero.

    Una ruta no resuelve si no existe bajo `root`. Un nombre no resuelve si
    no hay ningún fichero con ese nombre en el árbol: se busca primero en
    la documentación, que es donde se citan los documentos, y después en
    el resto del repositorio, porque un documento puede vivir al lado de
    lo que lo cita.
    """
    missing = [p for p in paths if not (root / p).exists()]
    if names:
        # Un nombre cited resuelve si hay un fichero con ese nombre en
        # cualquier parte del árbol: un documento puede vivir en `docs/`,
        # junto a lo que lo cita, o en la raíz. Se busca en todo el árbol
        # porque una cita sin ruta no dice dónde mirar.
        present = {path.name for path in root.rglob("*.md")}
        present |= set(PRODUCED_NAMES)
        missing += [n for n in sorted(names) if n not in present]
    return sorted(missing)


def test_no_executable_config_cites_an_absent_document() -> None:
    """Ninguna configuración ejecutable cita una ruta de `docs/` que falte."""
    problems: list[str] = []
    for path in config_files():
        text = path.read_text(encoding="utf-8")
        missing = unresolved(cited_paths(text), cited_names(text))
        for route in missing:
            problems.append(f"{path.relative_to(REPO_ROOT)}: {route}")

    assert not problems, (
        "FAIL: estas configuraciones ejecutables apuntan a documentos que no "
        "existen en el arbol:\n  "
        + "\n  ".join(problems)
        + "\nUna configuracion que apunta a un documento ausente describe mal "
        "lo que ejecuta: el generador que la lee falla al escribir, y el "
        "fallo habla de rutas o de permisos en vez de del documento que "
        "faltaba."
    )


def test_the_extractor_sees_citations_at_all() -> None:
    """Control: el extractor tiene que encontrar citas, o no vigila nada.

    Una regla que no puede encontrar una cita rota pasa siempre. Este
    control mide que el extractor lee de verdad, y sobre el árbol real: si
    nadie citara documentos, el contrato no podría distinguir "todo
    correcto" de "no miré nada".

    Cuenta citas de cualquier forma —con ruta o por nombre— porque las
    dos son citas, y porque un árbol donde solo quedaran las formas con
    ruta sería un árbol donde la regla ha dejado de mirar.
    """
    found = 0
    for path in config_files():
        text = path.read_text(encoding="utf-8")
        found += len(cited_paths(text)) + len(cited_names(text))
    assert found > 0, (
        "FAIL: el extractor no ha encontrado ninguna cita en las "
        "configuraciones ejecutables. Si el extractor dejo de ver citas, "
        "este contrato pasaria siempre y no vigilaria nada."
    )


def test_a_synthetic_broken_citation_is_caught() -> None:
    """Control negativo: una cita rota inventada tiene que salir en rojo.

    Con un fichero de configuración real, con una cita a un documento que no
    existe. Si la comprobacion no lo detecta, no esta vigilando: estara
    leyendo el arbol y no lo que dice.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "docs").mkdir()
        config = root / "fake.config.ts"
        config.write_text(
            'const coverageMatrix = "docs/inventory/este-fichero-no-existe.md";\n',
            encoding="utf-8",
        )
        missing = unresolved(
            cited_paths(config.read_text(encoding="utf-8")),
            cited_names(config.read_text(encoding="utf-8")),
            root=root,
        )
        assert missing, (
            "FAIL: una cita a un documento inexistente no fue detectada. "
            "La comprobacion no esta mirando lo que dice mirar."
        )
        assert missing[0] == "docs/inventory/este-fichero-no-existe.md", (
            f"FAIL: la cita detectada no es la que se escribio: {missing}."
        )


def test_the_existing_tree_has_no_absent_citation() -> None:
    """Control de solo lectura: el árbol real no tiene citas rotas.

    Es el mismo hecho que la primera comprobación, escrito desde el otro
    lado: que la comprobación pase tiene que ser porque el árbol está bien,
    y no porque la comprobación no mire.
    """
    total = 0
    missing_total = 0
    for path in config_files():
        text = path.read_text(encoding="utf-8")
        cited = cited_paths(text) | cited_names(text)
        total += len(cited)
        missing_total += len(unresolved(cited_paths(text), cited_names(text)))
    assert total > 0 and missing_total == 0, (
        f"FAIL: {missing_total} citas rotas sobre {total} citas inspeccionadas."
    )


# ---------------------------------------------------------------------------
# III. `openspec_conformance.py --validate-paths` termina y nombra lo que falta
# ---------------------------------------------------------------------------
#
# Con un árbol sano y sin directorio de especificaciones el comando termina
# con código 0 (sin `KeyError`); con una ruta de evidencia ausente falla con
# código 1 y nombra tanto la clave como la ruta.

OPENSPEC_SCRIPT = REPO_ROOT / "sandbox" / "scripts" / "openspec_conformance.py"


def run_validate_paths(cwd: Path, evidence: str) -> subprocess.CompletedProcess:
    (cwd / "map.json").write_text(evidence)
    return subprocess.run(
        [sys.executable, "-I", str(OPENSPEC_SCRIPT), "--validate-paths",
         "--evidence-map", "map.json", "--specs-dir", "none",
         "--output-prefix", "out/m"],
        cwd=cwd, capture_output=True, text=True, timeout=30,
    )


class ValidatePaths(unittest.TestCase):
    def test_healthy_tree_without_specs_dir_ends_with_rc_0(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "ev.txt").write_text("x")
            done = run_validate_paths(
                root, '{"a": {"status": "verified", "evidence_path": "ev.txt"}}')
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertNotIn("KeyError", done.stderr)

    def test_missing_path_is_named_and_fails(self):
        with tempfile.TemporaryDirectory() as d:
            done = run_validate_paths(
                Path(d), '{"a": {"status": "verified", "evidence_path": "gone.txt"}}')
            self.assertEqual(done.returncode, 1)
            self.assertIn("a", done.stderr)
            self.assertIn("gone.txt", done.stderr)


def main() -> int:
    """Ejecuta las comprobaciones y devuelve 1 si alguna falla.

    El contrato de los contratos exige esta forma: un fichero que acumula
    sus hallazgos en una lista tiene que convertir esa lista en un código
    de salida distinto de cero, o ejecutarlo a mano informaría de un
    éxito que no ocurrió.
    """
    for name, function in sorted(globals().items()):
        if not name.startswith("test_") or not callable(function):
            continue
        before = len(failures)
        try:
            function()
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
        except Exception as error:  # noqa: BLE001 — el contrato informa, no aborta
            failures.append(f"ERROR: {name}: {error}")
        if len(failures) > before:
            print(f"FAIL: {name}", file=sys.stderr)
            for finding in failures[before:]:
                print(f"  - {finding}", file=sys.stderr)
        else:
            print(f"PASS: {name}")

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ValidatePaths)
    result = unittest.TextTestRunner(stream=sys.stderr, verbosity=0).run(suite)
    for case, trace in result.failures + result.errors:
        failures.append(f"FAIL: {case.id()}: {trace.strip().splitlines()[-1]}")

    if failures:
        print(f"\n{len(failures)} problem(s) found", file=sys.stderr)
        return 1
    print("\nall contracts passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
