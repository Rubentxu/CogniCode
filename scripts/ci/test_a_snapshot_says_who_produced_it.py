#!/usr/bin/env python3
"""El snapshot dice quien lo produjo.

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
4. La base que el producto publica declara el analizador, y el catalogo
   publicado declara lo mismo: un documento que describe una forma que el
   producto ya no tiene es una promesa falsa.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

Ejecutar:  python3 scripts/ci/test_a_snapshot_says_who_produced_it.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

KEY_OWNER = "crates/cognicode-core/src/domain/value_objects/snapshot_key.rs"
HANDLERS = "crates/cognicode-core/src/interface/mcp/handlers/mod.rs"
CORE_MANIFEST = "crates/cognicode-core/Cargo.toml"
CATALOG = "product/tools.json"

KEY_COMPONENTS = ("workspace", "analyzer", "schema")
BUILD_COMPONENTS = ("version", "commit", "features")
PUBLISHED_COMPONENTS = (
    "analyzer_version",
    "analyzer_commit",
    "analyzer_features",
    "snapshot_schema",
)


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


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


def the_snapshot_carries_the_key(text: str) -> list[str]:
    """El snapshot se guarda con la clave y se decide con la clave.

    Un snapshot sin clave es un grafo sin autor, y cualquier otro build lo
    serviria como suyo. Y una decision que no compara la clave deja el
    defecto por la puerta de atras en cuanto se toca el formato.
    """
    findings: list[str] = []
    if "SNAPSHOT_SCHEMA" not in text:
        findings.append(
            f"{HANDLERS}: el snapshot no se escribe con el esquema que declara "
            "la clave"
        )
    if "same_producer_as" not in text:
        findings.append(
            f"{HANDLERS}: el sitio que carga el snapshot no compara la clave; "
            "un snapshot de otro analizador se serviria como suyo"
        )
    save = re.search(r"(?s)fn save_durable_snapshot\((.*?)\) -> std::io::Result", text)
    if not save:
        findings.append(f"{HANDLERS}: no se encuentra el guardado del snapshot")
    elif "key: &SnapshotKey" not in save.group(1):
        findings.append(
            f"{HANDLERS}: el snapshot se guarda sin la clave; queda un grafo "
            "sin autor"
        )
    load = re.search(r"(?s)fn load_durable_snapshot\((.*?)\) -> Option", text)
    if not load:
        findings.append(f"{HANDLERS}: no se encuentra la carga del snapshot")
    elif "SnapshotKey" not in text.split("fn load_durable_snapshot", 1)[1][:800]:
        findings.append(
            f"{HANDLERS}: la carga no devuelve la clave; el producto no puede "
            "saber quien produjo lo que carga"
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
                f"{HANDLERS}: la base publicada no declara `{component}`"
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
    findings = the_snapshot_carries_the_key(source(HANDLERS))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_published_basis_declares_the_analyzer() -> None:
    """La base publicada declara el analizador."""
    findings = the_published_basis_declares_the_analyzer(source(HANDLERS))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_published_catalog_declares_the_same() -> None:
    """El catalogo publicado declara lo mismo."""
    findings = the_published_catalog_declares_the_same()
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
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
    checks = [
        test_the_key_exists_and_declares_what_makes_it_a_key,
        test_the_key_is_built_without_touching_the_world,
        test_the_analyzer_declares_a_real_identity,
        test_the_declared_capabilities_match_the_manifest,
        test_the_snapshot_carries_the_key,
        test_the_published_basis_declares_the_analyzer,
        test_the_published_catalog_declares_the_same,
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
        print("el snapshot dice quien lo produjo: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "el snapshot dice quien lo produjo: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
