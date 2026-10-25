#!/usr/bin/env python3
"""El servicio de analisis se parte por responsabilidad, y cada parte tiene su nombre.

## El hueco

`crates/cognicode-core/src/application/services/analysis_service.rs` es un
modulo de 4.354 lineas. Dentro vive el servicio entero: la construccion del
grafo, la cache, la resolucion de simbolos, las metricas, el analisis del
dominio y, ademas, 2.268 lineas de pruebas en linea. Son cinco
responsabilidades que no se hablan entre si, y el fichero es el unico sitio
donde se puede tocar cualquiera de ellas.

Un modulo que tiene todo no se revisa: un cambio en la cache obliga a leer el
constructor del grafo, y un cambio en las metricas obliga a leer el lector de
ficheros. El coste no es el tamano —es el tiempo de lectura que cada cambio
gasta en codigo que no tiene nada que ver con el.

## Lo que se anade

Una regla derivada del criterio de cierre, no una segunda lista de ficheros:
**cada responsabilidad vive en su fichero, y un fichero no vive dos
responsabilidades**.

La regla no dice que fichero es cada cosa —eso lo decide el codigo—: dice
que las cuatro responsabilidades no pueden seguir compartiendo el mismo
fichero, y que el fichero que define el servicio no puede seguir siendo el
que las implementa.

## Lo que NO se comprueba, y por que

- **Que el nombre del fichero sea el correcto.** El contrato mide que las
  responsabilidades estan separadas; que el fichero se llame como su
  responsabilidad es una convencion que la revision lee, no una regla que
  este fichero pueda aplicar sin inventar una lista de nombres que habria
  que mantener aparte.
- **Que el codigo que se movio siga haciendo lo mismo.** Eso lo mide la
  suite del crate, no un contrato que lee texto. Un contrato de estructura no
  puede afirmar que un refactor no rompio nada: solo puede afirmar que la
  forma cambio.
- **Que no sobre un modulo nuevo.** Una regla que solo prohibe la
  concentracion no impide que la concentracion vuelva por la puerta de
  atras. El limite es el otro contrato: el que cuenta las responsabilidades
  que comparten fichero, no el que cuenta los ficheros.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SERVICE_REL = Path("crates/cognicode-core/src/application/services/analysis_service.rs")
SERVICE_DIR_REL = Path("crates/cognicode-core/src/application/services/analysis_service")

# Las responsabilidades, nombradas por los metodos que las definen. Cada
# metodo aparece en un unico sitio: `build_project_graph` es el constructor
# completo, `build_project_graph_filtered` y `build_project_graph_async` son
# las otras dos vias. La coincidencia es por nombre exacto, no por prefijo —
# `find_symbol` no es `find_symbol_usages`.
RESPONSIBILITIES: dict[str, tuple[str, ...]] = {
    "construccion del grafo": (
        "build_project_graph",
        "build_graph_per_file_with_report",
        "build_project_graph_filtered",
        "build_project_graph_async",
    ),
    "cache e indice de simbolos": (
        "invalidate_file_cache_for",
        "set_symbol_index",
        "ensure_symbol_index",
    ),
    "resolucion de simbolos": (
        "find_symbol",
        "find_symbol_usages",
        "get_file_symbols",
    ),
    "metricas del grafo": (
        "get_graph_stats",
        "get_coverage_metrics",
        "get_last_build_report",
    ),
}

# El tipo que define el servicio. El fichero que lo declara es la fachada:
# puede declarar el estado y la construccion, pero no puede ser el fichero
# que implementa las cuatro responsabilidades.
FACADE_TYPE = "AnalysisService"

FN_DECL = re.compile(r"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+(\w+)\s*[(<]")
MOD_DECL = re.compile(r"^\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+)\s*;")
MOD_OPEN_RE = re.compile(r"^\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+)\s*\{")
TEST_MOD_DECL = re.compile(r"^\s*#\[cfg\(test\)\]\s*$")
# La misma marca, pero buscada sobre el fichero entero: sin `MULTILINE`, los
# anclores solo casan con el principio y el final del texto, y una comprobacion
# que no puede casar con nada pasa en verde sin comprobar nada.
TEST_MOD_ANYWHERE = re.compile(r"^[ \t]*#\[cfg\(test\)\][ \t]*$", re.MULTILINE)


def _brace_delta(line: str) -> int:
    """Delta de llaves de una linea, ignorando literales de cadena y caracter.

    Una llave dentro de una cadena no abre ni cierra un ambito, asi que
    contarla haria que el modulo de pruebas pareciese no cerrarse nunca.
    """
    delta = 0
    in_str = False
    in_char = False
    escaped = False
    for ch in line:
        if escaped:
            escaped = False
            continue
        if in_str:
            if ch == "\\":
                escaped = True
            elif ch == '"':
                in_str = False
            continue
        if in_char:
            if ch == "\\":
                escaped = True
            elif ch == "'":
                in_char = False
            continue
        if ch == '"':
            in_str = True
        elif ch == "'":
            in_char = True
        elif ch == "{":
            delta += 1
        elif ch == "}":
            delta -= 1
    return delta


def _inline_test_modules(text: str) -> list[int]:
    """Lineas (1-based) de los items que abre un `#[cfg(test)]` en linea.

    Un `#[cfg(test)] mod X;` no abre un ambito: declara un modulo que vive
    en otro fichero. Un `#[cfg(test)] mod X { ... }` si: su cuerpo es codigo
    de prueba escrito en linea. Solo el segundo caso duplica el volumen del
    modulo que describe.
    """
    lines = text.splitlines()
    spans: list[int] = []
    index = 0
    while index < len(lines):
        if not TEST_MOD_DECL.match(lines[index]):
            index += 1
            continue
        cursor = index + 1
        while cursor < len(lines) and not lines[cursor].strip():
            cursor += 1
        if cursor >= len(lines) or not MOD_OPEN_RE.match(lines[cursor]):
            index = cursor + 1
            continue
        depth = 0
        opened = False
        while cursor < len(lines):
            depth += _brace_delta(lines[cursor])
            if depth > 0:
                opened = True
            spans.append(cursor + 1)
            cursor += 1
            if opened and depth <= 0:
                break
        index = cursor
    return spans


def _production_lines(text: str) -> list[str]:
    """Las lineas del fichero que no estan dentro de un modulo `#[cfg(test)]`.

    Un modulo de pruebas en linea no es codigo de produccion: sus funciones
    no son la implementacion de ninguna responsabilidad, y contarlas haria
    que un fichero de pruebas pareciese dueño de metodos que no posee.
    """
    drop = set(_inline_test_modules(text))
    return [
        line
        for number, line in enumerate(text.splitlines(), start=1)
        if number not in drop
    ]


def _defined_functions(text: str) -> set[str]:
    """Nombres de funcion definidos fuera de cualquier modulo de pruebas."""
    names: set[str] = set()
    for line in _production_lines(text):
        match = FN_DECL.match(line)
        if match:
            names.add(match.group(1))
    return names


def _test_module_names(text: str) -> set[str]:
    """Nombres de los modulos declarados bajo `#[cfg(test)]`."""
    names: set[str] = set()
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if not TEST_MOD_DECL.match(line):
            continue
        for follow in lines[index + 1 : index + 4]:
            match = MOD_DECL.match(follow)
            if match:
                names.add(match.group(1))
                break
    return names


def _service_files() -> list[Path]:
    root = REPO_ROOT / SERVICE_REL
    files = [root]
    directory = REPO_ROOT / SERVICE_DIR_REL
    if directory.is_dir():
        files.extend(sorted(directory.rglob("*.rs")))
    return files


def _module_path(name: str) -> Path:
    """El fichero que implementa un modulo `mod <name>;` del servicio."""
    directory = REPO_ROOT / SERVICE_DIR_REL
    return directory / f"{name}.rs"


def _production_files() -> list[Path]:
    """Los ficheros del servicio que no son un modulo de pruebas."""
    root = REPO_ROOT / SERVICE_REL
    text = root.read_text(encoding="utf-8")
    test_modules = _test_module_names(text)
    out: list[Path] = []
    for path in _service_files():
        if path == root:
            out.append(path)
            continue
        if path.stem in test_modules:
            continue
        out.append(path)
    return out


def _owners() -> dict[str, set[Path]]:
    """Que fichero de produccion define cada metodo de cada responsabilidad."""
    functions: dict[Path, set[str]] = {}
    for path in _production_files():
        functions[path] = _defined_functions(path.read_text(encoding="utf-8"))

    owners: dict[str, set[Path]] = {}
    for responsibility, markers in RESPONSIBILITIES.items():
        found: set[Path] = set()
        for path, names in functions.items():
            if any(marker in names for marker in markers):
                found.add(path)
        owners[responsibility] = found
    return owners


def _facade() -> Path | None:
    """El fichero que declara el tipo del servicio."""
    for path in _production_files():
        text = path.read_text(encoding="utf-8")
        if re.search(rf"^\s*pub\s+struct\s+{FACADE_TYPE}\b", text, re.MULTILINE):
            return path
    return None


def _relative(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


# ---------- assertions (each is one mutation seen to fail) ----------


def test_every_responsibility_still_exists() -> None:
    """Control anti-vacuidad: las cuatro responsabilidades tienen que existir.

    Sin este control, un contrato que mide "las responsabilidades estan
    separadas" pasa de verdad si el codigo desaparece: separarse de algo
    que no esta es la forma mas economica de no hacer nada.
    """
    owners = _owners()
    missing = [name for name, files in owners.items() if not files]
    assert not missing, (
        "FAIL: responsabilidades sin implementacion en el servicio:\n  - "
        + "\n  - ".join(missing)
        + "\nLa regla mide que las responsabilidades estan separadas; si no "
        "existen, la regla pasa sin comprobar nada."
    )


def test_no_file_owns_two_responsibilities() -> None:
    """M1: un fichero, una responsabilidad.

    Es la regla que deshace el mega-modulo. Si un fichero puede vivir dos
    responsabilidades, entonces el fichero sigue siendo el sitio donde todo
    se toca, y partirlo no ha cambiado nada.
    """
    owners = _owners()
    collisions: list[str] = []
    for path in _production_files():
        held = [name for name, files in owners.items() if path in files]
        if len(held) > 1:
            collisions.append(f"{_relative(path)} -> {', '.join(sorted(held))}")
    assert not collisions, (
        "FAIL: ficheros que siguen siendo dueño de mas de una "
        "responsabilidad:\n  - "
        + "\n  - ".join(collisions)
        + "\nLa construccion del grafo, la cache, la resolucion y las metricas "
        "deben vivir en ficheros distintos. Un fichero que las tiene todas "
        "sigue siendo el modulo que hay que partir."
    )


def test_the_facade_does_not_implement_the_responsibilities() -> None:
    """M2: la fachada no hace el trabajo.

    El fichero que declara el servicio puede declarar su estado y su
    construccion; lo que no puede es seguir siendo el fichero que construye
    el grafo, guarda la cache, resuelve simbolos y calcula metricas.
    """
    facade = _facade()
    assert facade is not None, (
        "FAIL: no se encontro el fichero que declara `pub struct "
        f"{FACADE_TYPE}`. El servicio cambio de sitio o cambio de forma."
    )
    owners = _owners()
    held = [name for name, files in owners.items() if facade in files]
    assert not held, (
        f"FAIL: la fachada ({_relative(facade)}) sigue implementando: "
        + ", ".join(sorted(held))
        + "\nDeclarar el servicio y construir el grafo son dos "
        "responsabilidades. La fachada declara; el trabajo vive en su sitio."
    )


def test_the_service_has_no_inline_test_bulk() -> None:
    """M3: las pruebas en linea no son parte del modulo de produccion.

    La mitad del fichero eran pruebas dentro del mismo modulo. Sacarlas a su
    propio fichero no cambia el comportamiento: cambia lo que hay que leer
    para cambiar el servicio.
    """
    offenders: list[str] = []
    for path in _production_files():
        text = path.read_text(encoding="utf-8")
        if _inline_test_modules(text):
            offenders.append(_relative(path))
    assert not offenders, (
        "FAIL: ficheros de produccion con modulo de pruebas en linea:\n  - "
        + "\n  - ".join(offenders)
        + "\nLas pruebas en linea duplican el volumen del modulo que "
        "describe. Su sitio es su propio fichero."
    )


def test_control_all_green() -> None:
    """Control: en estado nominal, las anteriores pasan."""
    test_every_responsibility_still_exists()
    test_no_file_owns_two_responsibilities()
    test_the_facade_does_not_implement_the_responsibilities()
    test_the_service_has_no_inline_test_bulk()


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_every_responsibility_still_exists,
        test_no_file_owns_two_responsibilities,
        test_the_facade_does_not_implement_the_responsibilities,
        test_the_service_has_no_inline_test_bulk,
        test_control_all_green,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("Service split by responsibility: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    owners = _owners()
    parts = sorted({_relative(p) for files in owners.values() for p in files})
    print(
        "Service split by responsibility: OK "
        f"({len(RESPONSIBILITIES)} responsabilidades en {len(parts)} ficheros)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
