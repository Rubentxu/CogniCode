#!/usr/bin/env python3
"""A `sh` block with many commands has to carry its own fail-fast.

MEDIDO 2026-10-15. El merge gate se colapso de 73 etapas a 8, y al pegar los
cuerpos en un unico `sh(...)` la etapa `tests` quedo con 39 `cargo test`
seguidos. Devolvio `success` con 35 sin ejecutar.

La causa fue una linea heredada de un sub-bloque:

    set +e ; cargo test … > log 2>&1 ; rc=$? ; set -e
    tail -40 log ; exit $rc            <-- mata el PROCESO

`exit` en bash termina el proceso, no el sub-bloque que lo escribio. Pegado en
medio de una etapa de 39 comandos, se come los 35 siguientes. Y como la etapa
devolvia el codigo del test numero 4 —que si paso— nadie lo noto.

Detras hay una suposicion que resulto falsa. La cabecera del gate afirmaba:

    // That is what `set -euo pipefail` gave the old workflow, implicitly, and
    // it is why no stage here needs a `set -e` of its own.

La sonda que respaldaba esa frase media otra cosa: media que un codigo de
salida no nulo de una ETAPA aborta la lane. No media nada sobre una orden que
falla a mitad de un bloque. Son dos cosas distintas, y la confusion entre ellas
es exactamente lo que dejo 35 tests sin correr.

Este contrato fija las dos mitades:

  1. Toda etapa con mas de un comando abre con su propio fail-fast. PipelineK
     ejecuta el cuerpo con `bash script.sh` y sin opciones de shell, asi que
     sin esa linea el veredicto es el del ULTIMO comando.
  2. Ninguna etapa sale del proceso en su camino de exito. Un `exit` en la
     rama de fallo esta bien —para eso existe—; un `exit` que se ejecuta
     cuando todo va bien trunca el resto de la etapa en silencio.

Y la tercera funcion no mira el repo: ejecuta bash de verdad, con y sin el
guard, y comprueba que el guard hace lo que este contrato dice que hace. Es la
parte que sustituye a una suposicion por una medida.

Run:
    python3 scripts/ci/test_pipeline_fail_fast.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from test_release_lane_wiring import STAGE_OPEN, stage_blocks

REPO_ROOT = Path(__file__).resolve().parents[2]

# Las lanes que deciden que se integra y que se publica. Un gate que no se
# mira a si mismo no protege de nada.
LANES = ("merge-gate.pipeline.kts", "release-candidate.pipeline.kts", "release.pipeline.kts")

# `set -e`, `set -eo pipefail`, `set -euo pipefail`… Lo que importa es el `-e`.
FAIL_FAST = re.compile(r"^\s*set\s+-[a-z]*e", re.M)

# Una linea que solo abre o cierra un bloque de control.
OPENER = re.compile(r"(\bthen$)|(^\s*(if|while|until|for|case)\b)|(\{\s*$)|(\bdo\s*$)")
CLOSER = re.compile(r"^\s*(fi|esac|done)\b|^\s*\}")


def executable_lines(body: str) -> list[tuple[int, str]]:
    """(indice, texto) de las lineas que llegan a bash, en ambos dialectos.

    Se filtran los comentarios de Kotlin (`//`) y los de shell (`#`): un
    pipeline esta lleno de `# --- core-lib ---` entre etapas, y un `exit` que
    aparece en la prosa que explica por que existe el `exit` no trunca nada.
    """
    out: list[tuple[int, str]] = []
    for index, raw in enumerate(body.splitlines()):
        line = raw.strip()
        if not line or line.startswith("//") or line.startswith("#"):
            continue
        if line.startswith('"""') or line.endswith('"""') or line.endswith("trimIndent())"):
            continue
        # `sh(` en su propia linea solo abre la llamada; el cuerpo empieza en
        # la triple comilla siguiente. Contarlo como orden desplaza al guard una
        # posicion y hace creer que protege tarde cuando no.
        if line == "sh(":
            continue
        out.append((index, raw))
    return out


def stage_commands(body: str) -> list[str]:
    """Las ordenes que la etapa ejecuta, sin sus envolturas de control."""
    structurals = {"fi", "esac", "done", "{", "}", "then", "else", ";;", "esac"}
    out = []
    for _, raw in executable_lines(body):
        line = raw.strip()
        if line in structurals:
            continue
        out.append(line)
    return out


SH_PREAMBLE = re.compile(r'sh\(\s*(\w+)\s*\+')
PREAMBLE = re.compile(r'val (\w+)\s*=\s*"""\n(.*?)"""\.trimIndent\(\)', re.S)


def preambles(text: str) -> dict[str, str]:
    """Los `val <nombre>` de un pipeline, con su cuerpo de triple comilla, por nombre.

    Un preambulo compartido es la forma correcta de declarar una vez lo que
    todas las etapas necesitan — y por eso el contrato tiene que RESOLVERLO en
    vez de buscar el guard letra a letra en cada cuerpo. Si exigiera que cada
    etapa repita el `set -e`, la siguiente persona que anada una etapa lo
    escribiria… o no, y el preambulo seguiria siendo la via de hecho.
    """
    return {name: body for name, body in PREAMBLE.findall(text)}


def fail_fast_position(body: str, preamble: str | None = None) -> int | None:
    """Donde aparece el fail-fast, como indice dentro de `executable_lines`.

    None si no aparece. La posicion importa: un `set -e` debajo del primer
    comando protege al segundo pero no al primero, que es exactamente el hueco
    que busco cerrar.

    `preamble` es el cuerpo de un `val` que la etapa antepone a su propio shell.
    Cuando la etapa lo usa, el guard cuenta como PRIMERA linea: en tiempo de
    ejecucion el preambulo se ejecuta antes que el cuerpo de la etapa, y el
    contrato no deberia obligar a repetirlo en cinco sitios.
    """
    if preamble is not None and FAIL_FAST.search(preamble):
        return 0
    for slot, (_, raw) in enumerate(executable_lines(body)):
        if FAIL_FAST.match(raw.strip()):
            return slot
    return None


def unguarded_exits(body: str) -> list[tuple[int, str]]:
    """`exit` que NO estan dentro de una rama de fallo.

    Se acepta como guardados:
      * los que son parte de la propia linea (`|| exit 1`, `&& exit 1`);
      * los que estan dentro de un `if … then … fi`, `while`, `case` o un
        grupo de llaves `{ … }`, porque ahi solo se llega cuando algo fallo.

    Todo lo demas se considera trunca: en un bloque de varias ordenes,
    ejecutarse y salir sin error se come lo que venia despues.
    """
    offenders: list[tuple[int, str]] = []
    depth = 0
    lines = executable_lines(body)
    for slot, (_, raw) in enumerate(lines):
        line = raw.strip()
        if CLOSER.match(line):
            depth = max(0, depth - 1)
            continue
        if line.startswith("exit "):
            # `cmd || exit 1` — el `exit` es parte de la propia orden.
            if line.endswith("|| exit 1") or "|| exit " in line or "&& exit " in line:
                continue
            if depth == 0:
                offenders.append((slot, line))
            continue
        if OPENER.search(line):
            depth += 1
    return offenders


def test_every_multi_command_stage_brings_its_own_fail_fast() -> None:
    """Ninguna etapa se fia de las opciones de shell de su entorno.

    PipelineK escribe el cuerpo en un fichero y lo ejecuta con
    `bash script.sh` (wrapper en `.pipelinek/durable-shell/<id>/wrapper.sh`).
    Sin `-e` explicito, una orden que falla a mitad no interrumpe: el codigo
    de salida es el de la ultima, y las de en medio pueden estar todas rotas.
    """
    unguarded: list[str] = []
    late: list[str] = []
    checked = 0

    for lane in LANES:
        text = (REPO_ROOT / lane).read_text(encoding="utf-8")
        for name, body in stage_blocks(text):
            # Un contenedor no es una hoja: `stage("supply-chain")` contiene
            # las etapas hijas, asi que su cuerpo trae las ordenes de ellas y
            # contarlas seria medir el cableado de otra etapa dos veces. Solo
            # las hojas ejecutan.
            if STAGE_OPEN.search(body):
                continue
            commands = stage_commands(body)
            # Una etapa de una sola orden no tiene nada que tragarse: su
            # codigo de salida ES el veredicto. Exigirle un guard seria ruido.
            if len(commands) < 2:
                continue
            checked += 1
            # Si la etapa antepone un `val` compartido, ese es su entorno de
            # shell y el guard se busca ahi.
            # `sh(releasePaths + "\n" + """` — el nombre va en la llamada al
            # `sh`, no en las ordenes: la linea acaba en `"""` y el filtro de
            # lineas ejecutables la descarta. Buscarlo en el cuerpo crudo.
            shared = None
            head = SH_PREAMBLE.search(body)
            if head and head.group(1) in preambles(text):
                shared = preambles(text)[head.group(1)]
            position = fail_fast_position(body, shared)
            if position is None:
                unguarded.append(f"{lane}:{name}")
            elif position > 0:
                late.append(f"{lane}:{name} (fail-fast en la orden #{position + 1})")

    assert checked >= 8, (
        f"se esperaban al menos 8 etapas multi-comando entre las lanes y se "
        f"encontraron {checked}. Si el extractor dejo de verlas, este test no "
        f"esta midiendo nada."
    )
    assert not unguarded, (
        f"estas etapas ejecutan varias ordenes sin declarar `set -e`: "
        f"{unguarded}.\n"
        f"PipelineK las corre con `bash script.sh` sin opciones de shell, asi "
        f"que su veredicto es el codigo de la ULTIMA orden y todas las "
        f"anteriores pueden fallar sin que la etapa se ponga roja.\n"
        f"Añadir `set -eo pipefail` como PRIMERA linea del cuerpo del `sh`."
    )
    assert not late, (
        f"estas etapas declaran el fail-fast DESPUES de su primera orden: {late}. "
        f"La primera orden sigue sin quedar cubierta, que es el hueco que este "
        f"contrato existe para cerrar."
    )


def test_no_stage_exits_while_everything_is_going_well() -> None:
    """El `exit` que trunca una etapa es el que corre cuando todo va bien.

    MEDIDO 2026-10-15: la etapa `tests` del merge gate llevo un
    `exit $rc` heredado de un sub-bloque que capturaba el codigo de un test
    para imprimir su log. Al pegarlo en medio de 39 ordenes, ese `exit` —que
    se ejecutaba en el camino de EXITO— termino el proceso en la cuarta, y las
    35 siguientes nunca corrieron. La etapa devolvio `success`.

    Un `exit` dentro de `if … then` esta bien: ahi se llega porque algo fallo,
    y el codigo de salida es justo el que debe propagarse.
    """
    offenders: list[str] = []
    checked = 0

    for lane in LANES:
        text = (REPO_ROOT / lane).read_text(encoding="utf-8")
        for name, body in stage_blocks(text):
            if STAGE_OPEN.search(body):
                continue
            lines = executable_lines(body)
            if len(lines) < 2:
                continue
            checked += 1
            for slot, line in unguarded_exits(body):
                offenders.append(f"{lane}:{name}:{slot + 1}  {line}")

    assert checked >= 8, (
        f"se esperaban al menos 8 etapas multi-comando y se encontraron "
        f"{checked}; si el extractor dejo de verlas, este test no esta midiendo "
        f"nada"
    )
    assert not offenders, (
        f"estos `exit` se ejecutan en el camino de exito y truncan el resto de "
        f"la etapa sin decir nada:\n  "
        + "\n  ".join(offenders)
        + "\nEn bash, `exit` termina el PROCESO. Pegado en medio de una etapa "
        "de varias ordenes, se come las siguientes y la etapa reporta el "
        "codigo de la ultima que llego a correr.\n"
        "Sustituir por `if ! …; then …; exit 1; fi`, que es el patron que ya "
        "usan las dos stages que capturan un codigo para imprimir su log."
    )


def test_the_guard_does_what_this_contract_claims_it_does() -> None:
    """La regla anterior, ejecutada. Sin esto el contrato seria una opinion.

    Los dos primeros tests comprueban que el guard esta escrito. Este ejecuta
    bash de verdad, con el bloque que la etapa `tests` tenía —el que devolvio
    `success` con 35 tests sin correr— y con el guard puesto. Es la misma forma
    del defecto y de la correccion, medida en vez de supuesta.
    """
    before = (
        "cd \"$1\"\n"
        "cargo test core-1\n"          # imaginado: pasa
        "cargo test core-2\n"          # imaginado: PASA, pero aqui va a fallar
        "echo 'ESTOS TRES NO DEBEN LLEGAR A CORRER'\n"
        "echo ok-final\n"
    )
    after = "set -eo pipefail\n" + before

    def run(script: str, tmp: Path) -> tuple[int, str]:
        helper = tmp / "cargo"
        helper.write_text(
            "#!/usr/bin/env bash\n"
            "# El segundo test falla. Los demas pasan.\n"
            'if [ "$2" = "core-2" ]; then echo "test core-2 ... FAILED"; exit 101; fi\n'
            'echo "test $2 ... ok"\n',
            encoding="utf-8",
        )
        helper.chmod(0o755)
        script_path = tmp / "block.sh"
        script_path.write_text(script, encoding="utf-8")
        env = {"PATH": f"{tmp}:/usr/bin:/bin", "HOME": str(tmp)}
        proc = subprocess.run(
            ["bash", str(script_path), str(tmp)],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        return proc.returncode, proc.stdout + proc.stderr

    import tempfile

    with tempfile.TemporaryDirectory() as raw:
        tmp = Path(raw)
        rc_before, out_before = run(before, tmp)
        rc_after, out_after = run(after, tmp)

        # El defecto: el segundo test falla, el bloque lo dice en su salida y a
        # pesar de eso sale con 0. Eso es exactamente lo que hizo la etapa
        # `tests`: 35 tests sin correr y `success`.
        assert rc_before == 0, (
            f"el bloque SIN guard devolvio {rc_before}. Se esperaba 0: con un "
            f"test fallando en medio y sin `set -e`, el codigo de salida es el "
            f"de la ultima orden, que pasa."
        )
        assert "test core-2 ... FAILED" in out_before, (
            f"el test central deberia fallar en la sonda; si no falla, la sonda "
            f"no esta midiendo lo que dice medir. stdout fue:\n{out_before}"
        )
        assert "ESTOS TRES NO DEBEN LLEGAR A CORRER" in out_before, (
            "sin guard, el bloque deberia seguir tras el fallo hasta el final. "
            f"stdout fue:\n{out_before}"
        )

        # La correccion: el guard corta en el fallo y propaga el codigo.
        assert "ESTOS TRES NO DEBEN LLEGAR A CORRER" not in out_after, (
            "con `set -eo pipefail`, el bloque DEBERIA detenerse en el test que "
            f"falla. stdout fue:\n{out_after}"
        )
        assert rc_after == 101, (
            f"con guard, el bloque deberia salir con el codigo del test que "
            f"fallo (101) y salio con {rc_after}."
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