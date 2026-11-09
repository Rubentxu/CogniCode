#!/usr/bin/env python3
"""Dos contratos sobre lo que el producto persiste y publica del tiempo y del productor.

Los dos vigilan fuentes Rust del core (sin compilar nada) con la misma plantilla:
un dueño declarado, detectores que se aplican al texto de produccion y un test
que prueba que los detectores ven una violacion cuando la hay. Comparten
`REPO_ROOT` y `source`; cada parte conserva sus constantes, sus detectores y su
motivo.

Ejecutar:  python3 scripts/ci/test_snapshot_and_date_contracts.py

# PARTE A: una fecha se publica solo si se midio

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


# PARTE B: el snapshot dice quien lo produjo

MEDIDO: el producto persiste un snapshot del grafo y lo vuelve a servir cuando
las fuentes no han cambiado. Lo que el snapshot guardaba era el grafo y el
manifiesto de contenido. Lo que **no** guardaba era quien lo habia calculado:
ni la version del analizador, ni su commit, ni sus capacidades, ni el esquema.
Sin ese dato, un binario distinto --otra version, otro commit, otras
capacidades-- sirve el snapshot como si fuera suyo, y el consumidor recibe un
grafo que calculo otro analizador sin que nada en la respuesta lo diga.

Este contrato mira cuatro cosas y ninguna ejecuta el servicio:

1. La clave existe en el dominio, declara lo que la hace falta, y se construye
   con una funcion pura: no lee el disco ni pregunta al reloj.
2. El analizador declara una identidad real: la version del crate que calcula,
   el commit del que se construyo, y las capacidades con las que se compilo.
   Las capacidades se comparan con las que declara `Cargo.toml`: una feature
   anadida y no declarada es un analizador que no se puede identificar.
3. El snapshot persistido lleva la clave, y el sitio que decide si se sirve
   compara la clave antes de servirlo.
4. Las dos interfaces --el MCP y la CLI-- deciden con la misma funcion del
   dueno: una interface que decide por su cuenta puede servir un snapshot
   que nadie comparo con la clave.
5. La base que el producto publica declara el analizador, y el catalogo
   publicado declara lo mismo: un documento que describe una forma que el
   producto ya no tiene es una promesa falsa.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# Parte A: una fecha se publica solo si se midio
# ---------------------------------------------------------------------------
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


def test_the_date_detectors_see_a_violation_when_there_is_one() -> None:
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



# ---------------------------------------------------------------------------
# Parte B: el snapshot dice quien lo produjo
# ---------------------------------------------------------------------------
KEY_OWNER = "crates/cognicode-core/src/domain/value_objects/snapshot_key.rs"
HANDLERS = "crates/cognicode-core/src/interface/mcp/handlers/mod.rs"
# The published basis has its own module: it is a document with a
# published shape, and the contract follows the code to wherever the
# owner is.
BASIS = "crates/cognicode-core/src/interface/mcp/handlers/basis.rs"
# The persistence lives in its own module, and the owner is the
# application layer: it decides, writes and reads the file, so that the MCP
# and the CLI go through the same decision. The contract follows the code
# to wherever the owner is — a contract que mire un fichero que ya no
# guarda lo que dice guardar deja de comprobar nada.
SNAPSHOT_OWNER = "crates/cognicode-core/src/application/graph_snapshot.rs"
# The CLI's use case: the other half of the product that has to go through
# the same decision.
ANALYZER = "crates/cognicode-core/src/application/services/analyze.rs"
CORE_MANIFEST = "crates/cognicode-core/Cargo.toml"
CATALOG = "crates/cognicode-core/product/tools.json"

KEY_COMPONENTS = ("workspace", "analyzer", "schema")
BUILD_COMPONENTS = ("version", "commit", "features")
PUBLISHED_COMPONENTS = (
    "analyzer_version",
    "analyzer_commit",
    "analyzer_features",
    "snapshot_schema",
)


def declared_features() -> list[str]:
    """Las features que declara el `Cargo.toml` del analizador.

    `default` no es una capacidad: es el nombre del conjunto que Cargo activa
    cuando no se pide ninguna. Compararlo con la identidad seria comparar una
    lista de features con una feature.
    """
    text = source(CORE_MANIFEST)
    section = re.search(r"(?ms)^\[features\]\s*$(.*?)(?=^\[|\Z)", text)
    if not section:
        return []
    names: list[str] = []
    for line in section.group(1).splitlines():
        match = re.match(r"^([A-Za-z0-9_-]+)\s*=", line)
        if match and match.group(1) != "default":
            names.append(match.group(1))
    return sorted(names)


def declared_in_the_key(text: str) -> list[str]:
    """Las features que la identidad del analizador declara."""
    return sorted(set(re.findall(r'cfg!\(feature\s*=\s*"([^"]+)"\)', text)))


# ---------------------------------------------------------------------------
# Detectores
# ---------------------------------------------------------------------------


def the_key_exists_and_declares_what_makes_it_a_key(text: str) -> list[str]:
    """La clave existe, y declara lo que la hace falta.

    Sin el workspace, el snapshot podria servirse para otro arbol. Sin el
    analizador, podria servirse el grafo de otro build. Sin el esquema, un
    fichero de otra forma podria leerse como si fuera de esta.
    """
    findings: list[str] = []
    if "pub struct SnapshotKey" not in text:
        findings.append(f"{KEY_OWNER}: no existe el tipo `SnapshotKey`")
        return findings
    if "pub struct AnalyzerBuild" not in text:
        findings.append(f"{KEY_OWNER}: no existe el tipo `AnalyzerBuild`")
    for component in KEY_COMPONENTS:
        if not re.search(rf"(?m)^\s*pub {component}:", text):
            findings.append(
                f"{KEY_OWNER}: la clave no declara el componente `{component}`"
            )
    for component in BUILD_COMPONENTS:
        if not re.search(rf"(?m)^\s*pub {component}:", text):
            findings.append(
                f"{KEY_OWNER}: el analizador no declara el componente `{component}`"
            )
    if "SNAPSHOT_SCHEMA" not in text:
        findings.append(
            f"{KEY_OWNER}: el esquema del snapshot no tiene un unico sitio que "
            "lo declare"
        )
    return findings


def the_key_is_built_without_touching_the_world(text: str) -> list[str]:
    """La clave se construye con una funcion pura.

    Una clave que depende del disco, del reloj o del estado del sistema no es
    comparable: dos claves del mismo analisis podrian no coincidir, y el
    producto no sabria si el snapshot que tiene delante es suyo.
    """
    findings: list[str] = []
    body = re.split(r"(?m)^#\[cfg\(test\)\]", text, maxsplit=1)[0]
    for marker in ("std::fs::", "fs::read", "fs::write", "std::process::Command",
                   "SystemTime::now", "Instant::now", "thread::sleep"):
        if marker in body:
            findings.append(
                f"{KEY_OWNER}: la clave lee el mundo ({marker}); una clave "
                "comparable no puede depender del estado del sistema"
            )
    if "pub fn new(" not in body:
        findings.append(
            f"{KEY_OWNER}: la clave no tiene un constructor que se pueda llamar "
            "con datos"
        )
    return findings


def the_analyzer_declares_a_real_identity(text: str) -> list[str]:
    """El analizador dice quien es, con hechos y no con constantes inventadas.

    La version tiene que ser la del crate que calcula --el proveedor--, no la
    del consumidor que pregunta. El commit, cuando se puede saber, y ausente
    cuando no: un valor inventado seria una mentira.
    """
    findings: list[str] = []
    if 'env!("CARGO_PKG_VERSION")' not in text:
        findings.append(
            f"{KEY_OWNER}: el analizador no declara la version del crate que "
            "calcula"
        )
    if "GIT_SHA" not in text:
        findings.append(
            f"{KEY_OWNER}: el analizador no declara el commit del que se "
            "construyo"
        )
    build_script = REPO_ROOT / "crates/cognicode-core/build.rs"
    if not build_script.is_file():
        findings.append(
            "crates/cognicode-core/build.rs no existe: sin el, el commit nunca "
            "se conoce"
        )
    else:
        script = build_script.read_text(encoding="utf-8")
        if "GIT_SHA" not in script:
            findings.append(
                "crates/cognicode-core/build.rs no hornea el commit del que se "
                "construyo el analizador"
            )
    return findings


def the_declared_capabilities_match_the_manifest(
    key_text: str | None = None,
    manifest_features: list[str] | None = None,
) -> list[str]:
    """Las capacidades declaradas son las que el analizador tiene.

    La lista es explicita porque el lenguaje no deja preguntar en tiempo de
    ejecucion que features se activaron. Si se anade una feature y no se nombra
    aqui, el analizador no puede decir que puede hacer: dos builds con
    capacidades distintas se identificarian como el mismo.
    """
    findings: list[str] = []
    if key_text is None:
        key_text = source(KEY_OWNER)
    if manifest_features is None:
        manifest_features = declared_features()
    declared = declared_in_the_key(key_text)
    if not manifest_features:
        findings.append(
            f"{CORE_MANIFEST}: no se encuentra la seccion `[features]`; el "
            "contrato no compararia nada"
        )
        return findings
    if not declared:
        findings.append(
            f"{KEY_OWNER}: la identidad del analizador no declara ninguna "
            "capacidad; no se puede distinguir un build de otro"
        )
        return findings
    missing = [name for name in manifest_features if name not in declared]
    if missing:
        findings.append(
            f"{KEY_OWNER}: el analizador no declara las capacidades "
            f"{missing}, que `Cargo.toml` si declara; un build con esas "
            "capacidades no se puede identificar"
        )
    unknown = [name for name in declared if name not in manifest_features]
    if unknown:
        findings.append(
            f"{KEY_OWNER}: la identidad declara capacidades {unknown} que "
            f"`Cargo.toml` no declara: el `cfg!` no existe y el build se "
            "rompe"
        )
    return findings


def the_snapshot_carries_the_key(owner_text: str | None = None) -> list[str]:
    """El snapshot se guarda con la clave y se decide con la clave.

    Un snapshot sin clave es un grafo sin autor, y cualquier otro build lo
    serviria como suyo. Y una decision que no compara la clave deja el
    defecto por la puerta de atras en cuanto se toca el formato.

    `owner_text` es el sitio que GUARDA y DECIDE: el dueno del snapshot es
    uno solo, y vive en la capa de aplicacion.
    """
    if owner_text is None:
        owner_text = source(SNAPSHOT_OWNER)
    findings: list[str] = []
    if "SNAPSHOT_SCHEMA" not in owner_text:
        findings.append(
            f"{SNAPSHOT_OWNER}: el snapshot no se escribe con el esquema que "
            "declara la clave"
        )
    if "same_producer_as" not in owner_text:
        findings.append(
            f"{SNAPSHOT_OWNER}: el sitio que carga el snapshot no compara la "
            "clave; un snapshot de otro analizador se serviria como suyo"
        )
    save = re.search(
        r"(?s)fn save_durable_snapshot\((.*?)\) -> std::io::Result", owner_text
    )
    if not save:
        findings.append(
            f"{SNAPSHOT_OWNER}: no se encuentra el guardado del snapshot"
        )
    elif "key: &SnapshotKey" not in save.group(1):
        findings.append(
            f"{SNAPSHOT_OWNER}: el snapshot se guarda sin la clave; queda un "
            "grafo sin autor"
        )
    load = re.search(r"(?s)fn load_durable_snapshot\((.*?)\) -> Option", owner_text)
    if not load:
        findings.append(f"{SNAPSHOT_OWNER}: no se encuentra la carga del snapshot")
    elif "SnapshotKey" not in owner_text.split("fn load_durable_snapshot", 1)[1][:800]:
        findings.append(
            f"{SNAPSHOT_OWNER}: la carga no devuelve la clave; el producto no "
            "puede saber quien produjo lo que carga"
        )
    return findings


def the_interfaces_use_the_owners_decision(
    handlers_text: str | None = None, analyzer_text: str | None = None
) -> list[str]:
    """Las dos interfaces deciden con la misma funcion.

    Una interface que decide por su cuenta es una interface que puede
    servir un snapshot que nadie comparo con la clave. Las dos --el MCP y
    la CLI-- tienen que pasar por el dueno: es lo que hace que lo que una
    calcula, la otra lo sirva.
    """
    if handlers_text is None:
        handlers_text = source(HANDLERS)
    if analyzer_text is None:
        analyzer_text = source(ANALYZER)
    findings: list[str] = []
    if "load_or_build_graph" not in handlers_text:
        findings.append(
            f"{HANDLERS}: los handlers no preguntan al dueno del snapshot; "
            "pueden servir un grafo que nadie comparo con la clave"
        )
    if "load_or_build_graph" not in analyzer_text:
        findings.append(
            f"{ANALYZER}: la CLI no pregunta al dueno del snapshot; reconstruye "
            "el arbol en cada run y nada de lo que calcula se guarda"
        )
    return findings


def the_published_basis_declares_the_analyzer(text: str) -> list[str]:
    """Lo que el producto publica dice quien produjo el analisis.

    Una base que declara el workspace, la configuracion y las fuentes, y dice
    `complete: true`, esta diciendo que la identidad del analisis esta
    completa. Si le falta el analizador, no lo esta.
    """
    findings: list[str] = []
    for component in PUBLISHED_COMPONENTS:
        if not re.search(rf"(?m)^\s*pub {component}:", text):
            findings.append(
                f"{BASIS}: la base publicada no declara `{component}`"
            )
    return findings


def the_published_catalog_declares_the_same(catalog_text: str | None = None) -> list[str]:
    """El catalogo publicado declara lo mismo que el runtime.

    El catalogo es la descripcion de la superficie que el producto publica.
    Si el runtime gana un campo y el catalogo no, el documento describe una
    forma que ya no existe: es una promesa que el producto no cumple.
    """
    findings: list[str] = []
    if catalog_text is None:
        catalog_text = source(CATALOG)
    document = json.loads(catalog_text)
    tools = {tool["name"]: tool for tool in document["tools"]}
    build_graph = tools.get("build_graph")
    if build_graph is None:
        findings.append(f"{CATALOG}: no declara la tool `build_graph`")
        return findings
    definitions = (build_graph.get("output_schema") or {}).get("$defs") or {}
    basis = definitions.get("BasisDto")
    if basis is None:
        findings.append(
            f"{CATALOG}: el esquema publicado de `build_graph` no define "
            "`BasisDto`"
        )
        return findings
    properties = basis.get("properties") or {}
    for component in PUBLISHED_COMPONENTS:
        if component not in properties:
            findings.append(
                f"{CATALOG}: el esquema publicado de la base no declara "
                f"`{component}`; el documento describe una forma que el "
                "producto ya no tiene"
            )
    return findings


# ---------------------------------------------------------------------------
# Comprobaciones
# ---------------------------------------------------------------------------


def test_the_key_exists_and_declares_what_makes_it_a_key() -> None:
    """La clave existe y declara lo que la hace falta."""
    findings = the_key_exists_and_declares_what_makes_it_a_key(source(KEY_OWNER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_key_is_built_without_touching_the_world() -> None:
    """La clave se construye con una funcion pura."""
    findings = the_key_is_built_without_touching_the_world(source(KEY_OWNER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_analyzer_declares_a_real_identity() -> None:
    """El analizador dice quien es."""
    findings = the_analyzer_declares_a_real_identity(source(KEY_OWNER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_declared_capabilities_match_the_manifest() -> None:
    """Las capacidades declaradas son las que el analizador tiene."""
    findings = the_declared_capabilities_match_the_manifest()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_snapshot_carries_the_key() -> None:
    """El snapshot lleva la clave y se decide con la clave."""
    findings = the_snapshot_carries_the_key()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_interfaces_use_the_owners_decision() -> None:
    """Las dos interfaces pasan por el dueno del snapshot."""
    findings = the_interfaces_use_the_owners_decision()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_published_basis_declares_the_analyzer() -> None:
    """La base publicada declara el analizador."""
    findings = the_published_basis_declares_the_analyzer(source(BASIS))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_published_catalog_declares_the_same() -> None:
    """El catalogo publicado declara lo mismo."""
    findings = the_published_catalog_declares_the_same()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_snapshot_detectors_see_a_violation_when_there_is_one() -> None:
    """Control anti-vacuidad: cada detector nota un texto que la viola."""
    probes: list[tuple[object, tuple[object, ...]]] = [
        (
            the_key_exists_and_declares_what_makes_it_a_key,
            ("pub struct SnapshotKey {\n    pub workspace: String,\n}\n",),
        ),
        (
            the_key_is_built_without_touching_the_world,
            (
                "pub fn new() -> Self {\n"
                "    let bytes = std::fs::read(\"x\").unwrap();\n"
                "    Self { workspace: bytes }\n}\n",
            ),
        ),
        (
            the_analyzer_declares_a_real_identity,
            ("pub fn current() -> Self { Self { version: \"0.0.0\".into() } }\n",),
        ),
        (
            the_declared_capabilities_match_the_manifest,
            ("cfg!(feature = \"multimodal\")\n", ["persistence", "multimodal"]),
        ),
        (
            the_snapshot_carries_the_key,
            ("fn save_durable_snapshot(db_path: &Path, graph: &CallGraph) {}\n",),
        ),
        (
            the_interfaces_use_the_owners_decision,
            ("fn build_project_graph() {}\n", "fn build_project_graph() {}\n"),
        ),
        (
            the_published_basis_declares_the_analyzer,
            ("pub struct BasisDto {\n    pub workspace: String,\n}\n",),
        ),
        (
            the_published_catalog_declares_the_same,
            (
                json.dumps(
                    {
                        "tools": [
                            {
                                "name": "build_graph",
                                "output_schema": {"$defs": {"BasisDto": {"properties": {}}}},
                            }
                        ]
                    }
                ),
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
    module = sys.modules[__name__]
    checks = sorted(
        (
            getattr(module, name)
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda check: check.__code__.co_firstlineno,
    )
    assert checks, (
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria OK"
    )
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
        print("fecha medida y snapshot con productor: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "fecha medida y snapshot con productor: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
