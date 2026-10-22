#!/usr/bin/env python3
"""Una configuración ejecutable no puede apuntar a un documento que no existe.

## El hueco

La poda documental se llevó `docs/inventory/`. Lo que dejó detrás son
configuraciones ejecutables que apuntan allí:

  - `apps/explorer-ui/playwright.config.ts` declara
    `coverageMatrix: "docs/inventory/e17-coverage-matrix.md"`;
  - `apps/explorer-ui/scripts/coverage-matrix.ts` dice producir ese mismo
    fichero, y lo escribe con `writeFileSync` en un directorio que ya no
    existe;
  - dos specs de `apps/explorer-ui/e2e/` citan
    `docs/inventory/e17-deferred-bugs.md`.

El daño no es que el documento falte. El daño es que una configuración que
apunta a un documento ausente describe mal lo que ejecuta: el generador
levanta, recorre el árbol, y falla al escribir en un directorio que no está.
El fallo aparece tarde y habla de permisos o de rutas, no del documento que
faltaba.

## Lo que se comprueba

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

## Lo que NO se comprueba

  - **Que el documento citado sea el mejor.** Solo que exista. Si la cita
    apunta a un documento equivocado, eso es otra unidad.
  - **El contenido de las citas en prosa.** Solo se miran las rutas, que es
    lo que se puede comprobar sin depender de como esté redactada la frase.
  - **Las citas de los ficheros que no son configuración ejecutable.** Un
    documento puede citar a otro documento que se haya movido; eso lo
    vigila otro contrato, con otras reglas.
"""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

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

# Los hallazgos se acumulan aquí y `main()` los convierte en un código de
# salida. Un contrato que informa de un fallo y sale con cero informa de un
# verde, que es peor que no tener contrato.
failures: list[str] = []

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
        try:
            function()
            print(f"PASS: {name}")
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
            print(f"FAIL: {name}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001 — el contrato informa, no aborta
            failures.append(f"ERROR: {name}: {error}")
            print(f"ERROR: {name}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} contract(s) failed", file=sys.stderr)
        return 1
    print("\nall contracts passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
