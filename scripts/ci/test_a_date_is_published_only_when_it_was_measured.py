#!/usr/bin/env python3
"""Una fecha se publica solo si se midio.

MEDIDO: nueve sitios de produccion leian la marca de tiempo de un fichero y la
convertian a "segundos desde la epoca" por su cuenta. Tres de ellos la
publicaban como `0` cuando la fecha no se podia leer, y `0` dice "1 de enero
de 1970": una fecha que nadie midio. Uno --el grafo por fichero-- devolvia la
distancia a la epoca en vez del instante, de modo que un fichero de 1960 se
publicaba como 1980: una fecha plausible y equivocada. Y el manejador de
reparseo usaba `mtime == 0` como centinela de "borrado", con lo que un
fichero anterior a la epoca se reportaba como eliminado del grafo.

La ultima excepcion --la familia de la ingestion-- quedo cerrada: su fecha la
mide el dueno unico, un fichero cuya fecha no se puede leer declara su hueco
en vez de desaparecer del escaneo, y la columna que la guarda admite la
ausencia. Por eso el registro de huecos esta vacio, y por eso un registro
vacio es una declaracion y no un olvido.

La cache nunca se rompio: la invalidacion compara tambien el hash del
contenido. El defecto era de verdad, no de estabilidad.

Este contrato mira tres cosas y ninguna ejecuta el servicio:

1. El tipo publicado dice la diferencia entre una fecha medida y una fecha
   que no existe. El consumidor tiene que poder verlo.
2. El dueno unico existe y es el unico sitio que convierte un instante.
3. Ningun sitio de produccion convierte por su cuenta la fecha de un
   fichero. Los huecos que quedan estan registrados con su motivo, y el
   registro tiene que seguir vivo: si un hueco se cierra, el registro avisa.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

Ejecutar:  python3 scripts/ci/test_a_date_is_published_only_when_it_was_measured.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

OWNER = "crates/cognicode-core/src/domain/value_objects/file_time.rs"
DTO = "crates/cognicode-core/src/application/dto/common.rs"
CRATE_SRC = "crates"

OWNER_API = (
    "epoch_seconds",
    "epoch_millis",
    "epoch_nanos",
    "measured_seconds",
    "measured_millis",
    "measured_nanos",
    "seconds_of_millis",
)

# Huecos conocidos: sitios que todavia miden por su cuenta, con el motivo por
# el que no se han arreglado. Si uno se cierra, el registro avisa.
#
# El registro esta vacio, y vacio es una afirmacion: no queda ningun sitio de
# produccion que convierta la fecha de un fichero por su cuenta. La ultima
# excepcion --la familia de la ingestion-- quedo cerrada, asi que la razon
# se declara en vez de dejar el registro mudo: un registro vacio sin
# declaracion es un olvido, y el contrato no lo puede dar por bueno.
KNOWN_GAPS: dict[str, str] = {}

# La razon por la que no hay ningun hueco conocido. Solo tiene sentido
# cuando el registro esta vacio: las dos afirmaciones --"no hay ninguno" y
# "hay estos"-- no pueden ser verdad a la vez, y el contrato lo vigila.
NO_KNOWN_GAPS = (
    "La ingestion ya no mide por su cuenta: la fecha la mide el dueno unico "
    "(file_time::measured_millis), un fichero cuya fecha no se puede leer "
    "declara su hueco en vez de desaparecer del escaneo, y la columna que la "
    "guarda admite la ausencia."
)

# Las conversiones que delatan una fabricacion: un instante que se convierte
# a unidades de epoca y, si la conversion falla, se sustituye por un numero.
CONVERSION_MARKERS = (
    "duration_since",
    "UNIX_EPOCH",
    "as_nanos() as u64",
    "as_millis() as u64",
    "as_secs()",
)

# Cuantas lineas se miran despues de leer la fecha para decidir si el sitio
# convierte el instante por su cuenta.
WINDOW = 8


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


def production_files() -> list[Path]:
    """Los ficheros de produccion del workspace: sin tests, benches ni ejemplos.

    Un modulo `#[cfg(test)]` no es produccion: el codigo que se despliega no
    esta ahi, y un contrato que vigila el codigo de pruebas vigila otra cosa.
    """
    found: list[Path] = []
    for path in sorted((REPO_ROOT / CRATE_SRC).rglob("*.rs")):
        relative = path.relative_to(REPO_ROOT)
        parts = relative.parts
        if "tests" in parts or "benches" in parts or "examples" in parts:
            continue
        found.append(path)
    return found


def ranges_of_test_modules(lines: list[str]) -> list[tuple[int, int]]:
    """Los rangos de linea que ocupan los modulos de pruebas.

    Un `#[cfg(test)]` puede aparecer indentado --un import local-- y no
    delimita nada: solo cuenta el que abre un modulo o una funcion, y el
    cuerpo termina donde se cierran sus llaves. Contar las llaves y no
    buscar la primera aparicion es lo que hace que el recorte no se coma
    codigo de produccion que va despues de un import de pruebas.
    """
    ranges: list[tuple[int, int]] = []
    for index, line in enumerate(lines):
        if not line.startswith("#[cfg(test)]"):
            continue
        opener = ""
        for cursor in range(index + 1, len(lines)):
            stripped = lines[cursor].strip()
            if not stripped or stripped.startswith("//") or stripped.startswith("#["):
                continue
            opener = stripped
            break
        if not (opener.startswith("mod ") or opener.startswith("fn ")
                or opener.startswith("pub fn ") or opener.startswith("pub(crate) fn ")):
            continue
        depth = 0
        started = False
        end = len(lines) - 1
        for cursor in range(index, len(lines)):
            for char in lines[cursor]:
                if char == "{":
                    depth += 1
                    started = True
                elif char == "}":
                    depth -= 1
            if started and depth <= 0:
                end = cursor
                break
        ranges.append((index, end))
    return ranges


def production_text(path: Path) -> str:
    """El texto del fichero sin los modulos de pruebas."""
    return production_lines(path.read_text(encoding="utf-8"))


def production_lines(text: str) -> str:
    """El texto sin los modulos de pruebas."""
    lines = text.splitlines()
    excluded: set[int] = set()
    for start, end in ranges_of_test_modules(lines):
        excluded.update(range(start, end + 1))
    return "\n".join(line for number, line in enumerate(lines) if number not in excluded)


# ---------------------------------------------------------------------------
# Detectores
# ---------------------------------------------------------------------------


def the_published_type_says_the_difference(text: str) -> list[str]:
    """La fecha publicada distingue "medida" de "no existe".

    Con un `u64` no hay manera de decir la diferencia: `0` es a la vez "escrito
    en la epoca" y "no se pudo leer". El consumidor tiene que poder ver cual de
    los dos es, asi que el campo publicado tiene que poder estar vacio.
    """
    findings: list[str] = []
    declarations = re.findall(r"(?m)^\s*pub modified: ([^,]+),", text)
    if not declarations:
        findings.append(f"{DTO}: no se encuentra ningun campo `modified` publicado")
        return findings
    for declaration in declarations:
        if declaration.strip() != "Option<i64>":
            findings.append(
                f"{DTO}: el campo publicado `modified: {declaration.strip()}` no "
                "distingue una fecha medida de una fecha que no existe"
            )
    if len(declarations) < 2:
        findings.append(
            f"{DTO}: solo se encuentra {len(declarations)} campo(s) `modified` "
            "publicado(s); el contrato esperaria los dos"
        )
    return findings


def the_owner_exists_and_says_what_it_says(text: str) -> list[str]:
    """El dueno unico existe y no fabrica.

    Si el dueno vuelve a tener un `unwrap_or(0)`, el defecto vuelve por la
    puerta de atras con la forma de un modulo que parece la fuente de verdad.
    """
    findings: list[str] = []
    for name in OWNER_API:
        if f"pub fn {name}(" not in text:
            findings.append(f"{OWNER}: falta la funcion publica `{name}`")
    if "unwrap_or(0)" in text or "unwrap_or_default()" in text:
        findings.append(
            f"{OWNER}: el dueno sustituye una fecha por un numero; eso es "
            "exactamente lo que no debe hacer"
        )
    if "i64" not in text:
        findings.append(
            f"{OWNER}: el dueno no mide con signo; una fecha anterior a la "
            "epoca no cabe en un contador sin signo"
        )
    return findings


def the_conversion_of_an_instant_goes_through_the_owner(
    files: list[tuple[str, str]] | None = None,
) -> list[str]:
    """Ningun sitio de produccion convierte un instante a unidades de epoca.

    La regla mira el conversor, no el fichero: una conversion de un instante
    a unidades de epoca tiene que pasar por el dueno. Se exceptuan dos cosas
    que no son la fecha de un fichero: la marca de "ahora" (`now()`) y las
    duraciones monotonas (`Instant`, `Utc`), que no son fechas.

    Esta regla ve lo que la regla por ventana no ve: el grafo por fichero
    escondia la conversion en una funcion auxiliar, y ahi es donde publicaba
    una fecha plausible y equivocada.
    """
    findings: list[str] = []
    seen = 0
    if files is None:
        files = [
            (str(path.relative_to(REPO_ROOT)), production_text(path))
            for path in production_files()
        ]
    for relative, text in files:
        if relative == OWNER or relative in KNOWN_GAPS:
            continue
        lines = text.splitlines()
        for index, line in enumerate(lines):
            if "duration_since" not in line:
                continue
            if "saturating_duration_since" in line or "signed_duration_since" in line:
                continue
            # Solo es una conversion a unidades de epoca si el destino es
            # la epoca. Una duracion monotona (`Instant`) no es una fecha.
            window = "\n".join(lines[index : index + 3])
            if "UNIX_EPOCH" not in window:
                continue
            seen += 1
            before = "\n".join(lines[max(0, index - 2) : index])
            if "now()" in before:
                # La marca de "ahora" no es la fecha de un fichero.
                continue
            findings.append(
                f"{relative}:{index + 1}: un instante se convierte a unidades de "
                f"epoca aqui, por su cuenta: {line.strip()[:100]}"
            )
    if seen == 0:
        findings.append(
            "el repositorio no convierte ningun instante a unidades de epoca; "
            "el contrato no miraria nada"
        )
    return findings


def the_production_code_does_not_fabricate_a_date(
    files: list[tuple[str, str]] | None = None,
) -> list[str]:
    """Ningun sitio de produccion convierte la fecha de un fichero por su cuenta.

    La regla se deriva del codigo, no de una lista de ficheros: se busca
    donde se lee una fecha de modificacion y se mira si el sitio la convierte
    a unidades de epoca con la pantalla puesta. Los huecos registrados son la
    unica excepcion, y el registro dice por que.
    """
    findings: list[str] = []
    seen_sites = 0
    if files is None:
        files = [
            (str(path.relative_to(REPO_ROOT)), production_text(path))
            for path in production_files()
        ]
    for relative, text in files:
        lines = text.splitlines()
        for index, line in enumerate(lines):
            if ".modified()" not in line:
                continue
            seen_sites += 1
            window = "\n".join(lines[index : index + WINDOW])
            if not any(marker in window for marker in CONVERSION_MARKERS):
                continue
            if any(f"{name}(" in window for name in OWNER_API):
                continue
            if relative in KNOWN_GAPS:
                continue
            findings.append(
                f"{relative}:{index + 1}: la fecha del fichero se convierte aqui "
                f"por su cuenta: {line.strip()[:100]}"
            )
    if seen_sites == 0:
        findings.append(
            "el repositorio no tiene ningun sitio que lea la fecha de un "
            "fichero; el contrato no miraria nada"
        )
    return findings


# Sentinel: "no se paso declaracion". Distingue "no se dice nada" de "se dice
# que no hay ninguno", que son afirmaciones distintas.
UNDECLARED = object()


def the_gap_registry_is_alive(
    registry: dict[str, str] | None = None,
    declaration: object = UNDECLARED,
) -> list[str]:
    """El registro de huecos sigue siendo cierto.

    Un hueco que se cierra y sigue en el registro es un registro que ya no
    dice la verdad: el contrato tiene que avisar para que se cierre.

    Un registro sin huecos es una afirmacion y por eso tiene que decirla. Un
    registro vacio sin esa declaracion es un olvido, y el contrato no lo puede
    dar por bueno: por eso la declaracion y el registro no pueden ser verdad
    los dos a la vez.
    """
    findings: list[str] = []
    if registry is None:
        registry = KNOWN_GAPS
    if declaration is UNDECLARED:
        declaration = NO_KNOWN_GAPS

    if registry and declaration:
        findings.append(
            "el registro dice a la vez que no hay huecos y que hay "
            f"{len(registry)}: las dos afirmaciones no pueden ser verdad a la "
            "vez"
        )
    if not registry and not declaration:
        findings.append(
            "el registro de huecos esta vacio y no dice por que no hay ninguno: "
            "un registro vacio tiene que ser una declaracion, no un olvido"
        )
    for relative in registry:
        path = REPO_ROOT / relative
        if not path.is_file():
            findings.append(
                f"el registro declara el hueco {relative}, pero ese fichero no "
                "existe: el registro tiene que estar vivo"
            )
            continue
        text = production_text(path)
        still_open = any(marker in text for marker in CONVERSION_MARKERS)
        if not still_open:
            findings.append(
                f"el registro declara el hueco {relative}, pero ese fichero ya "
                "no convierte la fecha por su cuenta: el hueco se cerro y el "
                "registro tiene que decirlo"
            )
    return findings


# ---------------------------------------------------------------------------
# Comprobaciones
# ---------------------------------------------------------------------------


def test_the_published_type_says_the_difference() -> None:
    """La fecha publicada puede estar vacia."""
    findings = the_published_type_says_the_difference(source(DTO))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_owner_exists_and_says_what_it_says() -> None:
    """El dueno unico existe y no fabrica."""
    findings = the_owner_exists_and_says_what_it_says(source(OWNER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_production_code_does_not_fabricate_a_date() -> None:
    """Ningun sitio de produccion fabrica una fecha."""
    findings = the_production_code_does_not_fabricate_a_date()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_conversion_of_an_instant_goes_through_the_owner() -> None:
    """Toda conversion de un instante pasa por el dueno."""
    findings = the_conversion_of_an_instant_goes_through_the_owner()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_gap_registry_is_alive() -> None:
    """El registro de huecos dice la verdad."""
    findings = the_gap_registry_is_alive()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_gap_registry_can_declare_that_there_are_no_gaps() -> None:
    """Un registro sin huecos es una afirmacion, y puede ser cierta.

    El registro esta vacio porque la ultima excepcion --la ingestion-- quedo
    cerrada. Cerrar un hueco no puede dejar al contrato sin poder declarar
    que no hay ninguno: si no, el registro se llenaria de huecos que ya no
    existen solo para no tener que escribir la afirmacion.
    """
    declared_empty = the_gap_registry_is_alive({}, "no hay ningun hueco conocido")
    assert not declared_empty, "FAIL: " + "; ".join(declared_empty)

    contradictory = the_gap_registry_is_alive(
        {"crates/cognicode-core/src/application/ingest/scan.rs": "motivo"},
        "no hay ningun hueco conocido",
    )
    assert contradictory, (
        "FAIL: el detector no nota un registro que dice a la vez que no hay "
        "huecos y que hay alguno"
    )


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
    """Control anti-vacuidad: cada detector nota un texto que la viola."""
    probes: list[tuple[object, tuple[object, ...]]] = [
        (
            the_published_type_says_the_difference,
            ("pub modified: u64,\n",),
        ),
        (
            the_owner_exists_and_says_what_it_says,
            (
                "pub fn epoch_seconds(instant: SystemTime) -> u64 {\n"
                "    instant.duration_since(UNIX_EPOCH).unwrap_or(0).as_secs()\n"
                "}\n",
            ),
        ),
        (
            the_production_code_does_not_fabricate_a_date,
            (
                [
                    (
                        "crates/cognicode-core/src/algo.rs",
                        "let mtime = meta.modified().ok()\n"
                        "    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())\n"
                        "    .map(|d| d.as_secs())\n"
                        "    .unwrap_or(0);\n",
                    )
                ],
            ),
        ),
        (
            the_conversion_of_an_instant_goes_through_the_owner,
            (
                [
                    (
                        "crates/cognicode-core/src/algo.rs",
                        "fn to_secs(t: SystemTime) -> u64 {\n"
                        "    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()\n"
                        "}\n",
                    ),
                ],
            ),
        ),
        (
            the_conversion_of_an_instant_goes_through_the_owner,
            (
                [
                    (
                        "crates/cognicode-core/src/algo.rs",
                        "fn elapsed(now: Instant, before: Instant) -> u64 {\n"
                        "    now.duration_since(before).as_secs()\n"
                        "}\n",
                    )
                ],
            ),
        ),
        (
            the_gap_registry_is_alive,
            ({}, None),
        ),
        (
            the_gap_registry_is_alive,
            ({"crates/cognicode-core/src/no_existe.rs": "motivo"}, None),
        ),
        (
            # Un hueco declarado sobre un fichero que ya no convierte la fecha
            # por su cuenta: el hueco se cerro y el registro tiene que decirlo.
            the_gap_registry_is_alive,
            (
                {
                    "crates/cognicode-core/src/application/ingest/types.rs": (
                        "motivo"
                    )
                },
                None,
            ),
        ),
    ]
    for detector, args in probes:
        found = detector(*args)  # type: ignore[operator]
        assert found, (
            f"FAIL: el detector {detector.__name__} no nota un texto que la "
            "viola; las comprobaciones de arriba pasarían sin comprobar nada"
        )


def main() -> int:
    checks = [
        test_the_published_type_says_the_difference,
        test_the_owner_exists_and_says_what_it_says,
        test_the_production_code_does_not_fabricate_a_date,
        test_the_conversion_of_an_instant_goes_through_the_owner,
        test_the_gap_registry_is_alive,
        test_the_gap_registry_can_declare_that_there_are_no_gaps,
        test_the_detectors_see_a_violation_when_there_is_one,
    ]
    failures: list[str] = []
    for check in checks:
        try:
            check()
        except AssertionError as error:
            failures.append(str(error))
            print(f"  FAIL {check.__name__}")
        else:
            print(f"  PASS {check.__name__}")

    if failures:
        print("una fecha se publica solo si se midio: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "una fecha se publica solo si se midio: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
