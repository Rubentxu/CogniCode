#!/usr/bin/env python3
"""El consumidor del protocolo puede declarar la version sobre la que escribe.

MEDIDO: la escritura condicional ya existia en la capa de aplicacion, pero en
la frontera del protocolo el campo se fijaba a `None`: el consumidor no podia
decir con que version estaba trabajando, y una edicion pisaba el trabajo de
otro editor sin que nadie se enterara. Ademas, el esquema que el consumidor
ve vive en `product/tools.json` —el runtime lo publica desde ahi, no desde el
tipo Rust—, asi que el documento publicado y el tipo tenian que contar la
misma historia.

Este contrato mira tres cosas y ninguna ejecuta el servicio:

1. El documento publicado declara el campo, en las tools que lo aceptan.
2. El tipo del protocolo declara el campo y la conversion lo lleva, en vez de
   fijarlo a `None`.
3. **La captura del runtime dice lo mismo que el documento publicado.** Esta es
   la que nadie vigilaba: `product/tool-catalog-runtime.json` es la captura de
   un `tools/list` real, y si el documento publicado cambia y la captura no, la
   captura pasa a ser una mentira y nada lo dice.

Y, desde el 2026-10-29, el documento publicado tiene que decir **lo mismo que
el runtime dice de si mismo**, no solo coincidir en el esquema de entrada: el
`runtime_metadata` del maestro es el espejo de lo que el runtime publica en
`_meta.cognicode`, y el `tool_version` de ahi lo inyecta el runtime en cada
respuesta. Comparar solo el esquema de entrada dejaba pasar una deriva que no
cambia ningun nombre de tool: el maestro decia `0.99.2` de un runtime que
reportaba `0.101.31`, en las 78 tools, y el documento publicado miente sobre
el runtime que lo publica.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

Ejecutar:  python3 scripts/ci/test_the_consumer_can_declare_the_version.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

CATALOG = "product/tools.json"
CAPTURE = "product/tool-catalog-runtime.json"
MAPPING = "crates/cognicode-core/src/interface/mcp/dto_mapping.rs"
SCHEMAS = "crates/cognicode-core/src/interface/mcp/schemas.rs"

# La sincronizacion del maestro la posee un solo modulo; el contrato lo usa en
# vez de reimplementar la regla. Ver `scripts/product/sync_tool_catalog_runtime_metadata.py`.
sys.path.insert(0, str(REPO_ROOT / "scripts" / "product"))
from generate_tool_catalog import render  # noqa: E402
from sync_tool_catalog_runtime_metadata import merge_runtime_metadata  # noqa: E402

# Las tools que aceptan la version que el consumidor leyo.
CONDITIONAL_TOOLS = ("write_file", "edit_file")

# Las claves de `runtime_metadata` que el runtime NO publica y que el maestro
# cura a mano: son la curacion del maestro, no un reflejo del runtime, y no se
# pueden perder al sincronizar. `check_architecture` las lleva desde que se
# escribieron; el runtime no las ha reportado nunca.
CURATED_RUNTIME_METADATA = {
    "check_architecture": ("gate_authority", "produces_observations_only"),
}


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


def document(relative: str) -> object:
    return json.loads(source(relative))


def summarize(findings: list[str], limit: int = 5) -> str:
    """El mensaje de un fallo que se repite 78 veces no se lee.

    El numero de hallazgos es exacto; lo que se acota es la lista que se
    imprime, porque un fallo de deriva reune una entrada por tool.
    """
    if len(findings) <= limit:
        return "; ".join(findings)
    rest = len(findings) - limit
    return f"{len(findings)} hallazgos: " + "; ".join(findings[:limit]) + f"; ... y {rest} mas"


# ---------------------------------------------------------------------------
# Detectores
# ---------------------------------------------------------------------------


def master_declares_the_version(master: object) -> list[str]:
    """El documento publicado declara el campo en las tools que lo aceptan."""
    findings: list[str] = []
    tools = {t["name"]: t for t in master["tools"]}  # type: ignore[index]
    for name in CONDITIONAL_TOOLS:
        tool = tools.get(name)
        if tool is None:
            findings.append(f"{name} no esta en el documento publicado")
            continue
        properties = (tool.get("input_schema") or {}).get("properties") or {}
        if "expected_content_hash" not in properties:
            findings.append(
                f"{name} no declara `expected_content_hash`: el consumidor no "
                "puede decir con que version esta trabajando"
            )
    return findings


def capture_declares_the_version(capture: object) -> list[str]:
    """La captura real del runtime declara el mismo campo."""
    findings: list[str] = []
    tools = {t["name"]: t for t in capture}  # type: ignore[union-attr]
    for name in CONDITIONAL_TOOLS:
        tool = tools.get(name)
        if tool is None:
            findings.append(f"{name} no esta en la captura")
            continue
        properties = (tool.get("inputSchema") or {}).get("properties") or {}
        if "expected_content_hash" not in properties:
            findings.append(
                f"la captura de {name} no declara `expected_content_hash`: la "
                "captura es de una build anterior al campo"
            )
    return findings


def the_capture_agrees_with_the_master(master: object, capture: object) -> list[str]:
    """La captura y el documento publicado declaran el mismo esquema de entrada.

    La captura es provenance: prueba de lo que el runtime publica. Si el
    documento publicado cambia y la captura no, la captura deja de probar
    nada, y el unico nombre que se compara —el conjunto de tools— no se
    entera porque no ha cambiado.

    Lo que se compara es el esquema de entrada, que es lo que el consumidor
    puede pasar. Los metadatos tienen su propia deriva y su propia
    comprobacion: `the_runtime_metadata_agrees_with_the_master`.
    """
    findings: list[str] = []
    captured = {t["name"]: t for t in capture}  # type: ignore[union-attr]
    for tool in master["tools"]:  # type: ignore[index]
        name = tool["name"]
        runtime = captured.get(name)
        if runtime is None:
            findings.append(f"{name} esta en el documento publicado y no en la captura")
            continue
        if tool.get("input_schema") != runtime.get("inputSchema"):
            findings.append(
                f"{name}: el documento publicado y la captura declaran "
                "distintos esquemas de entrada; la captura es de otra build"
            )
    return findings


def the_runtime_metadata_agrees_with_the_master(master: object, capture: object) -> list[str]:
    """El maestro dice, de cada clave que el runtime publica, lo que el runtime dice.

    El `runtime_metadata` del maestro es el espejo de lo que el runtime
    publica en `_meta.cognicode`. El `tool_version` de ahi lo inyecta el
    runtime en cada respuesta (`rmcp_adapter.rs`), asi que el maestro tiene que
    decir la version del runtime, no la version que el runtime tenia cuando esa
    entrada se escribio a mano.

    MEDIDO 2026-10-29: las 78 entradas del maestro declaraban una version
    distinta de la que reporta el runtime (`0.99.2` en 73, `0.101.11` en 3,
    `0.101.14` y `0.101.15` en las otras dos), y el runtime reportaba
    `0.101.31`. La deriva era invisible porque un nombre de tool no cambia
    porque cambie la version, y el unico nombre que se comparaba era el
    conjunto de tools.
    """
    findings: list[str] = []
    captured = {t["name"]: t for t in capture}  # type: ignore[union-attr]
    for tool in master["tools"]:  # type: ignore[index]
        name = tool["name"]
        runtime = captured.get(name)
        if runtime is None:
            # La captura no cubre esta tool: su metadato no se puede cotejar.
            # Lo declara la sincronizacion, no esta comparacion.
            continue
        published = (runtime.get("_meta") or {}).get("cognicode") or {}
        mirror = tool.get("runtime_metadata") or {}
        for key, value in published.items():
            if key not in mirror:
                findings.append(
                    f"{name}.{key}: el maestro no publica la clave que el runtime publica"
                )
            elif mirror[key] != value:
                findings.append(
                    f"{name}.{key}: el maestro dice {mirror[key]!r} y el runtime "
                    f"reporta {value!r}"
                )
    return findings


def the_curated_metadata_is_still_there(master: object) -> list[str]:
    """Las claves que el runtime no publica siguen curadas en el maestro.

    La curacion del maestro es suya: el runtime no reporta estas claves y no
    puede devolverlas. Perderlas no es una sincronizacion, es una amputacion.
    """
    findings: list[str] = []
    tools = {t["name"]: t for t in master["tools"]}  # type: ignore[index]
    for name, keys in CURATED_RUNTIME_METADATA.items():
        tool = tools.get(name)
        if tool is None:
            findings.append(
                f"{name} no esta en el maestro: sus claves curadas se han perdido"
            )
            continue
        mirror = tool.get("runtime_metadata") or {}
        for key in keys:
            if key not in mirror:
                findings.append(
                    f"{name}.{key}: la clave curada que el runtime no publica ya "
                    "no esta en el maestro"
                )
    return findings


def the_curated_metadata_survives_the_merge(master: object, merged: object) -> list[str]:
    """La sincronizacion no se lleva por delante lo curado a mano.

    La sincronizacion es una fusion, no una copia: las claves que el runtime no
    publica no tienen de donde volver, asi que una copia las perderia en
    silencio.
    """
    findings: list[str] = []
    before = {t["name"]: t.get("runtime_metadata") or {} for t in master["tools"]}  # type: ignore[index]
    after = {t["name"]: t.get("runtime_metadata") or {} for t in merged["tools"]}  # type: ignore[index]
    for name, keys in CURATED_RUNTIME_METADATA.items():
        for key in keys:
            if key not in before.get(name, {}):
                continue
            if key not in after.get(name, {}):
                findings.append(
                    f"{name}.{key}: la sincronizacion se ha llevado la clave curada"
                )
            elif after[name][key] != before[name][key]:
                findings.append(
                    f"{name}.{key}: la sincronizacion ha cambiado el valor de la "
                    "clave curada"
                )
    return findings


def the_schema_declares_the_version(text: str) -> list[str]:
    """El tipo del protocolo declara el campo donde tiene que declararlo."""
    findings: list[str] = []
    for struct in ("ReadFileOutput", "WriteFileInput", "EditFileInput"):
        if f"pub struct {struct}" not in text:
            findings.append(f"no existe {struct}")
            continue
        block = text.split(f"pub struct {struct}", 1)[1].split("}", 1)[0]
        if "content_hash" not in block:
            findings.append(f"{struct} no declara el campo de la version")
    return findings


def the_mapping_carries_the_version(text: str) -> list[str]:
    """La conversion lleva la version; no la fija a `None`."""
    findings: list[str] = []
    if "expected_content_hash: None" in text:
        findings.append(
            "la conversion sigue fijando la version a `None`: el campo llega "
            "al protocolo y se pierde en la frontera"
        )
    if "expected_content_hash: input.expected_content_hash" not in text:
        findings.append("la conversion no lleva la version declarada")
    if "content_hash: result.content_hash" not in text:
        findings.append("la conversion no lleva la version publicada")
    return findings


# ---------------------------------------------------------------------------
# Comprobaciones
# ---------------------------------------------------------------------------


def test_the_published_document_declares_the_version() -> None:
    """El documento publicado dice que el consumidor puede declarar la version."""
    findings = master_declares_the_version(document(CATALOG))
    assert not findings, "FAIL: " + summarize(findings)


def test_the_runtime_capture_declares_the_version() -> None:
    """La captura real del runtime declara el campo."""
    findings = capture_declares_the_version(document(CAPTURE))
    assert not findings, "FAIL: " + summarize(findings)


def test_the_capture_agrees_with_the_published_document() -> None:
    """La captura no se queda vieja cuando el documento publicado cambia."""
    findings = the_capture_agrees_with_the_master(
        document(CATALOG), document(CAPTURE)
    )
    assert not findings, "FAIL: " + summarize(findings)


def test_the_runtime_metadata_agrees_with_the_capture() -> None:
    """El maestro dice, de su runtime, la version que el runtime reporta."""
    findings = the_runtime_metadata_agrees_with_the_master(
        document(CATALOG), document(CAPTURE)
    )
    assert not findings, "FAIL: " + summarize(findings)


def test_the_curated_runtime_metadata_is_still_there() -> None:
    """Las claves que el runtime no publica siguen curadas en el maestro."""
    findings = the_curated_metadata_is_still_there(document(CATALOG))
    assert not findings, "FAIL: " + summarize(findings)


def test_the_synchronization_keeps_the_curated_metadata() -> None:
    """La sincronizacion no se lleva por delante lo curado a mano."""
    master = document(CATALOG)
    merged, findings = merge_runtime_metadata(master, document(CAPTURE))
    assert not findings, "FAIL: " + summarize(findings)
    drift = the_curated_metadata_survives_the_merge(master, merged)
    assert not drift, "FAIL: " + summarize(drift)


def test_the_committed_master_is_synchronized() -> None:
    """El maestro commiteado es el maestro sincronizado con la captura."""
    master = document(CATALOG)
    merged, _ = merge_runtime_metadata(master, document(CAPTURE))
    assert render(merged) == source(CATALOG), (
        "FAIL: el maestro commiteado no es el resultado de sincronizarlo con la "
        "captura; o su metadato de runtime esta viejo, o el documento lleva "
        "encima una edicion a mano que la sincronizacion no produce"
    )


def test_the_protocol_type_declares_the_version() -> None:
    """El tipo del protocolo declara el campo."""
    findings = the_schema_declares_the_version(source(SCHEMAS))
    assert not findings, "FAIL: " + summarize(findings)


def test_the_conversion_carries_the_version() -> None:
    """La conversion lleva la version en vez de fijarla a `None`."""
    findings = the_mapping_carries_the_version(source(MAPPING))
    assert not findings, "FAIL: " + summarize(findings)


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
    """Control anti-vacuidad: cada detector nota un texto que la viola."""
    probes: list[tuple[object, tuple[object, ...]]] = [
        (
            master_declares_the_version,
            (
                {
                    "tools": [
                        {
                            "name": "write_file",
                            "input_schema": {"properties": {"path": {}}},
                        },
                        {
                            "name": "edit_file",
                            "input_schema": {"properties": {"path": {}}},
                        },
                    ]
                },
            ),
        ),
        (
            capture_declares_the_version,
            (
                [
                    {
                        "name": "write_file",
                        "inputSchema": {"properties": {"path": {}}},
                    },
                    {
                        "name": "edit_file",
                        "inputSchema": {"properties": {"path": {}}},
                    },
                ],
            ),
        ),
        (
            the_capture_agrees_with_the_master,
            (
                {
                    "tools": [
                        {
                            "name": "write_file",
                            "input_schema": {"properties": {"path": {}}},
                        }
                    ]
                },
                [{"name": "write_file", "inputSchema": {"properties": {}}}],
            ),
        ),
        (
            the_schema_declares_the_version,
            (
                "pub struct ReadFileOutput { pub content: String }\n"
                "pub struct WriteFileInput { pub path: String }\n"
                "pub struct EditFileInput { pub path: String }\n",
            ),
        ),
        (
            the_mapping_carries_the_version,
            (
                "WriteFileRequest { expected_content_hash: None }\n",
            ),
        ),
        (
            the_runtime_metadata_agrees_with_the_master,
            (
                {
                    "tools": [
                        {
                            "name": "write_file",
                            "runtime_metadata": {"tool_version": "0.99.2"},
                        }
                    ]
                },
                [
                    {
                        "name": "write_file",
                        "_meta": {"cognicode": {"tool_version": "0.101.31"}},
                    }
                ],
            ),
        ),
        (
            the_curated_metadata_is_still_there,
            (
                {
                    "tools": [
                        {"name": "check_architecture", "runtime_metadata": {}},
                    ]
                },
            ),
        ),
        (
            the_curated_metadata_survives_the_merge,
            (
                {
                    "tools": [
                        {
                            "name": "check_architecture",
                            "runtime_metadata": {
                                "gate_authority": False,
                                "produces_observations_only": True,
                            },
                        }
                    ]
                },
                {
                    "tools": [
                        {"name": "check_architecture", "runtime_metadata": {}},
                    ]
                },
            ),
        ),
    ]
    for detector, args in probes:
        found = detector(*args)  # type: ignore[operator]
        assert found, (
            f"FAIL: el detector {detector.__name__} no nota un texto que la "
            "viola; las comprobaciones de arriba pasarian sin comprobar nada"
        )


def main() -> int:
    checks = [
        test_the_published_document_declares_the_version,
        test_the_runtime_capture_declares_the_version,
        test_the_capture_agrees_with_the_published_document,
        test_the_runtime_metadata_agrees_with_the_capture,
        test_the_curated_runtime_metadata_is_still_there,
        test_the_synchronization_keeps_the_curated_metadata,
        test_the_committed_master_is_synchronized,
        test_the_protocol_type_declares_the_version,
        test_the_conversion_carries_the_version,
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
        print("el consumidor puede declarar la version: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "el consumidor puede declarar la version: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
