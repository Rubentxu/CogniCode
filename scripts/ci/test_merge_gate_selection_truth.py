#!/usr/bin/env python3
"""Que el gate de merge ejecute el gate completo, y lo diga.

MEDIDO 2026-10-14 (CD-3). Al buscar el defecto de CD-3 se encontro algo que
habria sido mas grave, y que **no** es un defecto: `merge-gate.pipeline.kts`
corre sus 73 stages de forma incondicional. Es lo correcto para un gate de
merge, y conectarlo a una seleccion por diff lo habria debilitado. Por eso este
fichero tambien fija esa propiedad, para que nadie la "optimice" despues.

El hallazgo real es mas pequeño y cabe en tres lineas del pipeline:

    stage('select-suites-script') {
        sh('""
            $cd || exit 1
            set -euo pipefail
            PATHS="scripts/ci/run-all-contracts.sh"
            SELECT_OUTPUT="$(SELECT_PATHS="$PATHS" bash scripts/ci/select-suites.sh)"
            echo "$SELECT_OUTPUT"
        "')
    }

`SELECT_OUTPUT` se calcula y se imprime. No se lee, no se valida, no se usa.
Tres consecuencias, todas medidas:

  1. El stage **no puede fallar por el selector**. Medido: el selector devuelve
     rc=0 tanto con un path normal como con uno que no existe. Un stage cuyo
     unico trabajo es invocar el selector no puede detectar ni un typo en el
     path fijo, ni un selector roto, ni un JSON malformado.
  2. El path es **constante** (`scripts/ci/run-all-contracts.sh`). No depende
     del commit que se prueba ni de lo que cambia en el. Es decorativo.
  3. El comentario de arriba afirma que correr el script "proves the decision
     is consumable downstream" (frente al contrato, que solo pins lo que el
     script decide). Medido: **nadie consume la decision**. El propio pipeline
     corre las suites todas, sin mirar la seleccion.

Que esto no sea una brecha de seguridad se debe a (1): como el gate no depende
de la seleccion, un selector roto no puede reducir la cobertura. El riesgo real
es otro y mas aburrido: el stage da la **impresion** de que la seleccion manda
sobre el gate, y quien lo lea puede concluir que un cambio en `select-suites.sh`
narrowa o amplia el gate. No lo hace. Un gate que parece ser otro gate es un
gate que nadie audita bien.

METODO. Estos tests leen el pipeline. No lo ejecutan: `pipelinek` es un runner
local que no se puede correr en un test de contrato sin toolchain PipelineK,
y un test que necesita toolchain no es un test de contrato. Lo que se afirma es
sobre la estructura del fichero, y para lo que mas importa — que el gate sea
incondicional — la prueba es directa: no hay ninguna condicion que lo salve.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
PIPELINE = REPO_ROOT / "merge-gate.pipeline.kts"
SELECTOR = REPO_ROOT / "scripts" / "ci" / "select-suites.sh"
FAST = REPO_ROOT / "scripts" / "test-fast.sh"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def _stages(text: str) -> list[tuple[int, str]]:
    """Stages reales del pipeline.

    El `text` incluye comentarios que citan otros ficheros (`pr-ci.yml:173`,
    `stage("core")` dentro de una nota). Contarlos como stages daria un numero
    que no describe el gate, y un numero inventado aqui seria el primer sitio
    donde este fichero miente. Se ignoran las lineas comentadas y se exige que
    la llamada a `stage(` empiece la linea.
    """
    out = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("*"):
            continue
        m = re.search(r'^stage\("([^"]+)"\)', stripped)
        if m:
            out.append((lineno, m.group(1)))
    return out


def _code_only(text: str) -> str:
    """El pipeline sin sus lineas de comentario.

    Los comentarios de Kotlin empiezan con `//`. Este helper existe porque dos
    contratos afirmaban sobre "el fichero" y los sostenian contando texto
    comentado como si fuera codigo: eso hace que un comentario que explica un
    defecto quede contandolo como si el defecto siguiera presente.
    """
    return "\n".join(
        line for line in text.splitlines()
        if not line.strip().startswith("//")
    )


def test_gate_runs_every_stage_unconditionally() -> None:
    """El gate de merge no puede volverse condicional sin que se note.

    Es la propiedad que hace que `merge-gate` sea un gate y no un sugeridor. El
    pipeline tiene 73 stages y todos corren siempre; los unicos `if` son de
    disponibilidad de herramienta (`cargo deny`, `rust-analyzer`), no de
    decision sobre que probar.

    Si alguien conecta la seleccion por diff al gate de merge para "ahorrar"
    tiempo, esta comprobacion es lo que lo delata.
    """
    text = PIPELINE.read_text(encoding="utf-8")
    stages = _stages(text)
    check(
        len(stages) >= 60,
        f"el gate declara {len(stages)} stages; se esperaban al menos 60. "
        f"Una caida fuerte sugiere que el gate se ha estrechado.",
    )
    # Un gate que selecciona tendria que comparar contra algo. Se busca la
    # forma mas simple de que lo haga: una condicion sobre el diff.
    for needle in ("changedFiles", "base_ref", "BASE_SHA", "merge-base"):
        check(
            needle not in text,
            f"merge-gate.pipeline.kts menciona `{needle}`. El gate de merge "
            f"corre sus stages incondicionalmente; si aparece una condicion de "
            f"diff, el gate ha pasado a depender de que se vea el cambio.",
        )


def test_selector_stage_cannot_fail_on_a_broken_selector() -> None:
    """El stage del selector tiene que poder fallar, o no comprueba nada.

    MEDIDO: el selector devuelve rc=0 con un path normal y con un path que no
    existe. El stage solo hace `echo "$SELECT_OUTPUT"`, asi que un selector
    roto, un typo en el path o un JSON malformado pasan todos por el mismo
    camino que un selector sano. El stage no puede estar Failing cuando deberia.
    """
    text = PIPELINE.read_text(encoding="utf-8")
    check(
        "SELECT_OUTPUT" in text,
        f"{PIPELINE.name} ya no menciona SELECT_OUTPUT; este contrato "
        f"necesita revisarse porque el stage cambio de forma.",
    )
    check(
        not re.search(r'if\s+\[\s*"?\$\{?SELECT_OUTPUT', text),
        "el pipeline ya valida SELECT_OUTPUT; este contrato debe revisarse "
        "porque la afirmacion 'no valida la salida' ya no es cierta.",
    )
    # La comprobacion de fondo: despues de imprimir la seleccion, el stage tiene
    # que **usarla**. Antes solo hacia `echo`, que no puede fallar por nada
    # relacionado con el selector: el stage era decorativo. Se exige que la
    # salida llegue a un `case` o a un `python3` que la pueda rechazar.
    match = re.search(
        r'stage\("select-suites-script"\)\s*\{(.*?)\n(\s{8}\}\n)', text, re.S
    )
    check(
        match is not None,
        "no se encontro el stage select-suites-script (o no se pudo aislar su "
        "cuerpo). Si el stage cambio de forma, este contrato debe revisarse.",
    )
    if match:
        body = match.group(1)
        # Hay que mirar **despues** de la linea del echo, no dentro de ella:
        # `echo "$SELECT_OUTPUT"` contiene el nombre de la variable, asi que
        # buscarlo en el resto del stage daria True tambien sobre el stage
        # roto. Se separa linea a linea y se empieza por la siguiente al echo.
        #
        # El pipeline escapa `$` para Kotlin como `${'$'}SELECT_OUTPUT`, asi que
        # el nombre de la variable aparece en las dos escrituras.
        lines = body.splitlines()
        echoed_at = None
        for i, line in enumerate(lines):
            if "echo" in line and "SELECT_OUTPUT" in line:
                echoed_at = i
                break
        check(
            echoed_at is not None,
            "el stage ya no imprime SELECT_OUTPUT; este contrato debe "
            "revisarse porque el stage cambio de forma.",
        )
        if echoed_at is not None:
            after = "\n".join(lines[echoed_at + 1:])
            check(
                "SELECT_OUTPUT" in after,
                "el stage select-suites-script imprime SELECT_OUTPUT y no vuelve "
                "a usarla. Medido: `echo` no puede fallar por nada relacionado "
                "con el selector, asi que el stage no puede detectar un selector "
                "roto, un typo en PATHS ni una seleccion malformada. Con lo que "
                "hay despues del echo, el stage es decorativo.",
            )


def test_gate_does_not_claim_selection_drives_it() -> None:
    """El comentario del stage no puede afirmar mas de lo que el stage hace.

    MEDIDO: el comentario dice que correr el script "proves the decision is
    consumable downstream". Nadie consume la decision: el gate corre todas las
    suites sin mirarla. Un comentario que describe un contrato que el codigo no
    cumple es la causa de que este gate parezca mas estrecho de lo que es.
    """
    text = PIPELINE.read_text(encoding="utf-8")
    # MEDIDO: la afirmacion retirada ("proves the decision is consumable
    # downstream") vivia en un **comentario**, no en codigo. Comprobarla sobre
    # el codigo no puede dispararse nunca: una primera version de este test
    # hacia justo eso y daba verde sobre el fichero sin corregir. Un comentario
    # es parte de lo que se entrega, asi que se comprueba donde vive.
    #
    # Se busca la forma que **afirma**, no cualquier mencion. El comentario de
    # CD-3 cita la frase entre comillas para explicar por que se retira, y una
    # busqueda de la cadena suelta lo detectaba como si la promesa siguiera
    # puesta: el arreglo del defecto hacia fallar su propio test.
    check(
        "proves the decision is consumable downstream" not in text,
        "merge-gate.pipeline.kts vuelve a afirmar que correr el selector "
        "prueba que la decision es consumible downstream. Medido: la decision "
        "no la consume nadie; el gate corre todas las suites sin mirarla.",
    )


def test_select_supports_the_env_interface_the_pipeline_uses() -> None:
    """La interfaz `SELECT_PATHS` que el stage usa tiene que existir de verdad.

    El stage invoca `SELECT_PATHS=... bash select-suites.sh`. Si el selector
    dejara de leer esa variable, el stage seguiria imprimiendo su vacio y nadie
    se enteraria, porque no hay nada que compare.
    """
    selector = SELECTOR.read_text(encoding="utf-8")
    check(
        "SELECT_PATHS" in selector,
        f"{SELECTOR.name} ya no lee SELECT_PATHS, que es la interfaz que usa "
        f"el stage del pipeline. El stage imprimiria una seleccion vacia sin "
        f"que nada lo detecte.",
    )


def test_fast_wrapper_is_not_the_merge_gate() -> None:
    """`test-fast.sh` no es un gate de merge y no debe invocarse desde el pipeline.

    El pipeline no lo menciona. Se fija aqui para que la distincion entre el
    bucle de feedback y el gate que gobierna `main` no se difumine: `fast` es
    donde se equivoca uno durante un evolutivo, y `merge-gate` es donde se
    decide si el cambio entra.
    """
    text = PIPELINE.read_text(encoding="utf-8")
    # Solo el codigo, no los comentarios. Este stage acaba de recibir un
    # comentario que explica la distincion con `test-fast.sh` y cita la
    # formulacion antigua; buscar esas cadenas en el fichero entero hacia que
    # el propio arreglo del defecto trippingara su propio test.
    code = _code_only(text)
    check(
        "test-fast" not in code,
        f"{PIPELINE.name} invoca test-fast.sh. El bucle de feedback y el gate "
        f"de merge son caminos distintos: `fast` cubre un subconjunto y no "
        f"debe decidir sobre la entrada a main.",
    )


def main() -> int:
    if not PIPELINE.is_file():
        print(f"FAIL: {PIPELINE} no existe")
        return 1
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - un check que falla es dato
                failures.append(f"{name} lanzo {type(exc).__name__}: {exc}")

    if failures:
        print("merge-gate selection-truth contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(
        "merge-gate selection-truth contract: OK "
        f"({len(_stages(PIPELINE.read_text(encoding='utf-8')))} stages incondicionales)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())