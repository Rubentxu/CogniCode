#!/usr/bin/env python3
"""La escritura se publica entera, se borra si no se publica, y comprueba la
version que el llamante dice que leyo.

MEDIDO: el servicio de ficheros abria el temporal con `File::create`, que no
exige creacion exclusiva (un temporal que ya estaba se trunca), y la limpieza
existia solo en el error del cambio de nombre: si la escritura fallaba, el
temporal se quedaba en el disco. Tampoco habia sincronizacion, asi que un
corte de luz podia dejar el destino con el contenido viejo. Y no habia ninguna
comprobacion de version: una edicion pisaba el trabajo de otro editor sin decir
nada.

Este contrato no ejecuta el servicio: mira el codigo de produccion y comprueba
que las garantias estan escritas donde tienen que estar y en el orden en que
tienen que estar. El comportamiento observable lo fijan los tests de integracion
(`the_write_refuses_a_file_that_changed.rs`) y los tests unitarios del adaptador.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

Ejecutar:  python3 scripts/ci/test_the_write_is_atomic_and_declares_the_version.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

CORE = "crates/cognicode-core/src"
WRITER = f"{CORE}/application/services/atomic_file_writer.rs"
SERVICE = f"{CORE}/application/services/file_operations.rs"
DTO = f"{CORE}/application/dto/file_ops.rs"


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


def production(relative: str) -> str:
    """El codigo de produccion de un fichero.

    Los tests que viven en el mismo fichero no cuentan para estas reglas: un
    test que crea su propio fichero de trabajo no es una escritura manual.
    """
    text = source(relative)
    marker = text.find("#[cfg(test)]")
    return text if marker < 0 else text[:marker]


# ---------------------------------------------------------------------------
# Detectores. Cada uno devuelve los hallazgos que encuentra sobre un texto.
# ---------------------------------------------------------------------------


def exclusive_creation_findings(text: str) -> list[str]:
    """El temporal tiene que crearse en exclusiva."""
    findings: list[str] = []
    if "create_new(true)" not in text:
        findings.append(
            "el temporal no se crea en exclusiva (`create_new(true)`): un "
            "temporal que ya esta se trunca y se pierde lo que tenia"
        )
    if "File::create" in text:
        findings.append(
            "el temporal se abre con `File::create`, que no exige creacion "
            "exclusiva"
        )
    return findings


def durability_findings(text: str) -> list[str]:
    """El contenido tiene que estar en disco antes de que el destino exista."""
    findings: list[str] = []
    if "sync_all" not in text:
        findings.append(
            "no se sincroniza: un corte de luz puede dejar el destino con el "
            "contenido viejo"
        )
    elif "fs::rename" in text and text.find("sync_all") > text.find("fs::rename"):
        findings.append(
            "se publica antes de sincronizar: la sincronizacion tiene que ir "
            "antes del cambio de nombre"
        )
    return findings


def drop_cleanup_findings(text: str) -> list[str]:
    """Lo que no se publica tiene que borrarse solo."""
    findings: list[str] = []
    if "impl Drop for AtomicFileWriter" not in text:
        findings.append(
            "no borra el temporal al soltar el escritor: la limpieza tiene que "
            "estar en `Drop`, no en el camino de error"
        )
    else:
        drop_body = text.split("impl Drop for AtomicFileWriter", 1)[1]
        if "remove_file" not in drop_body:
            findings.append("el `Drop` no borra el temporal")
    return findings


def atomic_publication_findings(text: str) -> list[str]:
    """Quien escribe publica con el escritor atomico."""
    findings: list[str] = []
    if "File::create" in text:
        findings.append(
            "sigue abriendo un temporal a mano: la escritura tiene que pasar "
            "por el escritor atomico"
        )
    if "fs::rename" in text:
        findings.append(
            "sigue renombrando el temporal a mano: la publicacion tiene que "
            "pasar por el escritor atomico"
        )
    if text.count("AtomicFileWriter::create") < 2:
        findings.append(
            "no usa el escritor atomico en las dos rutas de escritura (una "
            "escritura y una edicion)"
        )
    return findings


def version_before_publication_findings(text: str) -> list[str]:
    """La version declarada se comprueba antes de publicar, no despues."""
    findings: list[str] = []
    commits = [match.start() for match in re.finditer(r"writer\.commit\(\)", text)]
    if not commits:
        findings.append("no hay ninguna publicacion que comprobar")
        return findings
    for commit in commits:
        created = text.rfind("AtomicFileWriter::create", 0, commit)
        if created < 0:
            findings.append("una publicacion no pasa por el escritor atomico")
            continue
        if "refuse_if_changed(" not in text[created:commit]:
            findings.append(
                "una publicacion no comprueba la version que el llamante "
                "declara antes de publicar"
            )
    return findings


def declared_version_findings(text: str) -> list[str]:
    """Las peticiones pueden declarar con que version estan trabajando."""
    findings: list[str] = []
    for request in ("WriteFileRequest", "EditFileRequest"):
        if f"pub struct {request}" not in text:
            findings.append(f"no existe {request}")
            continue
        block = text.split(f"pub struct {request}", 1)[1].split("}", 1)[0]
        if "expected_content_hash" not in block:
            findings.append(
                f"{request} no puede declarar la version del contenido que el "
                "llamante leyo"
            )
    return findings


# ---------------------------------------------------------------------------
# Comprobaciones
# ---------------------------------------------------------------------------


def test_the_temporary_file_is_created_in_exclusive_mode() -> None:
    """El temporal se crea en exclusiva: un nombre que ya existe no se trunca."""
    findings = exclusive_creation_findings(production(WRITER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_content_is_durable_before_it_is_published() -> None:
    """El contenido esta en disco antes de que el destino exista."""
    findings = durability_findings(production(WRITER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_temporary_file_does_not_survive_an_unpublished_write() -> None:
    """Lo que no se publica no se queda en el disco."""
    findings = drop_cleanup_findings(production(WRITER))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_service_publishes_through_the_atomic_writer() -> None:
    """Quien escribe usa el escritor atomico, no un temporal y un rename a mano."""
    findings = atomic_publication_findings(production(SERVICE))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_declared_version_is_checked_before_the_content_is_published() -> None:
    """La version declarada se comprueba antes de publicar, no despues."""
    findings = version_before_publication_findings(production(SERVICE))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_requests_can_declare_the_version() -> None:
    """Las peticiones pueden declarar con que version estan trabajando."""
    findings = declared_version_findings(production(DTO))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
    """Control anti-vacuidad: cada detector nota un texto que la viola.

    Sin esto, un detector que no mira nada declararia verde: las reglas de
    arriba pasarian sin comprobar nada.
    """
    probes: list[tuple[object, str]] = [
        (
            exclusive_creation_findings,
            "OpenOptions::new().write(true).open(&temporal)?;\n",
        ),
        (
            durability_findings,
            "fs::rename(&temporal, &destino)?;\n",
        ),
        (
            drop_cleanup_findings,
            "impl AtomicFileWriter { fn commit(self) {} }\n",
        ),
        (
            atomic_publication_findings,
            "let mut f = File::create(&temporal)?;\nfs::rename(&temporal, &d)?;\n",
        ),
        (
            version_before_publication_findings,
            "let mut writer = AtomicFileWriter::create(&destino)?;\n"
            "writer.write(b\"x\")?;\nwriter.commit()?;\n",
        ),
        (
            declared_version_findings,
            "pub struct WriteFileRequest { path: String }\n"
            "pub struct EditFileRequest { path: String }\n",
        ),
    ]
    for detector, probe in probes:
        found = detector(probe)  # type: ignore[operator]
        assert found, (
            f"FAIL: el detector {detector.__name__} no nota un texto que la "
            "viola; las comprobaciones de arriba pasarian sin comprobar nada"
        )


def main() -> int:
    checks = [
        test_the_temporary_file_is_created_in_exclusive_mode,
        test_the_content_is_durable_before_it_is_published,
        test_the_temporary_file_does_not_survive_an_unpublished_write,
        test_the_service_publishes_through_the_atomic_writer,
        test_the_declared_version_is_checked_before_the_content_is_published,
        test_the_requests_can_declare_the_version,
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
        print("la escritura se publica entera y comprueba la version: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "la escritura se publica entera y comprueba la version: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
