#!/usr/bin/env python3
"""Wiring de la lane de publicacion: lo que las stages usan, lo tienen que definir.

MEDIDO 2026-10-03. `release.pipeline.kts` resuelve el directorio de artefactos
con un preambulo que se repite stage a stage:

    val releasePaths = \"\"\"
        $cd || exit 1
        TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
    \"\"\".trimIndent()

`TARGET_DIR` es una variable de shell *sin exportar*, y cada `sh` corre en su
propio proceso. Medido con una sonda de dos stages sobre pipelinek 0.46.0
(`scripts/ci/probe-pipelinek-semantics.sh` la ejecuta a mano tras cada cambio
de version, y este contrato es la parte que si corre en todas partes):

    setter: TARGET_DIR=/tmp/pretend-target
    reader: TARGET_DIR='<VACIO>'
    reader: RELEASE_TAG='v0.101.5-probe'   <- el env del launcher si cruza

La consecuencia no es academica. `re-verify-after-upload` usaba
`$TARGET_DIR/release/cognicode-release` sin llevar el preambulo, y esta dentro
del grupo `publish`, DESPUES de `create-draft` y de `upload-payloads`: la lane
creaba el draft, subiaba todos los assets, y entonces ejecutaba
`/release/cognicode-release verify`, que no existe. Un fallo de cableado
oculto en el ultimo paso de la publicacion cuesta una release a medias que
alguien tiene que limpiar a mano.

El invariante que este contrato fija es pequeno y mecanico: **toda stage que
nombra `$TARGET_DIR` tiene que llevar `releasePaths`**. No interesa si la
stage hace lo correcto con la ruta; interesa que la ruta exista.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE = REPO_ROOT / "release.pipeline.kts"

# La forma en que el pipeline se refiere a la variable dentro de un raw string
# de Kotlin. `${'$'}TARGET_DIR` es "TARGET_DIR" con un dollar literal delante;
# el preambulo la define sin dollar porque ahi no hay que escaparlo.
SHELL_TARGET_REF = re.compile(r"\$\{'\$'\}TARGET_DIR|\$TARGET_DIR")
STAGE_OPEN = re.compile(r'stage\("([^"]+)"\)\s*\{')


def stage_blocks(text: str) -> list[tuple[str, str]]:
    """(nombre, cuerpo) de cada stage, emparejando llaves desde su apertura."""
    blocks: list[tuple[str, str]] = []
    for match in STAGE_OPEN.finditer(text):
        depth = 0
        index = match.end() - 1
        while index < len(text):
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    break
            index += 1
        blocks.append((match.group(1), text[match.end() : index]))
    return blocks


def stages_using_target_dir(text: str) -> list[tuple[str, str]]:
    """Las stages HOJA que nombran la variable, con su cuerpo.

    Solo hojas: `stage("publish")` es un contenedor, y su cuerpo contiene
    textualmente el `sh` de la stage que hay dentro. Contarlo seria medir el
   Cableado de otra stage dos veces y, peor, dejaria que arreglar la stage
    interior tapase un fallo real del contenedor. Una hoja no anida otra
    `stage(`.
    """
    return [
        (name, body)
        for name, body in stage_blocks(text)
        if not STAGE_OPEN.search(body) and SHELL_TARGET_REF.search(body)
    ]


def test_every_stage_that_uses_target_dir_declares_it() -> None:
    text = RELEASE.read_text(encoding="utf-8")
    users = stages_using_target_dir(text)

    # Sin este guardia el test pasa solo si no encuentra nada, que es
    # exactamente el falso verde que un parser roto produce.
    assert len(users) >= 4, (
        f"se esperaban al menos 4 stages que usen $TARGET_DIR y se encontraron "
        f"{len(users)}; si el parser dejo de verlas, este test no esta midiendo "
        f"nada. Vistas: {[n for n, _ in users]}"
    )

    missing = [name for name, body in users if "releasePaths" not in body]
    assert not missing, (
        f"estas stages usan $TARGET_DIR sin declarar el preambulo que lo define: "
        f"{missing}.\n"
        f"$TARGET_DIR es una variable de shell sin exportar y cada sh corre en su "
        f"propio proceso, asi que sin `releasePaths` la ruta queda vacia y la "
        f"stage ejecuta `/release/cognicode-release`, que no existe.\n"
        f"Anyadir `releasePaths + \"\\n\" +` delante del cuerpo de esas stages."
    )


def test_the_publication_lane_really_does_publish() -> None:
    """El stage que se rompe esta dentro del grupo que publica, no antes.

    No es una curiosidad: fija que el fallo ocurre DESPUES de que exista un
    draft con assets, que es el estado que deja trabajo a mano. Si alguien
    moviera la stage antes de `create-draft`, este test obliga a explicar por
    que deja de ser el mismo fallo.
    """
    text = RELEASE.read_text(encoding="utf-8")
    offenders = [
        name for name, body in stages_using_target_dir(text) if "releasePaths" not in body
    ]
    assert not offenders, f"mismo invariante, otro aserto: {offenders}"

    create = text.index('stage("create-draft")')
    assert create < text.index("stage(\"publish\")"), (
        "create-draft debe preceder al grupo publish; si el orden cambio, la "
        "conclusion sobre cuando se rompe la lane hay que volver a medirla"
    )


def main() -> int:
    module = sys.modules[__name__]
    tests = sorted(
        (
            name
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    if not tests:
        # El runner tambien falla cerrado ante esto, pero un contrato que no
        # ejecuta nada no debe poder decir que pasa.
        print("FAIL: este modulo no expone ninguna funcion test_*")
        return 1
    failures = 0
    for name in tests:
        try:
            getattr(module, name)()
        except AssertionError as exc:
            failures += 1
            print(f"  FAIL {name}")
            for line in str(exc).splitlines():
                print(f"       {line}")
        else:
            print(f"  PASS {name}")
    print(f"  -> {len(tests) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
